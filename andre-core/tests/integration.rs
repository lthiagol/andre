mod helpers;

use std::fs;

use andre_core::{check_stowed_status, StowStatus};
use helpers::TestEnv;

#[test]
fn test_full_stow_cycle() {
    let env = TestEnv::from_scenario("01-basic-stow");

    // Stow
    let result = env.run_andre(&["--yolo"]).unwrap();
    assert_eq!(result.exit_code, 0);

    // Should create symlink
    assert!(
        env.target_dir.join(".bashrc").is_symlink(),
        ".bashrc should be a symlink after stow"
    );

    env.assert_symlinks(&[(".bashrc", "../packages/bash-env/.bashrc")]);

    // Unstow via andre (reads action from config set to unstow)
    // For simplicity, run stow -D directly
    let stow_bin = TestEnv::stow_binary();
    let mut cmd = std::process::Command::new(&stow_bin);
    cmd.args(["-d", "packages", "-D", "-t", "target", "bash-env"])
        .current_dir(env.temp_dir.path())
        .env("HOME", env.temp_dir.path());
    let output = cmd.output().unwrap();
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    assert!(!env.target_dir.join(".bashrc").exists());
}

#[test]
fn test_conflict_detection() {
    let env = TestEnv::from_scenario("08-defer-existing-target");

    // There's a real .bashrc file at target — stow should handle this
    let result = env.run_andre(&["--yolo"]).unwrap();
    assert_eq!(result.exit_code, 0);
}

#[test]
fn test_empty_package_returns_empty_status() {
    let env = TestEnv::from_scenario("01-basic-stow");

    // Create an empty package dir
    let empty_dir = env.packages_dir.join("empty-pkg");
    fs::create_dir_all(&empty_dir).unwrap();

    let status =
        check_stowed_status(&env.packages_dir, &env.target_dir, "empty-pkg", None).unwrap();
    assert_eq!(status, StowStatus::Empty);
}

#[test]
fn test_missing_package_returns_missing_status() {
    let env = TestEnv::from_scenario("01-basic-stow");

    let status =
        check_stowed_status(&env.packages_dir, &env.target_dir, "nonexistent-pkg", None).unwrap();
    assert_eq!(status, StowStatus::Missing);
}

#[test]
fn test_partial_stow_status() {
    let env = TestEnv::from_scenario("01-basic-stow");

    // Create a package with two files, symlink only one
    let pkg_dir = env.packages_dir.join("partial-pkg");
    fs::create_dir_all(&pkg_dir).unwrap();
    fs::write(pkg_dir.join(".file1"), "content1").unwrap();
    fs::write(pkg_dir.join(".file2"), "content2").unwrap();

    #[cfg(unix)]
    std::os::unix::fs::symlink(pkg_dir.join(".file1"), env.target_dir.join(".file1")).unwrap();

    let status =
        check_stowed_status(&env.packages_dir, &env.target_dir, "partial-pkg", None).unwrap();
    assert_eq!(status, StowStatus::Partial);
}

#[test]
fn test_restow_cycle() {
    let env = TestEnv::from_scenario("12-restow");

    let result = env.run_andre(&["--yolo"]).unwrap();
    assert_eq!(result.exit_code, 0);

    assert!(
        env.target_dir.join(".bashrc").is_symlink(),
        ".bashrc should be symlinked after restow"
    );
}

#[test]
fn test_ignore_patterns_excluded() {
    let env = TestEnv::from_scenario("07-ignore-patterns");

    let result = env.run_andre(&["--yolo"]).unwrap();
    assert_eq!(result.exit_code, 0);
}

#[test]
fn test_hidden_dotfiles_stowed_correctly() {
    let env = TestEnv::from_scenario("05-hidden-dotfiles");

    let result = env.run_andre(&["--yolo"]).unwrap();
    assert_eq!(result.exit_code, 0);
}

