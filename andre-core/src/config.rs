use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fmt;
use std::path::{Path, PathBuf};

use crate::engine::EngineKind;
use crate::error::{Error, Result};
use crate::path::resolve_path;
use crate::state::{StowAction, StowFlags, Theme, MAX_VERBOSITY};

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Config {
    pub global: GlobalSettings,
    pub groups: BTreeMap<String, Group>,
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
                write!(f, "verbosity {} is out of range (0-{})", v, MAX_VERBOSITY)
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
            groups: BTreeMap::new(),
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
            if EngineKind::from_str(engine).is_none() {
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
                if v > MAX_VERBOSITY {
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
        self.resolve_group_path(name, config_dir, home_dir, |group| &group.source)
    }

    /// Returns the source path only if the group exists AND the resolved path exists on disk.
    pub fn get_group_source(
        &self,
        name: &str,
        config_dir: &Path,
        home_dir: &Path,
    ) -> Option<PathBuf> {
        self.resolve_group_source(name, config_dir, home_dir)
            .filter(|path| path.exists())
    }

    /// Returns the resolved target path for a group, or `None` if the group doesn't exist.
    /// Unlike `get_group_target`, this does NOT check whether the path exists on disk.
    pub fn resolve_group_target(
        &self,
        name: &str,
        config_dir: &Path,
        home_dir: &Path,
    ) -> Option<PathBuf> {
        self.resolve_group_path(name, config_dir, home_dir, |group| &group.target)
    }

    /// Returns the target path only if the group exists AND the resolved path exists on disk.
    pub fn get_group_target(
        &self,
        name: &str,
        config_dir: &Path,
        home_dir: &Path,
    ) -> Option<PathBuf> {
        self.resolve_group_target(name, config_dir, home_dir)
            .filter(|path| path.exists())
    }

    fn resolve_group_path(
        &self,
        name: &str,
        config_dir: &Path,
        home_dir: &Path,
        field: fn(&Group) -> &str,
    ) -> Option<PathBuf> {
        let group = self.groups.get(name)?;
        Some(resolve_path(field(group), config_dir, home_dir))
    }

    /// The stow options from `global.stow`. Missing or unrecognised values fall back to
    /// the defaults (`StowFlags::default()`), which is also how `validate` warns about them.
    pub fn stow_flags(&self) -> StowFlags {
        let stow = self.global.stow.clone().unwrap_or_default();
        StowFlags {
            action: stow
                .action
                .as_deref()
                .and_then(StowAction::from_str)
                .unwrap_or_default(),
            verbosity: stow.verbosity.unwrap_or_default(),
            dry_run: stow.dry_run.unwrap_or_default(),
            no_folding: stow.no_folding.unwrap_or_default(),
            adopt: stow.adopt.unwrap_or_default(),
            dotfiles: stow.dotfiles.unwrap_or_default(),
        }
    }

    /// Resolved link engine. An unrecognised value falls back to `Native`
    /// (a warning is emitted by `validate`).
    pub fn effective_engine(&self) -> EngineKind {
        self.global
            .engine
            .as_deref()
            .and_then(EngineKind::from_str)
            .unwrap_or_default()
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

    /// Writes every stow option to `global.stow`.
    pub fn set_stow_flags(&mut self, flags: StowFlags) {
        let stow = self.ensure_stow();
        stow.verbosity = Some(flags.verbosity);
        stow.dry_run = Some(flags.dry_run);
        stow.no_folding = Some(flags.no_folding);
        stow.adopt = Some(flags.adopt);
        stow.dotfiles = Some(flags.dotfiles);
        stow.action = Some(flags.action.as_str().to_string());
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

    /// Renames a group, keeping its source, target and ignores. Returns `false` when
    /// `old` does not exist or `new` is already taken by a different group (nothing changes).
    pub fn rename_group(&mut self, old: &str, new: &str) -> bool {
        if old == new {
            return self.groups.contains_key(old);
        }
        if self.groups.contains_key(new) {
            return false;
        }
        match self.groups.remove(old) {
            Some(group) => {
                self.groups.insert(new.to_string(), group);
                true
            }
            None => false,
        }
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

    /// Sets per-group ignores; an empty list clears them. Returns `true` if the group existed.
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

    /// Returns `true` if the group existed.
    pub fn update_group_source(&mut self, group_name: &str, source: String) -> bool {
        match self.groups.get_mut(group_name) {
            Some(group) => {
                group.source = source;
                true
            }
            None => false,
        }
    }

    /// Returns `true` if the group existed.
    pub fn update_group_target(&mut self, group_name: &str, target: String) -> bool {
        match self.groups.get_mut(group_name) {
            Some(group) => {
                group.target = target;
                true
            }
            None => false,
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
        assert_eq!(config.stow_flags().verbosity, 2);
        assert!(config.stow_flags().dry_run);
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
    fn test_stow_flags_verbosity_default() {
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
        assert_eq!(config.stow_flags().verbosity, 0);
    }

    #[test]
    fn test_stow_flags_dry_run_default() {
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
        assert!(!config.stow_flags().dry_run);
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
    fn test_stow_flags_action_default() {
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
        assert_eq!(config.stow_flags().action, StowAction::Stow);
    }

    #[test]
    fn test_stow_flags_dotfiles_default() {
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
        assert!(!config.stow_flags().dotfiles);
    }

    #[test]
    fn test_stow_flags_dotfiles_true() {
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
        assert!(config.stow_flags().dotfiles);
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
        assert_eq!(reloaded.stow_flags().verbosity, 2);
        assert!(reloaded.stow_flags().dry_run);
        assert_eq!(reloaded.stow_flags().action, StowAction::Restow);
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
        config.set_stow_flags(StowFlags {
            verbosity: 3,
            dry_run: true,
            no_folding: false,
            adopt: true,
            dotfiles: false,
            action: StowAction::Unstow,
        });
        config.set_theme(Some("dracula".to_string()));
        config.add_global_ignore(".git".to_string());

        let save_path = tmp.path().join("saved.yml");
        config.save(&save_path).unwrap();

        let reloaded = Config::load(&save_path).unwrap();
        assert_eq!(reloaded.stow_flags().verbosity, 3);
        assert!(reloaded.stow_flags().dry_run);
        assert!(!reloaded.stow_flags().no_folding);
        assert!(reloaded.stow_flags().adopt);
        assert!(!reloaded.stow_flags().dotfiles);
        assert_eq!(reloaded.stow_flags().action, StowAction::Unstow);
        assert_eq!(reloaded.global.theme.as_deref(), Some("dracula"));
        assert_eq!(reloaded.global_ignore(), vec![".git".to_string()]);
    }

    #[test]
    fn test_set_stow_flags_round_trips_each_bool_alone() {
        let tmp = TempDir::new().unwrap();
        let save_path = tmp.path().join("saved.yml");
        // One bool true at a time: a swap between any two bools changes the reloaded value.
        let cases = [
            StowFlags {
                dry_run: true,
                ..StowFlags::default()
            },
            StowFlags {
                no_folding: true,
                ..StowFlags::default()
            },
            StowFlags {
                adopt: true,
                ..StowFlags::default()
            },
            StowFlags {
                dotfiles: true,
                ..StowFlags::default()
            },
        ];
        for flags in cases {
            let mut config = Config::default_empty();
            config.set_stow_flags(flags);
            config.save(&save_path).unwrap();
            assert_eq!(Config::load(&save_path).unwrap().stow_flags(), flags);
        }
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
    fn test_rename_group_keeps_source_target_and_ignore() {
        let mut config = Config::default_empty();
        config.add_group("home".into(), "dotfiles/home".into(), "~".into());
        config.set_group_ignore("home", vec![".git".into()]);

        assert!(config.rename_group("home", "work"));
        assert!(!config.groups.contains_key("home"));
        let renamed = &config.groups["work"];
        assert_eq!(renamed.source, "dotfiles/home");
        assert_eq!(renamed.target, "~");
        assert_eq!(renamed.ignore, Some(vec![".git".to_string()]));
    }

    #[test]
    fn test_rename_group_refuses_missing_or_taken_names() {
        let mut config = Config::default_empty();
        config.add_group("home".into(), "dotfiles/home".into(), "~".into());
        config.add_group(
            "config".into(),
            "dotfiles/config".into(),
            "~/.config".into(),
        );

        assert!(!config.rename_group("missing", "work"));
        assert!(!config.rename_group("home", "config"));
        assert_eq!(config.groups["home"].source, "dotfiles/home");
        assert_eq!(config.groups["config"].source, "dotfiles/config");
        assert_eq!(config.groups.len(), 2);
    }

    #[test]
    fn test_rename_group_to_same_name_is_noop() {
        let mut config = Config::default_empty();
        config.add_group("home".into(), "dotfiles/home".into(), "~".into());

        assert!(config.rename_group("home", "home"));
        assert_eq!(config.groups["home"].source, "dotfiles/home");
        assert!(!config.rename_group("missing", "missing"));
    }

    #[test]
    fn test_groups_serialize_in_sorted_order() {
        let tmp = TempDir::new().unwrap();
        let mut config = Config::default_empty();
        for name in ["zeta", "alpha", "mid"] {
            config.add_group(name.into(), format!("dotfiles/{name}"), "~".into());
        }

        let names: Vec<&str> = config.groups.keys().map(String::as_str).collect();
        assert_eq!(names, ["alpha", "mid", "zeta"]);

        let save_path = tmp.path().join("andre.yml");
        config.save(&save_path).unwrap();
        let yaml = fs::read_to_string(&save_path).unwrap();
        let alpha = yaml.find("alpha:").unwrap();
        let mid = yaml.find("mid:").unwrap();
        let zeta = yaml.find("zeta:").unwrap();
        assert!(alpha < mid && mid < zeta, "groups not sorted:\n{yaml}");
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
            vec![".git".to_string(), ".DS_Store".to_string()]
        );

        config.remove_global_ignore(".git");
        assert_eq!(config.global_ignore(), vec![".DS_Store".to_string()]);

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

        config.set_stow_flags(StowFlags {
            verbosity: 2,
            ..StowFlags::default()
        });
        assert!(config.global.stow.is_some());
        assert_eq!(config.global.stow.as_ref().unwrap().verbosity, Some(2));
    }

    #[test]
    fn test_set_group_ignore_empty_clears_and_reports_missing_group() {
        let mut config = Config::default_empty();
        config.add_group("main".into(), "src".into(), "dst".into());
        assert!(config.set_group_ignore("main", vec![".git".into()]));
        assert!(config.set_group_ignore("main", vec![]));
        assert_eq!(config.groups["main"].ignore, None);
        assert!(!config.set_group_ignore("missing", vec![".git".into()]));
    }

    #[test]
    fn test_update_group_paths_report_missing_group() {
        let mut config = Config::default_empty();
        config.add_group("main".into(), "src".into(), "dst".into());
        assert!(config.update_group_source("main", "new/src".into()));
        assert!(config.update_group_target("main", "/new/dst".into()));
        assert!(!config.update_group_source("missing", "x".into()));
        assert!(!config.update_group_target("missing", "x".into()));
        assert_eq!(config.groups["main"].source, "new/src");
        assert_eq!(config.groups["main"].target, "/new/dst");
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
                let mut g = BTreeMap::new();
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
                let mut g = BTreeMap::new();
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
                let mut g = BTreeMap::new();
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
                let mut g = BTreeMap::new();
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
            groups: BTreeMap::new(),
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
        assert_eq!(config.effective_engine(), EngineKind::Native);
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
        assert_eq!(config.effective_engine(), EngineKind::Stow);

        // save/load round-trip preserves the field.
        let path2 = tmp.path().join("andre2.yml");
        config.save(&path2).unwrap();
        let reloaded = Config::load(&path2).unwrap();
        assert_eq!(reloaded.global.engine.as_deref(), Some("STOW"));
        assert_eq!(reloaded.effective_engine(), EngineKind::Stow);
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
            groups: BTreeMap::new(),
        };
        let warnings = config.validate();
        assert!(
            matches!(&warnings[..], [ConfigWarning::NoGroups, ConfigWarning::UnknownEngine(e)] if e == "ruby")
        );
        assert_eq!(config.effective_engine(), EngineKind::Native);
    }
}
