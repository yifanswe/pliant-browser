#![allow(unsafe_code)]

use std::cell::RefCell;
use std::collections::HashMap;
use std::env;
use std::ffi::{CStr, c_char, c_void};
use std::fs::OpenOptions;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::ptr::NonNull;

use pliant_browser::native::{
    EVENT_ADDRESS_SUBMITTED, EVENT_APPLY, EVENT_BUTTON_ACTIVATED, EVENT_LOAD_DEFINITION,
    EVENT_PAGE_SELECTED, EVENT_PREVIEW, EVENT_PREVIEW_STATUS, EVENT_REJECT, EVENT_RESTORE_DEFAULT,
    EVENT_RETRY_SAVE, EVENT_WINDOW_CLOSED, NativeHost,
};
use pliant_browser::{
    BrowserController, DefinitionPersistence, DefinitionStore, EnginePort, FixtureServer,
    HostPageId, LayoutRenderer, PreviewCoordinator, PreviewStatus, ProcessPreviewLauncher,
    render_initial_layout,
};
use pliant_embedder::{Engine, Event, NativeContainer, PageId};
use pliant_ui_definition::{CandidateId, parse_definition};

struct Options {
    state_directory: PathBuf,
    definition_path: Option<PathBuf>,
    preview_snapshot: Option<PathBuf>,
    preview_ack: Option<PathBuf>,
    preview_token: Option<String>,
    initial_url: Option<String>,
    fixture_setup: bool,
}

impl Options {
    fn parse() -> Result<Self, String> {
        let mut state_directory = default_state_directory()?;
        let mut definition_path = None;
        let mut preview_snapshot = None;
        let mut preview_ack = None;
        let mut preview_token = None;
        let mut initial_url = None;
        let mut fixture_setup = false;
        let mut arguments = env::args_os().skip(1);
        while let Some(argument) = arguments.next() {
            match argument.to_str() {
                Some("--state-dir") => {
                    state_directory = PathBuf::from(
                        arguments
                            .next()
                            .ok_or_else(|| "--state-dir requires a path".to_owned())?,
                    );
                }
                Some("--definition") => {
                    definition_path = Some(PathBuf::from(
                        arguments
                            .next()
                            .ok_or_else(|| "--definition requires a path".to_owned())?,
                    ));
                }
                Some("--preview-snapshot") => {
                    preview_snapshot =
                        Some(PathBuf::from(arguments.next().ok_or_else(|| {
                            "--preview-snapshot requires a path".to_owned()
                        })?));
                }
                Some("--preview-ack") => {
                    preview_ack = Some(PathBuf::from(
                        arguments
                            .next()
                            .ok_or_else(|| "--preview-ack requires a path".to_owned())?,
                    ));
                }
                Some("--preview-token") => {
                    preview_token = Some(
                        arguments
                            .next()
                            .ok_or_else(|| "--preview-token requires a value".to_owned())?
                            .into_string()
                            .map_err(|_| "--preview-token must be valid UTF-8".to_owned())?,
                    );
                }
                Some("--initial-url") => {
                    initial_url = Some(
                        arguments
                            .next()
                            .ok_or_else(|| "--initial-url requires a URL".to_owned())?
                            .into_string()
                            .map_err(|_| "--initial-url must be valid UTF-8".to_owned())?,
                    );
                }
                Some("--fixture-setup" | "--e2e") => fixture_setup = true,
                Some(other) => return Err(format!("unknown argument `{other}`")),
                None => return Err("arguments must be valid UTF-8".to_owned()),
            }
        }
        if preview_snapshot.is_some() && definition_path.is_some() {
            return Err("--preview-snapshot cannot be combined with --definition".to_owned());
        }
        if preview_snapshot.is_some() != (preview_ack.is_some() && preview_token.is_some()) {
            return Err(
                "--preview-snapshot, --preview-ack, and --preview-token must be used together"
                    .to_owned(),
            );
        }
        Ok(Self {
            state_directory,
            definition_path,
            preview_snapshot,
            preview_ack,
            preview_token,
            initial_url,
            fixture_setup,
        })
    }
}

fn default_state_directory() -> Result<PathBuf, String> {
    let home = env::var_os("HOME").ok_or_else(|| "HOME is not set".to_owned())?;
    Ok(PathBuf::from(home).join("Library/Application Support/PliantDemo"))
}

struct RealEngine<'a> {
    engine: &'a Engine,
    handles: &'a mut HashMap<HostPageId, PageId>,
    container: NativeContainer,
}