#[test]
fn test_two_profiles_stow_every_package() {
    let env = TestEnv::from_scenario("03-two-profiles");

    let result = env.run_andre(&["--yolo"]).unwrap();
    assert_eq!(result.exit_code, 0);
    env.assert_symlinks(&[
        (".bashrc", "../packages/bash-env/.bashrc"),
        (".vimrc", "../packages/vim-env/.vimrc"),
        (".gitconfig", "../packages/git-env/.gitconfig"),
    ]);
}

#[test]
fn test_stow_unstow_then_stow_again() {
    let env = TestEnv::from_scenario("01-basic-stow");

    // Stow
    env.run_andre(&["--yolo"]).unwrap();
    assert!(env.target_dir.join(".bashrc").is_symlink());

    // Unstow via stow -D
    let stow_bin = TestEnv::stow_binary();
    let mut cmd = std::process::Command::new(&stow_bin);
    cmd.args(["-d", "packages", "-D", "-t", "target", "bash-env"])
        .current_dir(env.temp_dir.path())
        .env("HOME", env.temp_dir.path());
    cmd.output().unwrap();

    assert!(!env.target_dir.join(".bashrc").exists());

    // Restow
    env.run_andre(&["--yolo"]).unwrap();
    assert!(env.target_dir.join(".bashrc").is_symlink());
}

#[cfg(test)]
#[allow(clippy::expect_fun_call)]
mod helper_tests {
    use super::*;

    #[test]
    fn test_temp_dir_created() {
        let env = TestEnv::from_scenario("01-basic-stow");
        assert!(env.temp_dir.path().exists());
        assert!(env.config_path.exists());
    }

    #[test]
    fn test_target_snapshot_empty() {
        let env = TestEnv::from_scenario("01-basic-stow");
        let snapshot = env.target_snapshot();
        assert!(snapshot.is_empty());
    }

    #[test]
    fn test_stow_binary_exists() {
        let stow_path = TestEnv::stow_binary();
        assert!(stow_path.exists(), "stow binary should be in PATH");
    }

