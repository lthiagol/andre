use andre_core::{AppState, Config, IconSet, StowFlags, Theme};

pub struct PendingSettings {
    pub config: Config,
    pub flags: StowFlags,
    pub theme: Theme,
    pub icons: IconSet,
}

impl PendingSettings {
    pub fn from_core(core: &AppState) -> Self {
        Self {
            config: core.config.clone(),
            flags: core.flags,
            theme: core.theme,
            icons: core.icons,
        }
    }

    /// The config as it should be written to disk: the groups plus every pending setting.
    pub fn to_config(&self) -> Config {
        let mut config = self.config.clone();
        config.set_stow_flags(self.flags);
        config.set_theme(Some(self.theme.name().to_string()));
        config.set_icons(Some(self.icons.name().to_string()));
        config
    }

    pub fn apply_to(&self, core: &mut AppState) {
        core.config = self.to_config();
        core.flags = self.flags;
        core.theme = self.theme;
        core.icons = self.icons;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use andre_core::StowAction;

    fn pending_with(theme: Theme) -> PendingSettings {
        PendingSettings {
            config: Config::default_empty(),
            flags: StowFlags {
                verbosity: 3,
                dry_run: true,
                adopt: true,
                action: StowAction::Restow,
                ..StowFlags::default()
            },
            theme,
            icons: IconSet::Unicode,
        }
    }

    #[test]
    fn test_to_config_writes_every_pending_setting() {
        let config = pending_with(Theme::CatppuccinLatte).to_config();
        let flags = config.stow_flags();
        assert_eq!(flags.verbosity, 3);
        assert!(flags.dry_run);
        assert!(!flags.no_folding);
        assert!(flags.adopt);
        assert!(!flags.dotfiles);
        assert_eq!(flags.action, StowAction::Restow);
        assert_eq!(config.global.theme.as_deref(), Some("catppuccin-latte"));
    }

    #[test]
    fn test_to_config_persists_default_theme_explicitly() {
        let config = pending_with(Theme::Default).to_config();
        assert_eq!(config.global.theme.as_deref(), Some("default"));
    }

    #[test]
    fn test_to_config_persists_icons_explicitly() {
        let config = pending_with(Theme::Default).to_config();
        assert_eq!(config.global.icons.as_deref(), Some("unicode"));
    }

    #[test]
    fn test_apply_to_commits_icons_with_config() {
        let mut core = AppState::from_config(Config::default_empty());
        assert_eq!(core.icons, IconSet::Nerd);
        pending_with(Theme::Nord).apply_to(&mut core);
        assert_eq!(core.icons, IconSet::Unicode);
        assert_eq!(core.config.global.icons.as_deref(), Some("unicode"));
    }

    #[test]
    fn test_apply_to_commits_theme_and_config_together() {
        let mut core = AppState::from_config(Config::default_empty());
        pending_with(Theme::Nord).apply_to(&mut core);
        assert_eq!(core.theme, Theme::Nord);
        assert_eq!(core.config.global.theme.as_deref(), Some("nord"));
        assert_eq!(core.flags.verbosity, 3);
        assert_eq!(core.flags.action, StowAction::Restow);
    }
}
