use andre_core::{Config, IconSet, StowFlag, Theme};

use super::pending::GroupChange;
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
    GlobalIgnore,
    Group(String),
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

/// Which pattern list an ignore editor edits.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum IgnoreScope {
    Global,
    Group(String),
}

/// What a directory picker chose a path for.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Pick {
    /// A new group's source, then its target.
    NewSource {
        name: String,
    },
    NewTarget {
        name: String,
        source: String,
    },
    /// One path of an existing group.
    Source {
        group: String,
    },
    Target {
        group: String,
    },
}

/// The group name dialog: a new group, or a rename.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NameTarget {
    New,
    Rename { original: String },
}

/// The pattern being typed in an ignore editor. `index` is the pattern it replaces, or
/// `None` for a new pattern.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LineEdit {
    pub index: Option<usize>,
    pub field: TextField,
}

/// What a group menu restores when it is cancelled: the group's name when the menu opened,
/// the config, the group changes and the dirty flag.
#[derive(Debug, Clone)]
pub struct Snapshot {
    pub group: String,
    pub config: Config,
    pub changes: Vec<GroupChange>,
    pub dirty: bool,
}

/// A dialog on top of the settings list. The last one in `SettingsComponent::overlays`
/// takes keys and is the only one painted.
#[derive(Debug, Clone)]
pub enum Overlay {
    /// The action picker; `cursor` indexes `StowAction::ALL`.
    Action {
        cursor: usize,
    },
    /// The theme picker. Moving the cursor previews the theme; Esc puts `original` back.
    Theme {
        cursor: usize,
        original: Theme,
    },
    /// The icon set picker, with the same preview and revert as the theme picker.
    Icons {
        cursor: usize,
        original: IconSet,
    },
    /// A group's edit menu. Rows: name, source, target, ignore, done.
    Group {
        group: String,
        cursor: usize,
        snapshot: Snapshot,
    },
    Name {
        target: NameTarget,
        field: TextField,
    },
    Browse {
        pick: Pick,
        browser: Browser,
    },
    /// Confirm removing a group. `refs` counts the profile refs into it.
    Remove {
        group: String,
        refs: usize,
    },
    /// A pattern list. The row after the last pattern adds one.
    Ignore {
        scope: IgnoreScope,
        cursor: usize,
        edit: Option<LineEdit>,
    },
    /// Unsaved edits: save, discard or stay.
    Leave,
}

/// Rows of a group's edit menu, in display order.
pub const GROUP_NAME: usize = 0;
pub const GROUP_SOURCE: usize = 1;
pub const GROUP_TARGET: usize = 2;
pub const GROUP_IGNORE: usize = 3;
pub const GROUP_DONE: usize = 4;
pub const GROUP_ROWS: usize = 5;