    #[test]
    fn test_from_scenario_panics_on_missing_fixture() {
        // Regression for the inverted-condition bug fixed in 1ea831a:
        // `from_scenario` used to gate on the destination tempdir and
        // panic with "Fixture scenario ... not found" — the message and
        // the condition it claimed to guard did not match. Guard against
        // an actually-missing fixture directory producing a silent copy
        // of nothing.
        let bogus = "this-fixture-does-not-exist-zzz-9999";
        let result = std::panic::catch_unwind(|| {
            let _ = TestEnv::from_scenario(bogus);
        });
        let payload = result.expect_err("from_scenario should panic when fixture is missing");
        let msg = payload
            .downcast_ref::<String>()
            .map(String::as_str)
            .or_else(|| payload.downcast_ref::<&'static str>().copied())
            .unwrap_or("");
        assert!(
            msg.contains(bogus) && msg.contains("not found"),
            "panic message should mention the scenario name and 'not found'; got: {msg:?}"
        );
    }
}

// ---------------------------------------------------------------------------
// Ignore lists: native engine vs GNU stow on the same fixture.
// ---------------------------------------------------------------------------

use andre_core::engine::{LinkOp, NativeEngine, StowEngine};
use andre_core::ignore::global_ignore_in;
use andre_core::stow::collect_package_files;
use andre_core::{
    check_stowed_status_with_ignores, IgnoreRules, LinkEngine, RuleSource, StowAction, StowFlags,
};
use std::path::Path;
use tempfile::TempDir;

/// Relative paths (slash-separated) of every symlink under `root`. Directories are not
/// followed, so only links that stow created appear.
fn symlinks_under(root: &Path) -> Vec<String> {
    fn walk(root: &Path, dir: &Path, out: &mut Vec<String>) {
        let Ok(entries) = fs::read_dir(dir) else {
            return;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            let meta = fs::symlink_metadata(&path).unwrap();
            if meta.file_type().is_symlink() {
                let rel = path.strip_prefix(root).unwrap();
                out.push(rel.to_string_lossy().replace('\\', "/"));
            } else if meta.is_dir() {
                walk(root, &path, out);
            }
        }
    }
    let mut out = Vec::new();
    walk(root, root, &mut out);
    out.sort();
    out
}

fn write_package(packages: &Path, files: &[(&str, &str)]) {
    let pkg = packages.join("pkg");
    for (rel, content) in files {
        let path = pkg.join(rel);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(&path, content).unwrap();
    }
}

/// Links `pkg` natively. `global` is the global list the engine is given; the parity
/// tests pass a path inside their temp dir, never the host's.
fn native_link(packages: &Path, target: &Path, global: &Path) {
    NativeEngine
        .apply(&LinkOp {
            package: "pkg".into(),
            source: packages.to_path_buf(),
            target: target.to_path_buf(),
            flags: StowFlags {
                action: StowAction::Stow,
                ..StowFlags::default()
            },
            ignores: vec![],
            global_ignore: Some(global.to_path_buf()),
        })
        .unwrap();
}

/// Runs GNU stow (`--no-folding`, so both engines link per file) with `HOME` set to
/// `home` on the child only, so a global list is read from there and never from the
/// host's real home.
fn gnu_stow_link(packages: &Path, target: &Path, home: &Path) {
    let output = std::process::Command::new(TestEnv::stow_binary())
        .arg("-d")
        .arg(packages)
        .arg("-t")
        .arg(target)
        .arg("--no-folding")
        .arg("pkg")
        .env("HOME", home)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "stow failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

/// Writes `files` into `packages/pkg`, links them natively and with GNU stow into two
/// targets, and returns both link sets. The temp dir is returned to keep it alive.
fn link_both_ways(files: &[(&str, &str)]) -> (Vec<String>, Vec<String>, TempDir) {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path();
    let packages = root.join("packages");
    write_package(&packages, files);
    let native_target = root.join("native-target");
    let stow_target = root.join("stow-target");
    fs::create_dir_all(&native_target).unwrap();
    fs::create_dir_all(&stow_target).unwrap();

    native_link(&packages, &native_target, &root.join(".stow-global-ignore"));
    gnu_stow_link(&packages, &stow_target, root);

    let native = symlinks_under(&native_target);
    let stow = symlinks_under(&stow_target);
    (native, stow, tmp)
}

#[test]
fn ignore_local_list_native_matches_gnu_stow() {
    let (native, stow, _tmp) = link_both_ways(&[
        (".bashrc", "rc"),
        ("notes.txt", "n"),
        ("draft.swp", "s"),
        ("README.md", "r"),
        ("tmp~", "t"),
        ("cache/data.bin", "d"),
        ("docs/cache/x", "x"),
        ("docs/guide.md", "g"),
        ("foo/bar/bazqux", "b"),
        ("foo/bar/keep", "k"),
        ("sub/.stow-local-ignore", "nested list is linked"),
        (
            ".stow-local-ignore",
            "# comment\n.*\\.swp     # swap files\ncache\n^/README\\.md\nbazqux\n\\#.*\\#\nbar/keep\n",
        ),
    ]);
    assert_eq!(native, stow, "native and GNU stow link different files");
    // Excluded: .stow-local-ignore (always), draft.swp (.*\.swp), README.md (^/README\.md),
    // cache dirs (basename), bazqux (basename), foo/bar/keep (bar/keep sub-path).
    // tmp~ is linked: the local list replaces the built-in list, which would drop it.
    assert_eq!(
        native,
        vec![
            ".bashrc",
            "docs/guide.md",
            "notes.txt",
            "sub/.stow-local-ignore",
            "tmp~"
        ]
    );
}

#[test]
fn ignore_builtin_defaults_native_matches_gnu_stow() {
    // No .stow-local-ignore: GNU's built-in list applies in both engines.
    let (native, stow, _tmp) = link_both_ways(&[
        (".bashrc", "rc"),
        ("README.md", "r"),
        ("LICENSE", "l"),
        ("docs/README.md", "d"),
        ("notes~", "n"),
        ("#draft#", "d"),
        ("x,v", "v"),
        (".gitignore", "g"),
        (".git/config", "c"),
        ("src/CVS/Root", "c"),
    ]);
    assert_eq!(native, stow, "native and GNU stow link different files");
    assert_eq!(
        native,
        vec![".bashrc".to_string(), "docs/README.md".to_string()]
    );
}

#[test]
fn ignore_global_list_matches_gnu_stow() {
    // No package list, so `$HOME/.stow-global-ignore` decides. Stow reads it from HOME;
    // native gets the same rules from the explicit path, since the test does not
    // change the process environment.
    let tmp = TempDir::new().unwrap();
    let root = tmp.path();
    fs::write(root.join(".stow-global-ignore"), "secret\n").unwrap();
    let packages = root.join("packages");
    write_package(
        &packages,
        &[("secret", "s"), ("open", "o"), ("sub/secret", "s")],
    );
    let pkg = packages.join("pkg");
    let target = root.join("stow-target");
    fs::create_dir_all(&target).unwrap();
    gnu_stow_link(&packages, &target, root);

    let rules = IgnoreRules::load(&pkg, Some(&root.join(".stow-global-ignore"))).unwrap();
    assert_eq!(rules.source(), RuleSource::Global);
    let native: Vec<String> = collect_package_files(&pkg, &rules)
        .iter()
        .map(|f| f.strip_prefix(&pkg).unwrap().to_string_lossy().into_owned())
        .collect();
    assert_eq!(native, vec!["open".to_string()]);
    assert_eq!(symlinks_under(&target), native);
}

#[test]
fn stow_engine_reads_the_global_list_of_the_home_it_is_given() {
    // The GNU child gets HOME from the op's global list, so stow reads this temp home's
    // list and not the host's. This process's environment is not changed.
    let tmp = TempDir::new().unwrap();
    let root = tmp.path();
    fs::write(root.join(".stow-global-ignore"), "secret\n").unwrap();
    let packages = root.join("packages");
    write_package(&packages, &[("secret", "s"), ("open", "o")]);
    let target = root.join("target");
    fs::create_dir_all(&target).unwrap();
    StowEngine
        .apply(&LinkOp {
            package: "pkg".into(),
            source: packages,
            target: target.clone(),
            flags: StowFlags {
                action: StowAction::Stow,
                ..StowFlags::default()
            },
            ignores: vec![],
            global_ignore: Some(global_ignore_in(root)),
        })
        .unwrap();
    assert_eq!(symlinks_under(&target), vec!["open".to_string()]);
}

#[test]
fn ignore_status_matches_what_native_links() {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path();
    let packages = root.join("packages");
    write_package(
        &packages,
        &[
            (".bashrc", "rc"),
            ("draft.swp", "s"),
            ("README.md", "r"),
            (".stow-local-ignore", ".*\\.swp\n"),
        ],
    );
    let target = root.join("target");
    fs::create_dir_all(&target).unwrap();
    native_link(&packages, &target, &root.join(".stow-global-ignore"));

    // README.md is linked (the local list replaces the defaults), draft.swp is not.
    assert_eq!(
        symlinks_under(&target),
        vec![".bashrc".to_string(), "README.md".to_string()]
    );
    let status = check_stowed_status_with_ignores(&packages, &target, "pkg", &[], None).unwrap();
    assert_eq!(status, StowStatus::Stowed);

    // Group ignores count too. Drop README.md's link: the package is Partial without the
    // group ignore, and Stowed with it (README.md is then not counted).
    fs::remove_file(target.join("README.md")).unwrap();
    let plain = check_stowed_status_with_ignores(&packages, &target, "pkg", &[], None).unwrap();
    assert_eq!(plain, StowStatus::Partial);
    let grouped =
        check_stowed_status_with_ignores(&packages, &target, "pkg", &["README".to_string()], None)
            .unwrap();
    assert_eq!(grouped, StowStatus::Stowed);
}
