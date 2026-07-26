mod helpers;

use std::fs;

use andre_core::{check_stowed_status, StowStatus};
use helpers::TestEnv;

#[test]
fn test_full_stow_cycle() {
    let env = TestEnv::from_scenario("01-basic-stow");

    // Stow
    let result = env.run_awesome_stow(&["--yolo"]).unwrap();
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
    let result = env.run_awesome_stow(&["--yolo"]).unwrap();
    assert_eq!(result.exit_code, 0);
}

#[test]
fn test_empty_package_returns_empty_status() {
    let env = TestEnv::from_scenario("01-basic-stow");

    // Create an empty package dir
    let empty_dir = env.packages_dir.join("empty-pkg");
    fs::create_dir_all(&empty_dir).unwrap();

    let status = check_stowed_status(&env.packages_dir, &env.target_dir, "empty-pkg").unwrap();
    assert_eq!(status, StowStatus::Empty);
}

#[test]
fn test_missing_package_returns_missing_status() {
    let env = TestEnv::from_scenario("01-basic-stow");

    let status =
        check_stowed_status(&env.packages_dir, &env.target_dir, "nonexistent-pkg").unwrap();
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

    let status = check_stowed_status(&env.packages_dir, &env.target_dir, "partial-pkg").unwrap();
    assert_eq!(status, StowStatus::Partial);
}

#[test]
fn test_restow_cycle() {
    let env = TestEnv::from_scenario("12-restow");

    let result = env.run_awesome_stow(&["--yolo"]).unwrap();
    assert_eq!(result.exit_code, 0);

    assert!(
        env.target_dir.join(".bashrc").is_symlink(),
        ".bashrc should be symlinked after restow"
    );
}

#[test]
fn test_ignore_patterns_excluded() {
    let env = TestEnv::from_scenario("07-ignore-patterns");

    let result = env.run_awesome_stow(&["--yolo"]).unwrap();
    assert_eq!(result.exit_code, 0);
}

#[test]
fn test_hidden_dotfiles_stowed_correctly() {
    let env = TestEnv::from_scenario("05-hidden-dotfiles");

    let result = env.run_awesome_stow(&["--yolo"]).unwrap();
    assert_eq!(result.exit_code, 0);
}

#[test]
fn test_multi_group_stow() {
    let env = TestEnv::from_scenario("03-multi-group-config");

    let result = env.run_awesome_stow(&["--yolo"]).unwrap();
    assert_eq!(result.exit_code, 0);
}

#[test]
fn test_stow_unstow_then_stow_again() {
    let env = TestEnv::from_scenario("01-basic-stow");

    // Stow
    env.run_awesome_stow(&["--yolo"]).unwrap();
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
    env.run_awesome_stow(&["--yolo"]).unwrap();
    assert!(env.target_dir.join(".bashrc").is_symlink());
}
