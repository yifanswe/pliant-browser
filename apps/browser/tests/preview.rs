use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

use pliant_browser::{
    BrowserController, EnginePort, HostPageId, LayoutRenderer, PageState, PreviewCoordinator,
    PreviewLauncher, PreviewStatus, ProcessPreviewLauncher,
};
use pliant_ui_definition::{BUILT_IN_SAFE_DEFINITION, BrowserDefinition};

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
        if self.0.exists() {
            fs::remove_dir_all(&self.0).unwrap();
        }
    }
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

#[derive(Default)]
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

#[derive(Default)]
struct Launcher {
    launched: Vec<String>,
    stopped: Vec<u64>,
}

impl PreviewLauncher for Launcher {
    type Handle = u64;

    fn launch(&mut self, source: &str) -> Result<Self::Handle, String> {
        self.launched.push(source.to_owned());
        Ok(self.launched.len() as u64)
    }

    fn stop(&mut self, handle: &mut Self::Handle) -> Result<(), String> {
        self.stopped.push(*handle);
        Ok(())
    }

    fn status(&mut self, _handle: &mut Self::Handle) -> Result<PreviewStatus, String> {
        Ok(PreviewStatus::Ready)
    }
}

#[test]
fn replacement_preview_stops_owned_child_and_only_latest_can_apply() {
    let mut engine = Engine::default();
    let mut renderer = Renderer;
    let mut controller =
        BrowserController::new(BUILT_IN_SAFE_DEFINITION, "about:blank", &mut engine).unwrap();
    let mut previews = PreviewCoordinator::new(Launcher::default());
    let classic = include_str!("../../../presets/classic/definition.json");
    let workspace = include_str!("../../../presets/workspace/definition.json");

    let candidate_a = previews.preview(&mut controller, classic).unwrap();
    let candidate_b = previews.preview(&mut controller, workspace).unwrap();

    assert_eq!(previews.candidate_id(), Some(candidate_b));
    assert_eq!(previews.launcher().launched, [classic, workspace]);
    assert_eq!(previews.launcher().stopped, [1]);
    assert!(
        previews
            .apply(candidate_a, &mut controller, &mut engine, &mut renderer)
            .is_err()
    );
    previews
        .apply(candidate_b, &mut controller, &mut engine, &mut renderer)
        .unwrap();
    assert_eq!(previews.launcher().stopped, [1, 2]);
    assert_eq!(previews.candidate_id(), None);
}

#[test]
fn failed_preview_launch_keeps_existing_candidate_and_child() {
    struct FailsSecond {
        calls: usize,
        stopped: Vec<u64>,
    }

    impl PreviewLauncher for FailsSecond {
        type Handle = u64;

        fn launch(&mut self, _source: &str) -> Result<Self::Handle, String> {
            self.calls += 1;
            if self.calls == 2 {
                Err("preview executable did not launch".to_owned())
            } else {
                Ok(self.calls as u64)
            }
        }

        fn stop(&mut self, handle: &mut Self::Handle) -> Result<(), String> {
            self.stopped.push(*handle);
            Ok(())
        }

        fn status(&mut self, _handle: &mut Self::Handle) -> Result<PreviewStatus, String> {
            Ok(PreviewStatus::Ready)
        }
    }

    let mut engine = Engine::default();
    let mut renderer = Renderer;
    let mut controller =
        BrowserController::new(BUILT_IN_SAFE_DEFINITION, "about:blank", &mut engine).unwrap();
    let mut previews = PreviewCoordinator::new(FailsSecond {
        calls: 0,
        stopped: vec![],
    });
    let classic = include_str!("../../../presets/classic/definition.json");
    let workspace = include_str!("../../../presets/workspace/definition.json");
    let candidate = previews.preview(&mut controller, classic).unwrap();

    assert!(previews.preview(&mut controller, workspace).is_err());
    assert!(previews.launcher().stopped.is_empty());
    previews
        .apply(candidate, &mut controller, &mut engine, &mut renderer)
        .unwrap();
}