impl EnginePort for RealEngine<'_> {
    fn create_page(&mut self, url: &str) -> Result<HostPageId, String> {
        let native = self
            .engine
            .create_page_in(&self.container, url)
            .map_err(|error| error.to_string())?;
        let host = HostPageId::new(native.as_u64());
        self.handles.insert(host, native);
        Ok(host)
    }

    fn load_url(&mut self, page: HostPageId, url: &str) -> Result<(), String> {
        self.engine
            .load_url(self.native(page)?, url)
            .map_err(|error| error.to_string())
    }

    fn back(&mut self, page: HostPageId) -> Result<(), String> {
        self.engine
            .back(self.native(page)?)
            .map_err(|error| error.to_string())
    }

    fn forward(&mut self, page: HostPageId) -> Result<(), String> {
        self.engine
            .forward(self.native(page)?)
            .map_err(|error| error.to_string())
    }

    fn close_page(&mut self, page: HostPageId) -> Result<(), String> {
        self.engine
            .close_page(self.native(page)?)
            .map_err(|error| error.to_string())
    }

    fn attach_page(&mut self, page: HostPageId) -> Result<(), String> {
        self.engine
            .attach_page(self.native(page)?, &self.container)
            .map_err(|error| error.to_string())
    }

    fn detach_page(&mut self, page: HostPageId) -> Result<(), String> {
        self.engine
            .detach_page(self.native(page)?)
            .map_err(|error| error.to_string())
    }
}

impl RealEngine<'_> {
    fn native(&self, page: HostPageId) -> Result<PageId, String> {
        self.handles
            .get(&page)
            .copied()
            .ok_or_else(|| format!("page {} is no longer live", page.get()))
    }
}

struct AppRuntime {
    initial_source: String,
    initial_url: String,
    initial_warning: Option<String>,
    definition_path: Option<PathBuf>,
    preview_mode: bool,
    persistence: DefinitionPersistence,
    previews: PreviewCoordinator<ProcessPreviewLauncher>,
    pending_candidate: Option<CandidateId>,
    preview_ready: bool,
    preview_acknowledgement: Option<PreviewAcknowledgement>,
    native_pages: HashMap<HostPageId, PageId>,
    controller: Option<BrowserController>,
    host: Option<NativeHost>,
    _fixture: Option<FixtureServer>,
}

impl AppRuntime {
    fn start(&mut self, engine: &Engine, callback_context: *mut c_void) -> Result<(), String> {
        let mut host = unsafe { NativeHost::new(host_callback, callback_context)? };
        let initial_layout = render_initial_layout(
            &self.initial_source,
            self.initial_warning.take(),
            self.preview_mode,
            |definition| host.render(definition, 1, &[], None),
        )?;
        let effective_source = initial_layout.source().to_owned();
        let warning = initial_layout.warning().map(str::to_owned);
        let container = native_container(&host)?;
        let mut port = RealEngine {
            engine,
            handles: &mut self.native_pages,
            container,
        };
        let controller = BrowserController::new(&effective_source, &self.initial_url, &mut port)?;
        host.update_pages(controller.pages(), controller.selected_page())?;
        host.update_control_states(&controller)?;
        if let Some(path) = &self.definition_path {
            host.set_definition_path(&path.to_string_lossy())?;
        }
        host.set_preview_mode(self.preview_mode);
        host.set_preview_pending(false);
        host.set_preview_checking(false);
        host.set_save_pending(false);
        if let Some(warning) = &warning {
            host.set_status(warning, true)?;
        } else if self.preview_mode {
            host.set_status("Isolated preview: browsing data is disposable", false)?;
        } else {
            host.set_status("Ready", false)?;
        }
        self.controller = Some(controller);
        self.initial_source = effective_source;
        self.initial_warning = warning;
        self.host = Some(host);
        self.host.as_ref().expect("host assigned above").show();
        if let Some(acknowledgement) = &self.preview_acknowledgement {
            acknowledgement.ready()?;
        }
        Ok(())
    }

