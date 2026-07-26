//! Local replacement for the thin `dirs` helpers andre used (M06 DIY-2).
//!
//! Product semantics are **XDG-first on Unix** (matches README/REFERENCE discovery
//! docs: `$XDG_CONFIG_HOME` → `$HOME/.config`). That is intentionally not full
//! `dirs::config_dir()` parity on macOS (`dirs` used `~/Library/Application Support`).
//! `config_search_dirs` still probes the old macOS path so existing installs keep
//! working. Windows uses `%APPDATA%`. No filesystem access, no external crate.

use std::path::PathBuf;

/// The user's home directory: `$HOME` (Unix) or `$USERPROFILE` (Windows).
/// `None` when neither is set.
pub fn home_dir() -> Option<PathBuf> {
    std::env::var_os("HOME")
        .or_else(|| std::env::var_os("USERPROFILE"))
        .filter(|s| !s.is_empty())
        .map(PathBuf::from)
}

/// Primary user config directory (XDG-first on Unix).
///
/// Unix: `$XDG_CONFIG_HOME`, falling back to `$HOME/.config`.
/// Windows: `%APPDATA%`.
/// `None` when no base can be resolved.
pub fn config_dir() -> Option<PathBuf> {
    if cfg!(target_os = "windows") {
        std::env::var_os("APPDATA")
            .filter(|s| !s.is_empty())
            .map(PathBuf::from)
    } else {
        std::env::var_os("XDG_CONFIG_HOME")
            .filter(|s| !s.is_empty())
            .map(PathBuf::from)
            .or_else(|| home_dir().map(|h| h.join(".config")))
    }
}

/// Bases used for auto-discovery of `andre.yml` under each `…/andre/` subdir.
///
/// Always includes [`config_dir`] when set. On macOS, also includes
/// `~/Library/Application Support` so configs placed via the former `dirs`
/// crate path still resolve.
pub fn config_search_dirs() -> Vec<PathBuf> {
    let mut dirs = Vec::new();
    if let Some(primary) = config_dir() {
        dirs.push(primary);
    }
    if cfg!(target_os = "macos") {
        if let Some(legacy) = home_dir().map(|h| h.join("Library/Application Support")) {
            if !dirs.iter().any(|d| d == &legacy) {
                dirs.push(legacy);
            }
        }
    }
    dirs
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn home_dir_reads_home_env() {
        // HOME is set in essentially every test runner; assert it resolves.
        if std::env::var_os("HOME").is_some() {
            assert!(
                home_dir().is_some(),
                "home_dir must resolve when HOME is set"
            );
        }
    }

    #[test]
    fn config_dir_falls_back_to_home_config() {
        // With XDG_CONFIG_HOME unset, config_dir should still resolve via $HOME/.config.
        if std::env::var_os("HOME").is_some() && std::env::var_os("XDG_CONFIG_HOME").is_none() {
            let cfg = config_dir().expect("config_dir must fall back to $HOME/.config");
            assert!(cfg.ends_with(".config"));
        }
    }

    #[test]
    fn config_search_dirs_includes_primary() {
        if let Some(primary) = config_dir() {
            let dirs = config_search_dirs();
            assert!(
                dirs.iter().any(|d| d == &primary),
                "config_search_dirs must include config_dir primary"
            );
        }
    }

    #[test]
    #[cfg(target_os = "macos")]
    fn config_search_dirs_includes_macos_legacy_app_support() {
        if let Some(home) = home_dir() {
            let legacy = home.join("Library/Application Support");
            let dirs = config_search_dirs();
            assert!(
                dirs.iter().any(|d| d == &legacy),
                "macOS must probe Application Support for dirs back-compat"
            );
        }
    }
}
