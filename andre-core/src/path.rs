use std::path::{Path, PathBuf};

pub fn resolve_path(path: &str, config_dir: &Path, home_dir: &Path) -> PathBuf {
    if path.is_empty() {
        return config_dir.to_path_buf();
    }

    if path.starts_with('/') {
        return PathBuf::from(path);
    }

    if path.starts_with('~') {
        return resolve_tilde(path, home_dir);
    }

    if path.starts_with('$') {
        return resolve_env_var(path);
    }

    config_dir.join(path)
}

fn resolve_tilde(path: &str, home_dir: &Path) -> PathBuf {
    if path == "~" {
        return home_dir.to_path_buf();
    }

    if path.starts_with("~/") {
        return home_dir.join(path.trim_start_matches("~/"));
    }

    home_dir.join(path.trim_start_matches("~"))
}

fn resolve_env_var(path: &str) -> PathBuf {
    let mut result = path.to_string();
    let mut vars: Vec<_> = std::env::vars().collect();
    vars.sort_by_key(|b| std::cmp::Reverse(b.0.len()));
    for (key, val) in &vars {
        let placeholder = format!("${}", key);
        result = result.replace(&placeholder, val);
    }
    PathBuf::from(result)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_resolve_absolute() {
        let config_dir = PathBuf::from("/some/config/dir");
        let home_dir = PathBuf::from("/home/user");

        let result = resolve_path("/absolute/path", &config_dir, &home_dir);
        assert_eq!(result, PathBuf::from("/absolute/path"));
    }

    #[test]
    fn test_resolve_tilde_home() {
        let config_dir = PathBuf::from("/some/config/dir");
        let home_dir = PathBuf::from("/home/user");

        let result = resolve_path("~/.config", &config_dir, &home_dir);
        assert_eq!(result, PathBuf::from("/home/user/.config"));
    }

    #[test]
    fn test_resolve_tilde_only() {
        let config_dir = PathBuf::from("/some/config/dir");
        let home_dir = PathBuf::from("/home/user");

        let result = resolve_path("~", &config_dir, &home_dir);
        assert_eq!(result, home_dir);
    }

    #[test]
    fn test_resolve_env_var() {
        std::env::set_var("TEST_HOME", "/test/home");
        let config_dir = PathBuf::from("/some/config/dir");
        let home_dir = PathBuf::from("/home/user");

        let result = resolve_path("$TEST_HOME/subdir", &config_dir, &home_dir);
        assert_eq!(result, PathBuf::from("/test/home/subdir"));
    }

    #[test]
    fn test_resolve_relative() {
        let config_dir = PathBuf::from("/some/config/dir");
        let home_dir = PathBuf::from("/home/user");

        let result = resolve_path("relative/path", &config_dir, &home_dir);
        assert_eq!(result, PathBuf::from("/some/config/dir/relative/path"));
    }

    #[test]
    fn test_resolve_empty() {
        let config_dir = PathBuf::from("/some/config/dir");
        let home_dir = PathBuf::from("/home/user");

        let result = resolve_path("", &config_dir, &home_dir);
        assert_eq!(result, config_dir);
    }

    #[test]
    fn test_resolve_undefined_env_var() {
        std::env::remove_var("UNDEFINED_VAR_EXPLICITLY_FOR_TEST");
        let config_dir = PathBuf::from("/some/config/dir");
        let home_dir = PathBuf::from("/home/user");

        let result = resolve_path(
            "$UNDEFINED_VAR_EXPLICITLY_FOR_TEST/subdir",
            &config_dir,
            &home_dir,
        );
        assert_eq!(
            result,
            PathBuf::from("$UNDEFINED_VAR_EXPLICITLY_FOR_TEST/subdir")
        );
    }

    #[test]
    fn test_resolve_relative_with_dotdot() {
        let config_dir = PathBuf::from("/some/config/dir");
        let home_dir = PathBuf::from("/home/user");

        let result = resolve_path("../other/path", &config_dir, &home_dir);
        assert_eq!(result, PathBuf::from("/some/config/dir/../other/path"));
    }

    #[test]
    fn test_resolve_trailing_slash() {
        let config_dir = PathBuf::from("/some/config/dir");
        let home_dir = PathBuf::from("/home/user");

        let result = resolve_path("relative/path/", &config_dir, &home_dir);
        assert_eq!(result, PathBuf::from("/some/config/dir/relative/path/"));
    }

    #[test]
    fn test_relative_from_config_dir_with_absolute_base() {
        let config_dir = PathBuf::from("/home/user/dotfiles");
        let home_dir = PathBuf::from("/home/user");

        let result = resolve_path("packages/home", &config_dir, &home_dir);
        assert_eq!(result, PathBuf::from("/home/user/dotfiles/packages/home"));
    }

    #[test]
    fn test_config_dir_at_filesystem_root() {
        let config_dir = PathBuf::from("/");
        let home_dir = PathBuf::from("/home/user");

        let result = resolve_path("packages", &config_dir, &home_dir);
        assert_eq!(result, PathBuf::from("/packages"));
    }

    #[test]
    fn test_relative_path_is_independent_of_home_dir() {
        let config_dir = PathBuf::from("/opt/config");
        let home_dir = PathBuf::from("/home/user");

        let result = resolve_path("packages", &config_dir, &home_dir);
        assert_eq!(result, PathBuf::from("/opt/config/packages"));
    }
}
