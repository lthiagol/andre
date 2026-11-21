//! Config discovery, end to end. Every test injects its own folders (or sets
//! `HOME`/`XDG_CONFIG_HOME` on the spawned child only), so nothing reads the
//! developer's real home or the test runner's environment.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};

use tempfile::TempDir;

use andre::app::App;
use andre::discovery::ConfigSearch;

const MINIMAL_CONFIG: &str = "global: {}\n";

fn write(path: &Path, body: &str) {
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, body).unwrap();
}

/// A search with no flags, whose only XDG base is `xdg` (no legacy dirs).
fn xdg_only(xdg: &Path) -> ConfigSearch {
    ConfigSearch {
        explicit: None,
        config_folder: None,
        xdg_dirs: vec![xdg.join("andre")],
        legacy_dirs: Vec::new(),
    }
}

// ---- library level: injected folders -------------------------------------------

#[test]
fn xdg_config_is_found_when_no_flag_is_given() {
    let tmp = TempDir::new().unwrap();
    let xdg = tmp.path().join("xdg");
    write(&xdg.join("andre").join("andre.yml"), MINIMAL_CONFIG);

    assert_eq!(
        xdg_only(&xdg).discover(),
        Some(xdg.join("andre").join("andre.yml"))
    );
}

#[test]
fn config_folder_is_used_and_profiles_load_next_to_it() {
    let tmp = TempDir::new().unwrap();
    let repo = tmp.path().join("clone");
    write(&repo.join("andre.yml"), MINIMAL_CONFIG);
    write(&repo.join("profiles").join("work.yml"), "packages: [git]\n");
    let home = tmp.path().join("homedir");
    fs::create_dir_all(&home).unwrap();

    let search = ConfigSearch {
        explicit: None,
        config_folder: Some(repo.clone()),
        xdg_dirs: vec![tmp.path().join("xdg").join("andre")],
        legacy_dirs: Vec::new(),
    };
    let config_path = search.discover().expect("config folder holds andre.yml");
    assert_eq!(config_path, repo.join("andre.yml"));

    let app = App::with_home(config_path, home).unwrap();
    let work = app
        .ctx
        .core
        .profiles
        .get("work")
        .expect("profile loaded from <config folder>/profiles");
    assert_eq!(work.packages, ["git"]);
    assert_eq!(
        app.ctx.core.profiles.dir(),
        fs::canonicalize(&repo).unwrap().join("profiles")
    );
}

#[test]
fn config_folder_does_not_fall_back_to_xdg() {
    let tmp = TempDir::new().unwrap();
    let repo = tmp.path().join("empty-repo");
    fs::create_dir_all(&repo).unwrap();
    let xdg = tmp.path().join("xdg");
    write(&xdg.join("andre").join("andre.yml"), MINIMAL_CONFIG);

    let search = ConfigSearch {
        explicit: None,
        config_folder: Some(repo.clone()),
        xdg_dirs: vec![xdg.join("andre")],
        legacy_dirs: Vec::new(),
    };
    assert_eq!(search.discover(), None);
}

#[test]
fn explicit_config_wins_over_xdg_and_never_falls_back() {
    let tmp = TempDir::new().unwrap();
    let xdg = tmp.path().join("xdg");
    write(&xdg.join("andre").join("andre.yml"), MINIMAL_CONFIG);
    let mine = tmp.path().join("mine.yml");
    write(&mine, MINIMAL_CONFIG);

    let mut search = xdg_only(&xdg);
    search.explicit = Some(mine.clone());
    assert_eq!(search.discover(), Some(mine));

    search.explicit = Some(tmp.path().join("missing.yml"));
    assert_eq!(search.discover(), None);
}

#[test]
fn legacy_locations_are_named_but_never_loaded() {
    let tmp = TempDir::new().unwrap();
    let dotfiles = tmp.path().join("homedir").join("dotfiles");
    write(&dotfiles.join("andre.yml"), MINIMAL_CONFIG);

    let search = ConfigSearch {
        explicit: None,
        config_folder: None,
        xdg_dirs: vec![tmp.path().join("xdg").join("andre")],
        legacy_dirs: vec![tmp.path().join("cwd"), dotfiles.clone()],
    };
    assert_eq!(search.discover(), None);
    assert_eq!(search.legacy_config(), Some(dotfiles.join("andre.yml")));
}

