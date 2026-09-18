#![allow(unsafe_code)]

use pliant_embedder::{Engine, Error, Event, NativeContainer, PageId};
use pliant_embedder_test::fixture_response;
use std::cell::RefCell;
use std::ffi::{CStr, CString, OsString, c_char, c_void};
use std::fs::{File, OpenOptions};
use std::io::{Read, Write};
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::os::unix::ffi::OsStrExt;
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::ptr::NonNull;
use std::sync::{
    Arc, Mutex, OnceLock,
    atomic::{AtomicBool, Ordering},
};
use std::thread::{self, JoinHandle};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

const DIAGNOSTIC_LOG_ENV: &str = "PLIANT_EMBEDDER_DIAGNOSTIC_LOG";
static DIAGNOSTIC_LOG: OnceLock<Mutex<File>> = OnceLock::new();

fn diagnostic_path(
    arguments: impl IntoIterator<Item = OsString>,
    environment_path: Option<OsString>,
) -> Result<Option<PathBuf>, String> {
    let mut arguments = arguments.into_iter();
    let _executable = arguments.next();
    let mut explicit_path = None;
    while let Some(argument) = arguments.next() {
        if argument == "--diagnostic-log" {
            let path = arguments
                .next()
                .ok_or_else(|| "--diagnostic-log requires a path".to_owned())?;
            if explicit_path.replace(path).is_some() {
                return Err("--diagnostic-log may only be specified once".to_owned());
            }
        } else if let Some(path) = argument
            .to_str()
            .and_then(|argument| argument.strip_prefix("--diagnostic-log="))
            && explicit_path.replace(OsString::from(path)).is_some()
        {
            return Err("--diagnostic-log may only be specified once".to_owned());
        }
    }
    let path = explicit_path.or(environment_path).map(PathBuf::from);
    if path
        .as_ref()
        .is_some_and(|path| path.as_os_str().is_empty())
    {
        return Err("diagnostic log path must not be empty".to_owned());
    }
    Ok(path)
}

fn configure_diagnostics() -> Result<Option<PathBuf>, String> {
    let path = diagnostic_path(std::env::args_os(), std::env::var_os(DIAGNOSTIC_LOG_ENV))?;
    let Some(path) = path else {
        return Ok(None);
    };
    let file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(&path)
        .map_err(|error| format!("failed to open diagnostic log {}: {error}", path.display()))?;
    DIAGNOSTIC_LOG
        .set(Mutex::new(file))
        .map_err(|_| "diagnostic logging was configured more than once".to_owned())?;
    let native_path = CString::new(path.as_os_str().as_bytes())
        .map_err(|_| "diagnostic log path contains an interior NUL byte".to_owned())?;
    unsafe { ffi::pliant_embedder_test_host_set_diagnostic_log(native_path.as_ptr()) };
    Ok(Some(path))
}

fn diagnostic(message: impl AsRef<str>) {
    let Some(log) = DIAGNOSTIC_LOG.get() else {
        return;
    };
    let mut log = match log.lock() {
        Ok(log) => log,
        Err(poisoned) => poisoned.into_inner(),
    };
    let elapsed = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or(Duration::ZERO);
    if let Err(error) = writeln!(
        log,
        "{}.{:03} pid={} rust: {}",
        elapsed.as_secs(),
        elapsed.subsec_millis(),
        std::process::id(),
        message.as_ref()
    )
    .and_then(|_| log.flush())
    {
        eprintln!("failed to write diagnostic log: {error}");
    }
}

fn panic_message(payload: &(dyn std::any::Any + Send)) -> &str {
    payload
        .downcast_ref::<&str>()
        .copied()
        .or_else(|| payload.downcast_ref::<String>().map(String::as_str))
        .unwrap_or("non-string panic payload")
}

fn install_panic_logging() {
    let previous = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        diagnostic(format!("panic: {info}"));
        previous(info);
    }));
}