#[test]
fn process_preview_owns_exact_snapshot_and_cleans_only_its_directory() {
    let directory = TestDirectory::new("process-preview");
    let executable = directory.0.join("preview-probe.sh");
    fs::write(&executable, "#!/bin/sh\nsleep 30\n").unwrap();
    let mut permissions = fs::metadata(&executable).unwrap().permissions();
    permissions.set_mode(0o700);
    fs::set_permissions(&executable, permissions).unwrap();
    let preview_root = directory.0.join("owned-previews");
    let mut launcher = ProcessPreviewLauncher::new(&executable, &preview_root);
    let source = include_str!("../../../presets/classic/definition.json");

    let mut handle = launcher.launch(source).unwrap();
    let owned_directory = handle.directory().to_owned();
    assert_eq!(fs::read_to_string(handle.snapshot_path()).unwrap(), source);
    assert!(owned_directory.starts_with(&preview_root));

    launcher.stop(&mut handle).unwrap();
    assert!(!owned_directory.exists());
    assert!(executable.exists());
}

#[test]
fn sleeping_process_that_never_renders_cannot_authorize_apply() {
    let directory = TestDirectory::new("unready-process");
    let executable = directory.0.join("sleeping-preview.sh");
    fs::write(&executable, "#!/bin/sh\nsleep 30\n").unwrap();
    let mut permissions = fs::metadata(&executable).unwrap().permissions();
    permissions.set_mode(0o700);
    fs::set_permissions(&executable, permissions).unwrap();

    let mut engine = Engine::default();
    let mut renderer = Renderer;
    let mut controller =
        BrowserController::new(BUILT_IN_SAFE_DEFINITION, "about:blank", &mut engine).unwrap();
    let original_generation = controller.layout_generation();
    let mut previews = PreviewCoordinator::new(ProcessPreviewLauncher::new(
        &executable,
        directory.0.join("owned-previews"),
    ));
    let classic = include_str!("../../../presets/classic/definition.json");
    let candidate = previews.preview(&mut controller, classic).unwrap();

    let error = previews
        .apply(candidate, &mut controller, &mut engine, &mut renderer)
        .unwrap_err();

    assert!(error.contains("not ready"));
    assert_eq!(controller.layout_generation(), original_generation);
}

#[test]
fn real_ready_acknowledgement_authorizes_only_its_candidate() {
    let directory = TestDirectory::new("ready-process");
    let executable = write_preview_script(
        &directory,
        "ready-preview.sh",
        r#"printf 'ready\n%s\n' "$token" > "$ack.tmp"
mv "$ack.tmp" "$ack"
sleep 30"#,
    );
    let mut engine = Engine::default();
    let mut renderer = Renderer;
    let mut controller =
        BrowserController::new(BUILT_IN_SAFE_DEFINITION, "about:blank", &mut engine).unwrap();
    let mut previews = PreviewCoordinator::new(ProcessPreviewLauncher::new(
        &executable,
        directory.0.join("owned-previews"),
    ));
    let classic = include_str!("../../../presets/classic/definition.json");
    let candidate = previews.preview(&mut controller, classic).unwrap();

    let mut status = PreviewStatus::Starting;
    for _ in 0..100 {
        status = previews.refresh(candidate, &mut controller).unwrap();
        if status == PreviewStatus::Ready {
            break;
        }
        std::thread::sleep(Duration::from_millis(5));
    }
    assert_eq!(status, PreviewStatus::Ready);
    previews
        .apply(candidate, &mut controller, &mut engine, &mut renderer)
        .unwrap();
}

