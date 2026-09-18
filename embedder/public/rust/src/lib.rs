//! Minimal, trusted-host API over Chromium Content.
//!
//! Run the engine once on the process's main thread. Commands are accepted
//! synchronously; navigation and destruction are reported through events.
//! The macOS MVP hosts each page in a plain native window, uses one disposable
//! context, and does not open a remote debugging endpoint.

#[cfg(not(target_os = "macos"))]
compile_error!("The native-window MVP currently targets macOS only.");

use std::error;
use std::ffi::{CString, c_char, c_int, c_void};
use std::fmt;
use std::marker::PhantomData;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::ptr::NonNull;
use std::rc::Rc;
use std::slice;

/// Opaque instance identity. Closing it permanently invalidates it.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PageId(u64);

impl PageId {
    /// Stable only for this engine run. The trusted host may use it as a UI key.
    pub const fn as_u64(self) -> u64 {
        self.0
    }
}

/// A host-owned AppKit `NSView` that may contain one page's native view.
///
/// The trusted host must keep the view alive while a page is attached and must
/// detach or close every attached page before destroying the view.
pub struct NativeContainer {
    raw: NonNull<c_void>,
    _main_thread: PhantomData<Rc<()>>,
}

impl NativeContainer {
    /// Construct a container from a retained, host-owned `NSView*`.
    ///
    /// # Safety
    ///
    /// `raw` must be a valid `NSView*` on the process main thread. Its lifetime
    /// must satisfy the requirements documented on [`NativeContainer`].
    pub unsafe fn from_raw_ns_view(raw: *mut c_void) -> Result<Self, Error> {
        let raw = NonNull::new(raw).ok_or(Error::CreationFailed)?;
        Ok(Self {
            raw,
            _main_thread: PhantomData,
        })
    }
}

#[derive(Debug)]
pub enum Event {
    Ready,
    /// A primary-frame navigation committed. This is not network-idle or
    /// a guarantee that a dynamic document has finished all its work.
    Navigated {
        page: PageId,
        url: String,
        can_go_back: bool,
        can_go_forward: bool,
    },
    NavigationFailed {
        page: PageId,
        code: i32,
    },
    Closed {
        page: PageId,
    },
    /// A requested close was accepted for asynchronous processing, but a
    /// user-activated beforeunload handler refused destruction.
    CloseRefused {
        page: PageId,
    },
    RendererFailed {
        page: PageId,
    },
    /// Chromium reported its first non-empty paint while the native window
    /// was visible. This is not a promise that a dynamic page is finished.
    Painted {
        page: PageId,
        url: String,
    },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Error {
    NotReady,
    WrongThread,
    InvalidPage,
    InvalidUrl,
    NoHistoryEntry,
    /// Reserved for older native implementations. Current close refusal is
    /// asynchronous and arrives as [`Event::CloseRefused`].
    CloseNeedsConfirmation,
    CreationFailed,
    CallbackPanicked,
    Native(i32),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{self:?}")
    }
}

impl error::Error for Error {}

/// Available only during `run` callbacks; deliberately not Send or Sync.
pub struct Engine {
    _main_thread: PhantomData<Rc<()>>,
}

impl Engine {
    /// Create a page in the disposable context, show its plain native test
    /// window, and start a GET navigation.
    pub fn create_page(&self, url: &str) -> Result<PageId, Error> {
        let url = CString::new(url).map_err(|_| Error::InvalidUrl)?;
        let mut page = 0;
        // SAFETY: URL and out pointer remain valid for the synchronous call.
        check(unsafe { native::pliant_page_create(url.as_ptr(), &mut page) })?;
        Ok(PageId(page))
    }

    /// Create a page attached to a trusted host-owned native container.
    pub fn create_page_in(&self, container: &NativeContainer, url: &str) -> Result<PageId, Error> {
        let url = CString::new(url).map_err(|_| Error::InvalidUrl)?;
        let mut page = 0;
        // SAFETY: The trusted host upholds the container lifetime contract.
        check(unsafe {
            native::pliant_page_create_in(url.as_ptr(), container.raw.as_ptr(), &mut page)
        })?;
        Ok(PageId(page))
    }

    pub fn load_url(&self, page: PageId, url: &str) -> Result<(), Error> {
        let url = CString::new(url).map_err(|_| Error::InvalidUrl)?;
        // SAFETY: The native registry validates the ID; the URL is borrowed.
        check(unsafe { native::pliant_page_load(page.0, url.as_ptr()) })
    }

    pub fn back(&self, page: PageId) -> Result<(), Error> {
        // SAFETY: The native registry checks both the ID and history bounds.
        check(unsafe { native::pliant_page_back(page.0) })
    }

    pub fn forward(&self, page: PageId) -> Result<(), Error> {
        // SAFETY: The native registry checks both the ID and history bounds.
        check(unsafe { native::pliant_page_forward(page.0) })
    }

    /// Attach an existing page without navigating or recreating it.
    pub fn attach_page(&self, page: PageId, container: &NativeContainer) -> Result<(), Error> {
        // SAFETY: The native registry validates the ID and the trusted host
        // upholds the container lifetime contract.
        check(unsafe { native::pliant_page_attach(page.0, container.raw.as_ptr()) })
    }

    /// Remove a page from its host container without destroying it.
    pub fn detach_page(&self, page: PageId) -> Result<(), Error> {
        // SAFETY: The native registry validates the page ID.
        check(unsafe { native::pliant_page_detach(page.0) })
    }

    /// Request asynchronous beforeunload authorization for a normal page
    /// close. Acceptance is not destruction: retain the host container until
    /// `Event::Closed`, and handle `Event::CloseRefused` when a dirty page stays
    /// alive.
    pub fn close_page(&self, page: PageId) -> Result<(), Error> {
        // SAFETY: Native code owns and invalidates the page instance.
        check(unsafe { native::pliant_page_close(page.0) })
    }

