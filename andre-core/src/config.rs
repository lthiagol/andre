use serde::{Deserialize, Serialize};
use std::fmt;
use std::path::{Component, Path, PathBuf};

use crate::engine::EngineKind;
use crate::error::{Error, Result};
use crate::packages::discover_packages;
use crate::path::resolve_path;
use crate::state::{IconSet, StowAction, StowFlags, Theme, MAX_VERBOSITY};

/// Load error for a config in the removed format. `main` prints the file path in front.
pub const GROUPS_REMOVED: &str = "`groups:` is no longer supported.
andre reads one packages folder now:
  packages: ~/dotfiles   # every subfolder is a package; relative paths start at the config folder
  target: ~              # optional; where links go (default: your home folder)
Profile refs are plain package names: `nvim`, not `home/nvim`.";

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Config {
    /// The packages folder, as written. `None` until set (a warning, not an error).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub packages: Option<String>,
    /// Where every package links. `None` = the home folder.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub target: Option<String>,
    #[serde(default)]
    pub global: GlobalSettings,
}

/// One package as every consumer sees it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PackageEntry {
    /// The folder name, `nvim` for `<packages>/nvim`. Also the name `stow` takes.
    pub name: String,
    /// `<packages>/nvim`.
    pub dir: PathBuf,
    /// `<packages>`, the folder passed to `stow -d`.
    pub source: PathBuf,
    /// The resolved target, passed to `stow -t`.
    pub target: PathBuf,
    /// `global.ignore`.
    pub ignores: Vec<String>,
}

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct GlobalSettings {
    /// Link engine: "native" (default, no GNU stow) or "stow" (shells out to GNU stow).
    pub engine: Option<String>,
    pub theme: Option<String>,
    /// Glyph set for the TUI: "nerd" (default, needs a Nerd Font) or "unicode".
    pub icons: Option<String>,
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConfigWarning {
    UnknownTheme(String),
    UnknownAction(String),
    UnknownEngine(String),
    UnknownIcons(String),
    VerbosityOutOfRange(u8),
    NoPackagesFolder,
}

impl fmt::Display for ConfigWarning {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ConfigWarning::UnknownTheme(t) => write!(f, "unknown theme '{}', using default", t),
            ConfigWarning::UnknownAction(a) => write!(f, "unknown action '{}', using stow", a),
            ConfigWarning::UnknownEngine(e) => {
                write!(f, "unknown engine '{}', using native", e)
            }
            ConfigWarning::UnknownIcons(i) => write!(f, "unknown icons '{}', using nerd", i),
            ConfigWarning::VerbosityOutOfRange(v) => {
                write!(f, "verbosity {} is out of range (0-{})", v, MAX_VERBOSITY)
            }
            ConfigWarning::NoPackagesFolder => {
                write!(f, "no packages folder: set `packages:` in andre.yml")
            }
        }
    }
}

impl Config {
    pub fn default_empty() -> Self {
        Self {
            packages: None,
            target: None,
            global: GlobalSettings {
                engine: None,
                theme: None,
                icons: None,
                ignore: None,
                stow: None,
            },
        }
    }

