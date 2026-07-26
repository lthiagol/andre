use std::path::PathBuf;
use std::process::Command;
use std::time::Duration;

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use tempfile::TempDir;

use andre::app::App;
use andre::components::execute::ExecuteComponent;
use andre::execute::ExecutionState;

fn binary_path() -> PathBuf {
    // Try llvm-cov target dir first, then default debug dir
    let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let root = manifest.parent().unwrap();
    let candidates = [
        root.join("target").join("llvm-cov-target").join("debug"),
        root.join("target").join("debug"),
    ];
    for dir in &candidates {
        let path = dir.join("andre");
        if path.exists() {
            return path;
        }
    }
    candidates[1].join("andre")
}

fn make_tui_e2e_app() -> (App, TempDir) {
    let tmp = TempDir::new().unwrap();
    let source_base = tmp.path().join("packages");
    let target = tmp.path().join("target");

    let pkg_dir = source_base.join("bash-env");
    std::fs::create_dir_all(&pkg_dir).unwrap();
    std::fs::write(pkg_dir.join(".bashrc"), "export FOO=1\n").unwrap();
    std::fs::create_dir_all(&target).unwrap();

    let yaml = format!(
        r#"
global:
  stow:
    verbosity: 0
groups:
  home:
    source: {}
    target: {}
"#,
        source_base.display(),
        target.display()
    );

    let config_path = tmp.path().join("andre.yml");
    std::fs::write(&config_path, &yaml).unwrap();
    let app = App::new(config_path, false).unwrap();
    (app, tmp)
}

fn key(code: KeyCode) -> KeyEvent {
    KeyEvent::new(code, KeyModifiers::NONE)
}

fn run_tui_flow<F>(test_fn: F)
where
    F: FnOnce(&mut App, &TempDir),
{
    let (mut app, tmp) = make_tui_e2e_app();
    let rt = tokio::runtime::Runtime::new().unwrap();
    let _guard = rt.enter();

    test_fn(&mut app, &tmp);

    rt.shutdown_timeout(Duration::from_secs(5));
}

fn pump_until_complete(app: &mut App, timeout_secs: u64) {
    let deadline = std::time::Instant::now() + Duration::from_secs(timeout_secs);
    loop {
        app.update_components();
        if let Some(comp) = app.component_stack.last() {
            if comp.id() != "Execute" {
                return;
            }
            if let Some(exec) = comp.as_any().downcast_ref::<ExecuteComponent>() {
                if exec.state == ExecutionState::Completed {
                    return;
                }
            }
        }
        if std::time::Instant::now() > deadline {
            panic!("Timed out waiting for execution to complete");
        }
        std::thread::sleep(Duration::from_millis(20));
    }
}

fn navigate_to_confirm(app: &mut App) {
    app.component_dispatch(key(KeyCode::Enter));
    app.component_dispatch(key(KeyCode::Char(' ')));
    app.component_dispatch(key(KeyCode::Enter));
    app.component_dispatch(key(KeyCode::Char(' ')));
    app.component_dispatch(key(KeyCode::Enter));
}

#[test]
fn test_tui_full_stow_creates_symlink() {
    run_tui_flow(|app, tmp| {
        let symlink = tmp.path().join("target/.bashrc");

        navigate_to_confirm(app);
        assert_eq!(app.component_stack.last().unwrap().id(), "Confirm");

        app.component_dispatch(key(KeyCode::Char('y')));
        assert_eq!(app.component_stack.last().unwrap().id(), "Execute");

        // M05 AC-02: App owns the AsyncExecutor for Execute's lifetime;
        // Confirm must not have dropped the only handle.
        assert!(
            app.executor.is_some(),
            "App must own the AsyncExecutor while Execute is on the stack"
        );

        pump_until_complete(app, 10);

        assert!(symlink.exists(), ".bashrc should exist in target");
        assert!(symlink.is_symlink(), ".bashrc should be a symlink");

        let link_target = std::fs::read_link(&symlink).unwrap();
        assert!(
            link_target.to_string_lossy().contains("bash-env"),
            "symlink should point to package file, got: {}",
            link_target.display()
        );

        let top = app.component_stack.last().unwrap();
        let exec = top.as_any().downcast_ref::<ExecuteComponent>().unwrap();
        assert_eq!(exec.state, ExecutionState::Completed);
        assert_eq!(exec.results.len(), 1);
        assert!(exec.results[0].success);
        // Finished UI summary (N succeeded, M failed) is asserted via render buffer
        // in test_execute_completion_summary_on_finished (L1: not recomputed here).

        // Leaving Execute (PopAll) releases the executor handle.
        app.component_dispatch(key(KeyCode::Enter));
        assert!(
            app.executor.is_none(),
            "executor must be released once Execute leaves the stack"
        );
        assert_ne!(
            app.component_stack.last().unwrap().id(),
            "Execute",
            "Enter on completed Execute must leave the screen"
        );
    });
}

