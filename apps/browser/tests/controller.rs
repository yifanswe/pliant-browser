use pliant_browser::{BrowserController, EnginePort, HostPageId, LayoutRenderer, PageState};
use pliant_ui_definition::{BUILT_IN_SAFE_DEFINITION, BrowserDefinition};

#[derive(Default)]
struct RecordingEngine {
    next_id: u64,
    created: Vec<(HostPageId, String)>,
    loaded: Vec<(HostPageId, String)>,
    backed: Vec<HostPageId>,
    forwarded: Vec<HostPageId>,
    closed: Vec<HostPageId>,
    attached: Vec<HostPageId>,
    detached: Vec<HostPageId>,
    attach_failures: usize,
}

impl EnginePort for RecordingEngine {
    fn create_page(&mut self, url: &str) -> Result<HostPageId, String> {
        self.next_id += 1;
        let page = HostPageId::new(self.next_id);
        self.created.push((page, url.to_owned()));
        Ok(page)
    }

    fn load_url(&mut self, page: HostPageId, url: &str) -> Result<(), String> {
        self.loaded.push((page, url.to_owned()));
        Ok(())
    }

    fn back(&mut self, page: HostPageId) -> Result<(), String> {
        self.backed.push(page);
        Ok(())
    }

    fn forward(&mut self, page: HostPageId) -> Result<(), String> {
        self.forwarded.push(page);
        Ok(())
    }

    fn close_page(&mut self, page: HostPageId) -> Result<(), String> {
        self.closed.push(page);
        Ok(())
    }

    fn attach_page(&mut self, page: HostPageId) -> Result<(), String> {
        if self.attach_failures > 0 {
            self.attach_failures -= 1;
            return Err("native attach failed".to_owned());
        }
        self.attached.push(page);
        Ok(())
    }

    fn detach_page(&mut self, page: HostPageId) -> Result<(), String> {
        self.detached.push(page);
        Ok(())
    }
}

#[derive(Default)]
struct RecordingRenderer {
    renders: Vec<(u64, String, Vec<HostPageId>, Option<HostPageId>)>,
}

impl LayoutRenderer for RecordingRenderer {
    fn render(
        &mut self,
        definition: &BrowserDefinition,
        generation: u64,
        pages: &[PageState],
        selected: Option<HostPageId>,
    ) -> Result<(), String> {
        self.renders.push((
            generation,
            definition.root().id().to_owned(),
            pages.iter().map(PageState::id).collect(),
            selected,
        ));
        Ok(())
    }
}

struct FailingRenderer {
    roots: Vec<String>,
    fail_first: bool,
}

impl LayoutRenderer for FailingRenderer {
    fn render(
        &mut self,
        definition: &BrowserDefinition,
        _generation: u64,
        _pages: &[PageState],
        _selected: Option<HostPageId>,
    ) -> Result<(), String> {
        self.roots.push(definition.root().id().to_owned());
        if self.fail_first {
            self.fail_first = false;
            Err("native layout failed".to_owned())
        } else {
            Ok(())
        }
    }
}

#[test]
fn current_page_address_open_reuses_identity() {
    let mut engine = RecordingEngine::default();
    let mut controller = BrowserController::new(
        BUILT_IN_SAFE_DEFINITION,
        "https://fixture.test/a",
        &mut engine,
    )
    .unwrap();
    let original = controller.selected_page().unwrap();

    controller
        .submit_address(
            controller.layout_generation(),
            "https://fixture.test/b",
            &mut engine,
        )
        .unwrap();

    assert_eq!(controller.selected_page(), Some(original));
    assert_eq!(controller.pages().len(), 1);
    assert_eq!(
        engine.loaded,
        vec![(original, "https://fixture.test/b".to_owned())]
    );
}

#[test]
fn new_page_address_open_preserves_the_existing_page() {
    let workspace = include_str!("../../../presets/workspace/definition.json");
    let mut engine = RecordingEngine::default();
    let mut controller =
        BrowserController::new(workspace, "https://fixture.test/a", &mut engine).unwrap();
    let original = controller.selected_page().unwrap();

    controller
        .submit_address(
            controller.layout_generation(),
            "https://fixture.test/b",
            &mut engine,
        )
        .unwrap();

    let selected = controller.selected_page().unwrap();
    assert_ne!(selected, original);
    assert_eq!(controller.pages().len(), 2);
    assert_eq!(
        engine.created,
        vec![
            (original, "https://fixture.test/a".to_owned()),
            (selected, "https://fixture.test/b".to_owned())
        ]
    );
    assert!(engine.loaded.is_empty());
}