    fn handle_engine_event(&mut self, engine: &Engine, event: Event) -> Result<(), String> {
        match event {
            Event::Ready => return Err("Ready must be handled by the composition root".to_owned()),
            Event::Navigated {
                page,
                url,
                can_go_back,
                can_go_forward,
            } => {
                let host_page = HostPageId::new(page.as_u64());
                self.controller_mut()?.navigation_committed(
                    host_page,
                    &url,
                    can_go_back,
                    can_go_forward,
                )?;
                self.refresh_pages()?;
                self.status(&format!("Page {} loaded {url}", host_page.get()), false)?;
            }
            Event::NavigationFailed { page, code } => {
                self.status(
                    &format!("Page {} navigation failed ({code})", page.as_u64()),
                    true,
                )?;
            }
            Event::Closed { page } => {
                let host_page = HostPageId::new(page.as_u64());
                self.native_pages.remove(&host_page);
                let container = native_container(self.host_ref()?)?;
                let mut port = RealEngine {
                    engine,
                    handles: &mut self.native_pages,
                    container,
                };
                self.controller
                    .as_mut()
                    .ok_or_else(|| "browser controller is not ready".to_owned())?
                    .page_closed(host_page, &mut port)?;
                self.refresh_pages()?;
                self.status(&format!("Closed page {}", host_page.get()), false)?;
            }
            Event::CloseRefused { page } => {
                self.status(
                    &format!(
                        "Page {} stayed open because its beforeunload handler refused the close",
                        page.as_u64()
                    ),
                    true,
                )?;
            }
            Event::RendererFailed { page } => {
                self.status(&format!("Page {} renderer failed", page.as_u64()), true)?;
            }
            Event::Painted { page, url } => {
                self.status(&format!("Page {} painted {url}", page.as_u64()), false)?;
            }
        }
        Ok(())
    }

    fn handle_host_event(
        &mut self,
        engine: &Engine,
        kind: u32,
        generation: u64,
        page: u64,
        value: &str,
    ) -> Result<(), String> {
        match kind {
            EVENT_LOAD_DEFINITION => self.load_definition_path(value),
            EVENT_PREVIEW => self.preview(value),
            EVENT_PREVIEW_STATUS => self.refresh_preview(),
            EVENT_APPLY => self.apply(engine),
            EVENT_REJECT => self.reject(),
            EVENT_RESTORE_DEFAULT => self.restore(engine),
            EVENT_RETRY_SAVE => self.retry_save(),
            EVENT_ADDRESS_SUBMITTED => {
                let container = native_container(self.host_ref()?)?;
                let mut port = RealEngine {
                    engine,
                    handles: &mut self.native_pages,
                    container,
                };
                self.controller
                    .as_mut()
                    .ok_or_else(|| "browser controller is not ready".to_owned())?
                    .submit_address(generation, value, &mut port)?;
                self.refresh_pages()
            }
            EVENT_BUTTON_ACTIVATED => {
                let container = native_container(self.host_ref()?)?;
                let mut port = RealEngine {
                    engine,
                    handles: &mut self.native_pages,
                    container,
                };
                self.controller
                    .as_mut()
                    .ok_or_else(|| "browser controller is not ready".to_owned())?
                    .activate_button(generation, value, &mut port)?;
                self.refresh_pages()
            }
            EVENT_PAGE_SELECTED => {
                let container = native_container(self.host_ref()?)?;
                let mut port = RealEngine {
                    engine,
                    handles: &mut self.native_pages,
                    container,
                };
                self.controller
                    .as_mut()
                    .ok_or_else(|| "browser controller is not ready".to_owned())?
                    .select_page(generation, HostPageId::new(page), &mut port)?;
                self.refresh_pages()
            }
            EVENT_WINDOW_CLOSED => {
                self.previews.stop()?;
                engine.shutdown().map_err(|error| error.to_string())
            }
            _ => Err(format!("unknown native host event {kind}")),
        }
    }

    fn load_definition_path(&mut self, value: &str) -> Result<(), String> {
        let path = required_path(value)?;
        let source = read_definition(&path)?;
        let definition = parse_definition(&source).map_err(|error| error.to_string())?;
        pliant_browser::validate_native_definition(&definition)?;
        self.definition_path = Some(path.clone());
        self.host_ref()?
            .set_definition_path(&path.to_string_lossy())?;
        self.status("Definition is valid; choose Preview to inspect it", false)
    }

    fn preview(&mut self, value: &str) -> Result<(), String> {
        let path = required_path(value)?;
        let source = read_definition(&path)?;
        let controller = self
            .controller
            .as_mut()
            .ok_or_else(|| "browser controller is not ready".to_owned())?;
        let candidate = self.previews.preview(controller, &source)?;
        self.pending_candidate = Some(candidate);
        self.preview_ready = false;
        self.host_ref()?.set_preview_pending(false);
        self.host_ref()?.set_preview_checking(true);
        self.definition_path = Some(path);
        self.status(
            "Preview is starting from the validated snapshot; Apply remains disabled",
            false,
        )
    }

