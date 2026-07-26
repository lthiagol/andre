use std::path::PathBuf;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PickerField {
    Action,
    Theme,
    DryRun,
    NoFolding,
    Adopt,
    Dotfiles,
}

#[derive(Debug, Clone)]
pub struct PickerState {
    pub options: Vec<String>,
    pub selected_index: usize,
    pub title: String,
    pub field: PickerField,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GroupEditField {
    Source,
    Target,
    Ignore,
}

#[derive(Debug, Clone)]
pub enum GroupDialog {
    Actions {
        cursor: usize,
    },
    AddName {
        name: String,
    },
    AddBrowse {
        name: String,
        source: Option<String>,
        path: PathBuf,
        cursor: usize,
    },
    RemoveSelect {
        cursor: usize,
    },
    RemoveConfirm {
        name: String,
    },
    EditSelect {
        cursor: usize,
    },
    EditField {
        group: String,
        cursor: usize,
    },
    EditName {
        group: String,
    },
    EditBrowse {
        group: String,
        field: GroupEditField,
        path: PathBuf,
        cursor: usize,
    },
    IgnoreManage {
        group: Option<String>,
        cursor: usize,
    },
    IgnoreAdd {
        group: Option<String>,
        pattern: String,
    },
}
