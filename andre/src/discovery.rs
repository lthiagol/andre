use std::path::{Path, PathBuf};

pub struct ConfigSearch {
    pub explicit: Option<PathBuf>,
    pub cwd: Option<PathBuf>,
    pub home_dotfiles: Option<PathBuf>,
    pub home_dot_dotfiles: Option<PathBuf>,
    pub xdg_dirs: Vec<PathBuf>,
}

impl Default for ConfigSearch {
    fn default() -> Self {
        Self::new()
    }
}

impl ConfigSearch {
    pub fn new() -> Self {
        // Primary XDG/APPDATA path plus macOS Application Support back-compat
        // (former dirs::config_dir location). See paths::config_search_dirs.
        let xdg_dirs = crate::paths::config_search_dirs()
            .into_iter()
            .map(|d| d.join("andre"))
            .collect();

        let home = crate::paths::home_dir();

        Self {
            explicit: None,
            cwd: Some(std::env::current_dir().unwrap_or_default()),
            home_dotfiles: home.as_ref().map(|h| h.join("dotfiles")),
            home_dot_dotfiles: home.as_ref().map(|h| h.join(".dotfiles")),
            xdg_dirs,
        }
    }

    fn try_location(base: &Path, names: &[&str]) -> Option<PathBuf> {
        for name in names {
            let path = base.join(name);
            if path.exists() {
                return Some(path);
            }
        }
        None
    }

    pub fn discover(&self) -> Option<PathBuf> {
        if let Some(ref explicit) = self.explicit {
            if explicit.exists() {
                return Some(explicit.clone());
            }
        }

        let names = ["andre.yaml", "andre.yml"];

        if let Some(ref cwd) = self.cwd {
            if let Some(path) = Self::try_location(cwd, &names) {
                return Some(path);
            }
        }

        if let Some(ref dir) = self.home_dotfiles {
            if let Some(path) = Self::try_location(dir, &names) {
                return Some(path);
            }
        }

        if let Some(ref dir) = self.home_dot_dotfiles {
            if let Some(path) = Self::try_location(dir, &names) {
                return Some(path);
            }
        }

        for xdg_dir in &self.xdg_dirs {
            if let Some(path) = Self::try_location(xdg_dir, &names) {
                return Some(path);
            }
        }

        None
    }

    pub fn search_locations(&self) -> Vec<PathBuf> {
        let mut locations = Vec::new();
        let names = ["andre.yaml", "andre.yml"];

        if let Some(ref cwd) = self.cwd {
            for name in &names {
                locations.push(cwd.join(name));
            }
        }
        if let Some(ref dir) = self.home_dotfiles {
            for name in &names {
                locations.push(dir.join(name));
            }
        }
        if let Some(ref dir) = self.home_dot_dotfiles {
            for name in &names {
                locations.push(dir.join(name));
            }
        }
        for xdg_dir in &self.xdg_dirs {
            for name in &names {
                locations.push(xdg_dir.join(name));
            }
        }
        locations
    }
}