fn native_error(operation: &str) -> String {
    let pointer = unsafe { ffi::pliant_embedder_test_host_last_error() };
    if pointer.is_null() {
        return format!("{operation} failed without a native error");
    }
    let detail = unsafe { CStr::from_ptr(pointer) }.to_string_lossy();
    if detail.is_empty() {
        format!("{operation} failed without a native error")
    } else {
        format!("{operation} failed: {detail}")
    }
}

const ACTION_CREATE_A: u32 = 0;
const ACTION_LOAD_A: u32 = 1;
const ACTION_LOAD_B: u32 = 2;
const ACTION_BACK: u32 = 3;
const ACTION_FORWARD: u32 = 4;
const ACTION_CLOSE: u32 = 5;
const ACTION_PROBE_STALE: u32 = 6;
const ACTION_QUIT: u32 = 7;

struct FixtureServer {
    address: SocketAddr,
    stopping: Arc<AtomicBool>,
    thread: Option<JoinHandle<()>>,
}

impl FixtureServer {
    fn start() -> Result<Self, String> {
        let listener = TcpListener::bind("127.0.0.1:0")
            .map_err(|error| format!("failed to bind loopback fixture: {error}"))?;
        listener
            .set_nonblocking(true)
            .map_err(|error| format!("failed to configure loopback fixture: {error}"))?;
        let address = listener
            .local_addr()
            .map_err(|error| format!("failed to read loopback address: {error}"))?;
        let stopping = Arc::new(AtomicBool::new(false));
        let worker_stopping = Arc::clone(&stopping);
        let thread = thread::Builder::new()
            .name("pliant-embedder-test-fixture".to_owned())
            .spawn(move || {
                while !worker_stopping.load(Ordering::Acquire) {
                    match listener.accept() {
                        Ok((stream, _)) => serve_fixture(stream),
                        Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                            thread::sleep(Duration::from_millis(5));
                        }
                        Err(error) => {
                            let message =
                                format!("loopback fixture stopped after accept error: {error}");
                            diagnostic(&message);
                            eprintln!("{message}");
                            break;
                        }
                    }
                }
            })
            .map_err(|error| format!("failed to start loopback fixture: {error}"))?;
        Ok(Self {
            address,
            stopping,
            thread: Some(thread),
        })
    }

    fn url(&self, path: &str) -> String {
        format!("http://{}{}", self.address, path)
    }
}

