use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fmt;
use std::path::{Path, PathBuf};

use crate::error::{Error, Result};
use crate::path::resolve_path;
use crate::{StowAction, Theme};

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Config {
    pub global: GlobalSettings,
    pub groups: HashMap<String, Group>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct GlobalSettings {
    /// Link engine: "native" (default, no GNU stow) or "stow" (shells out to GNU stow).
    pub engine: Option<String>,
    pub theme: Option<String>,
    #[serde(alias = "ignores")]
    pub ignore: Option<Vec<String>>,
    pub stow: Option<StowConfig>,
}

#[derive(Debug, Clone, Deserialize, Serialize, Default)]
#[serde(deny_unknown_fields)]
pub struct StowConfig {
    pub verbosity: Option<u8>,
    pub dry_run: Option<bool>,
    pub no_folding: Option<bool>,
    pub adopt: Option<bool>,
    pub dotfiles: Option<bool>,
    pub action: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Group {
    pub source: String,
    pub target: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ignore: Option<Vec<String>>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConfigWarning {
    UnknownTheme(String),
    UnknownAction(String),
    UnknownEngine(String),
    VerbosityOutOfRange(u8),
    NoGroups,
}

impl fmt::Display for ConfigWarning {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ConfigWarning::UnknownTheme(t) => write!(f, "unknown theme '{}', using default", t),
            ConfigWarning::UnknownAction(a) => write!(f, "unknown action '{}', using stow", a),
            ConfigWarning::UnknownEngine(e) => {
                write!(f, "unknown engine '{}', using native", e)
            }
            ConfigWarning::VerbosityOutOfRange(v) => {
                write!(f, "verbosity {} is out of range (0-5)", v)
            }
            ConfigWarning::NoGroups => write!(f, "no groups defined in config"),
        }
    }
}

impl Config {
    pub fn default_empty() -> Self {
        Self {
            global: GlobalSettings {
                engine: None,
                theme: None,
                ignore: None,
                stow: None,
            },
            groups: HashMap::new(),
        }
    }

    pub fn validate(&self) -> Vec<ConfigWarning> {
        let mut warnings = Vec::new();

        if self.groups.is_empty() {
            warnings.push(ConfigWarning::NoGroups);
        }

        if let Some(ref theme) = self.global.theme {
            if Theme::from_str(theme).is_none() {
                warnings.push(ConfigWarning::UnknownTheme(theme.clone()));
            }
        }

        if let Some(ref engine) = self.global.engine {
            if !engine.eq_ignore_ascii_case("native") && !engine.eq_ignore_ascii_case("stow") {
                warnings.push(ConfigWarning::UnknownEngine(engine.clone()));
            }
        }

        if let Some(ref stow) = self.global.stow {
            if let Some(ref action) = stow.action {
                if StowAction::from_str(action).is_none() {
                    warnings.push(ConfigWarning::UnknownAction(action.clone()));
                }
            }
            if let Some(v) = stow.verbosity {
                if v > 5 {
                    warnings.push(ConfigWarning::VerbosityOutOfRange(v));
                }
            }
        }

        warnings
    }

    pub fn load(path: &Path) -> Result<Self> {
        if !path.exists() {
            return Err(Error::ConfigNotFound(path.to_path_buf()));
        }
        let content = std::fs::read_to_string(path)?;
        let config: Config = serde_yaml_ng::from_str(&content).map_err(Error::YamlParse)?;
        Ok(config)
    }

    /// Validates that every group's resolved source directory exists on disk.
    pub fn validate_sources(&self, config_dir: &Path, home_dir: &Path) -> Result<()> {
        for group in self.groups.values() {
            let path = resolve_path(&group.source, config_dir, home_dir);
            if !path.exists() {
                return Err(Error::SourceNotFound(path));
            }
        }
        Ok(())
    }

    /// Returns the resolved source path for a group, or `None` if the group doesn't exist.
    /// Unlike `get_group_source`, this does NOT check whether the path exists on disk.
    pub fn resolve_group_source(
        &self,
        name: &str,
        config_dir: &Path,
        home_dir: &Path,
    ) -> Option<PathBuf> {
        let group = self.groups.get(name)?;
        Some(resolve_path(&group.source, config_dir, home_dir))
    }

    /// Returns the source path only if the group exists AND the resolved path exists on disk.
    pub fn get_group_source(
        &self,
        name: &str,
        config_dir: &Path,
        home_dir: &Path,
    ) -> Option<PathBuf> {
        let path = self.resolve_group_source(name, config_dir, home_dir)?;
        if path.exists() {
            Some(path)
        } else {
            None
        }
    }

    /// Returns the resolved target path for a group, or `None` if the group doesn't exist.
    /// Unlike `get_group_target`, this does NOT check whether the path exists on disk.
    pub fn resolve_group_target(
        &self,
        name: &str,
        config_dir: &Path,
        home_dir: &Path,
    ) -> Option<PathBuf> {
        let group = self.groups.get(name)?;
        Some(resolve_path(&group.target, config_dir, home_dir))
    }

    /// Returns the target path only if the group exists AND the resolved path exists on disk.
    pub fn get_group_target(
        &self,
        name: &str,
        config_dir: &Path,
        home_dir: &Path,
    ) -> Option<PathBuf> {
        let path = self.resolve_group_target(name, config_dir, home_dir)?;
        if path.exists() {
            Some(path)
        } else {
            None
        }
    }

    pub fn effective_verbosity(&self) -> u8 {
        self.global
            .stow
            .as_ref()
            .and_then(|s| s.verbosity)
            .unwrap_or(0)
    }

    pub fn effective_dry_run(&self) -> bool {
        self.global
            .stow
            .as_ref()
            .and_then(|s| s.dry_run)
            .unwrap_or(false)
    }

    pub fn effective_no_folding(&self) -> bool {
        self.global
            .stow
            .as_ref()
            .and_then(|s| s.no_folding)
            .unwrap_or(false)
    }

    pub fn effective_adopt(&self) -> bool {
        self.global
            .stow
            .as_ref()
            .and_then(|s| s.adopt)
            .unwrap_or(false)
    }

    pub fn effective_dotfiles(&self) -> bool {
        self.global
            .stow
            .as_ref()
            .and_then(|s| s.dotfiles)
            .unwrap_or(false)
    }

    pub fn effective_action(&self) -> String {
        self.global
            .stow
            .as_ref()
            .and_then(|s| s.action.clone())
            .unwrap_or_else(|| String::from("stow"))
    }

    /// Resolved link engine: "native" (default) or "stow". Case-insensitive;
    /// an unrecognized value falls back to "native" (a warning is emitted by `validate`).
    pub fn effective_engine(&self) -> String {
        match self.global.engine.as_deref() {
            Some(e) if e.eq_ignore_ascii_case("stow") => String::from("stow"),
            _ => String::from("native"),
        }
    }

    pub fn global_ignore(&self) -> Vec<String> {
        self.global.ignore.clone().unwrap_or_default()
    }

    pub fn get_group_ignore(&self, group_name: &str) -> Option<&[String]> {
        self.groups
            .get(group_name)
            .and_then(|g| g.ignore.as_deref())
    }

    pub fn effective_ignores(&self, group_name: &str) -> Vec<String> {
        let global = self.global_ignore();
        let group = self
            .groups
            .get(group_name)
            .and_then(|g| g.ignore.clone())
            .unwrap_or_default();
        global.into_iter().chain(group).collect()
    }

    pub fn save(&self, path: &Path) -> Result<()> {
        let yaml = serde_yaml_ng::to_string(self).map_err(Error::YamlParse)?;
        std::fs::write(path, yaml).map_err(Error::Io)?;
        Ok(())
    }

    fn ensure_stow(&mut self) -> &mut StowConfig {
        self.global.stow.get_or_insert_with(StowConfig::default)
    }

    pub fn set_verbosity(&mut self, v: u8) {
        self.ensure_stow().verbosity = Some(v);
    }

    pub fn set_dry_run(&mut self, b: bool) {
        self.ensure_stow().dry_run = Some(b);
    }

    pub fn set_no_folding(&mut self, b: bool) {
        self.ensure_stow().no_folding = Some(b);
    }

    pub fn set_adopt(&mut self, b: bool) {
        self.ensure_stow().adopt = Some(b);
    }

    pub fn set_dotfiles(&mut self, b: bool) {
        self.ensure_stow().dotfiles = Some(b);
    }

    pub fn set_action(&mut self, a: &str) {
        self.ensure_stow().action = Some(a.to_string());
    }

    pub fn set_theme(&mut self, t: Option<String>) {
        self.global.theme = t;
    }

    /// Adds a group. Returns `true` if a group with the same name was overwritten.
    pub fn add_group(&mut self, name: String, source: String, target: String) -> bool {
        self.groups
            .insert(
                name,
                Group {
                    source,
                    target,
                    ignore: None,
                },
            )
            .is_some()
    }

    /// Removes a group. Returns `true` if the group existed and was removed.
    pub fn remove_group(&mut self, name: &str) -> bool {
        self.groups.remove(name).is_some()
    }

    pub fn add_global_ignore(&mut self, pattern: String) {
        self.global
            .ignore
            .get_or_insert_with(Vec::new)
            .push(pattern);
    }

    pub fn remove_global_ignore(&mut self, pattern: &str) {
        if let Some(ref mut ignores) = self.global.ignore {
            ignores.retain(|i| i != pattern);
        }
    }

    /// Sets per-group ignores. Returns `true` if the group existed.
    pub fn set_group_ignore(&mut self, group_name: &str, ignores: Vec<String>) -> bool {
        if let Some(group) = self.groups.get_mut(group_name) {
            group.ignore = if ignores.is_empty() {
                None
            } else {
                Some(ignores)
            };
            true
        } else {
            false
        }
    }

    pub fn update_group_source(&mut self, group_name: &str, source: String) {
        if let Some(group) = self.groups.get_mut(group_name) {
            group.source = source;
        }
    }

    pub fn update_group_target(&mut self, group_name: &str, target: String) {
        if let Some(group) = self.groups.get_mut(group_name) {
            group.target = target;
        }
    }
}

#[cfg(test)]
mod tests {
    use std::fs;
    use tempfile::TempDir;

    use super::*;

    #[test]
    fn test_config_load_valid() {
        let tmp = TempDir::new().unwrap();
        let config_path = tmp.path().join("andre.yml");

        fs::write(
            &config_path,
            r#"
global:
  stow:
    verbosity: 2
    dry_run: true
groups:
  main:
    source: packages
    target: target
"#,
        )
        .unwrap();

        let config = Config::load(&config_path).unwrap();
        assert_eq!(config.effective_verbosity(), 2);
        assert!(config.effective_dry_run());
        assert_eq!(config.groups.len(), 1);
    }

    #[test]
    fn test_config_load_missing_file() {
        let tmp = TempDir::new().unwrap();
        let config_path = tmp.path().join("nonexistent.yml");

        let result = Config::load(&config_path);
        assert!(result.is_err());
    }

    #[test]
    fn test_config_load_invalid_yaml() {
        let tmp = TempDir::new().unwrap();
        let config_path = tmp.path().join("bad.yml");

        fs::write(&config_path, "invalid: yaml: content: [").unwrap();

        let result = Config::load(&config_path);
        assert!(result.is_err());
    }

    #[test]
    fn test_effective_verbosity_default() {
        let tmp = TempDir::new().unwrap();
        let config_path = tmp.path().join("andre.yml");

        fs::write(
            &config_path,
            r#"
global:
  stow:
    dry_run: false
groups:
  main:
    source: packages
    target: target
"#,
        )
        .unwrap();

        let config = Config::load(&config_path).unwrap();
        assert_eq!(config.effective_verbosity(), 0);
    }

    #[test]
    fn test_effective_dry_run_default() {
        let tmp = TempDir::new().unwrap();
        let config_path = tmp.path().join("andre.yml");

        fs::write(
            &config_path,
            r#"
global:
  stow:
    verbosity: 1
groups:
  main:
    source: packages
    target: target
"#,
        )
        .unwrap();

        let config = Config::load(&config_path).unwrap();
        assert!(!config.effective_dry_run());
    }

    #[test]
    fn test_global_ignore() {
        let tmp = TempDir::new().unwrap();
        let config_path = tmp.path().join("andre.yml");

        fs::write(
            &config_path,
            r#"
global:
  ignore:
    - .git
    - .DS_Store
  stow:
    verbosity: 0
groups:
  main:
    source: packages
    target: target
"#,
        )
        .unwrap();

        let config = Config::load(&config_path).unwrap();
        let ignore = config.global_ignore();
        assert_eq!(ignore, vec![".git", ".DS_Store"]);
    }

    #[test]
    fn test_effective_action_default() {
        let tmp = TempDir::new().unwrap();
        let config_path = tmp.path().join("andre.yml");

        fs::write(
            &config_path,
            r#"
global:
  stow:
    verbosity: 0
groups:
  main:
    source: packages
    target: target
"#,
        )
        .unwrap();

        let config = Config::load(&config_path).unwrap();
        assert_eq!(config.effective_action(), "stow");
    }

    #[test]
    fn test_effective_dotfiles_default() {
        let tmp = TempDir::new().unwrap();
        let config_path = tmp.path().join("andre.yml");

        fs::write(
            &config_path,
            r#"
global:
  stow:
    verbosity: 0
groups:
  main:
    source: packages
    target: target
"#,
        )
        .unwrap();

        let config = Config::load(&config_path).unwrap();
        assert!(!config.effective_dotfiles());
    }

    #[test]
    fn test_effective_dotfiles_true() {
        let tmp = TempDir::new().unwrap();
        let config_path = tmp.path().join("andre.yml");

        fs::write(
            &config_path,
            r#"
global:
  stow:
    dotfiles: true
groups:
  main:
    source: packages
    target: target
"#,
        )
        .unwrap();

        let config = Config::load(&config_path).unwrap();
        assert!(config.effective_dotfiles());
    }

    #[test]
    fn test_config_save_and_reload() {
        let tmp = TempDir::new().unwrap();
        let config_path = tmp.path().join("andre.yml");

        fs::write(
            &config_path,
            r#"
global:
  theme: dracula
  ignore:
    - .git
    - .DS_Store
  stow:
    verbosity: 2
    dry_run: true
    action: restow
groups:
  home:
    source: dotfiles/home
    target: "~"
  config:
    source: dotfiles/config
    target: ~/.config
"#,
        )
        .unwrap();

        let config = Config::load(&config_path).unwrap();
        let save_path = tmp.path().join("saved.yml");
        config.save(&save_path).unwrap();

        let reloaded = Config::load(&save_path).unwrap();
        assert_eq!(reloaded.effective_verbosity(), 2);
        assert!(reloaded.effective_dry_run());
        assert_eq!(reloaded.effective_action(), "restow");
        assert_eq!(reloaded.global.theme.as_deref(), Some("dracula"));
        assert_eq!(reloaded.groups.len(), 2);
        assert_eq!(reloaded.groups.get("home").unwrap().source, "dotfiles/home");
    }

    #[test]
    fn test_save_persists_mutations() {
        let tmp = TempDir::new().unwrap();
        let config_path = tmp.path().join("andre.yml");

        fs::write(
            &config_path,
            r#"
global:
  stow:
    verbosity: 0
    dry_run: false
groups:
  main:
    source: packages
    target: target
"#,
        )
        .unwrap();

        let mut config = Config::load(&config_path).unwrap();
        config.set_verbosity(3);
        config.set_dry_run(true);
        config.set_adopt(true);
        config.set_dotfiles(true);
        config.set_action("unstow");
        config.set_theme(Some("dracula".to_string()));
        config.add_global_ignore(".git".to_string());

        let save_path = tmp.path().join("saved.yml");
        config.save(&save_path).unwrap();

        let reloaded = Config::load(&save_path).unwrap();
        assert_eq!(reloaded.effective_verbosity(), 3);
        assert!(reloaded.effective_dry_run());
        assert!(reloaded.effective_adopt());
        assert!(reloaded.effective_dotfiles());
        assert_eq!(reloaded.effective_action(), "unstow");
        assert_eq!(reloaded.global.theme.as_deref(), Some("dracula"));
        assert_eq!(reloaded.global_ignore(), vec!["".to_string() + ".git"]);
    }

    #[test]
    fn test_add_and_remove_group() {
        let tmp = TempDir::new().unwrap();
        let config_path = tmp.path().join("andre.yml");

        fs::write(
            &config_path,
            r#"
global: {}
groups:
  main:
    source: packages
    target: target
"#,
        )
        .unwrap();

        let mut config = Config::load(&config_path).unwrap();
        assert_eq!(config.groups.len(), 1);

        config.add_group(
            "shell".to_string(),
            "dotfiles/shell".to_string(),
            "~".to_string(),
        );
        assert_eq!(config.groups.len(), 2);
        assert_eq!(config.groups["shell"].source, "dotfiles/shell");
        assert!(config.groups["shell"].ignore.is_none());

        config.remove_group("main");
        assert_eq!(config.groups.len(), 1);
        assert!(!config.groups.contains_key("main"));
        assert!(config.groups.contains_key("shell"));

        let save_path = tmp.path().join("saved.yml");
        config.save(&save_path).unwrap();
        let reloaded = Config::load(&save_path).unwrap();
        assert_eq!(reloaded.groups.len(), 1);
        assert!(reloaded.groups.contains_key("shell"));
    }

    #[test]
    fn test_add_remove_global_ignore() {
        let tmp = TempDir::new().unwrap();
        let config_path = tmp.path().join("andre.yml");

        fs::write(
            &config_path,
            r#"
global: {}
groups:
  main:
    source: packages
    target: target
"#,
        )
        .unwrap();

        let mut config = Config::load(&config_path).unwrap();
        assert!(config.global_ignore().is_empty());

        config.add_global_ignore(".git".to_string());
        config.add_global_ignore(".DS_Store".to_string());
        assert_eq!(
            config.global_ignore(),
            vec!["".to_string() + ".git", "".to_string() + ".DS_Store"]
        );

        config.remove_global_ignore(".git");
        assert_eq!(config.global_ignore(), vec!["".to_string() + ".DS_Store"]);

        config.remove_global_ignore(".DS_Store");
        assert!(config.global_ignore().is_empty());
    }

    #[test]
    fn test_group_ignore_round_trip() {
        let tmp = TempDir::new().unwrap();
        let config_path = tmp.path().join("andre.yml");

        fs::write(
            &config_path,
            r#"
global: {}
groups:
  main:
    source: packages
    target: target
"#,
        )
        .unwrap();

        let mut config = Config::load(&config_path).unwrap();
        config.set_group_ignore("main", vec!["*.log".to_string(), "tmp".to_string()]);

        let save_path = tmp.path().join("saved.yml");
        config.save(&save_path).unwrap();

        let reloaded = Config::load(&save_path).unwrap();
        let group = reloaded.groups.get("main").unwrap();
        assert_eq!(
            group.ignore.as_ref().unwrap(),
            &vec!["*.log".to_string(), "tmp".to_string()]
        );
    }

    #[test]
    fn test_ensure_stow_creates_on_first_mutation() {
        let tmp = TempDir::new().unwrap();
        let config_path = tmp.path().join("andre.yml");

        fs::write(
            &config_path,
            r#"
global: {}
groups:
  main:
    source: packages
    target: target
"#,
        )
        .unwrap();

        let mut config = Config::load(&config_path).unwrap();
        assert!(config.global.stow.is_none());

        config.set_verbosity(2);
        assert!(config.global.stow.is_some());
        assert_eq!(config.global.stow.as_ref().unwrap().verbosity, Some(2));
    }

    #[test]
    fn test_update_group_source_target() {
        let tmp = TempDir::new().unwrap();
        let config_path = tmp.path().join("andre.yml");

        fs::write(
            &config_path,
            r#"
global: {}
groups:
  main:
    source: packages
    target: target
"#,
        )
        .unwrap();

        let mut config = Config::load(&config_path).unwrap();
        config.update_group_source("main", "new/source".to_string());
        config.update_group_target("main", "/new/target".to_string());

        let save_path = tmp.path().join("saved.yml");
        config.save(&save_path).unwrap();

        let reloaded = Config::load(&save_path).unwrap();
        let group = reloaded.groups.get("main").unwrap();
        assert_eq!(group.source, "new/source");
        assert_eq!(group.target, "/new/target");
    }

    #[test]
    fn test_get_group_source_returns_none_when_missing() {
        let tmp = TempDir::new().unwrap();
        let config_path = tmp.path().join("andre.yml");
        fs::write(
            &config_path,
            r#"
global: {}
groups:
  main:
    source: /nonexistent/source/path
    target: /nonexistent/target/path
"#,
        )
        .unwrap();
        let config = Config::load(&config_path).unwrap();
        let result = config.get_group_source("main", tmp.path(), tmp.path());
        assert!(
            result.is_none(),
            "get_group_source should return None when source dir doesn't exist"
        );
    }

    #[test]
    fn test_get_group_target_returns_none_when_missing() {
        let tmp = TempDir::new().unwrap();
        let config_path = tmp.path().join("andre.yml");
        fs::write(
            &config_path,
            r#"
global: {}
groups:
  main:
    source: /nonexistent/source/path
    target: /nonexistent/target/path
"#,
        )
        .unwrap();
        let config = Config::load(&config_path).unwrap();
        let result = config.get_group_target("main", tmp.path(), tmp.path());
        assert!(
            result.is_none(),
            "get_group_target should return None when target dir doesn't exist"
        );
    }

    #[test]
    fn test_validate_clean_config_returns_no_warnings() {
        let config = Config {
            global: GlobalSettings {
                engine: None,
                theme: Some("nord".to_string()),
                ignore: None,
                stow: Some(StowConfig {
                    verbosity: Some(2),
                    dry_run: Some(false),
                    no_folding: Some(false),
                    adopt: Some(false),
                    dotfiles: Some(false),
                    action: Some("stow".to_string()),
                }),
            },
            groups: {
                let mut g = HashMap::new();
                g.insert(
                    "test".to_string(),
                    Group {
                        source: "/tmp".to_string(),
                        target: "/tmp".to_string(),
                        ignore: None,
                    },
                );
                g
            },
        };
        let warnings = config.validate();
        assert!(
            warnings.is_empty(),
            "expected no warnings, got: {:?}",
            warnings
        );
    }

    #[test]
    fn test_validate_unknown_theme_returns_warning() {
        let config = Config {
            global: GlobalSettings {
                engine: None,
                theme: Some("nonexistent-theme".to_string()),
                ignore: None,
                stow: None,
            },
            groups: {
                let mut g = HashMap::new();
                g.insert(
                    "test".to_string(),
                    Group {
                        source: "/tmp".to_string(),
                        target: "/tmp".to_string(),
                        ignore: None,
                    },
                );
                g
            },
        };
        let warnings = config.validate();
        assert_eq!(warnings.len(), 1);
        assert!(matches!(&warnings[0], ConfigWarning::UnknownTheme(t) if t == "nonexistent-theme"));
    }

    #[test]
    fn test_validate_unknown_action_returns_warning() {
        let config = Config {
            global: GlobalSettings {
                engine: None,
                theme: None,
                ignore: None,
                stow: Some(StowConfig {
                    verbosity: None,
                    dry_run: None,
                    no_folding: None,
                    adopt: None,
                    dotfiles: None,
                    action: Some("invalid-action".to_string()),
                }),
            },
            groups: {
                let mut g = HashMap::new();
                g.insert(
                    "test".to_string(),
                    Group {
                        source: "/tmp".to_string(),
                        target: "/tmp".to_string(),
                        ignore: None,
                    },
                );
                g
            },
        };
        let warnings = config.validate();
        assert_eq!(warnings.len(), 1);
        assert!(matches!(&warnings[0], ConfigWarning::UnknownAction(a) if a == "invalid-action"));
    }

    #[test]
    fn test_validate_verbosity_out_of_range_returns_warning() {
        let config = Config {
            global: GlobalSettings {
                engine: None,
                theme: None,
                ignore: None,
                stow: Some(StowConfig {
                    verbosity: Some(99),
                    dry_run: None,
                    no_folding: None,
                    adopt: None,
                    dotfiles: None,
                    action: None,
                }),
            },
            groups: {
                let mut g = HashMap::new();
                g.insert(
                    "test".to_string(),
                    Group {
                        source: "/tmp".to_string(),
                        target: "/tmp".to_string(),
                        ignore: None,
                    },
                );
                g
            },
        };
        let warnings = config.validate();
        assert_eq!(warnings.len(), 1);
        assert!(matches!(&warnings[0], ConfigWarning::VerbosityOutOfRange(v) if v == &99));
    }

    #[test]
    fn test_validate_no_groups_returns_warning() {
        let config = Config {
            global: GlobalSettings {
                engine: None,
                theme: None,
                ignore: None,
                stow: None,
            },
            groups: HashMap::new(),
        };
        let warnings = config.validate();
        assert_eq!(warnings.len(), 1);
        assert_eq!(warnings[0], ConfigWarning::NoGroups);
    }

    #[test]
    fn test_unknown_yaml_field_rejected() {
        let tmp = tempfile::TempDir::new().unwrap();
        let config_path = tmp.path().join("andre.yml");
        std::fs::write(
            &config_path,
            r#"
global:
  stow:
    verbosity: 0
    nonexistent_field: true
groups: {}
"#,
        )
        .unwrap();
        let result = Config::load(&config_path);
        assert!(
            result.is_err(),
            "expected error for unknown field, got: {:?}",
            result
        );
        let err = result.unwrap_err().to_string();
        assert!(
            err.contains("nonexistent_field") || err.contains("unknown field"),
            "error should mention the unknown field: {}",
            err
        );
    }

    #[test]
    fn test_engine_defaults_to_native_when_absent() {
        let config = Config::default_empty();
        assert_eq!(config.effective_engine(), "native");
        assert!(config
            .validate()
            .iter()
            .all(|w| !matches!(w, ConfigWarning::UnknownEngine(_))));
    }

    #[test]
    fn test_engine_stow_is_case_insensitive_and_round_trips() {
        let tmp = tempfile::TempDir::new().unwrap();
        let path = tmp.path().join("andre.yml");
        std::fs::write(
            &path,
            "global:\n  engine: STOW\n  stow:\n    verbosity: 0\ngroups: {}\n",
        )
        .unwrap();
        let config = Config::load(&path).unwrap();
        assert_eq!(config.effective_engine(), "stow");

        // save/load round-trip preserves the field.
        let path2 = tmp.path().join("andre2.yml");
        config.save(&path2).unwrap();
        let reloaded = Config::load(&path2).unwrap();
        assert_eq!(reloaded.global.engine.as_deref(), Some("STOW"));
        assert_eq!(reloaded.effective_engine(), "stow");
    }

    #[test]
    fn test_engine_unknown_value_warns_and_falls_back_to_native() {
        let config = Config {
            global: GlobalSettings {
                engine: Some("ruby".into()),
                theme: None,
                ignore: None,
                stow: None,
            },
            groups: HashMap::new(),
        };
        let warnings = config.validate();
        assert!(
            matches!(&warnings[..], [ConfigWarning::NoGroups, ConfigWarning::UnknownEngine(e)] if e == "ruby")
        );
        assert_eq!(config.effective_engine(), "native");
    }
}