#[test]
fn preview_child_receives_the_main_process_fixture_origin() {
    let directory = TestDirectory::new("shared-origin");
    let executable = write_preview_script(
        &directory,
        "shared-origin-preview.sh",
        r#"printf '%s' "$initial_url" > "$observed.tmp"
mv "$observed.tmp" "$observed"
printf 'ready\n%s\n' "$token" > "$ack.tmp"
mv "$ack.tmp" "$ack"
sleep 30"#,
    );
    let expected = "http://127.0.0.1:43123/a";
    let mut launcher = ProcessPreviewLauncher::new(&executable, directory.0.join("owned-previews"))
        .with_initial_url(expected);
    let source = include_str!("../../../presets/classic/definition.json");
    let mut handle = launcher.launch(source).unwrap();
    let observed = handle.directory().join("observed-url");

    for _ in 0..100 {
        if observed.exists() {
            break;
        }
        std::thread::sleep(Duration::from_millis(5));
    }

    assert_eq!(fs::read_to_string(observed).unwrap(), expected);
    launcher.stop(&mut handle).unwrap();
}

#[test]
fn child_error_and_mismatched_acknowledgement_reject_candidates() {
    for (name, body, expected) in [
        (
            "failed-preview.sh",
            r#"printf 'error\n%s\nnative host failed' "$token" > "$ack.tmp"
mv "$ack.tmp" "$ack"
sleep 30"#,
            "native host failed",
        ),
        (
            "mismatched-preview.sh",
            r#"printf 'ready\nwrong-candidate\n' > "$ack.tmp"
mv "$ack.tmp" "$ack"
sleep 30"#,
            "did not match",
        ),
    ] {
        let directory = TestDirectory::new(name);
        let executable = write_preview_script(&directory, name, body);
        let mut engine = Engine::default();
        let mut controller =
            BrowserController::new(BUILT_IN_SAFE_DEFINITION, "about:blank", &mut engine).unwrap();
        let mut previews = PreviewCoordinator::new(ProcessPreviewLauncher::new(
            &executable,
            directory.0.join("owned-previews"),
        ));
        let classic = include_str!("../../../presets/classic/definition.json");
        let candidate = previews.preview(&mut controller, classic).unwrap();

        let error = loop {
            match previews.refresh(candidate, &mut controller) {
                Ok(PreviewStatus::Starting) => std::thread::sleep(Duration::from_millis(5)),
                Ok(PreviewStatus::Ready) => panic!("invalid acknowledgement became ready"),
                Err(error) => break error,
            }
        };

        assert!(error.contains(expected), "{error}");
        assert_eq!(previews.candidate_id(), None);
    }
}

#[test]
fn readiness_timeout_rejects_the_candidate() {
    let directory = TestDirectory::new("timeout-process");
    let executable = write_preview_script(&directory, "timeout-preview.sh", "sleep 30");
    let mut engine = Engine::default();
    let mut controller =
        BrowserController::new(BUILT_IN_SAFE_DEFINITION, "about:blank", &mut engine).unwrap();
    let mut previews = PreviewCoordinator::new(
        ProcessPreviewLauncher::new(&executable, directory.0.join("owned-previews"))
            .with_timeout(Duration::ZERO),
    );
    let classic = include_str!("../../../presets/classic/definition.json");

    let candidate = previews.preview(&mut controller, classic).unwrap();
    let error = previews.refresh(candidate, &mut controller).unwrap_err();

    assert!(error.contains("timed out"), "{error}");
    assert_eq!(previews.candidate_id(), None);
}

#[test]
fn first_ready_observation_after_deadline_rejects_candidate() {
    let directory = TestDirectory::new("late-ready-process");
    let executable = write_preview_script(
        &directory,
        "late-ready-preview.sh",
        r#"sleep 0.08
printf 'ready\n%s\n' "$token" > "$ack.tmp"
mv "$ack.tmp" "$ack"
sleep 30"#,
    );
    let mut engine = Engine::default();
    let mut controller =
        BrowserController::new(BUILT_IN_SAFE_DEFINITION, "about:blank", &mut engine).unwrap();
    let mut previews = PreviewCoordinator::new(
        ProcessPreviewLauncher::new(&executable, directory.0.join("owned-previews"))
            .with_timeout(Duration::from_millis(30)),
    );
    let classic = include_str!("../../../presets/classic/definition.json");
    let candidate = previews.preview(&mut controller, classic).unwrap();
    std::thread::sleep(Duration::from_millis(150));

    let error = previews.refresh(candidate, &mut controller).unwrap_err();

    assert!(error.contains("timed out"), "{error}");
    assert_eq!(previews.candidate_id(), None);
}