impl Drop for FixtureServer {
    fn drop(&mut self) {
        self.stopping.store(true, Ordering::Release);
        let _ = TcpStream::connect(self.address);
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

fn serve_fixture(mut stream: TcpStream) {
    let _ = stream.set_read_timeout(Some(Duration::from_secs(2)));
    let mut request = [0_u8; 4096];
    let Ok(length) = stream.read(&mut request) else {
        return;
    };
    let request = String::from_utf8_lossy(&request[..length]);
    let path = request
        .lines()
        .next()
        .and_then(|line| line.split_whitespace().nth(1))
        .unwrap_or("/");
    let (status, body) = fixture_response(path);
    let response = format!(
        "HTTP/1.1 {status}\r\nContent-Type: text/html; charset=utf-8\r\nContent-Length: {}\r\nCache-Control: no-store\r\nConnection: close\r\n\r\n{body}",
        body.len()
    );
    if let Err(error) = stream.write_all(response.as_bytes()) {
        diagnostic(format!(
            "failed to serve loopback fixture response: {error}"
        ));
        eprintln!("failed to serve loopback fixture response: {error}");
    }
}

struct NativeHost {
    raw: NonNull<c_void>,
}

impl NativeHost {
    unsafe fn new(
        callback: unsafe extern "C" fn(*mut c_void, u32),
        context: *mut c_void,
        url_a: &str,
        url_b: &str,
    ) -> Result<Self, String> {
        let url_a = c_string(url_a, "fixture A URL")?;
        let url_b = c_string(url_b, "fixture B URL")?;
        let raw = unsafe {
            ffi::pliant_embedder_test_host_create(callback, context, url_a.as_ptr(), url_b.as_ptr())
        };
        Ok(Self {
            raw: NonNull::new(raw).ok_or_else(|| native_error("AppKit host creation"))?,
        })
    }

    fn show(&self) -> Result<(), String> {
        let succeeded = unsafe { ffi::pliant_embedder_test_host_show(self.raw.as_ptr()) };
        if succeeded == 0 {
            Err(native_error("host show"))
        } else {
            Ok(())
        }
    }

    fn content_container(&self) -> Result<NativeContainer, String> {
        let raw = unsafe { ffi::pliant_embedder_test_host_content_container(self.raw.as_ptr()) };
        if raw.is_null() {
            return Err(native_error("content container lookup"));
        }
        unsafe { NativeContainer::from_raw_ns_view(raw) }.map_err(|error| error.to_string())
    }

    fn set_status(&self, text: &str, is_error: bool) -> Result<(), String> {
        let text = c_string(text, "status")?;
        let succeeded = unsafe {
            ffi::pliant_embedder_test_host_set_status(
                self.raw.as_ptr(),
                text.as_ptr(),
                u8::from(is_error),
            )
        };
        if succeeded == 0 {
            return Err(native_error("status update"));
        }
        println!(
            "{}: {}",
            if is_error { "ERROR" } else { "STATUS" },
            text.to_string_lossy()
        );
        diagnostic(format!(
            "{}: {}",
            if is_error { "ERROR" } else { "STATUS" },
            text.to_string_lossy()
        ));
        Ok(())
    }

    fn set_identity(&self, text: &str) -> Result<(), String> {
        let text = c_string(text, "identity")?;
        let succeeded = unsafe {
            ffi::pliant_embedder_test_host_set_identity(self.raw.as_ptr(), text.as_ptr())
        };
        if succeeded == 0 {
            Err(native_error("identity update"))
        } else {
            Ok(())
        }
    }
}

impl Drop for NativeHost {
    fn drop(&mut self) {
        unsafe { ffi::pliant_embedder_test_host_destroy(self.raw.as_ptr()) };
    }
}

struct AppRuntime {
    url_a: String,
    url_b: String,
    active: Option<PageId>,
    stale: Option<PageId>,
    container: Option<NativeContainer>,
    host: Option<NativeHost>,
    quit_requested: bool,
    _fixture: FixtureServer,
}

impl AppRuntime {
    fn start(&mut self, context: *mut c_void) -> Result<(), String> {
        let host = unsafe { NativeHost::new(host_callback, context, &self.url_a, &self.url_b)? };
        let container = host.content_container()?;
        self.container = Some(container);
        self.host = Some(host);
        self.update_identity()?;
        self.status(
            "Ready. Click Create A to create a page through Engine::create_page_in.",
            false,
        )?;
        self.host()?.show()?;
        diagnostic("native host ordered front without activation");
        Ok(())
    }

    fn handle_action(&mut self, engine: &Engine, action: u32) -> Result<(), String> {
        let result = match action {
            ACTION_CREATE_A => {
                if self.active.is_some() {
                    Err(
                        "an active page already exists; close it before creating another"
                            .to_owned(),
                    )
                } else {
                    let page = engine
                        .create_page_in(self.container()?, &self.url_a)
                        .map_err(|error| format!("create_page_in(A) failed: {error}"))?;
                    self.active = Some(page);
                    self.update_identity()?;
                    self.status(
                        &format!(
                            "create_page_in(A) accepted for page {}; awaiting navigation/paint",
                            page.as_u64()
                        ),
                        false,
                    )
                }
            }
            ACTION_LOAD_A => {
                let page = self.active_page()?;
                engine.load_url(page, &self.url_a).map_err(|error| {
                    format!("load_url(A) failed for page {}: {error}", page.as_u64())
                })?;
                self.status(
                    &format!("load_url(A) accepted for page {}", page.as_u64()),
                    false,
                )
            }
            ACTION_LOAD_B => {
                let page = self.active_page()?;
                engine.load_url(page, &self.url_b).map_err(|error| {
                    format!("load_url(B) failed for page {}: {error}", page.as_u64())
                })?;
                self.status(
                    &format!("load_url(B) accepted for page {}", page.as_u64()),
                    false,
                )
            }
            ACTION_BACK => {
                let page = self.active_page()?;
                engine
                    .back(page)
                    .map_err(|error| format!("back failed for page {}: {error}", page.as_u64()))?;
                self.status(&format!("back accepted for page {}", page.as_u64()), false)
            }
            ACTION_FORWARD => {
                let page = self.active_page()?;
                engine.forward(page).map_err(|error| {
                    format!("forward failed for page {}: {error}", page.as_u64())
                })?;
                self.status(
                    &format!("forward accepted for page {}", page.as_u64()),
                    false,
                )
            }
            ACTION_CLOSE => {
                let page = self.active_page()?;
                engine.close_page(page).map_err(|error| {
                    format!("close_page failed for page {}: {error}", page.as_u64())
                })?;
                self.status(
                    &format!(
                        "close_page accepted for page {}; awaiting Closed before stale-ID probe",
                        page.as_u64()
                    ),
                    false,
                )
            }
            ACTION_PROBE_STALE => self.probe_stale(engine),
            ACTION_QUIT => {
                self.quit_requested = true;
                if let Some(page) = self.active {
                    engine.close_page(page).map_err(|error| {
                        format!(
                            "window close could not close page {}: {error}",
                            page.as_u64()
                        )
                    })
                } else {
                    engine
                        .shutdown()
                        .map_err(|error| format!("engine shutdown failed: {error}"))
                }
            }
            other => Err(format!("unknown native control action {other}")),
        };
        if let Err(error) = &result {
            self.status(error, true)?;
        }
        result
    }

    fn probe_stale(&self, engine: &Engine) -> Result<(), String> {
        let page = self
            .stale
            .ok_or_else(|| "no closed page ID exists; close an active page first".to_owned())?;
        let checks = [
            ("load_url", engine.load_url(page, &self.url_a)),
            ("back", engine.back(page)),
            ("forward", engine.forward(page)),
            ("close_page", engine.close_page(page)),
        ];
        let failures = checks
            .into_iter()
            .filter(|(_, result)| *result != Err(Error::InvalidPage))
            .map(|(command, result)| format!("{command}={result:?}"))
            .collect::<Vec<_>>();
        if failures.is_empty() {
            self.status(
                &format!(
                    "PASS stale page {}: load_url/back/forward/close_page all returned InvalidPage",
                    page.as_u64()
                ),
                false,
            )
        } else {
            Err(format!(
                "stale page {} was not rejected by every API: {}",
                page.as_u64(),
                failures.join(", ")
            ))
        }
    }

    fn handle_event(&mut self, engine: &Engine, event: Event) -> Result<(), String> {
        match event {
            Event::Ready => Err("duplicate engine Ready event".to_owned()),
            Event::Navigated {
                page,
                url,
                can_go_back,
                can_go_forward,
            } => self.status(
                &format!(
                    "Navigated page {}: {url} | back={can_go_back} forward={can_go_forward}",
                    page.as_u64()
                ),
                false,
            ),
            Event::Painted { page, url } => {
                self.status(&format!("Painted page {}: {url}", page.as_u64()), false)
            }
            Event::NavigationFailed { page, code } => Err(format!(
                "navigation failed for page {} with native code {code}",
                page.as_u64()
            )),
            Event::RendererFailed { page } => {
                Err(format!("renderer failed for page {}", page.as_u64()))
            }
            Event::CloseRefused { page } => self.status(
                &format!(
                    "close refused for page {}; page remains active",
                    page.as_u64()
                ),
                true,
            ),
            Event::Closed { page } => {
                if self.active != Some(page) {
                    return Err(format!(
                        "Closed event targeted unknown page {}",
                        page.as_u64()
                    ));
                }
                self.active = None;
                self.stale = Some(page);
                self.update_identity()?;
                if self.quit_requested {
                    engine
                        .shutdown()
                        .map_err(|error| format!("engine shutdown after Closed failed: {error}"))
                } else {
                    self.status(
                        &format!(
                            "Closed page {}. Click Probe stale ID to verify InvalidPage from all APIs.",
                            page.as_u64()
                        ),
                        false,
                    )
                }
            }
        }
    }

    fn active_page(&self) -> Result<PageId, String> {
        self.active
            .ok_or_else(|| "no active page; click Create A first".to_owned())
    }

    fn container(&self) -> Result<&NativeContainer, String> {
        self.container
            .as_ref()
            .ok_or_else(|| "native content container is not ready".to_owned())
    }

    fn host(&self) -> Result<&NativeHost, String> {
        self.host
            .as_ref()
            .ok_or_else(|| "native host is not ready".to_owned())
    }

    fn status(&self, text: &str, is_error: bool) -> Result<(), String> {
        self.host()?.set_status(text, is_error)
    }

    fn update_identity(&self) -> Result<(), String> {
        let active = self
            .active
            .map(|page| page.as_u64().to_string())
            .unwrap_or_else(|| "none".to_owned());
        let stale = self
            .stale
            .map(|page| page.as_u64().to_string())
            .unwrap_or_else(|| "none".to_owned());
        self.host()?
            .set_identity(&format!("Active page: {active} | stale page: {stale}"))
    }
}

unsafe extern "C" fn host_callback(context: *mut c_void, action: u32) {
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let runtime = unsafe { &*context.cast::<RefCell<AppRuntime>>() };
        let mut runtime = runtime
            .try_borrow_mut()
            .map_err(|_| "native control arrived during another callback".to_owned())?;
        pliant_embedder::with_engine(|engine| runtime.handle_action(engine, action))
            .map_err(|error| format!("engine unavailable to native control: {error}"))?
    }));
    match result {
        Ok(Ok(())) => {}
        Ok(Err(error)) => {
            diagnostic(format!("native control action failed: {error}"));
            let runtime = unsafe { &*context.cast::<RefCell<AppRuntime>>() };
            if let Ok(runtime) = runtime.try_borrow() {
                let _ = runtime.status(&error, true);
            }
        }
        Err(_) => {
            diagnostic("native control callback panicked; shutting down engine");
            eprintln!("native control callback panicked; shutting down engine");
            let _ = pliant_embedder::with_engine(|engine| engine.shutdown());
        }
    }
}

