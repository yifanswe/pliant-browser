use std::ffi::{CString, c_char, c_void};
use std::marker::PhantomData;
use std::ptr::NonNull;
use std::rc::Rc;

use pliant_ui_definition::{BrowserDefinition, Node};

use crate::{
    BrowserController, HostPageId, LayoutAxis, LayoutRenderer, LayoutSink, PageState,
    render_definition, validate_native_definition,
};

pub const EVENT_LOAD_DEFINITION: u32 = 0;
pub const EVENT_PREVIEW: u32 = 1;
pub const EVENT_APPLY: u32 = 2;
pub const EVENT_REJECT: u32 = 3;
pub const EVENT_RESTORE_DEFAULT: u32 = 4;
pub const EVENT_ADDRESS_SUBMITTED: u32 = 5;
pub const EVENT_BUTTON_ACTIVATED: u32 = 6;
pub const EVENT_PAGE_SELECTED: u32 = 7;
pub const EVENT_WINDOW_CLOSED: u32 = 8;
pub const EVENT_PREVIEW_STATUS: u32 = 9;
pub const EVENT_RETRY_SAVE: u32 = 10;

pub type HostCallback = unsafe extern "C" fn(
    context: *mut c_void,
    kind: u32,
    generation: u64,
    page: u64,
    node_id: *const c_char,
    value: *const c_char,
);

pub struct NativeHost {
    raw: NonNull<c_void>,
    _main_thread: PhantomData<Rc<()>>,
}

impl NativeHost {
    /// # Safety
    ///
    /// `context` must remain valid until this host is dropped, and `callback`
    /// must not unwind across the native boundary.
    pub unsafe fn new(callback: HostCallback, context: *mut c_void) -> Result<Self, String> {
        let raw = unsafe { ffi::pliant_browser_host_create(callback, context) };
        let raw = NonNull::new(raw)
            .ok_or_else(|| "AppKit host requires the initialized engine main thread".to_owned())?;
        Ok(Self {
            raw,
            _main_thread: PhantomData,
        })
    }

    pub fn show(&self) {
        unsafe { ffi::pliant_browser_host_show(self.raw.as_ptr()) };
    }

    pub fn set_preview_mode(&self, preview_mode: bool) {
        unsafe {
            ffi::pliant_browser_host_set_preview_mode(self.raw.as_ptr(), u8::from(preview_mode))
        };
    }

    pub fn set_preview_pending(&self, pending: bool) {
        unsafe {
            ffi::pliant_browser_host_set_preview_pending(self.raw.as_ptr(), u8::from(pending))
        };
    }

    pub fn set_preview_checking(&self, checking: bool) {
        unsafe {
            ffi::pliant_browser_host_set_preview_checking(self.raw.as_ptr(), u8::from(checking))
        };
    }

    pub fn set_save_pending(&self, pending: bool) {
        unsafe { ffi::pliant_browser_host_set_save_pending(self.raw.as_ptr(), u8::from(pending)) };
    }

    pub fn set_definition_path(&self, path: &str) -> Result<(), String> {
        let path = c_string(path, "definition path")?;
        unsafe { ffi::pliant_browser_host_set_definition_path(self.raw.as_ptr(), path.as_ptr()) };
        Ok(())
    }

    pub fn set_status(&self, text: &str, is_error: bool) -> Result<(), String> {
        let text = c_string(text, "status")?;
        unsafe {
            ffi::pliant_browser_host_set_status(
                self.raw.as_ptr(),
                text.as_ptr(),
                u8::from(is_error),
            )
        };
        Ok(())
    }

    pub fn content_container(&self) -> Result<NonNull<c_void>, String> {
        NonNull::new(unsafe { ffi::pliant_browser_host_content_container(self.raw.as_ptr()) })
            .ok_or_else(|| "rendered layout has no content container".to_owned())
    }

    pub fn update_pages(
        &self,
        pages: &[PageState],
        selected: Option<HostPageId>,
    ) -> Result<(), String> {
        unsafe { ffi::pliant_browser_host_pages_begin(self.raw.as_ptr()) };
        for page in pages {
            let label = c_string(
                &format!("Page {} - {}", page.id().get(), page.url()),
                "page label",
            )?;
            unsafe {
                ffi::pliant_browser_host_page_add(
                    self.raw.as_ptr(),
                    page.id().get(),
                    label.as_ptr(),
                    u8::from(selected == Some(page.id())),
                )
            };
        }
        unsafe { ffi::pliant_browser_host_pages_end(self.raw.as_ptr()) };
        let current_url = selected
            .and_then(|selected| pages.iter().find(|page| page.id() == selected))
            .map(PageState::url)
            .unwrap_or_default();
        let current_url = c_string(current_url, "current URL")?;
        unsafe {
            ffi::pliant_browser_host_set_current_url(self.raw.as_ptr(), current_url.as_ptr())
        };
        Ok(())
    }