#[test]
fn ready_accepted_before_deadline_remains_applicable_after_deadline() {
    let directory = TestDirectory::new("latched-ready-process");
    let executable = write_preview_script(
        &directory,
        "latched-ready-preview.sh",
        r#"printf 'ready\n%s\n' "$token" > "$ack.tmp"
mv "$ack.tmp" "$ack"
sleep 30"#,
    );
    let mut engine = Engine::default();
    let mut renderer = Renderer;
    let mut controller =
        BrowserController::new(BUILT_IN_SAFE_DEFINITION, "about:blank", &mut engine).unwrap();
    let mut previews = PreviewCoordinator::new(
        ProcessPreviewLauncher::new(&executable, directory.0.join("owned-previews"))
            .with_timeout(Duration::from_millis(500)),
    );
    let classic = include_str!("../../../presets/classic/definition.json");
    let candidate = previews.preview(&mut controller, classic).unwrap();
    while let PreviewStatus::Starting = previews.refresh(candidate, &mut controller).unwrap() {
        std::thread::sleep(Duration::from_millis(5));
    }
    std::thread::sleep(Duration::from_millis(600));

    previews
        .apply(candidate, &mut controller, &mut engine, &mut renderer)
        .unwrap();

    assert_eq!(controller.active_definition_source(), classic);
}

#[test]
fn exited_preview_cannot_be_applied() {
    struct Exited;

    impl PreviewLauncher for Exited {
        type Handle = ();

        fn launch(&mut self, _source: &str) -> Result<Self::Handle, String> {
            Ok(())
        }

        fn stop(&mut self, _handle: &mut Self::Handle) -> Result<(), String> {
            Ok(())
        }

        fn status(&mut self, _handle: &mut Self::Handle) -> Result<PreviewStatus, String> {
            Err("preview process is no longer running".to_owned())
        }
    }

    let mut engine = Engine::default();
    let mut renderer = Renderer;
    let mut controller =
        BrowserController::new(BUILT_IN_SAFE_DEFINITION, "about:blank", &mut engine).unwrap();
    let original_generation = controller.layout_generation();
    let classic = include_str!("../../../presets/classic/definition.json");
    let mut previews = PreviewCoordinator::new(Exited);
    let candidate = previews.preview(&mut controller, classic).unwrap();

    let error = previews
        .apply(candidate, &mut controller, &mut engine, &mut renderer)
        .unwrap_err();

    assert!(error.contains("no longer running"));
    assert_eq!(controller.layout_generation(), original_generation);
}

fn write_preview_script(directory: &TestDirectory, name: &str, body: &str) -> PathBuf {
    let executable = directory.0.join(name);
    let script = format!(
        r#"#!/bin/sh
ack=
token=
initial_url=
observed=
while [ "$#" -gt 0 ]; do
  case "$1" in
    --preview-ack) ack="$2"; shift 2 ;;
    --preview-token) token="$2"; shift 2 ;;
    --initial-url) initial_url="$2"; shift 2 ;;
    *) shift ;;
  esac
done
observed="$(dirname "$ack")/observed-url"
{body}
"#
    );
    fs::write(&executable, script).unwrap();
    let mut permissions = fs::metadata(&executable).unwrap().permissions();
    permissions.set_mode(0o700);
    fs::set_permissions(&executable, permissions).unwrap();
    executable
}