#[test]
fn test_tui_dry_run_does_not_create_symlink() {
    run_tui_flow(|app, tmp| {
        let symlink = tmp.path().join("target/.bashrc");

        app.component_dispatch(key(KeyCode::Char('d')));
        assert!(app.ctx.core.dry_run);

        navigate_to_confirm(app);
        app.component_dispatch(key(KeyCode::Char('y')));
        assert_eq!(app.component_stack.last().unwrap().id(), "Execute");

        pump_until_complete(app, 10);

        assert!(!symlink.exists(), "dry-run should not create symlink");
    });
}

#[test]
fn test_tui_missing_target_skips_execution() {
    let tmp = TempDir::new().unwrap();
    let source_base = tmp.path().join("packages");
    let pkg_dir = source_base.join("bash-env");
    std::fs::create_dir_all(&pkg_dir).unwrap();
    std::fs::write(pkg_dir.join(".bashrc"), "export FOO=1\n").unwrap();

    let yaml = format!(
        r#"
global:
  stow:
    verbosity: 0
groups:
  home:
    source: {}
    target: {}
"#,
        source_base.display(),
        tmp.path().join("nonexistent-target").display()
    );

    let config_path = tmp.path().join("andre.yml");
    std::fs::write(&config_path, &yaml).unwrap();
    let mut app = App::new(config_path, false).unwrap();

    let rt = tokio::runtime::Runtime::new().unwrap();
    let _guard = rt.enter();

    navigate_to_confirm(&mut app);
    assert_eq!(app.component_stack.last().unwrap().id(), "Confirm");

    app.component_dispatch(key(KeyCode::Char('y')));
    assert_eq!(app.component_stack.last().unwrap().id(), "Execute");

    pump_until_complete(&mut app, 10);

    let top = app.component_stack.last().unwrap();
    let exec = top.as_any().downcast_ref::<ExecuteComponent>().unwrap();
    assert_eq!(
        exec.results.len(),
        1,
        "expected one skipped result when target dir is missing"
    );
    assert!(!exec.results[0].success);
    assert!(exec.results[0]
        .error_message
        .as_deref()
        .unwrap()
        .contains("Skipped"),);
}

#[test]
fn test_onboard_requires_config() {
    let output = Command::new(binary_path())
        .arg("--onboard")
        .output()
        .unwrap();
    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("--onboard requires --config"),
        "expected error about missing --config, got: {}",
        stderr
    );
}

#[test]
fn test_unknown_config_field_rejected() {
    let tmp = TempDir::new().unwrap();
    let config_path = tmp.path().join("andre.yml");
    std::fs::write(
        &config_path,
        r#"
global:
  stow:
    nonexistent: true
groups: {}
"#,
    )
    .unwrap();

    let output = Command::new(binary_path())
        .arg("--config")
        .arg(&config_path)
        .output()
        .unwrap();
    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("unknown field") || stderr.contains("nonexistent"),
        "expected error about unknown field, got: {}",
        stderr
    );
}

#[test]
fn test_onboard_creates_config_file() {
    let tmp = TempDir::new().unwrap();
    let config_path = tmp.path().join("andre.yaml");

    let mut child = Command::new(binary_path())
        .arg("--onboard")
        .arg("--config")
        .arg(&config_path)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn()
        .unwrap();

    // Wait for the file to appear (created before TUI entry), then kill
    let mut found = false;
    for _ in 0..50 {
        if config_path.exists() {
            found = true;
            break;
        }
        std::thread::sleep(Duration::from_millis(100));
    }
    let _ = child.kill();
    let _ = child.wait();

    assert!(
        found,
        "onboard should create config file at: {}",
        config_path.display()
    );
    let content = std::fs::read_to_string(&config_path).unwrap();
    assert!(
        content.contains("global:") && content.contains("groups:"),
        "config should contain global and groups"
    );
}
