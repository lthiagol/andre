use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

pub struct Input;

impl Input {
    pub fn is_quit_key(key: &KeyEvent) -> bool {
        key.code == KeyCode::Char('q')
            || (key.code == KeyCode::Char('c') && key.modifiers.contains(KeyModifiers::CONTROL))
    }

    pub fn is_up(key: &KeyEvent) -> bool {
        key.code == KeyCode::Up
            || (matches!(key.code, KeyCode::Char('k') | KeyCode::Char('K'))
                && key.modifiers.is_empty())
    }

    pub fn is_down(key: &KeyEvent) -> bool {
        key.code == KeyCode::Down
            || (matches!(key.code, KeyCode::Char('j') | KeyCode::Char('J'))
                && key.modifiers.is_empty())
    }

    pub fn handle_list_navigation(key: &KeyEvent, cursor: &mut usize, len: usize) -> bool {
        if len == 0 {
            return false;
        }
        if Self::is_up(key) {
            *cursor = (*cursor + len - 1) % len;
            true
        } else if Self::is_down(key) {
            *cursor = (*cursor + 1) % len;
            true
        } else {
            false
        }
    }

    /// Handle scrolling with line/page/home/end keys.
    /// Returns true if the key was handled.
    /// `page_size` is typically the visible viewport height; scrolling moves by `page_size / 2`.
    pub fn handle_list_scroll(
        key: &KeyEvent,
        cursor: &mut usize,
        len: usize,
        page_size: usize,
    ) -> bool {
        if len == 0 {
            return false;
        }

        let half_page = (page_size / 2).max(1);
        let max = len.saturating_sub(1);

        match key.code {
            _ if Self::is_up(key) => {
                *cursor = cursor.saturating_sub(1);
                true
            }
            _ if Self::is_down(key) => {
                *cursor = (*cursor + 1).min(max);
                true
            }
            KeyCode::PageUp => {
                *cursor = cursor.saturating_sub(half_page);
                true
            }
            KeyCode::PageDown => {
                *cursor = (*cursor + half_page).min(max);
                true
            }
            KeyCode::Home => {
                *cursor = 0;
                true
            }
            KeyCode::End => {
                *cursor = max;
                true
            }
            _ => false,
        }
    }
}