    fn refresh_preview(&mut self) -> Result<(), String> {
        let Some(candidate) = self.pending_candidate else {
            self.host_ref()?.set_preview_checking(false);
            return Ok(());
        };
        let status = {
            let controller = self
                .controller
                .as_mut()
                .ok_or_else(|| "browser controller is not ready".to_owned())?;
            self.previews.refresh(candidate, controller)
        };
        match status {
            Ok(PreviewStatus::Starting) => Ok(()),
            Ok(PreviewStatus::Ready) => {
                self.preview_ready = true;
                self.host_ref()?.set_preview_checking(false);
                self.host_ref()?.set_preview_pending(true);
                self.status(
                    "Isolated preview rendered the exact candidate; Apply is enabled",
                    false,
                )
            }
            Err(error) => {
                self.pending_candidate = self.previews.candidate_id();
                self.preview_ready = false;
                self.host_ref()?.set_preview_checking(false);
                self.host_ref()?.set_preview_pending(false);
                Err(error)
            }
        }
    }

    fn apply(&mut self, engine: &Engine) -> Result<(), String> {
        if !self.preview_ready {
            return Err("preview is not ready; Apply remains disabled".to_owned());
        }
        let candidate = self
            .pending_candidate
            .ok_or_else(|| "no successfully launched preview is pending".to_owned())?;
        let container = native_container(self.host_ref()?)?;
        let mut port = RealEngine {
            engine,
            handles: &mut self.native_pages,
            container,
        };
        let result = self.previews.apply(
            candidate,
            self.controller
                .as_mut()
                .ok_or_else(|| "browser controller is not ready".to_owned())?,
            &mut port,
            self.host
                .as_mut()
                .ok_or_else(|| "native host is not ready".to_owned())?,
        );
        if let Err(error) = result {
            self.pending_candidate = self.previews.candidate_id();
            self.preview_ready = false;
            self.host_ref()?.set_preview_pending(false);
            self.host_ref()?
                .set_preview_checking(self.pending_candidate.is_some());
            return Err(error);
        }
        self.pending_candidate = None;
        self.preview_ready = false;
        self.host_ref()?.set_preview_checking(false);
        self.host_ref()?.set_preview_pending(false);
        self.complete_live_change("Applied the exact previewed definition")
    }

    fn reject(&mut self) -> Result<(), String> {
        let candidate = self
            .pending_candidate
            .ok_or_else(|| "no preview is pending".to_owned())?;
        let controller = self
            .controller
            .as_mut()
            .ok_or_else(|| "browser controller is not ready".to_owned())?;
        self.previews.reject(candidate, controller)?;
        self.pending_candidate = None;
        self.preview_ready = false;
        self.host_ref()?.set_preview_checking(false);
        self.host_ref()?.set_preview_pending(false);
        self.status("Preview rejected; active browser is unchanged", false)
    }

    fn restore(&mut self, engine: &Engine) -> Result<(), String> {
        self.previews.stop()?;
        self.pending_candidate = None;
        self.preview_ready = false;
        self.host_ref()?.set_preview_checking(false);
        self.host_ref()?.set_preview_pending(false);
        let container = native_container(self.host_ref()?)?;
        let mut port = RealEngine {
            engine,
            handles: &mut self.native_pages,
            container,
        };
        self.controller
            .as_mut()
            .ok_or_else(|| "browser controller is not ready".to_owned())?
            .restore_default(
                &mut port,
                self.host
                    .as_mut()
                    .ok_or_else(|| "native host is not ready".to_owned())?,
            )?;
        self.complete_live_change("Restored the built-in safe definition")
    }

    fn complete_live_change(&mut self, success: &str) -> Result<(), String> {
        let source = self.controller_ref()?.active_definition_source().to_owned();
        let save = self.persistence.persist(&source);
        let refresh = self.refresh_pages();
        let unsaved = self.persistence.unsaved_source().is_some();
        self.host_ref()?.set_save_pending(unsaved);
        match (save, refresh) {
            (Ok(()), Ok(())) => self.status(success, false),
            (Ok(()), Err(refresh)) => Err(refresh),
            (Err(save), Ok(())) => self.status(
                &format!(
                    "{success}, but saving failed: {save}. The new state is active but unsaved; restart will load the previous saved state. Use Retry Save."
                ),
                true,
            ),
            (Err(save), Err(refresh)) => self.status(
                &format!(
                    "{success}, but saving failed: {save}. The new state is active but unsaved; restart will load the previous saved state. Control refresh also failed: {refresh}. Use Retry Save."
                ),
                true,
            ),
        }
    }

