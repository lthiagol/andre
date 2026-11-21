//! Which `andre.yml` andre loads.
//!
//! Order: `--config <FILE>`, then `--config-folder <DIR>`, then the XDG config
//! folder (`$XDG_CONFIG_HOME/andre/`, plus the macOS fallback in `paths.rs`).
//! A flag that is given is the only place searched: it never falls through to the
//! next step. The pre-1.0 cwd, `~/dotfiles/` and `~/.dotfiles/` lookups are gone;
//! `legacy_dirs` is probed only to name such a config in the not-found message.

use std::path::{Path, PathBuf};

/// File names tried in each folder. `.yaml` wins over `.yml` when both exist.
const CONFIG_NAMES: [&str; 2] = ["andre.yaml", "andre.yml"];

pub struct ConfigSearch {
    /// `--config <FILE>`: the one file to load.
    pub explicit: Option<PathBuf>,
    /// `--config-folder <DIR>`: a folder holding `andre.yaml` or `andre.yml`.
    pub config_folder: Option<PathBuf>,
    /// Folders searched when no flag is given, in order (`<base>/andre`).
    pub xdg_dirs: Vec<PathBuf>,
    /// Pre-1.0 folders. Probed only to name a config in the not-found message.
    pub legacy_dirs: Vec<PathBuf>,
}

impl Default for ConfigSearch {
    fn default() -> Self {
        Self::new()
    }
}

fn candidates(dir: &Path) -> impl Iterator<Item = PathBuf> + '_ {
    CONFIG_NAMES.iter().map(move |name| dir.join(name))
}

impl ConfigSearch {
    /// Search rooted in the real environment (`$HOME`, `$XDG_CONFIG_HOME`, cwd).
    pub fn new() -> Self {
        // Primary XDG/APPDATA path plus macOS Application Support back-compat
        // (former dirs::config_dir location). See paths::config_search_dirs.
        let xdg_dirs = crate::paths::config_search_dirs()
            .into_iter()
            .map(|d| d.join("andre"))
            .collect();

        let mut legacy_dirs = vec![std::env::current_dir().unwrap_or_default()];
        if let Some(home) = crate::paths::home_dir() {
            legacy_dirs.push(home.join("dotfiles"));
            legacy_dirs.push(home.join(".dotfiles"));
        }

        Self {
            explicit: None,
            config_folder: None,
            xdg_dirs,
            legacy_dirs,
        }
    }

    /// Every location the active step reads, in order. Used for the not-found message.
    pub fn search_locations(&self) -> Vec<PathBuf> {
        if let Some(file) = &self.explicit {
            return vec![file.clone()];
        }
        if let Some(dir) = &self.config_folder {
            return candidates(dir).collect();
        }
        self.xdg_dirs.iter().flat_map(|d| candidates(d)).collect()
    }

    /// The config file to load, or `None` when the active step finds nothing.
    pub fn discover(&self) -> Option<PathBuf> {
        self.search_locations().into_iter().find(|p| p.is_file())
    }

