use andre_core::{Config, StowFlag};

/// Rows in the settings list, in display order.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SettingsRow {
    ConfigPathInfo,
    ConfigResolveInfo,
    Verbosity,
    DryRun,
    NoFolding,
    Adopt,
    Dotfiles,
    Action,
    ThemeName,
    IconsName,
    IgnoreLine { count: usize },
    Spacer,
    GroupLine { count: usize },
    SaveButton,
    SavedMessage,
}

/// The stow option a settings row edits, if the row is a flag row.
pub fn flag_at_row(row: usize) -> Option<StowFlag> {
    match row {
        ROW_VERBOSITY => Some(StowFlag::Verbosity),
        ROW_DRY_RUN => Some(StowFlag::DryRun),
        ROW_NO_FOLDING => Some(StowFlag::NoFolding),
        ROW_ADOPT => Some(StowFlag::Adopt),
        ROW_DOTFILES => Some(StowFlag::Dotfiles),
        _ => None,
    }
}

/// Row index constants — used by mod.rs for cursor-based key dispatch.
pub const ROW_CONFIG_PATH: usize = 0;
pub const ROW_CONFIG_RESOLVE: usize = 1;
pub const ROW_VERBOSITY: usize = 2;
pub const ROW_DRY_RUN: usize = 3;
pub const ROW_NO_FOLDING: usize = 4;
pub const ROW_ADOPT: usize = 5;
pub const ROW_DOTFILES: usize = 6;
pub const ROW_ACTION: usize = 7;
pub const ROW_THEME: usize = 8;
pub const ROW_ICONS: usize = 9;
pub const ROW_IGNORE: usize = 10;
pub const ROW_SPACER: usize = 11;
pub const ROW_GROUP_LINE: usize = 12;
pub const ROW_SAVE_BUTTON: usize = 13;

/// Number of non-message rows (before optional SavedMessage).
pub const ROW_COUNT_BASE: usize = 14;

/// Build the flat list of settings rows, optionally appending the saved message.
pub fn build_flat_items(config: &Config) -> Vec<SettingsRow> {
    vec![
        SettingsRow::ConfigPathInfo,
        SettingsRow::ConfigResolveInfo,
        SettingsRow::Verbosity,
        SettingsRow::DryRun,
        SettingsRow::NoFolding,
        SettingsRow::Adopt,
        SettingsRow::Dotfiles,
        SettingsRow::Action,
        SettingsRow::ThemeName,
        SettingsRow::IconsName,
        SettingsRow::IgnoreLine {
            count: config.global_ignore().len(),
        },
        SettingsRow::Spacer,
        SettingsRow::GroupLine {
            count: config.groups.len(),
        },
        SettingsRow::SaveButton,
    ]
}

/// Total row count including optional saved-message row.
pub fn row_count(show_saved_message: bool) -> usize {
    if show_saved_message {
        ROW_COUNT_BASE + 1
    } else {
        ROW_COUNT_BASE
    }
}

/// Returns true if the row is interactive (navigable / actionable).
pub fn is_interactive_row(index: usize, show_saved_message: bool) -> bool {
    // ConfigPathInfo and ConfigResolveInfo are display-only
    if index < ROW_VERBOSITY {
        return false;
    }
    // Spacer row
    if index == ROW_SPACER {
        return false;
    }
    // SavedMessage row
    if show_saved_message && index == ROW_COUNT_BASE {
        return false;
    }
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_settings_row_layout_sync() {
        let config = andre_core::Config::default_empty();
        let base = build_flat_items(&config);
        assert_eq!(base.len(), ROW_COUNT_BASE);
        assert_eq!(row_count(false), ROW_COUNT_BASE);
        assert_eq!(row_count(true), ROW_COUNT_BASE + 1);

        // Each ROW_* constant must index the variant it is named for. Swapping two
        // constants keeps the length and every other check green, so pin them all.
        assert_eq!(base[ROW_CONFIG_PATH], SettingsRow::ConfigPathInfo);
        assert_eq!(base[ROW_CONFIG_RESOLVE], SettingsRow::ConfigResolveInfo);
        assert_eq!(base[ROW_VERBOSITY], SettingsRow::Verbosity);
        assert_eq!(base[ROW_DRY_RUN], SettingsRow::DryRun);
        assert_eq!(base[ROW_NO_FOLDING], SettingsRow::NoFolding);
        assert_eq!(base[ROW_ADOPT], SettingsRow::Adopt);
        assert_eq!(base[ROW_DOTFILES], SettingsRow::Dotfiles);
        assert_eq!(base[ROW_ACTION], SettingsRow::Action);
        assert_eq!(base[ROW_THEME], SettingsRow::ThemeName);
        assert_eq!(base[ROW_ICONS], SettingsRow::IconsName);
        assert!(matches!(base[ROW_IGNORE], SettingsRow::IgnoreLine { .. }));
        assert_eq!(base[ROW_SPACER], SettingsRow::Spacer);
        assert!(matches!(
            base[ROW_GROUP_LINE],
            SettingsRow::GroupLine { .. }
        ));
        assert_eq!(base[ROW_SAVE_BUTTON], SettingsRow::SaveButton);
    }

    #[test]
    fn test_flag_at_row_maps_each_flag_row() {
        assert_eq!(flag_at_row(ROW_VERBOSITY), Some(StowFlag::Verbosity));
        assert_eq!(flag_at_row(ROW_DRY_RUN), Some(StowFlag::DryRun));
        assert_eq!(flag_at_row(ROW_NO_FOLDING), Some(StowFlag::NoFolding));
        assert_eq!(flag_at_row(ROW_ADOPT), Some(StowFlag::Adopt));
        assert_eq!(flag_at_row(ROW_DOTFILES), Some(StowFlag::Dotfiles));
    }

    #[test]
    fn test_flag_at_row_is_none_for_non_flag_rows() {
        assert_eq!(flag_at_row(ROW_CONFIG_PATH), None);
        assert_eq!(flag_at_row(ROW_CONFIG_RESOLVE), None);
        assert_eq!(flag_at_row(ROW_ACTION), None);
        assert_eq!(flag_at_row(ROW_THEME), None);
        assert_eq!(flag_at_row(ROW_ICONS), None);
        assert_eq!(flag_at_row(ROW_IGNORE), None);
        assert_eq!(flag_at_row(ROW_SPACER), None);
        assert_eq!(flag_at_row(ROW_GROUP_LINE), None);
        assert_eq!(flag_at_row(ROW_SAVE_BUTTON), None);
    }
}
