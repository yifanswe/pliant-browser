use pliant_ui_definition::{
    AddressOpenResolution, BrowserDefinition, CandidateId, Command, DefinitionState, Node,
    resolve_address_open,
};
use std::fs::{self, OpenOptions};
use std::io::{Read, Write};
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::process::{Child, Command as ProcessCommand};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, atomic::AtomicBool};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

#[cfg(target_os = "macos")]
#[allow(unsafe_code)]
pub mod native;

static NEXT_ATOMIC_FILE: AtomicU64 = AtomicU64::new(1);
static NEXT_PREVIEW_DIRECTORY: AtomicU64 = AtomicU64::new(1);
const DEFAULT_PREVIEW_TIMEOUT: Duration = Duration::from_secs(15);

pub struct FixtureServer {
    address: SocketAddr,
    stopping: Arc<AtomicBool>,
    thread: Option<JoinHandle<()>>,
}

impl FixtureServer {
    pub fn start() -> Result<Self, String> {
        let listener = TcpListener::bind("127.0.0.1:0")
            .map_err(|error| format!("failed to bind acceptance fixture server: {error}"))?;
        listener
            .set_nonblocking(true)
            .map_err(|error| format!("failed to configure acceptance fixture server: {error}"))?;
        let address = listener
            .local_addr()
            .map_err(|error| format!("failed to read acceptance fixture address: {error}"))?;
        let stopping = Arc::new(AtomicBool::new(false));
        let worker_stopping = Arc::clone(&stopping);
        let events = Arc::new(Mutex::new(Vec::new()));
        let worker_events = Arc::clone(&events);
        let thread = thread::Builder::new()
            .name("pliant-acceptance-fixture".to_owned())
            .spawn(move || {
                while !worker_stopping.load(Ordering::Acquire) {
                    match listener.accept() {
                        Ok((stream, _)) => serve_fixture_request(stream, &worker_events),
                        Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                            thread::sleep(Duration::from_millis(5));
                        }
                        Err(_) => break,
                    }
                }
            })
            .map_err(|error| format!("failed to start acceptance fixture thread: {error}"))?;
        Ok(Self {
            address,
            stopping,
            thread: Some(thread),
        })
    }

    pub fn url(&self, path: &str) -> String {
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

fn serve_fixture_request(mut stream: TcpStream, events: &Mutex<Vec<String>>) {
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
    let (status, content_type, body) = match path {
        "/a" => ("200 OK", "text/html; charset=utf-8", FIXTURE_A),
        "/b" => ("200 OK", "text/html; charset=utf-8", FIXTURE_B),
        "/confirm" => ("200 OK", "text/html; charset=utf-8", FIXTURE_CONFIRM),
        "/events" => {
            let body = events
                .lock()
                .map(|events| {
                    format!(
                        "<!doctype html><meta charset=\"utf-8\"><title>Lifecycle events</title><h1>Lifecycle events</h1><pre id=\"events\">{}</pre>",
                        events.join("\n")
                    )
                })
                .unwrap_or_else(|_| "fixture event log unavailable".to_owned());
            write_fixture_response(&mut stream, "200 OK", "text/html; charset=utf-8", &body);
            return;
        }
        path if path.starts_with("/event?kind=") => {
            if let Ok(mut events) = events.lock() {
                events.push(path.trim_start_matches("/event?kind=").to_owned());
            }
            write_fixture_response(&mut stream, "204 No Content", "text/plain", "");
            return;
        }
        "/reset-events" => {
            if let Ok(mut events) = events.lock() {
                events.clear();
            }
            write_fixture_response(&mut stream, "204 No Content", "text/plain", "");
            return;
        }
        _ => (
            "404 Not Found",
            "text/html; charset=utf-8",
            FIXTURE_NOT_FOUND,
        ),
    };
    write_fixture_response(&mut stream, status, content_type, body);
}

fn write_fixture_response(stream: &mut TcpStream, status: &str, content_type: &str, body: &str) {
    let response = format!(
        "HTTP/1.1 {status}\r\nContent-Type: {content_type}\r\nContent-Length: {}\r\nCache-Control: no-store\r\nConnection: close\r\n\r\n{body}",
        body.len()
    );
    let _ = stream.write_all(response.as_bytes());
}

const FIXTURE_A: &str = r#"<!doctype html>
<meta charset="utf-8">
<title>Pliant fixture A</title>
<h1>Fixture A</h1>
<label>Draft <input id="draft" autocomplete="off"></label>
<p id="lifecycle">active</p>
<p id="storage"></p>
<button id="set-storage">Set process-isolation marker</button>
<button id="reset-events">Reset lifecycle log</button>
<a href="/events">View lifecycle log</a>
<script>
const lifecycle = document.getElementById("lifecycle");
const storage = document.getElementById("storage");
const setStorage = document.getElementById("set-storage");
const resetEvents = document.getElementById("reset-events");
function reportStorage() {
  storage.textContent = "cookie=" + (document.cookie || "(empty)") +
    "; localStorage=" + (localStorage.getItem("pliant-marker") || "(empty)");
}
setStorage.onclick = () => {
  document.cookie = "pliant-marker=main; SameSite=Strict";
  localStorage.setItem("pliant-marker", "main");
  reportStorage();
};
resetEvents.onclick = async () => {
  await fetch("/reset-events", {method: "POST"});
  lifecycle.textContent = "event log reset";
};
addEventListener("pagehide", () => {
  lifecycle.textContent = "pagehide";
  navigator.sendBeacon("/event?kind=pagehide");
});
document.addEventListener("visibilitychange", () => {
  lifecycle.dataset.visibility = document.visibilityState;
  if (document.visibilityState === "hidden") {
    navigator.sendBeacon("/event?kind=visibilitychange-hidden");
  }
});
reportStorage();
</script>
"#;

const FIXTURE_B: &str = r#"<!doctype html>
<meta charset="utf-8">
<title>Pliant fixture B</title>
<h1>Fixture B</h1>
<p>This page is visibly distinct from fixture A.</p>
"#;

const FIXTURE_CONFIRM: &str = r#"<!doctype html>
<meta charset="utf-8">
<title>Pliant beforeunload fixture</title>
<h1>Beforeunload confirmation</h1>
<label>Protected draft <input id="protected-draft" value="keep me"></label>
<p id="attempts">No close attempted</p>
<button id="allow-close">Allow close</button>
<script>
const protectedDraft = document.getElementById("protected-draft");
const attempts = document.getElementById("attempts");
const allowClose = document.getElementById("allow-close");
let dirty = false;
protectedDraft.addEventListener("input", () => dirty = true);
allowClose.onclick = () => {
  dirty = false;
  attempts.textContent = "Close is now allowed";
};
addEventListener("beforeunload", event => {
  if (!dirty) return;
  attempts.textContent = "Dirty close was refused";
  event.preventDefault();
  event.returnValue = "";
});
</script>
"#;

const FIXTURE_NOT_FOUND: &str = r#"<!doctype html><title>Not found</title><h1>Not found</h1>"#;

#[derive(Debug)]
pub struct InitialLayout {
    source: String,
    warning: Option<String>,
}

impl InitialLayout {
    pub fn source(&self) -> &str {
        &self.source
    }

    pub fn warning(&self) -> Option<&str> {
        self.warning.as_deref()
    }
}

pub fn render_initial_layout(
    initial_source: &str,
    initial_warning: Option<String>,
    preview_mode: bool,
    mut render: impl FnMut(&BrowserDefinition) -> Result<(), String>,
) -> Result<InitialLayout, String> {
    let definition = pliant_ui_definition::parse_definition(initial_source)
        .map_err(|error| error.to_string())?;
    let mut effective_source = initial_source.to_owned();
    let mut warning = initial_warning;
    if let Err(error) = render(&definition) {
        if preview_mode {
            return Err(format!("preview candidate failed to render: {error}"));
        }
        if effective_source == pliant_ui_definition::BUILT_IN_SAFE_DEFINITION {
            return Err(format!("built-in safe UI failed to render: {error}"));
        }
        let safe_definition =
            pliant_ui_definition::parse_definition(pliant_ui_definition::BUILT_IN_SAFE_DEFINITION)
                .map_err(|error| error.to_string())?;
        render(&safe_definition).map_err(|recovery| {
            format!(
                "initial customization failed to render: {error}; built-in safe UI also failed: {recovery}"
            )
        })?;
        effective_source = pliant_ui_definition::BUILT_IN_SAFE_DEFINITION.to_owned();
        let recovery = format!(
            "initial customization failed to render; built-in safe UI is active and the original file was preserved: {error}"
        );
        warning = Some(match warning {
            Some(existing) => format!("{existing}; {recovery}"),
            None => recovery,
        });
    }
    Ok(InitialLayout {
        source: effective_source,
        warning,
    })
}

pub struct DefinitionStore {
    directory: PathBuf,
    path: PathBuf,
}

pub trait PreviewLauncher {
    type Handle;

    fn launch(&mut self, source: &str) -> Result<Self::Handle, String>;
    fn stop(&mut self, handle: &mut Self::Handle) -> Result<(), String>;
    fn status(&mut self, handle: &mut Self::Handle) -> Result<PreviewStatus, String>;
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PreviewStatus {
    Starting,
    Ready,
}

pub struct ProcessPreviewLauncher {
    executable: PathBuf,
    root: PathBuf,
    timeout: Duration,
    initial_url: Option<String>,
}

impl ProcessPreviewLauncher {
    pub fn new(executable: impl AsRef<Path>, root: impl AsRef<Path>) -> Self {
        Self {
            executable: executable.as_ref().to_owned(),
            root: root.as_ref().to_owned(),
            timeout: DEFAULT_PREVIEW_TIMEOUT,
            initial_url: None,
        }
    }

    pub fn with_timeout(mut self, timeout: Duration) -> Self {
        self.timeout = timeout;
        self
    }

    pub fn with_initial_url(mut self, initial_url: impl Into<String>) -> Self {
        self.initial_url = Some(initial_url.into());
        self
    }
}

pub struct PreviewProcess {
    child: Option<Child>,
    directory: PathBuf,
    snapshot: PathBuf,
    acknowledgement: PathBuf,
    token: String,
    deadline: Instant,
    ready: bool,
}

impl PreviewProcess {
    pub fn directory(&self) -> &Path {
        &self.directory
    }

    pub fn snapshot_path(&self) -> &Path {
        &self.snapshot
    }

    fn cleanup(&mut self) -> Result<(), String> {
        if let Some(child) = self.child.as_mut() {
            if child
                .try_wait()
                .map_err(|error| format!("failed to inspect preview process: {error}"))?
                .is_none()
            {
                child
                    .kill()
                    .map_err(|error| format!("failed to stop preview process: {error}"))?;
            }
            child
                .wait()
                .map_err(|error| format!("failed to reap preview process: {error}"))?;
            self.child = None;
        }
        if self.directory.exists() {
            fs::remove_dir_all(&self.directory).map_err(|error| {
                format!(
                    "failed to remove owned preview directory {}: {error}",
                    self.directory.display()
                )
            })?;
        }
        Ok(())
    }
}

impl Drop for PreviewProcess {
    fn drop(&mut self) {
        let _ = self.cleanup();
    }
}

impl PreviewLauncher for ProcessPreviewLauncher {
    type Handle = PreviewProcess;

    fn launch(&mut self, source: &str) -> Result<Self::Handle, String> {
        fs::create_dir_all(&self.root).map_err(|error| {
            format!(
                "failed to create preview root {}: {error}",
                self.root.display()
            )
        })?;
        let directory = self.root.join(format!(
            "preview-{}-{}",
            std::process::id(),
            NEXT_PREVIEW_DIRECTORY.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&directory).map_err(|error| {
            format!(
                "failed to create owned preview directory {}: {error}",
                directory.display()
            )
        })?;
        let snapshot = directory.join("candidate.json");
        let state = directory.join("state");
        let acknowledgement = directory.join("startup.ack");
        let token = format!(
            "{}-{}",
            std::process::id(),
            NEXT_PREVIEW_DIRECTORY.fetch_add(1, Ordering::Relaxed)
        );
        let launch = (|| {
            let mut file = OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&snapshot)
                .map_err(|error| {
                    format!(
                        "failed to create preview snapshot {}: {error}",
                        snapshot.display()
                    )
                })?;
            file.write_all(source.as_bytes())
                .and_then(|()| file.sync_all())
                .map_err(|error| {
                    format!(
                        "failed to write preview snapshot {}: {error}",
                        snapshot.display()
                    )
                })?;
            fs::create_dir(&state).map_err(|error| {
                format!(
                    "failed to create preview state directory {}: {error}",
                    state.display()
                )
            })?;
            let mut command = ProcessCommand::new(&self.executable);
            command
                .arg("--preview-snapshot")
                .arg(&snapshot)
                .arg("--state-dir")
                .arg(&state)
                .arg("--preview-ack")
                .arg(&acknowledgement)
                .arg("--preview-token")
                .arg(&token);
            if let Some(initial_url) = &self.initial_url {
                command.arg("--initial-url").arg(initial_url);
            }
            command.spawn().map_err(|error| {
                format!(
                    "failed to launch preview executable {}: {error}",
                    self.executable.display()
                )
            })
        })();
        let child = match launch {
            Ok(child) => child,
            Err(error) => {
                let _ = fs::remove_dir_all(&directory);
                return Err(error);
            }
        };
        let mut handle = PreviewProcess {
            child: Some(child),
            directory,
            snapshot,
            acknowledgement,
            token,
            deadline: Instant::now() + self.timeout,
            ready: false,
        };
        let exited = handle
            .child
            .as_mut()
            .expect("preview child was just assigned")
            .try_wait()
            .map_err(|error| format!("failed to inspect preview process: {error}"))?;
        if let Some(status) = exited {
            let _ = handle.cleanup();
            return Err(format!(
                "preview process exited before launch completed ({status})"
            ));
        }
        Ok(handle)
    }

    fn stop(&mut self, handle: &mut Self::Handle) -> Result<(), String> {
        handle.cleanup()
    }

    fn status(&mut self, handle: &mut Self::Handle) -> Result<PreviewStatus, String> {
        let Some(child) = handle.child.as_mut() else {
            return Err("preview process is no longer running".to_owned());
        };
        if let Some(status) = child
            .try_wait()
            .map_err(|error| format!("failed to inspect preview process: {error}"))?
        {
            handle.child = None;
            return Err(format!(
                "preview process exited before readiness ({status})"
            ));
        }
        if handle.ready {
            return Ok(PreviewStatus::Ready);
        }
        match fs::read_to_string(&handle.acknowledgement) {
            Ok(acknowledgement) => {
                let mut lines = acknowledgement.lines();
                let state = lines.next().unwrap_or_default();
                let token = lines.next().unwrap_or_default();
                if token != handle.token {
                    return Err("preview acknowledgement did not match its candidate".to_owned());
                }
                match state {
                    "ready" => {
                        if Instant::now() >= handle.deadline {
                            return Err("preview startup timed out before readiness".to_owned());
                        }
                        handle.ready = true;
                        return Ok(PreviewStatus::Ready);
                    }
                    "error" => {
                        let message = lines.collect::<Vec<_>>().join("\n");
                        return Err(if message.is_empty() {
                            "preview reported a startup failure".to_owned()
                        } else {
                            format!("preview startup failed: {message}")
                        });
                    }
                    _ => return Err("preview acknowledgement is malformed".to_owned()),
                }
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => {
                return Err(format!(
                    "failed to read preview acknowledgement {}: {error}",
                    handle.acknowledgement.display()
                ));
            }
        }
        if Instant::now() >= handle.deadline {
            return Err("preview startup timed out before readiness".to_owned());
        }
        Ok(PreviewStatus::Starting)
    }
}

pub struct PreviewCoordinator<L: PreviewLauncher> {
    launcher: L,
    running: Option<(CandidateId, L::Handle)>,
}

impl<L: PreviewLauncher> PreviewCoordinator<L> {
    pub fn new(launcher: L) -> Self {
        Self {
            launcher,
            running: None,
        }
    }

    pub fn launcher(&self) -> &L {
        &self.launcher
    }

    pub fn candidate_id(&self) -> Option<CandidateId> {
        self.running.as_ref().map(|running| running.0)
    }

    pub fn refresh(
        &mut self,
        candidate: CandidateId,
        controller: &mut BrowserController,
    ) -> Result<PreviewStatus, String> {
        if self.running.as_ref().map(|running| running.0) != Some(candidate) {
            return Err("preview candidate is stale or was not launched".to_owned());
        }
        let status = {
            let running = self.running.as_mut().expect("candidate checked above");
            self.launcher.status(&mut running.1)
        };
        match status {
            Ok(status) => Ok(status),
            Err(error) => {
                let (_, mut handle) = self.running.take().expect("candidate checked above");
                let rejection = controller.reject_preview(candidate);
                let stopped = self.launcher.stop(&mut handle);
                if let Err(rejection) = rejection {
                    return Err(format!(
                        "{error}; rejecting the failed candidate also failed: {rejection}"
                    ));
                }
                if let Err(stopped) = stopped {
                    return Err(format!(
                        "{error}; stopping the failed preview also failed: {stopped}"
                    ));
                }
                Err(format!("{error}; candidate was rejected"))
            }
        }
    }

    pub fn preview(
        &mut self,
        controller: &mut BrowserController,
        source: &str,
    ) -> Result<CandidateId, String> {
        let definition =
            pliant_ui_definition::parse_definition(source).map_err(|error| error.to_string())?;
        validate_native_definition(&definition)?;
        let mut new_handle = self.launcher.launch(source)?;
        if let Some((old_candidate, old_handle)) = self.running.as_mut() {
            if let Err(error) = self.launcher.stop(old_handle) {
                let _ = self.launcher.stop(&mut new_handle);
                return Err(format!(
                    "failed to stop preview candidate {old_candidate:?}: {error}"
                ));
            }
            self.running = None;
        }
        match controller.preview_definition(source) {
            Ok(candidate) => {
                self.running = Some((candidate, new_handle));
                Ok(candidate)
            }
            Err(error) => {
                let _ = self.launcher.stop(&mut new_handle);
                Err(error)
            }
        }
    }

    pub fn apply(
        &mut self,
        candidate: CandidateId,
        controller: &mut BrowserController,
        engine: &mut impl EnginePort,
        renderer: &mut impl LayoutRenderer,
    ) -> Result<(), String> {
        if self.running.as_ref().map(|running| running.0) != Some(candidate) {
            return Err("preview candidate is stale or was not launched".to_owned());
        }
        if self.refresh(candidate, controller)? != PreviewStatus::Ready {
            return Err("preview is not ready; Apply remains disabled".to_owned());
        }
        let (_, mut handle) = self.running.take().expect("candidate checked above");
        if let Err(error) = self.launcher.stop(&mut handle) {
            let rejection = controller.reject_preview(candidate);
            return Err(match rejection {
                Ok(()) => format!("failed to stop preview before Apply: {error}"),
                Err(rejection) => format!(
                    "failed to stop preview before Apply: {error}; rejecting the candidate also failed: {rejection}"
                ),
            });
        }
        if let Err(error) = controller.apply_preview(candidate, engine, renderer) {
            let rejection = controller.reject_preview(candidate);
            return Err(match rejection {
                Ok(()) => error,
                Err(rejection) => {
                    format!("{error}; rejecting the failed candidate also failed: {rejection}")
                }
            });
        }
        Ok(())
    }

    pub fn reject(
        &mut self,
        candidate: CandidateId,
        controller: &mut BrowserController,
    ) -> Result<(), String> {
        if self.running.as_ref().map(|running| running.0) != Some(candidate) {
            return Err("preview candidate is stale or was not launched".to_owned());
        }
        controller.reject_preview(candidate)?;
        let (_, mut handle) = self.running.take().expect("candidate checked above");
        self.launcher.stop(&mut handle)
    }

    pub fn stop(&mut self) -> Result<(), String> {
        if let Some((_, mut handle)) = self.running.take() {
            self.launcher.stop(&mut handle)?;
        }
        Ok(())
    }
}

pub struct LoadedDefinition {
    source: String,
    used_safe_fallback: bool,
    warning: Option<String>,
}

impl LoadedDefinition {
    pub fn source(&self) -> &str {
        &self.source
    }

    pub fn used_safe_fallback(&self) -> bool {
        self.used_safe_fallback
    }

    pub fn warning(&self) -> Option<&str> {
        self.warning.as_deref()
    }
}

impl DefinitionStore {
    pub fn new(directory: impl AsRef<Path>) -> Self {
        let directory = directory.as_ref().to_owned();
        let path = directory.join("active-definition.json");
        Self { directory, path }
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn load(&self) -> Result<LoadedDefinition, String> {
        let bytes = match fs::read(&self.path) {
            Ok(bytes) => bytes,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                return Ok(LoadedDefinition {
                    source: pliant_ui_definition::BUILT_IN_SAFE_DEFINITION.to_owned(),
                    used_safe_fallback: false,
                    warning: None,
                });
            }
            Err(error) => return Ok(self.safe_fallback(format!(
                "saved definition is unreadable; using safe default without replacing it: {error}"
            ))),
        };
        let source = match String::from_utf8(bytes) {
            Ok(source) => source,
            Err(error) => {
                return Ok(self.safe_fallback(format!(
                    "saved definition is unreadable UTF-8; using safe default without replacing it: {error}"
                )));
            }
        };
        match pliant_ui_definition::parse_definition(&source) {
            Ok(definition) => match validate_native_definition(&definition) {
                Ok(()) => Ok(LoadedDefinition {
                    source,
                    used_safe_fallback: false,
                    warning: None,
                }),
                Err(error) => Ok(self.safe_fallback(format!(
                    "saved definition cannot be rendered by the native host; using safe default without replacing it: {error}"
                ))),
            },
            Err(error) => Ok(self.safe_fallback(format!(
                "saved definition is malformed or unsupported; using safe default: {error}"
            ))),
        }
    }

    pub fn save(&self, source: &str) -> Result<(), String> {
        let definition = pliant_ui_definition::parse_definition(source)
            .map_err(|error| format!("refusing to persist invalid definition: {error}"))?;
        validate_native_definition(&definition).map_err(|error| {
            format!("refusing to persist native-unrenderable definition: {error}")
        })?;
        fs::create_dir_all(&self.directory).map_err(|error| {
            format!(
                "failed to create state directory {}: {error}",
                self.directory.display()
            )
        })?;
        let temporary = self.directory.join(format!(
            ".active-definition-{}-{}.tmp",
            std::process::id(),
            NEXT_ATOMIC_FILE.fetch_add(1, Ordering::Relaxed)
        ));
        let result = (|| {
            let mut file = OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&temporary)
                .map_err(|error| {
                    format!(
                        "failed to create temporary state {}: {error}",
                        temporary.display()
                    )
                })?;
            file.write_all(source.as_bytes())
                .and_then(|()| file.sync_all())
                .map_err(|error| {
                    format!(
                        "failed to write temporary state {}: {error}",
                        temporary.display()
                    )
                })?;
            fs::rename(&temporary, &self.path).map_err(|error| {
                format!(
                    "failed to atomically replace {}: {error}",
                    self.path.display()
                )
            })?;
            let directory = fs::File::open(&self.directory).map_err(|error| {
                format!(
                    "failed to open state directory {}: {error}",
                    self.directory.display()
                )
            })?;
            directory.sync_all().map_err(|error| {
                format!(
                    "failed to sync state directory {}: {error}",
                    self.directory.display()
                )
            })
        })();
        if result.is_err() {
            let _ = fs::remove_file(&temporary);
        }
        result
    }

    fn safe_fallback(&self, warning: String) -> LoadedDefinition {
        LoadedDefinition {
            source: pliant_ui_definition::BUILT_IN_SAFE_DEFINITION.to_owned(),
            used_safe_fallback: true,
            warning: Some(warning),
        }
    }
}

pub struct DefinitionPersistence {
    store: Option<DefinitionStore>,
    unsaved_source: Option<String>,
}

impl DefinitionPersistence {
    pub fn new(store: Option<DefinitionStore>) -> Self {
        Self {
            store,
            unsaved_source: None,
        }
    }

    pub fn persist(&mut self, source: &str) -> Result<(), String> {
        let Some(store) = &self.store else {
            self.unsaved_source = None;
            return Ok(());
        };
        self.unsaved_source = Some(source.to_owned());
        store.save(source)?;
        self.unsaved_source = None;
        Ok(())
    }

    pub fn retry(&mut self) -> Result<(), String> {
        let source = self
            .unsaved_source
            .as_deref()
            .ok_or_else(|| "there is no active unsaved definition".to_owned())?;
        let store = self
            .store
            .as_ref()
            .ok_or_else(|| "this preview process does not persist definitions".to_owned())?;
        store.save(source)?;
        self.unsaved_source = None;
        Ok(())
    }

    pub fn unsaved_source(&self) -> Option<&str> {
        self.unsaved_source.as_deref()
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct HostPageId(u64);

impl HostPageId {
    pub const fn new(value: u64) -> Self {
        Self(value)
    }

    pub const fn get(self) -> u64 {
        self.0
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PageState {
    id: HostPageId,
    url: String,
    can_go_back: bool,
    can_go_forward: bool,
}

impl PageState {
    pub fn id(&self) -> HostPageId {
        self.id
    }

    pub fn url(&self) -> &str {
        &self.url
    }

    pub fn can_go_back(&self) -> bool {
        self.can_go_back
    }

    pub fn can_go_forward(&self) -> bool {
        self.can_go_forward
    }
}

pub trait EnginePort {
    fn create_page(&mut self, url: &str) -> Result<HostPageId, String>;
    fn load_url(&mut self, page: HostPageId, url: &str) -> Result<(), String>;
    fn back(&mut self, page: HostPageId) -> Result<(), String>;
    fn forward(&mut self, page: HostPageId) -> Result<(), String>;
    fn close_page(&mut self, page: HostPageId) -> Result<(), String>;
    fn attach_page(&mut self, page: HostPageId) -> Result<(), String>;
    fn detach_page(&mut self, page: HostPageId) -> Result<(), String>;
}

pub trait LayoutRenderer {
    fn render(
        &mut self,
        definition: &BrowserDefinition,
        generation: u64,
        pages: &[PageState],
        selected: Option<HostPageId>,
    ) -> Result<(), String>;
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LayoutAxis {
    Row,
    Column,
}

pub trait LayoutSink {
    fn begin(&mut self, generation: u64) -> Result<(), String>;
    fn container(
        &mut self,
        parent: Option<&str>,
        id: &str,
        axis: LayoutAxis,
        gap: Option<f64>,
    ) -> Result<(), String>;
    fn spacer(&mut self, parent: &str, id: &str, size: f64) -> Result<(), String>;
    fn label(&mut self, parent: &str, id: &str, text: &str) -> Result<(), String>;
    fn address_field(
        &mut self,
        parent: &str,
        id: &str,
        placeholder: Option<&str>,
    ) -> Result<(), String>;
    fn page_list(&mut self, parent: &str, id: &str) -> Result<(), String>;
    fn content_surface(&mut self, parent: &str, id: &str) -> Result<(), String>;
    fn button(&mut self, parent: &str, id: &str, label: &str) -> Result<(), String>;
    fn end(&mut self) -> Result<(), String>;
}

pub fn validate_native_definition(definition: &BrowserDefinition) -> Result<(), String> {
    #[derive(Default)]
    struct NativeTextPreflight;

    fn check(value: &str, field: &str) -> Result<(), String> {
        if value.contains('\0') {
            Err(format!("{field} contains a NUL byte"))
        } else {
            Ok(())
        }
    }

    impl LayoutSink for NativeTextPreflight {
        fn begin(&mut self, _generation: u64) -> Result<(), String> {
            Ok(())
        }

        fn container(
            &mut self,
            parent: Option<&str>,
            id: &str,
            _axis: LayoutAxis,
            _gap: Option<f64>,
        ) -> Result<(), String> {
            check(parent.unwrap_or_default(), "container parent")?;
            check(id, "container ID")
        }

        fn spacer(&mut self, parent: &str, id: &str, _size: f64) -> Result<(), String> {
            check(parent, "spacer parent")?;
            check(id, "spacer ID")
        }

        fn label(&mut self, parent: &str, id: &str, text: &str) -> Result<(), String> {
            check(parent, "label parent")?;
            check(id, "label ID")?;
            check(text, "label text")
        }

        fn address_field(
            &mut self,
            parent: &str,
            id: &str,
            placeholder: Option<&str>,
        ) -> Result<(), String> {
            check(parent, "address parent")?;
            check(id, "address ID")?;
            check(placeholder.unwrap_or_default(), "address placeholder")
        }

        fn page_list(&mut self, parent: &str, id: &str) -> Result<(), String> {
            check(parent, "page-list parent")?;
            check(id, "page-list ID")
        }

        fn content_surface(&mut self, parent: &str, id: &str) -> Result<(), String> {
            check(parent, "content parent")?;
            check(id, "content ID")
        }

        fn button(&mut self, parent: &str, id: &str, label: &str) -> Result<(), String> {
            check(parent, "button parent")?;
            check(id, "button ID")?;
            check(label, "button label")
        }

        fn end(&mut self) -> Result<(), String> {
            Ok(())
        }
    }

    render_definition(definition, 0, &mut NativeTextPreflight)
}

pub fn render_definition(
    definition: &BrowserDefinition,
    generation: u64,
    sink: &mut impl LayoutSink,
) -> Result<(), String> {
    fn render_node(
        node: &Node,
        parent: Option<&str>,
        sink: &mut impl LayoutSink,
    ) -> Result<(), String> {
        match node {
            Node::Row(container) | Node::Column(container) => {
                let axis = if matches!(node, Node::Row(_)) {
                    LayoutAxis::Row
                } else {
                    LayoutAxis::Column
                };
                sink.container(parent, container.id(), axis, container.gap())?;
                for child in container.children() {
                    render_node(child, Some(container.id()), sink)?;
                }
                Ok(())
            }
            Node::Spacer(spacer) => {
                sink.spacer(parent_required(parent)?, spacer.id(), spacer.size())
            }
            Node::Label(label) => sink.label(parent_required(parent)?, label.id(), label.text()),
            Node::AddressField(field) => {
                sink.address_field(parent_required(parent)?, field.id(), field.placeholder())
            }
            Node::PageList(list) => sink.page_list(parent_required(parent)?, list.id()),
            Node::ContentSurface(surface) => {
                sink.content_surface(parent_required(parent)?, surface.id())
            }
            Node::Button(button) => {
                sink.button(parent_required(parent)?, button.id(), button.label())
            }
        }
    }

    sink.begin(generation)?;
    render_node(definition.root(), None, sink)?;
    sink.end()
}

fn parent_required(parent: Option<&str>) -> Result<&str, String> {
    parent.ok_or_else(|| "only a row or column may be the root node".to_owned())
}

pub struct BrowserController {
    definitions: DefinitionState,
    pages: Vec<PageState>,
    selected_page: Option<HostPageId>,
    layout_generation: u64,
}

impl BrowserController {
    pub fn new(
        initial_definition: &str,
        initial_url: &str,
        engine: &mut impl EnginePort,
    ) -> Result<Self, String> {
        let definitions =
            DefinitionState::new(initial_definition).map_err(|error| error.to_string())?;
        let page = engine.create_page(initial_url)?;
        engine.attach_page(page)?;
        Ok(Self {
            definitions,
            pages: vec![PageState {
                id: page,
                url: initial_url.to_owned(),
                can_go_back: false,
                can_go_forward: false,
            }],
            selected_page: Some(page),
            layout_generation: 1,
        })
    }

    pub fn pages(&self) -> &[PageState] {
        &self.pages
    }

    pub fn active_definition_source(&self) -> &str {
        self.definitions.active().source()
    }

    pub fn active_definition(&self) -> &BrowserDefinition {
        self.definitions.active().definition()
    }

    pub fn button_enabled(&self, node_id: &str) -> bool {
        let Some(command) = find_button_command(self.active_definition().root(), node_id) else {
            return false;
        };
        let selected = self
            .selected_page
            .and_then(|selected| self.pages.iter().find(|page| page.id == selected));
        match command {
            Command::Back => selected.is_some_and(PageState::can_go_back),
            Command::Forward => selected.is_some_and(PageState::can_go_forward),
            Command::Navigate { .. } | Command::Close => selected.is_some(),
            Command::NewPage => true,
        }
    }

    pub fn selected_page(&self) -> Option<HostPageId> {
        self.selected_page
    }

    pub fn layout_generation(&self) -> u64 {
        self.layout_generation
    }

    pub fn preview_definition(&mut self, source: &str) -> Result<CandidateId, String> {
        self.definitions
            .preview(source)
            .map_err(|error| error.to_string())
    }

    pub fn reject_preview(&mut self, candidate_id: CandidateId) -> Result<(), String> {
        self.definitions
            .reject(candidate_id)
            .map_err(|error| error.to_string())
    }

    pub fn apply_preview(
        &mut self,
        candidate_id: CandidateId,
        engine: &mut impl EnginePort,
        renderer: &mut impl LayoutRenderer,
    ) -> Result<(), String> {
        let (pending_id, snapshot) = self
            .definitions
            .pending()
            .ok_or_else(|| "preview candidate is stale or no longer pending".to_owned())?;
        if pending_id != candidate_id {
            return Err("preview candidate is stale or no longer pending".to_owned());
        }
        let definition = snapshot.definition().clone();
        let next_generation = self
            .layout_generation
            .checked_add(1)
            .ok_or_else(|| "layout generation exhausted".to_owned())?;
        self.render_layout(&definition, next_generation, engine, renderer)?;
        self.definitions
            .apply(candidate_id)
            .map_err(|error| error.to_string())?;
        self.layout_generation = next_generation;
        Ok(())
    }

    pub fn restore_default(
        &mut self,
        engine: &mut impl EnginePort,
        renderer: &mut impl LayoutRenderer,
    ) -> Result<(), String> {
        let definition =
            pliant_ui_definition::parse_definition(pliant_ui_definition::BUILT_IN_SAFE_DEFINITION)
                .map_err(|error| error.to_string())?;
        let next_generation = self
            .layout_generation
            .checked_add(1)
            .ok_or_else(|| "layout generation exhausted".to_owned())?;
        self.render_layout(&definition, next_generation, engine, renderer)?;
        self.definitions.reset();
        self.layout_generation = next_generation;
        Ok(())
    }

    pub fn submit_address(
        &mut self,
        generation: u64,
        address: &str,
        engine: &mut impl EnginePort,
    ) -> Result<(), String> {
        self.ensure_current_generation(generation)?;
        let current_page = self.selected_page.map(|page| page.get().to_string());
        let resolution = resolve_address_open(
            self.definitions.active().definition().address_open_target(),
            current_page.as_deref(),
            address,
        )
        .map_err(|error| error.to_string())?;

        match resolution {
            AddressOpenResolution::NavigateCurrent { page_id, url } => {
                let page = HostPageId::new(
                    page_id
                        .parse()
                        .map_err(|_| "address policy returned an invalid page ID")?,
                );
                engine.load_url(page, &url)?;
            }
            AddressOpenResolution::OpenNewPage { url } => {
                let page = engine.create_page(&url)?;
                if let Some(selected) = self.selected_page {
                    engine.detach_page(selected)?;
                }
                engine.attach_page(page)?;
                self.pages.push(PageState {
                    id: page,
                    url,
                    can_go_back: false,
                    can_go_forward: false,
                });
                self.selected_page = Some(page);
            }
        }
        Ok(())
    }

    pub fn select_page(
        &mut self,
        generation: u64,
        page: HostPageId,
        engine: &mut impl EnginePort,
    ) -> Result<(), String> {
        self.ensure_current_generation(generation)?;
        if !self.pages.iter().any(|candidate| candidate.id == page) {
            return Err(format!("page {} is no longer live", page.get()));
        }
        if self.selected_page == Some(page) {
            return Ok(());
        }
        if let Some(selected) = self.selected_page {
            engine.detach_page(selected)?;
        }
        engine.attach_page(page)?;
        self.selected_page = Some(page);
        Ok(())
    }

    pub fn page_closed(
        &mut self,
        page: HostPageId,
        engine: &mut impl EnginePort,
    ) -> Result<(), String> {
        let Some(index) = self.pages.iter().position(|candidate| candidate.id == page) else {
            return Err(format!(
                "closed event referenced unknown page {}",
                page.get()
            ));
        };
        self.pages.remove(index);
        if self.selected_page == Some(page) {
            self.selected_page = self.pages.last().map(|candidate| candidate.id);
            if let Some(selected) = self.selected_page {
                engine.attach_page(selected)?;
            }
        }
        Ok(())
    }

    pub fn navigation_committed(
        &mut self,
        page: HostPageId,
        url: &str,
        can_go_back: bool,
        can_go_forward: bool,
    ) -> Result<(), String> {
        let state = self
            .pages
            .iter_mut()
            .find(|candidate| candidate.id == page)
            .ok_or_else(|| format!("navigation referenced unknown page {}", page.get()))?;
        state.url = url.to_owned();
        state.can_go_back = can_go_back;
        state.can_go_forward = can_go_forward;
        Ok(())
    }

    pub fn activate_button(
        &mut self,
        generation: u64,
        node_id: &str,
        engine: &mut impl EnginePort,
    ) -> Result<(), String> {
        self.ensure_current_generation(generation)?;
        let command = find_button_command(self.definitions.active().definition().root(), node_id)
            .ok_or_else(|| format!("active node `{node_id}` is not a button"))?
            .clone();
        match command {
            Command::Navigate { url } => {
                let page = self
                    .selected_page
                    .ok_or_else(|| "no page is selected".to_owned())?;
                engine.load_url(page, &url)
            }
            Command::Back => {
                let page = self
                    .selected_page
                    .ok_or_else(|| "no page is selected".to_owned())?;
                engine.back(page)
            }
            Command::Forward => {
                let page = self
                    .selected_page
                    .ok_or_else(|| "no page is selected".to_owned())?;
                engine.forward(page)
            }
            Command::Close => {
                let page = self
                    .selected_page
                    .ok_or_else(|| "no page is selected".to_owned())?;
                engine.close_page(page)
            }
            Command::NewPage => {
                let page = engine.create_page("about:blank")?;
                if let Some(selected) = self.selected_page {
                    engine.detach_page(selected)?;
                }
                engine.attach_page(page)?;
                self.pages.push(PageState {
                    id: page,
                    url: "about:blank".to_owned(),
                    can_go_back: false,
                    can_go_forward: false,
                });
                self.selected_page = Some(page);
                Ok(())
            }
        }
    }

    fn ensure_current_generation(&self, generation: u64) -> Result<(), String> {
        if generation != self.layout_generation {
            return Err(format!(
                "stale layout event for generation {generation}; active generation is {}",
                self.layout_generation
            ));
        }
        Ok(())
    }

    fn render_layout(
        &self,
        definition: &BrowserDefinition,
        generation: u64,
        engine: &mut impl EnginePort,
        renderer: &mut impl LayoutRenderer,
    ) -> Result<(), String> {
        if let Some(selected) = self.selected_page {
            engine.detach_page(selected)?;
        }
        let activation = renderer
            .render(definition, generation, &self.pages, self.selected_page)
            .and_then(|()| {
                if let Some(selected) = self.selected_page {
                    engine.attach_page(selected)?;
                }
                Ok(())
            });
        if let Err(error) = activation {
            let mut recovery_errors = Vec::new();
            if let Err(rollback_error) = renderer.render(
                self.active_definition(),
                self.layout_generation,
                &self.pages,
                self.selected_page,
            ) {
                recovery_errors.push(format!(
                    "restoring the active layout failed: {rollback_error}"
                ));
            }
            if let Some(selected) = self.selected_page
                && let Err(attach_error) = engine.attach_page(selected)
            {
                recovery_errors.push(format!(
                    "reattaching the active page also failed: {attach_error}"
                ));
            }
            if !recovery_errors.is_empty() {
                return Err(format!("{error}; {}", recovery_errors.join("; ")));
            }
            return Err(error);
        }
        Ok(())
    }
}

fn find_button_command<'a>(node: &'a Node, node_id: &str) -> Option<&'a Command> {
    match node {
        Node::Button(button) if button.id() == node_id => Some(button.command()),
        Node::Row(container) | Node::Column(container) => container
            .children()
            .iter()
            .find_map(|child| find_button_command(child, node_id)),
        _ => None,
    }
}