    pub fn update_control_states(&self, controller: &BrowserController) -> Result<(), String> {
        fn visit(
            host: &NativeHost,
            controller: &BrowserController,
            node: &Node,
        ) -> Result<(), String> {
            match node {
                Node::Row(container) | Node::Column(container) => {
                    for child in container.children() {
                        visit(host, controller, child)?;
                    }
                }
                Node::Button(button) => {
                    let id = c_string(button.id(), "button ID")?;
                    unsafe {
                        ffi::pliant_browser_host_set_node_enabled(
                            host.raw.as_ptr(),
                            id.as_ptr(),
                            u8::from(controller.button_enabled(button.id())),
                        )
                    };
                }
                Node::Spacer(_)
                | Node::Label(_)
                | Node::AddressField(_)
                | Node::PageList(_)
                | Node::ContentSurface(_) => {}
            }
            Ok(())
        }

        visit(self, controller, controller.active_definition().root())
    }

    fn check(code: i32, operation: &str) -> Result<(), String> {
        if code == 0 {
            Ok(())
        } else {
            Err(format!("AppKit host failed to {operation}"))
        }
    }
}

impl Drop for NativeHost {
    fn drop(&mut self) {
        unsafe { ffi::pliant_browser_host_destroy(self.raw.as_ptr()) };
    }
}

impl LayoutSink for NativeHost {
    fn begin(&mut self, generation: u64) -> Result<(), String> {
        Self::check(
            unsafe { ffi::pliant_browser_host_begin_layout(self.raw.as_ptr(), generation) },
            "begin layout",
        )
    }

    fn container(
        &mut self,
        parent: Option<&str>,
        id: &str,
        axis: LayoutAxis,
        gap: Option<f64>,
    ) -> Result<(), String> {
        let parent = c_string(parent.unwrap_or_default(), "container parent")?;
        let id = c_string(id, "container ID")?;
        Self::check(
            unsafe {
                ffi::pliant_browser_host_add_container(
                    self.raw.as_ptr(),
                    parent.as_ptr(),
                    id.as_ptr(),
                    match axis {
                        LayoutAxis::Row => 0,
                        LayoutAxis::Column => 1,
                    },
                    gap.unwrap_or(0.0),
                )
            },
            "add container",
        )
    }

    fn spacer(&mut self, parent: &str, id: &str, size: f64) -> Result<(), String> {
        let parent = c_string(parent, "spacer parent")?;
        let id = c_string(id, "spacer ID")?;
        Self::check(
            unsafe {
                ffi::pliant_browser_host_add_spacer(
                    self.raw.as_ptr(),
                    parent.as_ptr(),
                    id.as_ptr(),
                    size,
                )
            },
            "add spacer",
        )
    }

    fn label(&mut self, parent: &str, id: &str, text: &str) -> Result<(), String> {
        let parent = c_string(parent, "label parent")?;
        let id = c_string(id, "label ID")?;
        let text = c_string(text, "label text")?;
        Self::check(
            unsafe {
                ffi::pliant_browser_host_add_label(
                    self.raw.as_ptr(),
                    parent.as_ptr(),
                    id.as_ptr(),
                    text.as_ptr(),
                )
            },
            "add label",
        )
    }

    fn address_field(
        &mut self,
        parent: &str,
        id: &str,
        placeholder: Option<&str>,
    ) -> Result<(), String> {
        let parent = c_string(parent, "address parent")?;
        let id = c_string(id, "address ID")?;
        let placeholder = c_string(placeholder.unwrap_or_default(), "address placeholder")?;
        Self::check(
            unsafe {
                ffi::pliant_browser_host_add_address(
                    self.raw.as_ptr(),
                    parent.as_ptr(),
                    id.as_ptr(),
                    placeholder.as_ptr(),
                )
            },
            "add address field",
        )
    }

    fn page_list(&mut self, parent: &str, id: &str) -> Result<(), String> {
        let parent = c_string(parent, "page-list parent")?;
        let id = c_string(id, "page-list ID")?;
        Self::check(
            unsafe {
                ffi::pliant_browser_host_add_page_list(
                    self.raw.as_ptr(),
                    parent.as_ptr(),
                    id.as_ptr(),
                )
            },
            "add page list",
        )
    }

