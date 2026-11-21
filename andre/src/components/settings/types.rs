use andre_core::{IconSet, StowFlag, Theme};

use crate::components::browser::Browser;
use crate::ui::text_field::TextField;

/// What a list row edits. Headers are drawn but never selected.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Row {
    Header,
    Action,
    Flag(StowFlag),
    Theme,
    Icons,
    Packages,
    GlobalIgnore,
}

/// One row of the settings list, as painted: its label and its value.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Item {
    pub row: Row,
    pub label: String,
    pub value: String,
}

impl Item {
    pub fn selectable(&self) -> bool {
        self.row != Row::Header
    }
}

/// The pattern being typed in the ignore editor. `index` is the pattern it replaces, or
/// `None` for a new pattern.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LineEdit {
    pub index: Option<usize>,
    pub field: TextField,
}

/// A dialog on top of the settings list. The last one in `SettingsComponent::overlays`
/// takes keys and is the only one painted.
#[derive(Debug, Clone)]
pub enum Overlay {
    /// The action picker; `cursor` indexes `StowAction::ALL`.
    Action { cursor: usize },
    /// The theme picker. Moving the cursor previews the theme; Esc puts `original` back.
    Theme { cursor: usize, original: Theme },
    /// The icon set picker, with the same preview and revert as the theme picker.
    Icons { cursor: usize, original: IconSet },
    /// The packages folder picker. Enter chooses the highlighted folder.
    Browse { browser: Browser },
    /// The global ignore list. The row after the last pattern adds one.
    Ignore {
        cursor: usize,
        edit: Option<LineEdit>,
    },
    /// Unsaved edits: save, discard or stay.
    Leave,
}