fn c_string(value: &str, field: &str) -> Result<CString, String> {
    CString::new(value).map_err(|_| format!("{field} contains an interior NUL byte"))
}

fn run_app() -> Result<(), Box<dyn std::error::Error>> {
    diagnostic("starting loopback fixture");
    let fixture = FixtureServer::start()?;
    diagnostic(format!("loopback fixture listening on {}", fixture.address));
    let url_a = fixture.url("/a");
    let url_b = fixture.url("/b");
    let runtime = Box::new(RefCell::new(AppRuntime {
        url_a,
        url_b,
        active: None,
        stale: None,
        container: None,
        host: None,
        quit_requested: false,
        _fixture: fixture,
    }));
    let context = NonNull::from(runtime.as_ref()).cast::<c_void>().as_ptr();
    let mut failure = None;
    diagnostic("entering pliant_embedder::run");
    let result = pliant_embedder::run(|engine, event| {
        diagnostic(format!("engine event: {event:?}"));
        let action = match event {
            Event::Ready => runtime.borrow_mut().start(context),
            event => runtime.borrow_mut().handle_event(engine, event),
        };
        if let Err(error) = action {
            diagnostic(format!("engine event handling failed: {error}"));
            if let Ok(runtime) = runtime.try_borrow() {
                let _ = runtime.status(&error, true);
            }
            failure = Some(error);
            let _ = engine.shutdown();
        }
    });
    diagnostic(format!("pliant_embedder::run returned {result:?}"));
    result?;
    if let Some(error) = failure {
        diagnostic(format!("application failure after engine return: {error}"));
        return Err(error.into());
    }
    Ok(())
}

