use crate::config::Config;
use crate::profile::ProfileSet;

/// Highest verbosity level; `v` and the Settings verbosity row wrap back to 0 after it.
pub const MAX_VERBOSITY: u8 = 5;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Theme {
    #[default]
    Default,
    Dracula,
    CatppuccinMocha,
    CatppuccinLatte,
    CatppuccinFrappe,
    CatppuccinMacchiato,
    Nord,
    Gruvbox,
}

impl Theme {
    /// Every theme, in the order the Settings picker and `t` key cycle through them.
    pub const ALL: [Theme; 8] = [
        Theme::Default,
        Theme::Dracula,
        Theme::CatppuccinMocha,
        Theme::CatppuccinLatte,
        Theme::CatppuccinFrappe,
        Theme::CatppuccinMacchiato,
        Theme::Nord,
        Theme::Gruvbox,
    ];

    pub fn next(self) -> Self {
        let index = Self::ALL.iter().position(|t| *t == self).unwrap_or(0);
        Self::ALL[(index + 1) % Self::ALL.len()]
    }

    pub fn name(&self) -> &'static str {
        match self {
            Theme::Default => "default",
            Theme::Dracula => "dracula",
            Theme::CatppuccinMocha => "catppuccin-mocha",
            Theme::CatppuccinLatte => "catppuccin-latte",
            Theme::CatppuccinFrappe => "catppuccin-frappe",
            Theme::CatppuccinMacchiato => "catppuccin-macchiato",
            Theme::Nord => "nord",
            Theme::Gruvbox => "gruvbox",
        }
    }

    /// Case-insensitive lookup by `name()`. `_` is accepted in place of `-`.
    #[allow(clippy::should_implement_trait)]
    pub fn from_str(s: &str) -> Option<Self> {
        let normalised = s.replace('_', "-");
        Self::ALL
            .into_iter()
            .find(|theme| theme.name().eq_ignore_ascii_case(&normalised))
    }
}

/// Glyph family for the TUI's icons. `Nerd` needs a Nerd Font in the terminal;
/// `Unicode` uses single-width symbols that any font renders.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum IconSet {
    #[default]
    Nerd,
    Unicode,
}

impl IconSet {
    /// Every icon set, in the order the Settings row cycles through them.
    pub const ALL: [IconSet; 2] = [IconSet::Nerd, IconSet::Unicode];

    pub fn next(self) -> Self {
        let index = Self::ALL.iter().position(|s| *s == self).unwrap_or(0);
        Self::ALL[(index + 1) % Self::ALL.len()]
    }

    pub fn name(&self) -> &'static str {
        match self {
            IconSet::Nerd => "nerd",
            IconSet::Unicode => "unicode",
        }
    }

    /// Case-insensitive lookup by `name()`.
    #[allow(clippy::should_implement_trait)]
    pub fn from_str(s: &str) -> Option<Self> {
        Self::ALL
            .into_iter()
            .find(|set| set.name().eq_ignore_ascii_case(s))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum StowAction {
    #[default]
    Stow,
    Unstow,
    Restow,
}

impl StowAction {
    /// Every action, in the order the Settings picker lists them.
    pub const ALL: [StowAction; 3] = [StowAction::Stow, StowAction::Restow, StowAction::Unstow];

    pub fn as_str(&self) -> &'static str {
        match self {
            StowAction::Stow => "stow",
            StowAction::Unstow => "unstow",
            StowAction::Restow => "restow",
        }
    }

    /// The next action in the `s` cycle: stow, restow, unstow, then stow again.
    pub fn next(self) -> Self {
        match self {
            StowAction::Stow => StowAction::Restow,
            StowAction::Restow => StowAction::Unstow,
            StowAction::Unstow => StowAction::Stow,
        }
    }

    #[allow(clippy::should_implement_trait)]
    pub fn from_str(s: &str) -> Option<Self> {
        if s.eq_ignore_ascii_case("stow") {
            return Some(StowAction::Stow);
        }
        if s.eq_ignore_ascii_case("unstow") {
            return Some(StowAction::Unstow);
        }
        if s.eq_ignore_ascii_case("restow") {
            return Some(StowAction::Restow);
        }
        None
    }
}

/// Every stow option the TUI and the yolo path hand to an engine. One value travels
/// from config to settings to `LinkOp`, so the option set is never re-declared.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct StowFlags {
    pub action: StowAction,
    pub verbosity: u8,
    pub dry_run: bool,
    pub no_folding: bool,
    pub adopt: bool,
    pub dotfiles: bool,
}

/// One toggleable option. Each maps to a key shortcut or a Settings row.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StowFlag {
    Verbosity,
    DryRun,
    NoFolding,
    Adopt,
    Dotfiles,
}

