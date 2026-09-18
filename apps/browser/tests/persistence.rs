use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

use pliant_browser::{
    BrowserController, DefinitionPersistence, DefinitionStore, EnginePort, HostPageId,
    LayoutRenderer, PageState,
};
use pliant_ui_definition::{BUILT_IN_SAFE_DEFINITION, BrowserDefinition, parse_definition};

static NEXT_TEST_DIR: AtomicU64 = AtomicU64::new(1);

struct TestDirectory(PathBuf);

impl TestDirectory {
    fn new(name: &str) -> Self {
        let root = std::env::var_os("PLIANT_BROWSER_TEST_TMPDIR")
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("target/test-state"));
        let path = root.join(format!(
            "{name}-{}-{}",
            std::process::id(),
            NEXT_TEST_DIR.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir_all(&path).unwrap();
        Self(path)
    }
}

impl Drop for TestDirectory {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.0).unwrap();
    }
}

#[test]
fn malformed_startup_state_falls_back_to_the_safe_definition() {
    let directory = TestDirectory::new("malformed-startup");
    let store = DefinitionStore::new(&directory.0);
    fs::write(store.path(), r#"{"schema_version": 1"#).unwrap();

    let loaded = store.load().unwrap();

    assert_eq!(loaded.source(), BUILT_IN_SAFE_DEFINITION);
    assert!(loaded.used_safe_fallback());
    assert!(loaded.warning().unwrap().contains("malformed"));
    parse_definition(loaded.source()).unwrap();
}

#[test]
fn non_utf8_startup_state_falls_back_without_overwriting_the_bad_file() {
    let directory = TestDirectory::new("non-utf8-startup");
    let store = DefinitionStore::new(&directory.0);
    let bad_bytes = [0xff, 0xfe];
    fs::write(store.path(), bad_bytes).unwrap();

    let loaded = store.load().unwrap();

    assert_eq!(loaded.source(), BUILT_IN_SAFE_DEFINITION);
    assert!(loaded.used_safe_fallback());
    assert!(loaded.warning().unwrap().contains("unreadable"));
    assert_eq!(fs::read(store.path()).unwrap(), bad_bytes);
}

#[test]
fn unreadable_startup_state_falls_back_without_replacing_it() {
    let directory = TestDirectory::new("unreadable-startup");
    let store = DefinitionStore::new(&directory.0);
    fs::create_dir(store.path()).unwrap();

    let loaded = store.load().unwrap();

    assert_eq!(loaded.source(), BUILT_IN_SAFE_DEFINITION);
    assert!(loaded.used_safe_fallback());
    assert!(loaded.warning().unwrap().contains("unreadable"));
    assert!(store.path().is_dir());
}

#[test]
fn parser_valid_nul_text_falls_back_before_native_rendering() {
    let directory = TestDirectory::new("nul-startup");
    let store = DefinitionStore::new(&directory.0);
    let source = BUILT_IN_SAFE_DEFINITION.replacen(
        "\"placeholder\": \"Enter address\"",
        "\"placeholder\": \"Enter\\u0000address\"",
        1,
    );
    parse_definition(&source).unwrap();
    fs::write(store.path(), &source).unwrap();

    let loaded = store.load().unwrap();

    assert_eq!(loaded.source(), BUILT_IN_SAFE_DEFINITION);
    assert!(loaded.used_safe_fallback());
    assert!(loaded.warning().unwrap().contains("NUL byte"));
    assert_eq!(fs::read_to_string(store.path()).unwrap(), source);
}

#[test]
fn atomic_save_persists_the_exact_owned_snapshot() {
    let directory = TestDirectory::new("atomic-save");
    let store = DefinitionStore::new(&directory.0);
    let mut source = include_str!("../../../presets/classic/definition.json").to_owned();
    let exact = source.clone();

    store.save(&source).unwrap();
    source.clear();
    source.push_str(include_str!("../../../presets/workspace/definition.json"));

    let loaded = store.load().unwrap();
    assert_eq!(loaded.source(), exact);
    assert!(!loaded.used_safe_fallback());
    assert_eq!(
        fs::read_dir(&directory.0).unwrap().count(),
        1,
        "atomic temporary file should not remain"
    );
}

#[derive(Default)]
struct Engine {
    next: u64,
}

impl EnginePort for Engine {
    fn create_page(&mut self, _url: &str) -> Result<HostPageId, String> {
        self.next += 1;
        Ok(HostPageId::new(self.next))
    }

    fn load_url(&mut self, _page: HostPageId, _url: &str) -> Result<(), String> {
        Ok(())
    }

    fn back(&mut self, _page: HostPageId) -> Result<(), String> {
        Ok(())
    }

    fn forward(&mut self, _page: HostPageId) -> Result<(), String> {
        Ok(())
    }

    fn close_page(&mut self, _page: HostPageId) -> Result<(), String> {
        Ok(())
    }

    fn attach_page(&mut self, _page: HostPageId) -> Result<(), String> {
        Ok(())
    }

    fn detach_page(&mut self, _page: HostPageId) -> Result<(), String> {
        Ok(())
    }
}

struct Renderer;

impl LayoutRenderer for Renderer {
    fn render(
        &mut self,
        _definition: &BrowserDefinition,
        _generation: u64,
        _pages: &[PageState],
        _selected: Option<HostPageId>,
    ) -> Result<(), String> {
        Ok(())
    }
}

#[test]
fn failed_apply_save_retains_exact_active_source_for_retry_without_reapply() {
    let directory = TestDirectory::new("apply-save-retry");
    let store = DefinitionStore::new(&directory.0);
    store.save(BUILT_IN_SAFE_DEFINITION).unwrap();
    let mut persistence = DefinitionPersistence::new(Some(store));
    let mut engine = Engine::default();
    let mut renderer = Renderer;
    let mut controller =
        BrowserController::new(BUILT_IN_SAFE_DEFINITION, "about:blank", &mut engine).unwrap();
    let workspace = include_str!("../../../presets/workspace/definition.json");
    let candidate = controller.preview_definition(workspace).unwrap();
    controller
        .apply_preview(candidate, &mut engine, &mut renderer)
        .unwrap();
    let generation = controller.layout_generation();
    let backup = block_state_directory(&directory.0);

    let error = persistence
        .persist(controller.active_definition_source())
        .unwrap_err();

    assert!(error.contains("state directory"), "{error}");
    assert_eq!(persistence.unsaved_source(), Some(workspace));
    assert_eq!(controller.active_definition_source(), workspace);
    restore_state_directory(&directory.0, &backup);
    persistence.retry().unwrap();
    assert_eq!(controller.layout_generation(), generation);
    assert_eq!(
        DefinitionStore::new(&directory.0).load().unwrap().source(),
        workspace
    );
    assert_eq!(persistence.unsaved_source(), None);
}

#[test]
fn failed_restore_save_retains_safe_source_for_retry() {
    let directory = TestDirectory::new("restore-save-retry");
    let store = DefinitionStore::new(&directory.0);
    let workspace = include_str!("../../../presets/workspace/definition.json");
    store.save(workspace).unwrap();
    let mut persistence = DefinitionPersistence::new(Some(store));
    let mut engine = Engine::default();
    let mut renderer = Renderer;
    let mut controller = BrowserController::new(workspace, "about:blank", &mut engine).unwrap();
    controller
        .restore_default(&mut engine, &mut renderer)
        .unwrap();
    let generation = controller.layout_generation();
    let backup = block_state_directory(&directory.0);

    persistence.persist(BUILT_IN_SAFE_DEFINITION).unwrap_err();

    assert_eq!(persistence.unsaved_source(), Some(BUILT_IN_SAFE_DEFINITION));
    assert_eq!(
        controller.active_definition_source(),
        BUILT_IN_SAFE_DEFINITION
    );
    restore_state_directory(&directory.0, &backup);
    persistence.retry().unwrap();
    assert_eq!(controller.layout_generation(), generation);
    assert_eq!(
        DefinitionStore::new(&directory.0).load().unwrap().source(),
        BUILT_IN_SAFE_DEFINITION
    );
    assert_eq!(persistence.unsaved_source(), None);
}

fn block_state_directory(directory: &PathBuf) -> PathBuf {
    let backup = directory.with_extension("saved");
    fs::rename(directory, &backup).unwrap();
    fs::write(directory, b"blocks directory creation").unwrap();
    backup
}

fn restore_state_directory(directory: &PathBuf, backup: &PathBuf) {
    fs::remove_file(directory).unwrap();
    fs::rename(backup, directory).unwrap();
}