fn main() -> ExitCode {
    let diagnostic_path = match configure_diagnostics() {
        Ok(path) => path,
        Err(error) => {
            eprintln!("startup diagnostic configuration failed: {error}");
            return ExitCode::FAILURE;
        }
    };
    install_panic_logging();
    diagnostic(format!(
        "process entry; executable={}; diagnostic_log={}",
        std::env::current_exe()
            .map(|path| path.display().to_string())
            .unwrap_or_else(|error| format!("<unavailable: {error}>")),
        diagnostic_path
            .as_deref()
            .map(Path::display)
            .map(|path| path.to_string())
            .unwrap_or_else(|| "disabled".to_owned())
    ));
    match std::panic::catch_unwind(std::panic::AssertUnwindSafe(run_app)) {
        Ok(Ok(())) => {
            diagnostic("process exiting successfully");
            ExitCode::SUCCESS
        }
        Ok(Err(error)) => {
            diagnostic(format!("process exiting with error: {error}"));
            eprintln!("pliant embedder diagnostic failed: {error}");
            ExitCode::FAILURE
        }
        Err(payload) => {
            let message = panic_message(payload.as_ref());
            diagnostic(format!("process exiting after panic: {message}"));
            eprintln!("pliant embedder diagnostic panicked: {message}");
            ExitCode::FAILURE
        }
    }
}