    /// A config left in a pre-1.0 location, if any. For the hint only; never loaded.
    pub fn legacy_config(&self) -> Option<PathBuf> {
        self.legacy_dirs
            .iter()
            .flat_map(|d| candidates(d))
            .find(|p| p.is_file())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    fn touch(dir: &Path, name: &str) -> PathBuf {
        fs::create_dir_all(dir).unwrap();
        let path = dir.join(name);
        fs::write(&path, "global: {}\ngroups: {}\n").unwrap();
        path
    }

    fn search(
        explicit: Option<PathBuf>,
        folder: Option<PathBuf>,
        xdg: Vec<PathBuf>,
    ) -> ConfigSearch {
        ConfigSearch {
            explicit,
            config_folder: folder,
            xdg_dirs: xdg,
            legacy_dirs: Vec::new(),
        }
    }

    #[test]
    fn explicit_file_is_used_when_present() {
        let tmp = TempDir::new().unwrap();
        let file = touch(tmp.path(), "mine.yml");
        let found = search(Some(file.clone()), None, Vec::new()).discover();
        assert_eq!(found, Some(file));
    }

    #[test]
    fn explicit_missing_does_not_fall_through_to_xdg() {
        let tmp = TempDir::new().unwrap();
        let xdg = tmp.path().join("andre");
        touch(&xdg, "andre.yml");
        let s = search(Some(tmp.path().join("nope.yml")), None, vec![xdg]);
        assert_eq!(s.discover(), None);
        assert_eq!(s.search_locations(), vec![tmp.path().join("nope.yml")]);
    }

    #[test]
    fn folder_prefers_yaml_over_yml() {
        let tmp = TempDir::new().unwrap();
        touch(tmp.path(), "andre.yml");
        let yaml = touch(tmp.path(), "andre.yaml");
        let s = search(None, Some(tmp.path().to_path_buf()), Vec::new());
        assert_eq!(s.discover(), Some(yaml));
    }

    #[test]
    fn folder_falls_back_to_yml() {
        let tmp = TempDir::new().unwrap();
        let yml = touch(tmp.path(), "andre.yml");
        let s = search(None, Some(tmp.path().to_path_buf()), Vec::new());
        assert_eq!(s.discover(), Some(yml));
    }

    #[test]
    fn folder_without_config_does_not_fall_through_to_xdg() {
        let tmp = TempDir::new().unwrap();
        let folder = tmp.path().join("repo");
        fs::create_dir_all(&folder).unwrap();
        let xdg = tmp.path().join("xdg").join("andre");
        touch(&xdg, "andre.yml");
        let s = search(None, Some(folder.clone()), vec![xdg]);
        assert_eq!(s.discover(), None);
        assert_eq!(
            s.search_locations(),
            vec![folder.join("andre.yaml"), folder.join("andre.yml")]
        );
    }

    #[test]
    fn xdg_folders_are_searched_in_order() {
        let tmp = TempDir::new().unwrap();
        let primary = tmp.path().join("primary").join("andre");
        let legacy = tmp.path().join("app-support").join("andre");
        touch(&legacy, "andre.yml");
        let s = search(None, None, vec![primary.clone(), legacy.clone()]);
        assert_eq!(s.discover(), Some(legacy.join("andre.yml")));

        touch(&primary, "andre.yml");
        assert_eq!(s.discover(), Some(primary.join("andre.yml")));
    }

    #[test]
    fn nothing_found_returns_none_and_lists_xdg_locations() {
        let tmp = TempDir::new().unwrap();
        let xdg = tmp.path().join("andre");
        let s = search(None, None, vec![xdg.clone()]);
        assert_eq!(s.discover(), None);
        assert_eq!(
            s.search_locations(),
            vec![xdg.join("andre.yaml"), xdg.join("andre.yml")]
        );
    }

    #[test]
    fn cwd_config_is_not_discovered() {
        let tmp = TempDir::new().unwrap();
        let cwd = tmp.path().join("cwd");
        touch(&cwd, "andre.yml");
        let s = ConfigSearch {
            explicit: None,
            config_folder: None,
            xdg_dirs: vec![tmp.path().join("xdg").join("andre")],
            legacy_dirs: vec![cwd.clone()],
        };
        assert_eq!(s.discover(), None);
    }

    #[test]
    fn legacy_config_is_reported_for_the_hint() {
        let tmp = TempDir::new().unwrap();
        let dotfiles = tmp.path().join("home").join("dotfiles");
        let file = touch(&dotfiles, "andre.yaml");
        let s = ConfigSearch {
            explicit: None,
            config_folder: None,
            xdg_dirs: vec![tmp.path().join("xdg").join("andre")],
            legacy_dirs: vec![tmp.path().join("cwd"), dotfiles],
        };
        assert_eq!(s.legacy_config(), Some(file));
        assert_eq!(s.discover(), None, "legacy configs are never loaded");
    }

    #[test]
    fn legacy_config_is_none_when_absent() {
        let tmp = TempDir::new().unwrap();
        let s = ConfigSearch {
            explicit: None,
            config_folder: None,
            xdg_dirs: Vec::new(),
            legacy_dirs: vec![tmp.path().join("nothing-here")],
        };
        assert_eq!(s.legacy_config(), None);
    }
}