impl StowFlags {
    /// Advance a flag: verbosity steps up and wraps past `MAX_VERBOSITY`; the rest flip.
    pub fn toggle(&mut self, flag: StowFlag) {
        match flag {
            StowFlag::Verbosity => self.verbosity = (self.verbosity + 1) % (MAX_VERBOSITY + 1),
            StowFlag::DryRun => self.dry_run = !self.dry_run,
            StowFlag::NoFolding => self.no_folding = !self.no_folding,
            StowFlag::Adopt => self.adopt = !self.adopt,
            StowFlag::Dotfiles => self.dotfiles = !self.dotfiles,
        }
    }
}

#[derive(Clone)]
pub struct AppState {
    pub config: Config,
    pub flags: StowFlags,
    pub theme: Theme,
    pub icons: IconSet,
    /// Profiles in `<config folder>/profiles/`. Empty until the config is loaded from
    /// disk (see `App::with_debug`); `from_config` alone does not read the folder.
    pub profiles: ProfileSet,
}

impl AppState {
    pub fn from_config(config: Config) -> Self {
        let flags = config.stow_flags();

        let theme_name = config
            .global
            .theme
            .as_deref()
            .and_then(Theme::from_str)
            .unwrap_or_default();

        let icons = config
            .global
            .icons
            .as_deref()
            .and_then(IconSet::from_str)
            .unwrap_or_default();

        Self {
            config,
            flags,
            theme: theme_name,
            icons,
            profiles: ProfileSet::default(),
        }
    }
}

#[cfg(test)]
mod tests {
    use std::fs;
    use tempfile::TempDir;

    use super::*;
    use crate::config::Config;

    #[test]
    fn test_theme_cycling() {
        let theme = Theme::Default;
        assert_eq!(theme.next(), Theme::Dracula);

        let theme = Theme::Gruvbox;
        assert_eq!(theme.next(), Theme::Default);
    }

    #[test]
    fn test_theme_string_conversions() {
        // names
        assert_eq!(Theme::Default.name(), "default");
        assert_eq!(Theme::Dracula.name(), "dracula");
        assert_eq!(Theme::CatppuccinMocha.name(), "catppuccin-mocha");
        assert_eq!(Theme::Nord.name(), "nord");
        assert_eq!(Theme::Gruvbox.name(), "gruvbox");
        // from_str valid
        assert_eq!(Theme::from_str("default"), Some(Theme::Default));
        assert_eq!(Theme::from_str("dracula"), Some(Theme::Dracula));
        assert_eq!(
            Theme::from_str("catppuccin-mocha"),
            Some(Theme::CatppuccinMocha)
        );
        assert_eq!(Theme::from_str("nord"), Some(Theme::Nord));
        assert_eq!(Theme::from_str("gruvbox"), Some(Theme::Gruvbox));
        // from_str invalid
        assert_eq!(Theme::from_str("unknown-theme"), None);
        assert_eq!(Theme::from_str("catppuccin"), None);
        assert_eq!(Theme::from_str(""), None);
    }

    #[test]
    fn test_theme_all_round_trips_through_name() {
        for theme in Theme::ALL {
            assert_eq!(Theme::from_str(theme.name()), Some(theme));
        }
        assert_eq!(
            Theme::from_str("catppuccin_latte"),
            Some(Theme::CatppuccinLatte)
        );
        assert_eq!(
            Theme::from_str("CATPPUCCIN-FRAPPE"),
            Some(Theme::CatppuccinFrappe)
        );
    }

    #[test]
    fn test_theme_next_visits_every_theme_once() {
        let mut theme = Theme::default();
        let mut seen = Vec::new();
        for _ in 0..Theme::ALL.len() {
            seen.push(theme);
            theme = theme.next();
        }
        assert_eq!(theme, Theme::Default);
        assert_eq!(seen, Theme::ALL);
    }

    #[test]
    fn test_stow_action_next_cycles_stow_restow_unstow() {
        assert_eq!(StowAction::Stow.next(), StowAction::Restow);
        assert_eq!(StowAction::Restow.next(), StowAction::Unstow);
        assert_eq!(StowAction::Unstow.next(), StowAction::Stow);
    }

    #[test]
    fn test_stow_action_all_round_trips_through_as_str() {
        for action in StowAction::ALL {
            assert_eq!(StowAction::from_str(action.as_str()), Some(action));
        }
    }

    #[test]
    fn test_app_state_from_config() {
        let tmp = TempDir::new().unwrap();
        let config_path = tmp.path().join("andre.yml");

        fs::write(
            &config_path,
            r#"
global:
  theme: dracula
  stow:
    verbosity: 2
    dry_run: true
    no_folding: true
    adopt: false
    action: restow
packages: packages
target: target
"#,
        )
        .unwrap();

        let config = Config::load(&config_path).unwrap();
        let state = AppState::from_config(config);

        assert_eq!(state.flags.verbosity, 2);
        assert!(state.flags.dry_run);
        assert!(state.flags.no_folding);
        assert!(!state.flags.adopt);
        assert!(!state.flags.dotfiles);
        assert_eq!(state.flags.action, StowAction::Restow);
        assert_eq!(state.theme, Theme::Dracula);
    }

