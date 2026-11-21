use std::path::PathBuf;
use std::process::Command;
use std::time::Duration;

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use tempfile::TempDir;

mod helpers;

use andre::app::App;
use andre::components::execute::ExecuteComponent;
use andre::components::wizard::NewPackageWizard;
use andre::execute::ExecutionState;
use andre_core::profile::ProfileSet;
use andre_core::StowAction;
use helpers::{make_bootstrap_app, make_flat_bootstrap_app};

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
    let app = App::new(config_path).unwrap();
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
    // `2` opens packages from the main menu (row 1 is profiles). Space on the group's header
    // marks its one package, and Enter runs the marks through the confirm screen.
    app.component_dispatch(key(KeyCode::Char('2')));
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

        // Enter on completed Execute leaves the screen.
        app.component_dispatch(key(KeyCode::Enter));
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
        assert!(app.ctx.core.flags.dry_run);

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
    let mut app = App::new(config_path).unwrap();

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
        content.contains("global:") && !content.contains("groups:"),
        "config should contain global and no groups key: {content}"
    );
}

// --- New-package wizard, end to end in a temp HOME ---

/// Config folder at the temp root: group `home` from `dotfiles/home` into `home/` (the temp
/// HOME), one profile `personal` holding `home/git`. Returns the app with the wizard on top.
fn wizard_app() -> (App, TempDir) {
    let tmp = TempDir::new().unwrap();
    std::fs::write(
        tmp.path().join("andre.yml"),
        "global:\n  stow:\n    verbosity: 0\ngroups:\n  home:\n    source: dotfiles/home\n    target: \"~\"\n",
    )
    .unwrap();
    std::fs::create_dir_all(tmp.path().join("dotfiles/home/git")).unwrap();
    std::fs::create_dir_all(tmp.path().join("home")).unwrap();
    std::fs::create_dir_all(tmp.path().join("profiles")).unwrap();
    std::fs::write(
        tmp.path().join("profiles/personal.yml"),
        "packages:\n  - home/git\n",
    )
    .unwrap();
    let mut app = App::with_home(tmp.path().join("andre.yml"), tmp.path().join("home")).unwrap();
    app.component_stack
        .push(Box::new(NewPackageWizard::default()));
    (app, tmp)
}

/// Presses each key in turn, as the event loop would.
fn press_all(app: &mut App, codes: &[KeyCode]) {
    for code in codes {
        app.component_dispatch(key(*code));
    }
}

fn typed(text: &str) -> Vec<KeyCode> {
    text.chars().map(KeyCode::Char).collect()
}

#[test]
fn test_new_package_empty_creates_the_package_its_gitignore_and_the_profile_ref() {
    let (mut app, tmp) = wizard_app();
    // group home, name tmux, create row, empty source, ignore defaults, profile personal.
    press_all(&mut app, &[KeyCode::Enter]);
    press_all(&mut app, &typed("tmux"));
    press_all(&mut app, &[KeyCode::Enter, KeyCode::Enter, KeyCode::Enter]);
    press_all(&mut app, &[KeyCode::Char(' '), KeyCode::Enter]);
    // Review, then create, then leave.
    press_all(&mut app, &[KeyCode::Enter, KeyCode::Enter]);

    let package = tmp.path().join("dotfiles/home/tmux");
    assert!(package.is_dir(), "package folder is created");
    let gitignore = std::fs::read_to_string(package.join(".gitignore")).unwrap();
    assert!(
        gitignore.contains("*.env"),
        "secrets template written: {gitignore:?}"
    );
    assert!(!package.join(".stow-local-ignore").exists());
    let profiles = ProfileSet::load(&tmp.path().join("profiles")).unwrap();
    assert!(profiles
        .get("personal")
        .unwrap()
        .packages
        .contains(&"home/tmux".to_string()));
    assert_eq!(
        app.component_stack.len(),
        1,
        "Enter after the result leaves to the menu"
    );
}