    /// End this disposable engine session. This is host teardown, not a
    /// substitute for `close_page` in an ordinary browsing operation.
    pub fn shutdown(&self) -> Result<(), Error> {
        // SAFETY: Native code queues shutdown after the callback returns.
        check(unsafe { native::pliant_engine_shutdown() })
    }
}

/// Run one synchronous command scope from a native main-thread host callback.
///
/// This does not retain an engine reference and fails outside Chromium's ready
/// browser-main-thread lifetime.
pub fn with_engine<T>(operation: impl FnOnce(&Engine) -> T) -> Result<T, Error> {
    // SAFETY: Native code checks both runtime lifetime and Chromium UI thread.
    check(unsafe { native::pliant_engine_check_ready() })?;
    let engine = Engine {
        _main_thread: PhantomData,
    };
    Ok(operation(&engine))
}

/// Enter Chromium's main loop and receive events on the browser main thread.
/// Dedicated native helpers run child processes without the Rust host callback.
/// Calling this more than once in a process is rejected by the native runtime.
pub fn run(mut handler: impl FnMut(&Engine, Event)) -> Result<(), Error> {
    let arguments = std::env::args_os()
        .map(|argument| {
            use std::os::unix::ffi::OsStrExt;
            CString::new(argument.as_os_str().as_bytes()).map_err(|_| Error::InvalidUrl)
        })
        .collect::<Result<Vec<_>, _>>()?;
    let pointers = arguments.iter().map(|arg| arg.as_ptr()).collect::<Vec<_>>();
    let mut state = CallbackState {
        handler: &mut handler,
        panicked: false,
    };
    // SAFETY: All arguments and the callback state outlive this blocking call.
    // Native code stops delivering callbacks before it returns.
    let code = unsafe {
        native::pliant_engine_run(
            pointers.len() as c_int,
            pointers.as_ptr(),
            callback,
            &mut state as *mut _ as *mut c_void,
        )
    };
    if state.panicked {
        Err(Error::CallbackPanicked)
    } else if code == 0 {
        Ok(())
    } else {
        Err(Error::Native(code))
    }
}

struct CallbackState<'a> {
    handler: &'a mut dyn FnMut(&Engine, Event),
    panicked: bool,
}

unsafe extern "C" fn callback(opaque: *mut c_void, event: *const native::Event) {
    // SAFETY: The native runtime serializes callbacks and borrows this state
    // and event only during `pliant_engine_run`.
    let state = unsafe { &mut *opaque.cast::<CallbackState<'_>>() };
    if state.panicked {
        return;
    }
    let event = unsafe { &*event };
    let engine = Engine {
        _main_thread: PhantomData,
    };
    let result = catch_unwind(AssertUnwindSafe(|| {
        let page = PageId(event.page);
        let url = || {
            if event.url_len == 0 {
                String::new()
            } else {
                // SAFETY: Native storage remains valid until callback return.
                String::from_utf8_lossy(unsafe {
                    slice::from_raw_parts(event.url.cast::<u8>(), event.url_len)
                })
                .into_owned()
            }
        };
        let event = match event.kind {
            0 => Event::Ready,
            1 => Event::Navigated {
                page,
                url: url(),
                can_go_back: event.can_go_back != 0,
                can_go_forward: event.can_go_forward != 0,
            },
            2 => Event::NavigationFailed {
                page,
                code: event.code,
            },
            3 => Event::Closed { page },
            4 => Event::RendererFailed { page },
            5 => Event::Painted { page, url: url() },
            6 => Event::CloseRefused { page },
            _ => return,
        };
        (state.handler)(&engine, event);
    }));
    if result.is_err() {
        state.panicked = true;
        let _ = engine.shutdown();
    }
}

fn check(code: i32) -> Result<(), Error> {
    match code {
        0 => Ok(()),
        1 => Err(Error::NotReady),
        2 => Err(Error::WrongThread),
        3 => Err(Error::InvalidPage),
        4 => Err(Error::InvalidUrl),
        5 => Err(Error::NoHistoryEntry),
        6 => Err(Error::CloseNeedsConfirmation),
        7 => Err(Error::CreationFailed),
        other => Err(Error::Native(other)),
    }
}

mod native {
    use super::{c_char, c_int, c_void};

    #[repr(C)]
    pub struct Event {
        pub kind: u32,
        pub page: u64,
        pub code: i32,
        pub url: *const c_char,
        pub url_len: usize,
        pub can_go_back: u8,
        pub can_go_forward: u8,
    }

    unsafe extern "C" {
        pub fn pliant_engine_run(
            argc: c_int,
            argv: *const *const c_char,
            callback: unsafe extern "C" fn(*mut c_void, *const Event),
            user_data: *mut c_void,
        ) -> c_int;
        pub fn pliant_page_create(url: *const c_char, page: *mut u64) -> c_int;
        pub fn pliant_page_create_in(
            url: *const c_char,
            container: *mut c_void,
            page: *mut u64,
        ) -> c_int;
        pub fn pliant_page_load(page: u64, url: *const c_char) -> c_int;
        pub fn pliant_page_back(page: u64) -> c_int;
        pub fn pliant_page_forward(page: u64) -> c_int;
        pub fn pliant_page_attach(page: u64, container: *mut c_void) -> c_int;
        pub fn pliant_page_detach(page: u64) -> c_int;
        pub fn pliant_page_close(page: u64) -> c_int;
        pub fn pliant_engine_check_ready() -> c_int;
        pub fn pliant_engine_shutdown() -> c_int;
    }
}