    #[test]
    fn test_app_state_defaults() {
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
        let state = AppState::from_config(config);

        assert_eq!(state.flags, StowFlags::default());
        assert_eq!(state.theme, Theme::Default);
    }

    #[test]
    fn test_theme_from_unknown_string() {
        assert_eq!(Theme::from_str("unknown-theme"), None);
        assert_eq!(Theme::from_str("catppuccin"), None);
        assert_eq!(Theme::from_str(""), None);
    }

    #[test]
    fn test_theme_full_cycle() {
        let themes = [
            Theme::Default,
            Theme::Dracula,
            Theme::CatppuccinMocha,
            Theme::CatppuccinLatte,
            Theme::CatppuccinFrappe,
            Theme::CatppuccinMacchiato,
            Theme::Nord,
            Theme::Gruvbox,
        ];

        let mut current = Theme::Default;
        for expected in &themes {
            assert_eq!(current, *expected);
            current = current.next();
        }
        assert_eq!(current, Theme::Default);
    }

    #[test]
    fn test_stow_action_strings() {
        // from_str
        assert_eq!(StowAction::from_str("stow"), Some(StowAction::Stow));
        assert_eq!(StowAction::from_str("unstow"), Some(StowAction::Unstow));
        assert_eq!(StowAction::from_str("restow"), Some(StowAction::Restow));
        assert_eq!(StowAction::from_str("invalid"), None);
        assert_eq!(StowAction::from_str("STOW"), Some(StowAction::Stow));
        assert_eq!(StowAction::from_str(""), None);
        // as_str
        assert_eq!(StowAction::Stow.as_str(), "stow");
        assert_eq!(StowAction::Unstow.as_str(), "unstow");
        assert_eq!(StowAction::Restow.as_str(), "restow");
    }

    #[test]
    fn test_app_state_stow_action() {
        let tmp = TempDir::new().unwrap();
        let config_path = tmp.path().join("andre.yml");

        fs::write(
            &config_path,
            r#"
global:
  stow:
    action: unstow
packages: packages
target: target
"#,
        )
        .unwrap();

        let config = Config::load(&config_path).unwrap();
        let state = AppState::from_config(config);
        assert_eq!(state.flags.action, StowAction::Unstow);
    }

    #[test]
    fn test_stow_flag_toggle_flips_booleans_and_wraps_verbosity() {
        let mut flags = StowFlags::default();
        flags.toggle(StowFlag::DryRun);
        flags.toggle(StowFlag::NoFolding);
        flags.toggle(StowFlag::Adopt);
        flags.toggle(StowFlag::Dotfiles);
        assert!(flags.dry_run && flags.no_folding && flags.adopt && flags.dotfiles);
        flags.toggle(StowFlag::DryRun);
        assert!(!flags.dry_run);

        for expected in 1..=MAX_VERBOSITY {
            flags.toggle(StowFlag::Verbosity);
            assert_eq!(flags.verbosity, expected);
        }
        flags.toggle(StowFlag::Verbosity);
        assert_eq!(
            flags.verbosity, 0,
            "verbosity wraps to 0 after MAX_VERBOSITY"
        );
    }

    #[test]
    fn test_stow_flags_default_is_stow_with_everything_off() {
        assert_eq!(
            StowFlags::default(),
            StowFlags {
                action: StowAction::Stow,
                verbosity: 0,
                dry_run: false,
                no_folding: false,
                adopt: false,
                dotfiles: false,
            }
        );
    }

    #[test]
    fn test_icons_default_is_nerd() {
        assert_eq!(IconSet::default(), IconSet::Nerd);
        assert_eq!(IconSet::default().name(), "nerd");
    }

    #[test]
    fn test_icons_string_conversions_are_case_insensitive() {
        assert_eq!(IconSet::from_str("nerd"), Some(IconSet::Nerd));
        assert_eq!(IconSet::from_str("UNICODE"), Some(IconSet::Unicode));
        assert_eq!(IconSet::from_str("emoji"), None);
        assert_eq!(IconSet::from_str(""), None);
    }

    #[test]
    fn test_icons_all_round_trips_and_next_cycles() {
        for set in IconSet::ALL {
            assert_eq!(IconSet::from_str(set.name()), Some(set));
        }
        assert_eq!(IconSet::Nerd.next(), IconSet::Unicode);
        assert_eq!(IconSet::Unicode.next(), IconSet::Nerd);
    }

    #[test]
    fn test_app_state_reads_icons_from_config() {
        let mut config = Config::default_empty();
        assert_eq!(AppState::from_config(config.clone()).icons, IconSet::Nerd);

        config.set_icons(Some("unicode".to_string()));
        assert_eq!(
            AppState::from_config(config.clone()).icons,
            IconSet::Unicode
        );

        config.set_icons(Some("bogus".to_string()));
        assert_eq!(AppState::from_config(config).icons, IconSet::Nerd);
    }
}
