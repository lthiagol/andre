use andre_core::StowFlag;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

/// A key with no modifiers, as a mouse click sends it to the screen it lands on.
pub fn key(code: KeyCode) -> KeyEvent {
    KeyEvent::new(code, KeyModifiers::NONE)
}

/// The stow option behind a flag shortcut key (`v`, `d`, `n`, `a`, `o`), if any.
pub fn flag_for_key(code: KeyCode) -> Option<StowFlag> {
    match code {
        KeyCode::Char('v') => Some(StowFlag::Verbosity),
        KeyCode::Char('d') => Some(StowFlag::DryRun),
        KeyCode::Char('n') => Some(StowFlag::NoFolding),
        KeyCode::Char('a') => Some(StowFlag::Adopt),
        KeyCode::Char('o') => Some(StowFlag::Dotfiles),
        _ => None,
    }
}

/// Moves a list cursor one row down or up, wrapping at both ends. An empty list keeps 0.
pub fn step_cursor(cursor: usize, len: usize, down: bool) -> usize {
    if len == 0 {
        0
    } else if down {
        (cursor + 1) % len
    } else if cursor == 0 {
        len - 1
    } else {
        cursor - 1
    }
}

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
        let down = if Self::is_up(key) {
            false
        } else if Self::is_down(key) {
            true
        } else {
            return false;
        };
        *cursor = step_cursor(*cursor, len, down);
        true
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_flag_for_key_maps_shortcut_keys() {
        assert_eq!(flag_for_key(KeyCode::Char('v')), Some(StowFlag::Verbosity));
        assert_eq!(flag_for_key(KeyCode::Char('d')), Some(StowFlag::DryRun));
        assert_eq!(flag_for_key(KeyCode::Char('n')), Some(StowFlag::NoFolding));
        assert_eq!(flag_for_key(KeyCode::Char('a')), Some(StowFlag::Adopt));
        assert_eq!(flag_for_key(KeyCode::Char('o')), Some(StowFlag::Dotfiles));
    }

    #[test]
    fn test_step_cursor_wraps_and_keeps_zero_for_empty_list() {
        assert_eq!(step_cursor(0, 3, false), 2);
        assert_eq!(step_cursor(2, 3, true), 0);
        assert_eq!(step_cursor(1, 3, false), 0);
        assert_eq!(step_cursor(0, 0, true), 0);
    }

    #[test]
    fn test_flag_for_key_is_none_for_other_keys() {
        assert_eq!(flag_for_key(KeyCode::Char('t')), None);
        assert_eq!(flag_for_key(KeyCode::Char('q')), None);
        assert_eq!(flag_for_key(KeyCode::Enter), None);
    }
}
