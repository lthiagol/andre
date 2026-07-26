use std::fs;

use tempfile::TempDir;

use andre::discovery::ConfigSearch;

fn write_config_file(dir: &TempDir, name: &str) {
    fs::write(
        dir.path().join(name),
        r#"
global: {}
groups:
  test:
    source: src
    target: tgt
"#,
    )
    .unwrap();
}

fn search_with_cwd(cwd: &TempDir) -> ConfigSearch {
    ConfigSearch {
        cwd: Some(cwd.path().to_path_buf()),
        home_dotfiles: None,
        home_dot_dotfiles: None,
        xdg_dirs: Vec::new(),
        explicit: None,
    }
}

#[test]
fn test_explicit_config_takes_priority() {
    let tmp_cwd = TempDir::new().unwrap();
    let tmp_xdg = TempDir::new().unwrap();
    write_config_file(&tmp_cwd, "andre.yml");
    write_config_file(&tmp_xdg, "andre.yml");

    let mut search = search_with_cwd(&tmp_cwd);
    search.xdg_dirs = vec![tmp_xdg.path().to_path_buf()];
    search.explicit = Some(tmp_xdg.path().join("andre.yml"));

    let result = search.discover().unwrap();
    assert_eq!(result, tmp_xdg.path().join("andre.yml"));
}

#[test]
fn test_cwd_finds_config() {
    let tmp = TempDir::new().unwrap();
    write_config_file(&tmp, "andre.yml");

    let search = search_with_cwd(&tmp);
    let result = search.discover().unwrap();
    assert_eq!(result, tmp.path().join("andre.yml"));
}

#[test]
fn test_yaml_takes_priority_over_yml_in_same_dir() {
    let tmp = TempDir::new().unwrap();
    write_config_file(&tmp, "andre.yaml");
    write_config_file(&tmp, "andre.yml");

    let search = search_with_cwd(&tmp);
    let result = search.discover().unwrap();
    assert_eq!(
        result,
        tmp.path().join("andre.yaml"),
        ".yaml should be found before .yml"
    );
}

#[test]
fn test_falls_back_to_yml_when_yaml_missing() {
    let tmp = TempDir::new().unwrap();
    write_config_file(&tmp, "andre.yml");

    let search = search_with_cwd(&tmp);
    let result = search.discover().unwrap();
    assert_eq!(result, tmp.path().join("andre.yml"));
}

#[test]
fn test_home_dotfiles_is_searched() {
    let tmp = TempDir::new().unwrap();
    write_config_file(&tmp, "andre.yaml");

    let search = ConfigSearch {
        cwd: Some(std::env::temp_dir()),
        home_dotfiles: Some(tmp.path().to_path_buf()),
        home_dot_dotfiles: None,
        xdg_dirs: Vec::new(),
        explicit: None,
    };

    let result = search.discover().unwrap();
    assert_eq!(result, tmp.path().join("andre.yaml"));
}

#[test]
fn test_home_dot_dotfiles_is_searched() {
    let tmp = TempDir::new().unwrap();
    write_config_file(&tmp, "andre.yaml");

    let search = ConfigSearch {
        cwd: Some(std::env::temp_dir()),
        home_dotfiles: None,
        home_dot_dotfiles: Some(tmp.path().to_path_buf()),
        xdg_dirs: Vec::new(),
        explicit: None,
    };

    let result = search.discover().unwrap();
    assert_eq!(result, tmp.path().join("andre.yaml"));
}

#[test]
fn test_xdg_fallback_when_earlier_locations_empty() {
    let tmp = TempDir::new().unwrap();
    write_config_file(&tmp, "andre.yml");

    let search = ConfigSearch {
        cwd: Some(std::env::temp_dir()),
        home_dotfiles: None,
        home_dot_dotfiles: None,
        xdg_dirs: vec![tmp.path().to_path_buf()],
        explicit: None,
    };

    let result = search.discover().unwrap();
    assert_eq!(result, tmp.path().join("andre.yml"));
}

#[test]
fn test_returns_none_when_no_config_found() {
    let search = ConfigSearch {
        cwd: Some(std::env::temp_dir()),
        home_dotfiles: None,
        home_dot_dotfiles: None,
        xdg_dirs: vec![std::env::temp_dir().join("nonexistent")],
        explicit: None,
    };

    assert!(search.discover().is_none());
}

#[test]
fn test_search_locations_returns_all_paths() {
    let search = ConfigSearch {
        cwd: Some(std::path::PathBuf::from("/tmp/cwd")),
        home_dotfiles: Some(std::path::PathBuf::from("/home/user/dotfiles")),
        home_dot_dotfiles: Some(std::path::PathBuf::from("/home/user/.dotfiles")),
        xdg_dirs: vec![std::path::PathBuf::from("/home/user/.config/andre")],
        explicit: None,
    };

    let locations = search.search_locations();
    assert_eq!(locations.len(), 8);
    assert!(locations.contains(&std::path::PathBuf::from("/tmp/cwd/andre.yaml")));
    assert!(locations.contains(&std::path::PathBuf::from("/tmp/cwd/andre.yml")));
    assert!(locations.contains(&std::path::PathBuf::from("/home/user/dotfiles/andre.yaml")));
    assert!(locations.contains(&std::path::PathBuf::from("/home/user/.dotfiles/andre.yaml")));
    assert!(locations.contains(&std::path::PathBuf::from(
        "/home/user/.config/andre/andre.yaml"
    )));
}

#[test]
fn test_discover_is_deterministic() {
    let tmp = TempDir::new().unwrap();
    write_config_file(&tmp, "andre.yml");

    let search = search_with_cwd(&tmp);
    let first = search.discover();
    let second = search.discover();
    assert_eq!(first, second);
}