mod ffi {
    use super::{c_char, c_void};

    unsafe extern "C" {
        pub fn pliant_embedder_test_host_set_diagnostic_log(path: *const c_char);
        pub fn pliant_embedder_test_host_last_error() -> *const c_char;
        pub fn pliant_embedder_test_host_create(
            callback: unsafe extern "C" fn(*mut c_void, u32),
            context: *mut c_void,
            url_a: *const c_char,
            url_b: *const c_char,
        ) -> *mut c_void;
        pub fn pliant_embedder_test_host_destroy(host: *mut c_void);
        pub fn pliant_embedder_test_host_show(host: *mut c_void) -> u8;
        pub fn pliant_embedder_test_host_set_status(
            host: *mut c_void,
            text: *const c_char,
            is_error: u8,
        ) -> u8;
        pub fn pliant_embedder_test_host_set_identity(host: *mut c_void, text: *const c_char)
        -> u8;
        pub fn pliant_embedder_test_host_content_container(host: *mut c_void) -> *mut c_void;
    }
}

#[cfg(test)]
mod tests {
    use super::diagnostic_path;
    use std::ffi::OsString;
    use std::path::PathBuf;

    #[test]
    fn explicit_diagnostic_log_path_takes_precedence_over_environment() {
        let path = diagnostic_path(
            [
                OsString::from("app"),
                OsString::from("--diagnostic-log"),
                OsString::from("/tmp/explicit.log"),
            ],
            Some(OsString::from("/tmp/environment.log")),
        )
        .unwrap();

        assert_eq!(path, Some(PathBuf::from("/tmp/explicit.log")));
    }

    #[test]
    fn diagnostic_log_accepts_environment_and_equals_forms() {
        assert_eq!(
            diagnostic_path(
                [OsString::from("app")],
                Some(OsString::from("/tmp/environment.log"))
            )
            .unwrap(),
            Some(PathBuf::from("/tmp/environment.log"))
        );
        assert_eq!(
            diagnostic_path(
                [
                    OsString::from("app"),
                    OsString::from("--diagnostic-log=/tmp/equals.log"),
                ],
                None
            )
            .unwrap(),
            Some(PathBuf::from("/tmp/equals.log"))
        );
    }

    #[test]
    fn diagnostic_log_rejects_missing_empty_and_duplicate_paths() {
        assert!(
            diagnostic_path(
                [OsString::from("app"), OsString::from("--diagnostic-log")],
                None
            )
            .is_err()
        );
        assert!(
            diagnostic_path(
                [OsString::from("app"), OsString::from("--diagnostic-log=")],
                None
            )
            .is_err()
        );
        assert!(
            diagnostic_path(
                [
                    OsString::from("app"),
                    OsString::from("--diagnostic-log"),
                    OsString::from("/tmp/one.log"),
                    OsString::from("--diagnostic-log=/tmp/two.log"),
                ],
                None
            )
            .is_err()
        );
    }
}