// ---- process level: the real binary, with HOME and XDG_CONFIG_HOME on the child ----

/// Runs `andre` with `cwd`, `HOME` and `XDG_CONFIG_HOME` pointed at test folders.
fn run_andre(cwd: &Path, home: &Path, xdg: &Path, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_andre"))
        .args(args)
        .current_dir(cwd)
        .env("HOME", home)
        .env("XDG_CONFIG_HOME", xdg)
        .stdin(Stdio::null())
        .output()
        .expect("failed to spawn andre binary")
}

struct Sandbox {
    _tmp: TempDir,
    cwd: PathBuf,
    home: PathBuf,
    xdg: PathBuf,
}

impl Sandbox {
    fn new() -> Self {
        let tmp = TempDir::new().unwrap();
        let root = tmp.path().to_path_buf();
        let sandbox = Sandbox {
            _tmp: tmp,
            cwd: root.join("cwd"),
            home: root.join("homedir"),
            xdg: root.join("xdg"),
        };
        fs::create_dir_all(&sandbox.cwd).unwrap();
        fs::create_dir_all(&sandbox.home).unwrap();
        sandbox
    }

    fn run(&self, args: &[&str]) -> Output {
        run_andre(&self.cwd, &self.home, &self.xdg, args)
    }
}

fn stderr(out: &Output) -> String {
    String::from_utf8_lossy(&out.stderr).into_owned()
}

fn stdout(out: &Output) -> String {
    String::from_utf8_lossy(&out.stdout).into_owned()
}

#[test]
fn process_loads_xdg_config_without_flags() {
    let sb = Sandbox::new();
    // An empty packages folder: `--yolo` with no `packages:` exits 1 instead.
    let folder = sb.xdg.join("andre");
    write(&folder.join("andre.yml"), "packages: packages\n");
    fs::create_dir_all(folder.join("packages")).unwrap();

    let out = sb.run(&["--yolo"]);
    assert!(out.status.success(), "stderr: {}", stderr(&out));
    assert!(
        stdout(&out).contains("0 succeeded, 0 failed (total: 0)"),
        "stdout: {}",
        stdout(&out)
    );
}

#[test]
fn process_loads_config_folder() {
    let sb = Sandbox::new();
    let repo = sb.cwd.join("repo");
    write(&repo.join("andre.yml"), "packages: packages\n");
    fs::create_dir_all(repo.join("packages")).unwrap();

    let out = sb.run(&["--yolo", "--config-folder", repo.to_str().unwrap()]);
    assert!(out.status.success(), "stderr: {}", stderr(&out));
}

#[test]
fn process_config_and_config_folder_are_a_usage_error() {
    let sb = Sandbox::new();
    let out = sb.run(&["--config", "a.yml", "--config-folder", "repo"]);
    assert_eq!(out.status.code(), Some(2), "stderr: {}", stderr(&out));
    assert!(
        stderr(&out).contains("'--config' cannot be used with '--config-folder'"),
        "stderr: {}",
        stderr(&out)
    );
}

#[test]
fn process_cwd_config_is_not_loaded() {
    let sb = Sandbox::new();
    write(&sb.cwd.join("andre.yml"), MINIMAL_CONFIG);

    let out = sb.run(&["--yolo"]);
    assert_eq!(out.status.code(), Some(1));
    assert!(stderr(&out).contains("config file not found"));
}

#[test]
fn process_not_found_lists_xdg_and_suggests_config_folder() {
    let sb = Sandbox::new();
    let out = sb.run(&["--yolo"]);
    assert_eq!(out.status.code(), Some(1));

    let err = stderr(&out);
    let expected = sb.xdg.join("andre").join("andre.yml");
    assert!(
        err.contains(&expected.display().to_string()),
        "stderr should list {}: {err}",
        expected.display()
    );
    assert!(err.contains("--config-folder"), "stderr: {err}");
    assert!(!err.contains("pre-1.0"), "no legacy config, no hint: {err}");
}