    fn retry_save(&mut self) -> Result<(), String> {
        match self.persistence.retry() {
            Ok(()) => {
                self.host_ref()?.set_save_pending(false);
                self.status("Saved the active definition; restart will retain it", false)
            }
            Err(error) => {
                self.host_ref()?.set_save_pending(true);
                self.status(
                    &format!(
                        "Retry Save failed: {error}. The current state remains active but unsaved; restart will load the previous saved state."
                    ),
                    true,
                )
            }
        }
    }

    fn refresh_pages(&self) -> Result<(), String> {
        let controller = self.controller_ref()?;
        let host = self.host_ref()?;
        host.update_pages(controller.pages(), controller.selected_page())?;
        host.update_control_states(controller)
    }

    fn status(&self, message: &str, error: bool) -> Result<(), String> {
        self.host_ref()?.set_status(message, error)
    }

    fn controller_ref(&self) -> Result<&BrowserController, String> {
        self.controller
            .as_ref()
            .ok_or_else(|| "browser controller is not ready".to_owned())
    }

    fn controller_mut(&mut self) -> Result<&mut BrowserController, String> {
        self.controller
            .as_mut()
            .ok_or_else(|| "browser controller is not ready".to_owned())
    }

    fn host_ref(&self) -> Result<&NativeHost, String> {
        self.host
            .as_ref()
            .ok_or_else(|| "native host is not ready".to_owned())
    }
}

fn native_container(host: &NativeHost) -> Result<NativeContainer, String> {
    let raw = host.content_container()?;
    unsafe { NativeContainer::from_raw_ns_view(raw.as_ptr()) }.map_err(|error| error.to_string())
}

fn required_path(value: &str) -> Result<PathBuf, String> {
    if value.trim().is_empty() {
        Err("enter a definition JSON path first".to_owned())
    } else {
        Ok(PathBuf::from(value))
    }
}

fn read_definition(path: &Path) -> Result<String, String> {
    std::fs::read_to_string(path)
        .map_err(|error| format!("failed to read definition {}: {error}", path.display()))
}

unsafe extern "C" fn host_callback(
    context: *mut c_void,
    kind: u32,
    generation: u64,
    page: u64,
    node_id: *const c_char,
    value: *const c_char,
) {
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let runtime = unsafe { &*context.cast::<RefCell<AppRuntime>>() };
        let node_id = c_text(node_id);
        let value = c_text(value);
        let mut runtime = runtime
            .try_borrow_mut()
            .map_err(|_| "native event arrived during another host callback".to_owned())?;
        pliant_embedder::with_engine(|engine| {
            let event_value = if kind == EVENT_BUTTON_ACTIVATED {
                node_id.as_str()
            } else {
                value.as_str()
            };
            runtime.handle_host_event(engine, kind, generation, page, event_value)
        })
        .map_err(|error| error.to_string())?
    }));
    match result {
        Ok(Ok(())) => {}
        Ok(Err(error)) => {
            let runtime = unsafe { &*context.cast::<RefCell<AppRuntime>>() };
            if let Ok(runtime) = runtime.try_borrow() {
                let _ = runtime.status(&error, true);
            }
        }
        Err(_) => {
            let _ = pliant_embedder::with_engine(|engine| engine.shutdown());
        }
    }
}

fn c_text(pointer: *const c_char) -> String {
    if pointer.is_null() {
        String::new()
    } else {
        unsafe { CStr::from_ptr(pointer) }
            .to_string_lossy()
            .into_owned()
    }
}

#[derive(Clone)]
struct PreviewAcknowledgement {
    path: PathBuf,
    token: String,
}

impl PreviewAcknowledgement {
    fn ready(&self) -> Result<(), String> {
        self.write("ready", None)
    }

    fn failed(&self, error: &str) -> Result<(), String> {
        self.write("error", Some(error))
    }

