use crate::components::browser::Browser;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PickerField {
    Action,
    Theme,
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
        browser: Browser,
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
        /// The group's current name, kept so Esc can return to it and Enter can rename it.
        original: String,
        /// The name being typed.
        name: String,
    },
    EditBrowse {
        group: String,
        field: GroupEditField,
        browser: Browser,
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