#[test]
fn process_names_legacy_dotfiles_config_without_loading_it() {
    let sb = Sandbox::new();
    let dotfiles = sb.home.join("dotfiles");
    write(&dotfiles.join("andre.yml"), MINIMAL_CONFIG);

    let out = sb.run(&["--yolo"]);
    assert_eq!(out.status.code(), Some(1));
    assert!(stdout(&out).is_empty(), "legacy config must not run");

    let err = stderr(&out);
    assert!(err.contains("pre-1.0 location"), "stderr: {err}");
    let suggestion = format!("--config-folder {}", dotfiles.display());
    assert!(
        err.contains(&suggestion),
        "stderr should suggest `{suggestion}`: {err}"
    );
}

#[test]
fn process_config_folder_without_config_does_not_fall_back_to_xdg() {
    let sb = Sandbox::new();
    write(&sb.xdg.join("andre").join("andre.yml"), MINIMAL_CONFIG);
    let repo = sb.cwd.join("empty-repo");
    fs::create_dir_all(&repo).unwrap();

    let out = sb.run(&["--yolo", "--config-folder", repo.to_str().unwrap()]);
    assert_eq!(out.status.code(), Some(1), "stderr: {}", stderr(&out));
    assert!(stderr(&out).contains("config file not found"));
}

#[test]
fn process_missing_explicit_config_does_not_fall_back_to_xdg() {
    let sb = Sandbox::new();
    write(&sb.xdg.join("andre").join("andre.yml"), MINIMAL_CONFIG);

    let out = sb.run(&["--yolo", "--config", "nope.yml"]);
    assert_eq!(out.status.code(), Some(1));
    assert!(
        stderr(&out).contains("nope.yml"),
        "stderr: {}",
        stderr(&out)
    );
}

#[test]
fn process_onboard_accepts_config_folder() {
    let sb = Sandbox::new();
    let repo = sb.cwd.join("fresh");

    // --onboard writes the file before it enters the TUI. Wait for the file, then kill.
    let mut child = Command::new(env!("CARGO_BIN_EXE_andre"))
        .args(["--onboard", "--config-folder", repo.to_str().unwrap()])
        .current_dir(&sb.cwd)
        .env("HOME", &sb.home)
        .env("XDG_CONFIG_HOME", &sb.xdg)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .unwrap();

    let target = repo.join("andre.yml");
    let mut found = false;
    for _ in 0..50 {
        if target.is_file() {
            found = true;
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(100));
    }
    let _ = child.kill();
    let _ = child.wait();
    assert!(found, "onboard should create {}", target.display());
}

#[test]
fn demo_fixture_profiles_resolve_through_their_include() {
    use andre_core::profile::known_packages;

    // The `make run` fixture: demo/andre.yml with demo/profiles/ beside it.
    let demo = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .join("demo")
        .join("andre.yml");
    let tmp = TempDir::new().unwrap();
    let app = App::with_home(demo, tmp.path().to_path_buf()).unwrap();
    assert!(app.ctx.core.profiles.issues().is_empty());

    let known = known_packages(&app.ctx.core.config, &app.ctx.config_dir, &app.ctx.home_dir);
    let editor = app
        .ctx
        .core
        .profiles
        .resolve("editor", &known)
        .expect("editor resolves");
    let packages: Vec<&str> = editor.entries.iter().map(|e| e.package.as_str()).collect();
    assert_eq!(packages, ["nvim", "shell", "git"]);
    assert_eq!(editor.missing().count(), 0);
}

#[test]
fn demo_fixture_screens_render_without_panic_at_120x40_and_200x50() {
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
    use ratatui::{backend::TestBackend, Terminal};

    // The `make run` fixture: the main menu, then the profiles, packages and settings
    // screens, at the two sizes the screens are checked at.
    let demo = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .join("demo")
        .join("andre.yml");
    for (cols, rows) in [(120, 40), (200, 50)] {
        for digit in ['1', '2', '6'] {
            let tmp = TempDir::new().unwrap();
            let mut app = App::with_home(demo.clone(), tmp.path().to_path_buf()).unwrap();
            app.set_terminal_size(cols, rows);
            app.component_dispatch(KeyEvent::new(KeyCode::Char(digit), KeyModifiers::NONE));
            let mut terminal = Terminal::new(TestBackend::new(cols, rows)).unwrap();
            terminal.draw(|f| andre::ui::render(&mut app, f)).unwrap();
        }
    }
}