#[test]
fn button_actions_are_resolved_from_active_node_identity() {
    let classic = include_str!("../../../presets/classic/definition.json");
    let mut engine = RecordingEngine::default();
    let mut controller =
        BrowserController::new(classic, "https://fixture.test/a", &mut engine).unwrap();
    let page = controller.selected_page().unwrap();
    let generation = controller.layout_generation();

    controller
        .activate_button(generation, "classic-back", &mut engine)
        .unwrap();
    controller
        .activate_button(generation, "classic-forward", &mut engine)
        .unwrap();
    controller
        .activate_button(generation, "classic-home", &mut engine)
        .unwrap();
    controller
        .activate_button(generation, "classic-close-page", &mut engine)
        .unwrap();
    controller
        .activate_button(generation, "classic-new-page", &mut engine)
        .unwrap();

    assert_eq!(engine.backed, vec![page]);
    assert_eq!(engine.forwarded, vec![page]);
    assert_eq!(
        engine.loaded,
        vec![(page, "https://example.com/".to_owned())]
    );
    assert_eq!(engine.closed, vec![page]);
    assert_eq!(controller.pages().len(), 2);
    assert_ne!(controller.selected_page(), Some(page));
    assert_eq!(engine.detached.last(), Some(&page));
    assert_eq!(engine.attached.last(), controller.selected_page().as_ref());

    let error = controller
        .activate_button(generation, "classic-tabs", &mut engine)
        .unwrap_err();
    assert!(error.contains("button"));
}

#[test]
fn page_selection_switches_attached_content_without_reloading() {
    let workspace = include_str!("../../../presets/workspace/definition.json");
    let mut engine = RecordingEngine::default();
    let mut controller =
        BrowserController::new(workspace, "https://fixture.test/a", &mut engine).unwrap();
    let first = controller.selected_page().unwrap();
    let generation = controller.layout_generation();
    controller
        .submit_address(generation, "https://fixture.test/b", &mut engine)
        .unwrap();
    let second = controller.selected_page().unwrap();

    controller
        .select_page(generation, first, &mut engine)
        .unwrap();

    assert_eq!(controller.selected_page(), Some(first));
    assert_eq!(engine.detached, vec![first, second]);
    assert_eq!(engine.attached, vec![first, second, first]);
    assert!(engine.loaded.is_empty());
}

#[test]
fn close_event_removes_actual_page_and_selects_a_survivor() {
    let workspace = include_str!("../../../presets/workspace/definition.json");
    let mut engine = RecordingEngine::default();
    let mut controller =
        BrowserController::new(workspace, "https://fixture.test/a", &mut engine).unwrap();
    let first = controller.selected_page().unwrap();
    let generation = controller.layout_generation();
    controller
        .submit_address(generation, "https://fixture.test/b", &mut engine)
        .unwrap();
    let second = controller.selected_page().unwrap();

    controller.page_closed(second, &mut engine).unwrap();

    assert_eq!(controller.pages().len(), 1);
    assert_eq!(controller.selected_page(), Some(first));
    assert_eq!(engine.attached.last(), Some(&first));
}

#[test]
fn applying_exact_preview_rebuilds_layout_without_recreating_pages() {
    let mut engine = RecordingEngine::default();
    let mut renderer = RecordingRenderer::default();
    let mut controller = BrowserController::new(
        BUILT_IN_SAFE_DEFINITION,
        "https://fixture.test/a",
        &mut engine,
    )
    .unwrap();
    let selected = controller.selected_page().unwrap();
    let old_generation = controller.layout_generation();
    let candidate_source = include_str!("../../../presets/workspace/definition.json");
    let candidate = controller.preview_definition(candidate_source).unwrap();
    let created_before_apply = engine.created.clone();

    controller
        .apply_preview(candidate, &mut engine, &mut renderer)
        .unwrap();

    assert_eq!(controller.pages().len(), 1);
    assert_eq!(controller.selected_page(), Some(selected));
    assert_eq!(engine.created, created_before_apply);
    assert_eq!(controller.layout_generation(), old_generation + 1);
    assert_eq!(
        renderer.renders,
        vec![(
            old_generation + 1,
            "workspace-root".to_owned(),
            vec![selected],
            Some(selected)
        )]
    );
    assert_eq!(engine.detached.last(), Some(&selected));
    assert_eq!(engine.attached.last(), Some(&selected));

    let error = controller
        .activate_button(old_generation, "safe-back", &mut engine)
        .unwrap_err();
    assert!(error.contains("stale layout event"));
}