    pub fn validate(&self) -> Vec<ConfigWarning> {
        let mut warnings = Vec::new();

        if self.packages.is_none() {
            warnings.push(ConfigWarning::NoPackagesFolder);
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

        if let Some(ref icons) = self.global.icons {
            if IconSet::from_str(icons).is_none() {
                warnings.push(ConfigWarning::UnknownIcons(icons.clone()));
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
        // Checked before the struct parse, so an old file gets this message rather than an
        // unknown-field error.
        let top: serde_yaml_ng::Value =
            serde_yaml_ng::from_str(&content).map_err(Error::YamlParse)?;
        if top.get("groups").is_some() {
            return Err(Error::InvalidConfig(GROUPS_REMOVED.to_string()));
        }
        serde_yaml_ng::from_str(&content).map_err(Error::YamlParse)
    }

    /// The packages folder, resolved. `None` when `packages:` is unset.
    pub fn packages_dir(&self, config_dir: &Path, home_dir: &Path) -> Option<PathBuf> {
        self.packages
            .as_deref()
            .map(|packages| resolve_path(packages, config_dir, home_dir))
    }

    /// Where every package links: `target:` resolved, or the home folder when unset.
    pub fn target_dir(&self, config_dir: &Path, home_dir: &Path) -> PathBuf {
        match self.target.as_deref() {
            Some(target) => resolve_path(target, config_dir, home_dir),
            None => home_dir.to_path_buf(),
        }
    }

    /// Every package in the packages folder, sorted by name. Empty without `packages:`.
    pub fn package_entries(&self, config_dir: &Path, home_dir: &Path) -> Vec<PackageEntry> {
        let Some(source) = self.packages_dir(config_dir, home_dir) else {
            return Vec::new();
        };
        let target = self.target_dir(config_dir, home_dir);
        let ignores = self.ignores();
        discover_packages(&source)
            .into_iter()
            .map(|name| PackageEntry {
                dir: source.join(&name),
                name,
                source: source.clone(),
                target: target.clone(),
                ignores: ignores.clone(),
            })
            .collect()
    }

    /// The entry called `name`, if that package exists.
    pub fn package(&self, name: &str, config_dir: &Path, home_dir: &Path) -> Option<PackageEntry> {
        self.package_entries(config_dir, home_dir)
            .into_iter()
            .find(|entry| entry.name == name)
    }

    /// Ignores that apply to every package: `global.ignore`.
    pub fn ignores(&self) -> Vec<String> {
        self.global_ignore()
    }

    pub fn set_packages(&mut self, packages: Option<String>) {
        self.packages = packages;
    }

    /// Validates that the packages folder exists and that the target is not the packages
    /// folder or inside it.
    pub fn validate_sources(&self, config_dir: &Path, home_dir: &Path) -> Result<()> {
        if let Some(packages) = self.packages_dir(config_dir, home_dir) {
            if !packages.exists() {
                return Err(Error::SourceNotFound(packages));
            }
            let target = self.target_dir(config_dir, home_dir);
            if same_or_inside(&target, &packages) {
                return Err(Error::InvalidConfig(format!(
                    "target {} is inside the packages folder {}",
                    target.display(),
                    packages.display()
                )));
            }
        }
        Ok(())
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

    pub fn set_icons(&mut self, icons: Option<String>) {
        self.global.icons = icons;
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
}

/// `target` is `packages` or lies under it. Both sides are resolved through symlinks, so
/// a path that reaches the packages folder by another route still matches.
fn same_or_inside(target: &Path, packages: &Path) -> bool {
    resolve_through_symlinks(target).starts_with(resolve_through_symlinks(packages))
}

/// Resolves `path` to where it points. A path that exists is canonicalized. For a missing
/// path (a target not created yet), the nearest existing ancestor is canonicalized and the
/// missing tail is appended, after `.` and `..` are applied lexically.
fn resolve_through_symlinks(path: &Path) -> PathBuf {
    if let Ok(canonical) = std::fs::canonicalize(path) {
        return canonical;
    }
    let mut normal = PathBuf::new();
    for component in path.components() {
        match component {
            Component::ParentDir => {
                normal.pop();
            }
            Component::CurDir => {}
            other => normal.push(other),
        }
    }
    let mut tail = Vec::new();
    let mut head = normal.as_path();
    loop {
        if let Ok(canonical) = std::fs::canonicalize(head) {
            return tail
                .iter()
                .rev()
                .fold(canonical, |acc, part| acc.join(part));
        }
        match (head.parent(), head.file_name()) {
            (Some(parent), Some(name)) => {
                tail.push(name);
                head = parent;
            }
            _ => return normal,
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
packages: packages
target: target
global:
  stow:
    verbosity: 2
    dry_run: true
"#,
        )
        .unwrap();

        let config = Config::load(&config_path).unwrap();
        assert_eq!(config.stow_flags().verbosity, 2);
        assert!(config.stow_flags().dry_run);
        assert_eq!(config.packages.as_deref(), Some("packages"));
        assert_eq!(config.target.as_deref(), Some("target"));
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
packages: packages
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
packages: packages
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
packages: packages
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
packages: packages
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
packages: packages
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
packages: packages
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
packages: dotfiles
target: "~"
global:
  theme: dracula
  ignore:
    - .git
    - .DS_Store
  stow:
    verbosity: 2
    dry_run: true
    action: restow
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
        assert_eq!(reloaded.packages.as_deref(), Some("dotfiles"));
        assert_eq!(reloaded.target.as_deref(), Some("~"));
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
packages: packages
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
    fn test_add_remove_global_ignore() {
        let tmp = TempDir::new().unwrap();
        let config_path = tmp.path().join("andre.yml");

        fs::write(
            &config_path,
            r#"
global: {}
packages: packages
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
    fn test_ensure_stow_creates_on_first_mutation() {
        let tmp = TempDir::new().unwrap();
        let config_path = tmp.path().join("andre.yml");

        fs::write(
            &config_path,
            r#"
global: {}
packages: packages
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
    fn test_validate_clean_config_returns_no_warnings() {
        let config = Config {
            packages: Some("packages".into()),
            target: None,
            global: GlobalSettings {
                engine: None,
                theme: Some("nord".to_string()),
                icons: None,
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
            packages: Some("packages".into()),
            target: None,
            global: GlobalSettings {
                engine: None,
                theme: Some("nonexistent-theme".to_string()),
                icons: None,
                ignore: None,
                stow: None,
            },
        };
        let warnings = config.validate();
        assert_eq!(warnings.len(), 1);
        assert!(matches!(&warnings[0], ConfigWarning::UnknownTheme(t) if t == "nonexistent-theme"));
    }

    #[test]
    fn test_validate_unknown_action_returns_warning() {
        let config = Config {
            packages: Some("packages".into()),
            target: None,
            global: GlobalSettings {
                engine: None,
                theme: None,
                icons: None,
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
        };
        let warnings = config.validate();
        assert_eq!(warnings.len(), 1);
        assert!(matches!(&warnings[0], ConfigWarning::UnknownAction(a) if a == "invalid-action"));
    }

    #[test]
    fn test_validate_verbosity_out_of_range_returns_warning() {
        let config = Config {
            packages: Some("packages".into()),
            target: None,
            global: GlobalSettings {
                engine: None,
                theme: None,
                icons: None,
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
        };
        let warnings = config.validate();
        assert_eq!(warnings.len(), 1);
        assert!(matches!(&warnings[0], ConfigWarning::VerbosityOutOfRange(v) if v == &99));
    }

    #[test]
    fn test_validate_no_packages_folder_returns_warning() {
        let config = Config {
            packages: None,
            target: None,
            global: GlobalSettings {
                engine: None,
                theme: None,
                icons: None,
                ignore: None,
                stow: None,
            },
        };
        let warnings = config.validate();
        assert_eq!(warnings.len(), 1);
        assert_eq!(warnings[0], ConfigWarning::NoPackagesFolder);
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
packages: packages
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
            "global:\n  engine: STOW\n  stow:\n    verbosity: 0\npackages: packages\n",
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
            packages: None,
            target: None,
            global: GlobalSettings {
                engine: Some("ruby".into()),
                theme: None,
                icons: None,
                ignore: None,
                stow: None,
            },
        };
        let warnings = config.validate();
        assert!(
            matches!(&warnings[..], [ConfigWarning::NoPackagesFolder, ConfigWarning::UnknownEngine(e)] if e == "ruby")
        );
        assert_eq!(config.effective_engine(), EngineKind::Native);
    }

    #[test]
    fn test_icons_unknown_value_warns() {
        let config = Config {
            packages: None,
            target: None,
            global: GlobalSettings {
                engine: None,
                theme: None,
                icons: Some("emoji".into()),
                ignore: None,
                stow: None,
            },
        };
        let warnings = config.validate();
        assert!(
            matches!(&warnings[..], [ConfigWarning::NoPackagesFolder, ConfigWarning::UnknownIcons(i)] if i == "emoji")
        );
    }

    #[test]
    fn test_icons_round_trips_through_yaml() {
        let tmp = tempfile::TempDir::new().unwrap();
        let path = tmp.path().join("andre.yml");
        std::fs::write(
            &path,
            "global:\n  icons: unicode\n  stow:\n    verbosity: 0\npackages: packages\n",
        )
        .unwrap();
        let config = Config::load(&path).unwrap();
        assert!(config
            .validate()
            .iter()
            .all(|w| !matches!(w, ConfigWarning::UnknownIcons(_))));

        let path2 = tmp.path().join("andre2.yml");
        config.save(&path2).unwrap();
        let reloaded = Config::load(&path2).unwrap();
        assert_eq!(reloaded.global.icons.as_deref(), Some("unicode"));
    }

    #[test]
    fn test_flat_config_round_trips_in_key_order() {
        let tmp = TempDir::new().unwrap();
        let config_path = tmp.path().join("andre.yml");
        fs::write(
            &config_path,
            "packages: ~/src/dotfiles\ntarget: \"~\"\nglobal:\n  ignore: [.DS_Store]\n",
        )
        .unwrap();

        let config = Config::load(&config_path).unwrap();
        assert_eq!(config.packages.as_deref(), Some("~/src/dotfiles"));
        assert_eq!(config.target.as_deref(), Some("~"));

        let yaml = serde_yaml_ng::to_string(&config).unwrap();
        let packages = yaml.find("packages:").unwrap();
        let target = yaml.find("target:").unwrap();
        let global = yaml.find("global:").unwrap();
        assert!(packages < target && target < global, "{yaml}");

        let reloaded: Config = serde_yaml_ng::from_str(&yaml).unwrap();
        assert_eq!(reloaded.packages, config.packages);
        assert_eq!(reloaded.target, config.target);
        assert_eq!(reloaded.global_ignore(), [".DS_Store"]);
    }

    #[test]
    fn test_global_may_be_omitted() {
        let tmp = TempDir::new().unwrap();
        let config_path = tmp.path().join("andre.yml");
        fs::write(&config_path, "packages: dotfiles\n").unwrap();

        let config = Config::load(&config_path).unwrap();
        assert!(config.global_ignore().is_empty());
        assert_eq!(config.stow_flags(), StowFlags::default());
    }

    #[test]
    fn test_target_defaults_to_home_and_resolves_relative_to_config() {
        let config_dir = Path::new("/cfg");
        let home = Path::new("/home/u");
        let mut config = Config::default_empty();
        assert_eq!(
            config.target_dir(config_dir, home),
            PathBuf::from("/home/u")
        );

        config.target = Some("out".into());
        assert_eq!(
            config.target_dir(config_dir, home),
            PathBuf::from("/cfg/out")
        );

        config.target = Some("~/links".into());
        assert_eq!(
            config.target_dir(config_dir, home),
            PathBuf::from("/home/u/links")
        );
    }

    #[test]
    fn test_package_entries_lists_flat_packages_in_name_order() {
        let tmp = TempDir::new().unwrap();
        let config_dir = tmp.path();
        let home = tmp.path().join("home");
        let packages = config_dir.join("dotfiles");
        fs::create_dir_all(packages.join("nvim")).unwrap();
        fs::create_dir_all(packages.join("git")).unwrap();
        fs::create_dir_all(packages.join(".git")).unwrap();
        fs::write(packages.join("README.md"), "").unwrap();

        let config: Config =
            serde_yaml_ng::from_str("packages: dotfiles\nglobal:\n  ignore: [.DS_Store]\n")
                .unwrap();
        let entries = config.package_entries(config_dir, &home);
        let names: Vec<&str> = entries.iter().map(|e| e.name.as_str()).collect();
        assert_eq!(names, ["git", "nvim"]);

        let git = &entries[0];
        assert_eq!(git.dir, packages.join("git"));
        assert_eq!(git.source, packages);
        assert_eq!(git.target, home);
        assert_eq!(git.ignores, [".DS_Store"]);

        assert_eq!(
            config.package("nvim", config_dir, &home).unwrap().name,
            "nvim"
        );
        assert_eq!(config.package(".git", config_dir, &home), None);
        assert_eq!(config.packages_dir(config_dir, &home), Some(packages));
    }

    #[test]
    fn test_demo_config_links_inside_the_demo_folder() {
        // `make run` loads demo/andre.yml: its packages and links must stay in the demo
        // folder, never in the real home.
        let demo = fs::canonicalize(Path::new(env!("CARGO_MANIFEST_DIR")).join("../demo")).unwrap();
        let config = Config::load(&demo.join("andre.yml")).unwrap();
        let home = Path::new("/nonexistent-home");

        assert!(config.packages_dir(&demo, home).unwrap().starts_with(&demo));
        assert!(config.target_dir(&demo, home).starts_with(&demo));
    }

    #[test]
    fn test_package_entries_empty_without_packages_folder() {
        let tmp = TempDir::new().unwrap();
        let config = Config::default_empty();
        assert!(config.package_entries(tmp.path(), tmp.path()).is_empty());
        assert_eq!(config.packages_dir(tmp.path(), tmp.path()), None);
    }

    #[test]
    fn test_load_refuses_the_removed_groups_key_with_the_migration_text() {
        let tmp = TempDir::new().unwrap();
        let config_path = tmp.path().join("andre.yml");
        fs::write(
            &config_path,
            "groups:\n  home:\n    source: packages\n    target: \"~\"\n",
        )
        .unwrap();

        let err = Config::load(&config_path).unwrap_err();
        assert!(matches!(err, Error::InvalidConfig(_)));
        let text = err.to_string();
        assert!(
            text.starts_with("`groups:` is no longer supported."),
            "{text}"
        );
        assert!(
            text.contains("packages:") && text.contains("target:"),
            "{text}"
        );
        assert!(text.contains("plain package names"), "{text}");
    }

    #[test]
    fn test_validate_sources_accepts_packages_inside_default_target() {
        let tmp = TempDir::new().unwrap();
        let home = tmp.path().join("home");
        fs::create_dir_all(home.join("src/dotfiles/nvim")).unwrap();

        // Packages inside the home folder, linked to the home folder: the normal case.
        let mut config = Config::default_empty();
        config.set_packages(Some("~/src/dotfiles".into()));
        assert!(config.validate_sources(tmp.path(), &home).is_ok());
    }

    #[test]
    fn test_validate_sources_rejects_target_equal_to_or_inside_packages() {
        let tmp = TempDir::new().unwrap();
        let home = tmp.path().join("home");
        fs::create_dir_all(home.join("src/dotfiles/nvim")).unwrap();

        let mut config = Config::default_empty();
        config.set_packages(Some("~/src/dotfiles".into()));
        config.target = Some("~/src/dotfiles".into());
        let err = config.validate_sources(tmp.path(), &home).unwrap_err();
        assert_eq!(
            err.to_string(),
            format!(
                "target {} is inside the packages folder {}",
                home.join("src/dotfiles").display(),
                home.join("src/dotfiles").display()
            )
        );

        config.target = Some("~/src/dotfiles/out".into());
        let err = config.validate_sources(tmp.path(), &home).unwrap_err();
        assert!(matches!(err, Error::InvalidConfig(_)));
    }

    #[cfg(unix)]
    #[test]
    fn test_validate_sources_rejects_missing_target_inside_packages_through_symlink() {
        let tmp = TempDir::new().unwrap();
        let real_home = tmp.path().join("real_home");
        fs::create_dir_all(real_home.join("dots/nvim")).unwrap();
        // `home` reaches the same folder through a symlink, as `/home` -> `/var/home` does.
        let home = tmp.path().join("home");
        std::os::unix::fs::symlink(&real_home, &home).unwrap();

        let mut config = Config::default_empty();
        config.set_packages(Some("~/dots".into()));

        // The target does not exist yet, so its path stays lexical past the last existing folder.
        config.target = Some("~/dots/out".into());
        let err = config.validate_sources(tmp.path(), &home).unwrap_err();
        assert!(matches!(err, Error::InvalidConfig(_)));

        // `..` in the missing tail is applied before the comparison.
        config.target = Some("~/dots/missing/../out".into());
        let err = config.validate_sources(tmp.path(), &home).unwrap_err();
        assert!(matches!(err, Error::InvalidConfig(_)));

        // A missing target outside the packages folder is still accepted.
        config.target = Some("~/elsewhere/out".into());
        assert!(config.validate_sources(tmp.path(), &home).is_ok());
    }

    #[test]
    fn test_validate_sources_reports_missing_packages_folder() {
        let tmp = TempDir::new().unwrap();
        let mut config = Config::default_empty();
        config.set_packages(Some("nowhere".into()));

        let err = config
            .validate_sources(tmp.path(), &tmp.path().join("home"))
            .unwrap_err();
        assert!(matches!(&err, Error::SourceNotFound(p) if *p == tmp.path().join("nowhere")));
        assert_eq!(
            err.to_string(),
            format!(
                "Packages folder not found: {}",
                tmp.path().join("nowhere").display()
            )
        );
    }

    #[test]
    fn test_validate_packages_folder_set_has_no_warning() {
        let mut config = Config::default_empty();
        assert_eq!(config.validate(), [ConfigWarning::NoPackagesFolder]);
        assert_eq!(
            ConfigWarning::NoPackagesFolder.to_string(),
            "no packages folder: set `packages:` in andre.yml"
        );

        config.set_packages(Some("dotfiles".into()));
        assert!(config.validate().is_empty());
    }
}