// ---- headless --profile (F5) ----

/// A config folder with two packages and one profile, `work`, that lists
/// only `git`.
fn profile_repo(sb: &Sandbox) -> PathBuf {
    let repo = sb.cwd.join("repo");
    write(
        &repo.join("andre.yml"),
        &format!(
            "global: {{}}\npackages: packages\ntarget: \"{}\"\n",
            sb.home.display()
        ),
    );
    write(&repo.join("packages/git/.gitconfig"), "[user]\n");
    write(&repo.join("packages/shell/.bashrc"), "alias l=ls\n");
    write(&repo.join("profiles/work.yml"), "packages: [git]\n");
    repo
}

#[test]
fn process_profile_with_yolo_stows_only_that_profile() {
    let sb = Sandbox::new();
    let repo = profile_repo(&sb);

    let out = sb.run(&[
        "--yolo",
        "--config-folder",
        repo.to_str().unwrap(),
        "--profile",
        "work",
    ]);
    assert!(out.status.success(), "stderr: {}", stderr(&out));
    assert!(
        sb.home.join(".gitconfig").is_symlink(),
        "the profile's package is linked"
    );
    assert!(
        !sb.home.join(".bashrc").exists(),
        "packages outside the profile are not"
    );
    assert!(
        stdout(&out).contains("1 succeeded, 0 failed (total: 1)"),
        "stdout: {}",
        stdout(&out)
    );
}

#[test]
fn process_profile_accepts_the_equals_form() {
    let sb = Sandbox::new();
    let repo = profile_repo(&sb);

    let out = sb.run(&[
        "--yolo",
        "--config-folder",
        repo.to_str().unwrap(),
        "--profile=work",
    ]);
    assert!(out.status.success(), "stderr: {}", stderr(&out));
    assert!(sb.home.join(".gitconfig").is_symlink());
}

#[test]
fn process_unknown_profile_exits_2_and_lists_the_profiles() {
    let sb = Sandbox::new();
    let repo = profile_repo(&sb);

    for extra in [&["--yolo"][..], &[][..]] {
        let mut args = extra.to_vec();
        args.extend([
            "--config-folder",
            repo.to_str().unwrap(),
            "--profile",
            "nope",
        ]);
        let out = sb.run(&args);
        assert_eq!(
            out.status.code(),
            Some(2),
            "args {args:?}: stderr: {}",
            stderr(&out)
        );
        assert!(
            stderr(&out).contains("no profile named 'nope'"),
            "stderr: {}",
            stderr(&out)
        );
        assert!(
            stderr(&out).contains("profiles: work"),
            "stderr: {}",
            stderr(&out)
        );
    }
    assert!(!sb.home.join(".gitconfig").exists(), "nothing is stowed");
}

#[test]
fn process_profile_without_a_value_is_a_usage_error() {
    let sb = Sandbox::new();
    let out = sb.run(&["--yolo", "--profile"]);
    assert_eq!(out.status.code(), Some(2), "stderr: {}", stderr(&out));
    assert!(
        stderr(&out).contains("a value is required for '--profile'"),
        "stderr: {}",
        stderr(&out)
    );
}

#[test]
fn process_unknown_profile_in_a_folder_without_profiles_says_so() {
    let sb = Sandbox::new();
    let repo = sb.cwd.join("bare");
    write(&repo.join("andre.yml"), MINIMAL_CONFIG);

    let out = sb.run(&[
        "--yolo",
        "--config-folder",
        repo.to_str().unwrap(),
        "--profile",
        "x",
    ]);
    assert_eq!(out.status.code(), Some(2), "stderr: {}", stderr(&out));
    assert!(
        stderr(&out).contains("no profiles exist yet"),
        "stderr: {}",
        stderr(&out)
    );
}