    fn write(&self, state: &str, message: Option<&str>) -> Result<(), String> {
        let temporary = self
            .path
            .with_extension(format!("ack-{}.tmp", std::process::id()));
        let result = (|| {
            let mut file = OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&temporary)
                .map_err(|error| {
                    format!(
                        "failed to create preview acknowledgement temporary file {}: {error}",
                        temporary.display()
                    )
                })?;
            writeln!(file, "{state}\n{}", self.token)
                .and_then(|()| {
                    if let Some(message) = message {
                        write!(file, "{message}")
                    } else {
                        Ok(())
                    }
                })
                .and_then(|()| file.sync_all())
                .map_err(|error| {
                    format!(
                        "failed to write preview acknowledgement {}: {error}",
                        temporary.display()
                    )
                })?;
            std::fs::hard_link(&temporary, &self.path).map_err(|error| {
                format!(
                    "failed to publish preview acknowledgement {}: {error}",
                    self.path.display()
                )
            })?;
            std::fs::remove_file(&temporary).map_err(|error| {
                format!(
                    "failed to remove preview acknowledgement temporary file {}: {error}",
                    temporary.display()
                )
            })
        })();
        if result.is_err() {
            let _ = std::fs::remove_file(&temporary);
        }
        result
    }
}

fn run_app(options: Options) -> Result<(), Box<dyn std::error::Error>> {
    let store = DefinitionStore::new(&options.state_directory);
    let (initial_source, initial_warning, store) = if let Some(snapshot) = &options.preview_snapshot
    {
        (read_definition(snapshot)?, None, None)
    } else if let Some(path) = &options.definition_path {
        if store.path().exists() {
            let loaded = store.load()?;
            (
                loaded.source().to_owned(),
                loaded.warning().map(str::to_owned),
                Some(store),
            )
        } else {
            let source = read_definition(path)?;
            parse_definition(&source)?;
            (source, None, Some(store))
        }
    } else {
        let loaded = store.load()?;
        (
            loaded.source().to_owned(),
            loaded.warning().map(str::to_owned),
            Some(store),
        )
    };
    parse_definition(&initial_source)?;

    let fixture = if options.fixture_setup {
        Some(FixtureServer::start()?)
    } else {
        None
    };
    let initial_url = options
        .initial_url
        .clone()
        .or_else(|| fixture.as_ref().map(|fixture| fixture.url("/a")))
        .unwrap_or_else(|| "about:blank".to_owned());
    let executable = env::current_exe()?;
    let preview_root = options.state_directory.join("previews");
    let mut preview_launcher = ProcessPreviewLauncher::new(executable, preview_root);
    if options.fixture_setup {
        preview_launcher = preview_launcher.with_initial_url(initial_url.clone());
    }
    let preview_acknowledgement = options
        .preview_ack
        .clone()
        .zip(options.preview_token.clone())
        .map(|(path, token)| PreviewAcknowledgement { path, token });
    let runtime = Box::new(RefCell::new(AppRuntime {
        initial_source,
        initial_url,
        initial_warning,
        definition_path: options
            .preview_snapshot
            .clone()
            .or(options.definition_path.clone()),
        preview_mode: options.preview_snapshot.is_some(),
        persistence: DefinitionPersistence::new(store),
        previews: PreviewCoordinator::new(preview_launcher),
        pending_candidate: None,
        preview_ready: false,
        preview_acknowledgement,
        native_pages: HashMap::new(),
        controller: None,
        host: None,
        _fixture: fixture,
    }));
    let context = NonNull::from(runtime.as_ref()).cast::<c_void>().as_ptr();
    let mut startup_error = None;
    let result = pliant_embedder::run(|engine, event| {
        let action = match event {
            Event::Ready => runtime.borrow_mut().start(engine, context),
            event => runtime.borrow_mut().handle_engine_event(engine, event),
        };
        if let Err(error) = action {
            if let Ok(runtime) = runtime.try_borrow() {
                let _ = runtime.status(&error, true);
            }
            startup_error = Some(error);
            let _ = engine.shutdown();
        }
    });
    let _ = runtime.borrow_mut().previews.stop();
    result?;
    if let Some(error) = startup_error {
        return Err(error.into());
    }
    Ok(())
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let options = Options::parse()?;
    let acknowledgement = options
        .preview_ack
        .clone()
        .zip(options.preview_token.clone())
        .map(|(path, token)| PreviewAcknowledgement { path, token });
    match run_app(options) {
        Ok(()) => Ok(()),
        Err(error) => {
            if let Some(acknowledgement) = acknowledgement {
                let _ = acknowledgement.failed(&error.to_string());
            }
            Err(error)
        }
    }
}