    fn content_surface(&mut self, parent: &str, id: &str) -> Result<(), String> {
        let parent = c_string(parent, "content parent")?;
        let id = c_string(id, "content ID")?;
        Self::check(
            unsafe {
                ffi::pliant_browser_host_add_content(
                    self.raw.as_ptr(),
                    parent.as_ptr(),
                    id.as_ptr(),
                )
            },
            "add content surface",
        )
    }

    fn button(&mut self, parent: &str, id: &str, label: &str) -> Result<(), String> {
        let parent = c_string(parent, "button parent")?;
        let id = c_string(id, "button ID")?;
        let label = c_string(label, "button label")?;
        Self::check(
            unsafe {
                ffi::pliant_browser_host_add_button(
                    self.raw.as_ptr(),
                    parent.as_ptr(),
                    id.as_ptr(),
                    label.as_ptr(),
                )
            },
            "add button",
        )
    }

    fn end(&mut self) -> Result<(), String> {
        Self::check(
            unsafe { ffi::pliant_browser_host_end_layout(self.raw.as_ptr()) },
            "finish layout",
        )
    }
}

impl LayoutRenderer for NativeHost {
    fn render(
        &mut self,
        definition: &BrowserDefinition,
        generation: u64,
        pages: &[PageState],
        selected: Option<HostPageId>,
    ) -> Result<(), String> {
        validate_native_definition(definition)?;
        render_definition(definition, generation, self)?;
        self.update_pages(pages, selected)
    }
}

fn c_string(value: &str, field: &str) -> Result<CString, String> {
    CString::new(value).map_err(|_| format!("{field} contains a NUL byte"))
}

mod ffi {
    use super::{HostCallback, c_char, c_void};

    unsafe extern "C" {
        pub fn pliant_browser_host_create(
            callback: HostCallback,
            context: *mut c_void,
        ) -> *mut c_void;
        pub fn pliant_browser_host_destroy(host: *mut c_void);
        pub fn pliant_browser_host_show(host: *mut c_void);
        pub fn pliant_browser_host_set_preview_mode(host: *mut c_void, preview_mode: u8);
        pub fn pliant_browser_host_set_preview_pending(host: *mut c_void, pending: u8);
        pub fn pliant_browser_host_set_preview_checking(host: *mut c_void, checking: u8);
        pub fn pliant_browser_host_set_save_pending(host: *mut c_void, pending: u8);
        pub fn pliant_browser_host_set_definition_path(host: *mut c_void, path: *const c_char);
        pub fn pliant_browser_host_set_status(host: *mut c_void, text: *const c_char, is_error: u8);
        pub fn pliant_browser_host_begin_layout(host: *mut c_void, generation: u64) -> i32;
        pub fn pliant_browser_host_add_container(
            host: *mut c_void,
            parent: *const c_char,
            node_id: *const c_char,
            axis: u8,
            gap: f64,
        ) -> i32;
        pub fn pliant_browser_host_add_spacer(
            host: *mut c_void,
            parent: *const c_char,
            node_id: *const c_char,
            size: f64,
        ) -> i32;
        pub fn pliant_browser_host_add_label(
            host: *mut c_void,
            parent: *const c_char,
            node_id: *const c_char,
            text: *const c_char,
        ) -> i32;
        pub fn pliant_browser_host_add_address(
            host: *mut c_void,
            parent: *const c_char,
            node_id: *const c_char,
            placeholder: *const c_char,
        ) -> i32;
        pub fn pliant_browser_host_add_page_list(
            host: *mut c_void,
            parent: *const c_char,
            node_id: *const c_char,
        ) -> i32;
        pub fn pliant_browser_host_add_content(
            host: *mut c_void,
            parent: *const c_char,
            node_id: *const c_char,
        ) -> i32;
        pub fn pliant_browser_host_add_button(
            host: *mut c_void,
            parent: *const c_char,
            node_id: *const c_char,
            label: *const c_char,
        ) -> i32;
        pub fn pliant_browser_host_end_layout(host: *mut c_void) -> i32;
        pub fn pliant_browser_host_pages_begin(host: *mut c_void);
        pub fn pliant_browser_host_page_add(
            host: *mut c_void,
            page: u64,
            label: *const c_char,
            selected: u8,
        );
        pub fn pliant_browser_host_pages_end(host: *mut c_void);
        pub fn pliant_browser_host_set_current_url(host: *mut c_void, url: *const c_char);
        pub fn pliant_browser_host_set_node_enabled(
            host: *mut c_void,
            node_id: *const c_char,
            enabled: u8,
        );
        pub fn pliant_browser_host_content_container(host: *mut c_void) -> *mut c_void;
    }
}