#[test]
fn test_new_package_from_live_files_moves_the_file_and_links_it_back() {
    let (mut app, tmp) = wizard_app();
    let live = tmp.path().join("home/.bashrc");
    std::fs::write(&live, "alias l='ls -la'\n").unwrap();
    // group home, name shell, create row, live files, the picker opens at the target.
    press_all(&mut app, &[KeyCode::Enter]);
    press_all(&mut app, &typed("shell"));
    press_all(&mut app, &[KeyCode::Enter, KeyCode::Down, KeyCode::Enter]);
    // The picker reads its listing on each frame, so update once before the keys.
    app.update_components();
    // Mark the only entry, then ignore defaults, no profile, review, create.
    press_all(
        &mut app,
        &[
            KeyCode::Char(' '),
            KeyCode::Enter,
            KeyCode::Enter,
            KeyCode::Enter,
        ],
    );
    press_all(&mut app, &[KeyCode::Enter]);

    let package = tmp.path().join("dotfiles/home/shell");
    let moved = package.join(".bashrc");
    assert_eq!(
        std::fs::read_to_string(&moved).unwrap(),
        "alias l='ls -la'\n"
    );
    // The link is relative (stow-compatible), so compare where it resolves.
    assert_eq!(
        std::fs::canonicalize(&live).unwrap(),
        std::fs::canonicalize(&moved).unwrap()
    );
    assert!(package.join(".gitignore").is_file());
}

#[test]
fn test_set_up_review_stows_its_package_when_the_global_action_is_unstow() {
    let (mut app, tmp) = make_bootstrap_app();
    app.ctx.core.flags.action = StowAction::Unstow;
    // Make `~/.config` exist, so the package's `andre` folder is linked inside it.
    std::fs::create_dir_all(tmp.path().join("home/.config")).unwrap();
    let rt = tokio::runtime::Runtime::new().unwrap();
    let _guard = rt.enter();

    // The menu learns of the set-up item on update, as it does each frame. Then `0` opens
    // the set-up review and `y` runs it.
    app.update_components();
    app.component_dispatch(key(KeyCode::Char('0')));
    app.component_dispatch(key(KeyCode::Char('y')));
    pump_until_complete(&mut app, 10);
    rt.shutdown_timeout(Duration::from_secs(5));

    // The native engine links files, so the config file is the symlink, into the package.
    let linked = tmp.path().join("home/.config/andre/andre.yml");
    assert!(
        std::fs::symlink_metadata(&linked)
            .unwrap()
            .file_type()
            .is_symlink(),
        "the set-up review linked the config file"
    );
    assert_eq!(
        std::fs::canonicalize(&linked).unwrap(),
        std::fs::canonicalize(
            tmp.path()
                .join("dotfiles/config/andre/.config/andre/andre.yml")
        )
        .unwrap(),
    );
    assert_eq!(
        app.ctx.core.flags.action,
        StowAction::Unstow,
        "the global action is not changed"
    );
}

#[test]
fn test_set_up_review_of_a_flat_config_links_its_config_folder() {
    let (mut app, tmp) = make_flat_bootstrap_app();
    app.ctx.core.flags.action = StowAction::Unstow;
    // Make `~/.config` exist, so the package's `andre` folder is linked inside it.
    std::fs::create_dir_all(tmp.path().join("home/.config")).unwrap();
    let rt = tokio::runtime::Runtime::new().unwrap();
    let _guard = rt.enter();

    app.update_components();
    app.component_dispatch(key(KeyCode::Char('0')));
    app.component_dispatch(key(KeyCode::Char('y')));
    pump_until_complete(&mut app, 10);
    rt.shutdown_timeout(Duration::from_secs(5));

    let linked = tmp.path().join("home/.config/andre/andre.yml");
    assert!(
        std::fs::symlink_metadata(&linked)
            .unwrap()
            .file_type()
            .is_symlink(),
        "the set-up review linked the config file"
    );
    assert_eq!(
        std::fs::canonicalize(&linked).unwrap(),
        std::fs::canonicalize(tmp.path().join("dotfiles/andre/.config/andre/andre.yml")).unwrap(),
    );
}
