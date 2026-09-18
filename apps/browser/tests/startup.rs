use pliant_browser::render_initial_layout;
use pliant_ui_definition::BUILT_IN_SAFE_DEFINITION;

#[test]
fn main_startup_render_failure_recovers_to_safe_layout() {
    let workspace = include_str!("../../../presets/workspace/definition.json");
    let mut rendered_roots = Vec::new();

    let layout = render_initial_layout(workspace, None, false, |definition| {
        rendered_roots.push(definition.root().id().to_owned());
        if rendered_roots.len() == 1 {
            Err("candidate render failed".to_owned())
        } else {
            Ok(())
        }
    })
    .unwrap();

    assert_eq!(rendered_roots, ["workspace-root", "safe-root"]);
    assert_eq!(layout.source(), BUILT_IN_SAFE_DEFINITION);
    assert!(
        layout
            .warning()
            .unwrap()
            .contains("candidate render failed")
    );
}

#[test]
fn preview_startup_render_failure_does_not_succeed_with_safe_layout() {
    let workspace = include_str!("../../../presets/workspace/definition.json");
    let mut rendered_roots = Vec::new();

    let error = render_initial_layout(workspace, None, true, |definition| {
        rendered_roots.push(definition.root().id().to_owned());
        if rendered_roots.len() == 1 {
            Err("candidate render failed".to_owned())
        } else {
            Ok(())
        }
    })
    .unwrap_err();

    assert!(
        error.contains("preview candidate failed to render"),
        "{error}"
    );
    assert_eq!(rendered_roots, ["workspace-root"]);
}
