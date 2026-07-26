use crate::config::Config;

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
    pub fn next(self) -> Self {
        match self {
            Theme::Default => Theme::Dracula,
            Theme::Dracula => Theme::CatppuccinMocha,
            Theme::CatppuccinMocha => Theme::CatppuccinLatte,
            Theme::CatppuccinLatte => Theme::CatppuccinFrappe,
            Theme::CatppuccinFrappe => Theme::CatppuccinMacchiato,
            Theme::CatppuccinMacchiato => Theme::Nord,
            Theme::Nord => Theme::Gruvbox,
            Theme::Gruvbox => Theme::Default,
        }
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

    #[allow(clippy::should_implement_trait)]
    pub fn from_str(s: &str) -> Option<Self> {
        if s.eq_ignore_ascii_case("default") {
            return Some(Theme::Default);
        }
        if s.eq_ignore_ascii_case("dracula") {
            return Some(Theme::Dracula);
        }
        if s.eq_ignore_ascii_case("catppuccin-mocha") || s.eq_ignore_ascii_case("catppuccin_mocha")
        {
            return Some(Theme::CatppuccinMocha);
        }
        if s.eq_ignore_ascii_case("catppuccin-latte") || s.eq_ignore_ascii_case("catppuccin_latte")
        {
            return Some(Theme::CatppuccinLatte);
        }
        if s.eq_ignore_ascii_case("catppuccin-frappe")
            || s.eq_ignore_ascii_case("catppuccin_frappe")
        {
            return Some(Theme::CatppuccinFrappe);
        }
        if s.eq_ignore_ascii_case("catppuccin-macchiato")
            || s.eq_ignore_ascii_case("catppuccin_macchiato")
        {
            return Some(Theme::CatppuccinMacchiato);
        }
        if s.eq_ignore_ascii_case("nord") {
            return Some(Theme::Nord);
        }
        if s.eq_ignore_ascii_case("gruvbox") {
            return Some(Theme::Gruvbox);
        }
        None
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StowAction {
    Stow,
    Unstow,
    Restow,
}

impl StowAction {
    pub fn as_str(&self) -> &'static str {
        match self {
            StowAction::Stow => "stow",
            StowAction::Unstow => "unstow",
            StowAction::Restow => "restow",
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

#[derive(Clone)]
pub struct AppState {
    pub config: Config,
    pub verbosity: u8,
    pub dry_run: bool,
    pub no_folding: bool,
    pub adopt: bool,
    pub dotfiles: bool,
    pub action: StowAction,
    pub theme: Theme,
    pub yolo_mode: bool,
}

impl AppState {
    pub fn from_config(config: Config, yolo_mode: bool) -> Self {
        let verbosity = config.effective_verbosity();
        let dry_run = config.effective_dry_run();
        let no_folding = config.effective_no_folding();
        let adopt = config.effective_adopt();
        let dotfiles = config.effective_dotfiles();

        let action_str = config.effective_action();
        let action = StowAction::from_str(&action_str).unwrap_or(StowAction::Stow);

        let theme_name = config
            .global
            .theme
            .as_deref()
            .and_then(Theme::from_str)
            .unwrap_or_default();

        Self {
            config,
            verbosity,
            dry_run,
            no_folding,
            adopt,
            dotfiles,
            action,
            theme: theme_name,
            yolo_mode,
        }
    }

    pub fn effective_action(&self) -> String {
        self.action.as_str().to_string()
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
groups:
  main:
    source: packages
    target: target
"#,
        )
        .unwrap();

        let config = Config::load(&config_path).unwrap();
        let state = AppState::from_config(config, false);

        assert_eq!(state.verbosity, 2);
        assert!(state.dry_run);
        assert!(state.no_folding);
        assert!(!state.adopt);
        assert!(!state.dotfiles);
        assert_eq!(state.action, StowAction::Restow);
        assert_eq!(state.theme, Theme::Dracula);
        assert!(!state.yolo_mode);
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
groups:
  main:
    source: packages
    target: target
"#,
        )
        .unwrap();

        let config = Config::load(&config_path).unwrap();
        let state = AppState::from_config(config, false);

        assert_eq!(state.verbosity, 0);
        assert!(!state.dry_run);
        assert!(!state.no_folding);
        assert!(!state.adopt);
        assert!(!state.dotfiles);
        assert_eq!(state.action, StowAction::Stow);
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
    fn test_app_state_effective_action() {
        let tmp = TempDir::new().unwrap();
        let config_path = tmp.path().join("andre.yml");

        fs::write(
            &config_path,
            r#"
global:
  stow:
    action: unstow
groups:
  main:
    source: packages
    target: target
"#,
        )
        .unwrap();

        let config = Config::load(&config_path).unwrap();
        let state = AppState::from_config(config, false);
        assert_eq!(state.effective_action(), "unstow");
    }
}
