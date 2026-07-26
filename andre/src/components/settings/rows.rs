use andre_core::Config;

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
    IgnoreLine { count: usize },
    Spacer,
    GroupLine { count: usize },
    SaveButton,
    SavedMessage,
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
pub const ROW_IGNORE: usize = 9;
pub const ROW_SPACER: usize = 10;
pub const ROW_GROUP_LINE: usize = 11;
pub const ROW_SAVE_BUTTON: usize = 12;

/// Number of non-message rows (before optional SavedMessage).
pub const ROW_COUNT_BASE: usize = 13;

/// Build the flat list of settings rows, optionally appending the saved message.
pub fn build_flat_items(config: &Config) -> Vec<SettingsRow> {
    let rows = vec![
        SettingsRow::ConfigPathInfo,
        SettingsRow::ConfigResolveInfo,
        SettingsRow::Verbosity,
        SettingsRow::DryRun,
        SettingsRow::NoFolding,
        SettingsRow::Adopt,
        SettingsRow::Dotfiles,
        SettingsRow::Action,
        SettingsRow::ThemeName,
        SettingsRow::IgnoreLine {
            count: config.global_ignore().len(),
        },
        SettingsRow::Spacer,
        SettingsRow::GroupLine {
            count: config.groups.len(),
        },
        SettingsRow::SaveButton,
    ];
    rows
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
    if index < 2 {
        return false;
    }
    // Spacer row
    if index == 10 {
        return false;
    }
    // SavedMessage row
    if show_saved_message && index == ROW_COUNT_BASE {
        return false;
    }
    true
}

/// Convert index to the corresponding SettingsRow variant.
pub fn row_at_index(
    index: usize,
    config: &Config,
    show_saved_message: bool,
) -> Option<SettingsRow> {
    let rows = build_flat_items(config);
    if show_saved_message && index == ROW_COUNT_BASE {
        return Some(SettingsRow::SavedMessage);
    }
    rows.get(index).cloned()
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
    }
}
