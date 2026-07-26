mod helpers;

use helpers::TestEnv;

#[test]
fn test_scenario_01_basic_stow() {
    let env = TestEnv::from_scenario("01-basic-stow");
    let result = env.run_awesome_stow(&["--yolo"]).unwrap();
    assert_eq!(result.exit_code, 0, "andre failed: {}", result.stderr);

    env.assert_symlinks(&[(".bashrc", "../packages/bash-env/.bashrc")]);
}

#[test]
fn test_scenario_02_multi_package_one_group() {
    let env = TestEnv::from_scenario("02-multi-package-one-group");
    let result = env.run_awesome_stow(&["--yolo"]).unwrap();
    assert_eq!(result.exit_code, 0);

    env.assert_symlinks(&[
        (".bashrc", "../packages/bash-env/.bashrc"),
        (".gitconfig", "../packages/git-env/.gitconfig"),
        (".vimrc", "../packages/vim-env/.vimrc"),
    ]);
}

#[test]
fn test_scenario_03_multi_group_config() {
    let env = TestEnv::from_scenario("03-multi-group-config");
    let result = env.run_awesome_stow(&["--yolo"]).unwrap();
    assert_eq!(result.exit_code, 0);

    env.assert_symlinks(&[
        ("home/.bashrc", "../../packages/shared/.bashrc"),
        ("config/.vimrc", "../../packages/shared/.vimrc"),
        ("emacs/.emacs.el", "../../packages/shared/.emacs.el"),
    ]);
}

#[test]
fn test_scenario_04_nested_directories() {
    let env = TestEnv::from_scenario("04-nested-directories");
    let result = env.run_awesome_stow(&["--yolo"]).unwrap();
    assert_eq!(result.exit_code, 0);

    env.assert_symlinks(&[("nested", "../packages/config/nested")]);
}

#[test]
fn test_scenario_05_hidden_dotfiles() {
    let env = TestEnv::from_scenario("05-hidden-dotfiles");
    let result = env.run_awesome_stow(&["--yolo"]).unwrap();
    assert_eq!(result.exit_code, 0);

    env.assert_symlinks(&[
        (".bashrc", "../packages/dotfiles/.bashrc"),
        (".inputrc", "../packages/dotfiles/.inputrc"),
        (".profile", "../packages/dotfiles/.profile"),
    ]);
}

#[test]
fn test_scenario_06_executables() {
    let env = TestEnv::from_scenario("06-executables");
    let result = env.run_awesome_stow(&["--yolo"]).unwrap();
    assert_eq!(result.exit_code, 0);

    env.assert_symlinks(&[
        ("bin/install.sh", "../../packages/scripts/install.sh"),
        ("bin/setup.sh", "../../packages/scripts/setup.sh"),
    ]);
}

#[test]
fn test_scenario_07_ignore_patterns() {
    let env = TestEnv::from_scenario("07-ignore-patterns");
    let result = env.run_awesome_stow(&["--yolo"]).unwrap();
    assert_eq!(result.exit_code, 0);

    env.assert_symlinks(&[
        (".bashrc", "../packages/dotfiles/.bashrc"),
        (".gitconfig", "../packages/dotfiles/.gitconfig"),
    ]);

    // debug.log should be ignored
    assert!(!env.target_dir.join("debug.log").exists());
}

#[test]
fn test_scenario_08_defer_existing_target() {
    let env = TestEnv::from_scenario("08-defer-existing-target");
    let result = env.run_awesome_stow(&["--yolo"]).unwrap();
    // Conflict exists, should report failure
    assert!(result.stderr.contains("FAILED") || result.stdout.contains("FAILED"));
}

#[test]
fn test_scenario_09_override_existing_target() {
    let env = TestEnv::from_scenario("09-override-existing-target");
    let result = env.run_awesome_stow(&["--yolo"]).unwrap();
    assert!(result.stderr.contains("FAILED") || result.stdout.contains("FAILED"));
}

#[test]
fn test_scenario_10_adopt_existing_files() {
    let env = TestEnv::from_scenario("10-adopt-existing-files");
    // Pre-existing .vimrc with different content
    // stow.yml has adopt: true
    let result = env.run_awesome_stow(&["--yolo"]).unwrap();
    assert_eq!(result.exit_code, 0);

    env.assert_symlinks(&[(".vimrc", "../packages/vim-env/.vimrc")]);
}

#[test]
fn test_scenario_11_no_folding() {
    let env = TestEnv::from_scenario("11-no-folding");
    let result = env.run_awesome_stow(&["--yolo"]).unwrap();
    assert_eq!(result.exit_code, 0);

    env.assert_symlinks(&[(
        "app/settings.toml",
        "../../packages/config/app/settings.toml",
    )]);
}

#[test]
fn test_scenario_12_restow() {
    let env = TestEnv::from_scenario("12-restow");
    // action: restow in stow.yml
    let result = env.run_awesome_stow(&["--yolo"]).unwrap();
    assert_eq!(result.exit_code, 0);

    env.assert_symlinks(&[(".bashrc", "../packages/bash-env/.bashrc")]);
}

#[test]
fn test_scenario_13_dotfiles() {
    let env = TestEnv::from_scenario("13-dotfiles");
    let result = env.run_awesome_stow(&["--yolo"]).unwrap();
    assert_eq!(result.exit_code, 0);

    env.assert_symlinks(&[(".bashrc", "../packages/dot-bashrc/.bashrc")]);

    assert!(
        !env.target_dir.join("dot-bashrc").exists(),
        "dot-bashrc should not exist in target when --dotfiles is enabled"
    );
}

#[test]
fn test_scenario_14_config_relative_paths() {
    let env = TestEnv::from_scenario("14-config-relative-paths");

    // Paths in andre.yml are relative to the config file's directory, not CWD.
    // Run from /tmp so a CWD-relative resolution would fail.
    let binary = TestEnv::awesome_stow_binary();
    let mut cmd = std::process::Command::new(&binary);
    cmd.args(["--yolo", "--config"])
        .arg(env.temp_dir.path().join("andre.yml"))
        .current_dir("/tmp")
        .env("HOME", env.temp_dir.path());
    let output = cmd.output().unwrap();
    let stderr = String::from_utf8_lossy(&output.stderr);

    assert!(output.status.success(), "stderr:\n{}", stderr);

    env.assert_symlinks(&[(".env", "../packages/test-pkg/.env")]);
}