#[test]
fn failed_preview_and_reject_leave_active_layout_and_pages_unchanged() {
    let mut engine = RecordingEngine::default();
    let mut renderer = RecordingRenderer::default();
    let mut controller = BrowserController::new(
        BUILT_IN_SAFE_DEFINITION,
        "https://fixture.test/a",
        &mut engine,
    )
    .unwrap();
    let generation = controller.layout_generation();
    let pages = controller.pages().to_vec();

    assert!(
        controller
            .preview_definition(r#"{"schema_version": 1"#)
            .is_err()
    );
    assert_eq!(controller.layout_generation(), generation);
    assert_eq!(controller.pages(), pages);
    assert!(renderer.renders.is_empty());

    let classic = include_str!("../../../presets/classic/definition.json");
    let candidate = controller.preview_definition(classic).unwrap();
    controller.reject_preview(candidate).unwrap();
    assert!(
        controller
            .apply_preview(candidate, &mut engine, &mut renderer)
            .is_err()
    );
    assert_eq!(controller.layout_generation(), generation);
    assert_eq!(controller.pages(), pages);
    assert!(renderer.renders.is_empty());
}

#[test]
fn restore_default_rebuilds_only_layout_and_keeps_browsing_state() {
    let workspace = include_str!("../../../presets/workspace/definition.json");
    let mut engine = RecordingEngine::default();
    let mut renderer = RecordingRenderer::default();
    let mut controller =
        BrowserController::new(workspace, "https://fixture.test/a", &mut engine).unwrap();
    let pages_before = controller.pages().to_vec();
    let created_before = engine.created.clone();
    let generation = controller.layout_generation();

    controller
        .restore_default(&mut engine, &mut renderer)
        .unwrap();

    assert_eq!(controller.pages(), pages_before);
    assert_eq!(engine.created, created_before);
    assert_eq!(controller.layout_generation(), generation + 1);
    assert_eq!(renderer.renders[0].1, "safe-root");
}

#[test]
fn committed_navigation_updates_actual_page_url_and_history_state() {
    let mut engine = RecordingEngine::default();
    let mut controller = BrowserController::new(
        BUILT_IN_SAFE_DEFINITION,
        "https://fixture.test/a",
        &mut engine,
    )
    .unwrap();
    let page = controller.selected_page().unwrap();

    controller
        .navigation_committed(page, "https://fixture.test/b", true, false)
        .unwrap();

    assert_eq!(controller.pages()[0].url(), "https://fixture.test/b");
    assert!(controller.pages()[0].can_go_back());
    assert!(!controller.pages()[0].can_go_forward());
}

#[test]
fn navigation_button_state_tracks_selected_page_history() {
    let mut engine = RecordingEngine::default();
    let mut controller = BrowserController::new(
        BUILT_IN_SAFE_DEFINITION,
        "https://fixture.test/a",
        &mut engine,
    )
    .unwrap();
    let page = controller.selected_page().unwrap();

    assert!(!controller.button_enabled("safe-back"));
    controller
        .navigation_committed(page, "https://fixture.test/b", true, false)
        .unwrap();
    assert!(controller.button_enabled("safe-back"));
    assert!(!controller.button_enabled("safe-forward"));
    assert!(controller.button_enabled("safe-new-page"));
}

#[test]
fn failed_native_layout_rebuild_restores_active_layout_and_page() {
    let mut engine = RecordingEngine::default();
    let mut renderer = FailingRenderer {
        roots: vec![],
        fail_first: true,
    };
    let mut controller = BrowserController::new(
        BUILT_IN_SAFE_DEFINITION,
        "https://fixture.test/a",
        &mut engine,
    )
    .unwrap();
    let page = controller.selected_page().unwrap();
    let generation = controller.layout_generation();
    let workspace = include_str!("../../../presets/workspace/definition.json");
    let candidate = controller.preview_definition(workspace).unwrap();

    let error = controller
        .apply_preview(candidate, &mut engine, &mut renderer)
        .unwrap_err();

    assert_eq!(error, "native layout failed");
    assert_eq!(controller.layout_generation(), generation);
    assert_eq!(renderer.roots, ["workspace-root", "safe-root"]);
    assert_eq!(engine.attached.last(), Some(&page));
}

#[test]
fn failed_post_render_attach_restores_layout_generation_and_old_attachment() {
    let mut engine = RecordingEngine::default();
    let mut renderer = FailingRenderer {
        roots: vec![],
        fail_first: false,
    };
    let mut controller = BrowserController::new(
        BUILT_IN_SAFE_DEFINITION,
        "https://fixture.test/a",
        &mut engine,
    )
    .unwrap();
    let page = controller.selected_page().unwrap();
    let generation = controller.layout_generation();
    let workspace = include_str!("../../../presets/workspace/definition.json");
    let candidate = controller.preview_definition(workspace).unwrap();
    engine.attach_failures = 1;

    let error = controller
        .apply_preview(candidate, &mut engine, &mut renderer)
        .unwrap_err();

    assert_eq!(error, "native attach failed");
    assert_eq!(controller.layout_generation(), generation);
    assert_eq!(renderer.roots, ["workspace-root", "safe-root"]);
    assert_eq!(engine.attached.last(), Some(&page));
}

#[test]
fn failed_post_render_attach_reports_recovery_attach_failure() {
    let mut engine = RecordingEngine::default();
    let mut renderer = FailingRenderer {
        roots: vec![],
        fail_first: false,
    };
    let mut controller = BrowserController::new(
        BUILT_IN_SAFE_DEFINITION,
        "https://fixture.test/a",
        &mut engine,
    )
    .unwrap();
    let generation = controller.layout_generation();
    let workspace = include_str!("../../../presets/workspace/definition.json");
    let candidate = controller.preview_definition(workspace).unwrap();
    engine.attach_failures = 2;

    let error = controller
        .apply_preview(candidate, &mut engine, &mut renderer)
        .unwrap_err();

    assert!(error.contains("native attach failed"), "{error}");
    assert!(
        error.contains("reattaching the active page also failed"),
        "{error}"
    );
    assert_eq!(controller.layout_generation(), generation);
    assert_eq!(renderer.roots, ["workspace-root", "safe-root"]);
}
