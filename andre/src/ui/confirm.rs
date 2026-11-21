//! The review screen's action bar. The review itself is a tree and a detail pane, built by
//! `components::confirm`; these are the buttons under it.

use crossterm::event::KeyCode;

use crate::ui::dialog::{key_button, Button};

/// `[ n cancel ]  [ y <action> N ]`: the run is the primary button, on the right.
pub fn review_buttons(action: &'static str, count: usize) -> Vec<Button> {
    vec![
        key_button(KeyCode::Char('n'), "cancel", false),
        key_button(KeyCode::Char('y'), format!("{action} {count}"), true),
    ]
}

/// `[ Enter done ]`, the one button on a finished run or a finished unlink.
pub fn done_buttons() -> Vec<Button> {
    vec![key_button(KeyCode::Enter, "done", true)]
}
