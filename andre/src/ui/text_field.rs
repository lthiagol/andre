//! A one-line text input: profile names, package names, patterns, descriptions. The cursor
//! is a character index, so multi-byte input edits cleanly.

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::{
    style::{Modifier, Style},
    text::{Line, Span},
};

use crate::theme::ThemeColors;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct TextField {
    pub value: String,
    /// Character index of the insertion point, `0..=chars`.
    pub cursor: usize,
    /// A validation message shown after the value. Any edit clears it.
    pub error: Option<String>,
}

impl TextField {
    fn chars(&self) -> usize {
        self.value.chars().count()
    }

    /// Byte index of character `at`, or the end of the value.
    fn byte_index(&self, at: usize) -> usize {
        self.value
            .char_indices()
            .nth(at)
            .map_or(self.value.len(), |(i, _)| i)
    }

    /// Applies a key. Returns `false` for keys the field does not use, so the caller can
    /// pass them on (Enter, Esc, Tab, Ctrl and Alt chords).
    pub fn handle_key(&mut self, key: &KeyEvent) -> bool {
        if key
            .modifiers
            .intersects(KeyModifiers::CONTROL | KeyModifiers::ALT)
        {
            return false;
        }
        match key.code {
            KeyCode::Char(c) => {
                let at = self.byte_index(self.cursor);
                self.value.insert(at, c);
                self.cursor += 1;
                self.error = None;
            }
            KeyCode::Backspace => {
                if self.cursor > 0 {
                    let at = self.byte_index(self.cursor - 1);
                    self.value.remove(at);
                    self.cursor -= 1;
                    self.error = None;
                }
            }
            KeyCode::Delete => {
                if self.cursor < self.chars() {
                    let at = self.byte_index(self.cursor);
                    self.value.remove(at);
                    self.error = None;
                }
            }
            KeyCode::Left => self.cursor = self.cursor.saturating_sub(1),
            KeyCode::Right => self.cursor = (self.cursor + 1).min(self.chars()),
            KeyCode::Home => self.cursor = 0,
            KeyCode::End => self.cursor = self.chars(),
            _ => return false,
        }
        true
    }
}

/// `label` in `text_dim`, two columns, then the value in `text`. When `focused`, the
/// character under the cursor is drawn reversed (a space at the end). An error follows in
/// `err`.
pub fn field_line(
    label: &str,
    field: &TextField,
    focused: bool,
    colors: &ThemeColors,
) -> Line<'static> {
    let mut spans = vec![Span::styled(
        format!("{label}  "),
        Style::default().fg(colors.text_dim),
    )];
    let text = Style::default().fg(colors.text);
    if focused {
        let before: String = field.value.chars().take(field.cursor).collect();
        let under: String = field
            .value
            .chars()
            .nth(field.cursor)
            .map_or(" ".to_string(), |c| c.to_string());
        let after: String = field.value.chars().skip(field.cursor + 1).collect();
        spans.push(Span::styled(before, text));
        spans.push(Span::styled(under, text.add_modifier(Modifier::REVERSED)));
        spans.push(Span::styled(after, text));
    } else {
        spans.push(Span::styled(field.value.clone(), text));
    }
    if let Some(error) = &field.error {
        spans.push(Span::styled(
            format!("  {error}"),
            Style::default().fg(colors.err),
        ));
    }
    Line::from(spans)
}

#[cfg(test)]
mod tests {
    use super::*;
    use andre_core::Theme;

    fn press(field: &mut TextField, code: KeyCode) -> bool {
        field.handle_key(&KeyEvent::new(code, KeyModifiers::NONE))
    }

    fn typed(text: &str) -> TextField {
        let mut field = TextField::default();
        for c in text.chars() {
            press(&mut field, KeyCode::Char(c));
        }
        field
    }

    #[test]
    fn test_characters_insert_at_the_cursor() {
        let mut field = typed("ac");
        press(&mut field, KeyCode::Left);
        press(&mut field, KeyCode::Char('b'));
        assert_eq!(field.value, "abc");
        assert_eq!(field.cursor, 2);
    }

    #[test]
    fn test_backspace_removes_the_character_before_the_cursor() {
        let mut field = typed("abc");
        press(&mut field, KeyCode::Left);
        press(&mut field, KeyCode::Backspace);
        assert_eq!(field.value, "ac");
        assert_eq!(field.cursor, 1);
        press(&mut field, KeyCode::Home);
        press(&mut field, KeyCode::Backspace);
        assert_eq!(field.value, "ac", "backspace at the start does nothing");
    }

    #[test]
    fn test_delete_removes_the_character_under_the_cursor() {
        let mut field = typed("abc");
        press(&mut field, KeyCode::Home);
        press(&mut field, KeyCode::Delete);
        assert_eq!(field.value, "bc");
        press(&mut field, KeyCode::End);
        press(&mut field, KeyCode::Delete);
        assert_eq!(field.value, "bc", "delete at the end does nothing");
    }

    #[test]
    fn test_arrows_stop_at_both_ends() {
        let mut field = typed("ab");
        press(&mut field, KeyCode::Right);
        assert_eq!(field.cursor, 2);
        press(&mut field, KeyCode::Left);
        press(&mut field, KeyCode::Left);
        press(&mut field, KeyCode::Left);
        assert_eq!(field.cursor, 0);
    }

    #[test]
    fn test_home_and_end_move_to_the_ends() {
        let mut field = typed("hello");
        press(&mut field, KeyCode::Home);
        assert_eq!(field.cursor, 0);
        press(&mut field, KeyCode::End);
        assert_eq!(field.cursor, 5);
    }

    #[test]
    fn test_multibyte_characters_edit_by_character() {
        let mut field = typed("日本");
        press(&mut field, KeyCode::Left);
        press(&mut field, KeyCode::Backspace);
        assert_eq!(field.value, "本");
        press(&mut field, KeyCode::Char('語'));
        assert_eq!(field.value, "語本");
    }

    #[test]
    fn test_keys_the_field_does_not_use_are_passed_on() {
        let mut field = typed("x");
        assert!(!press(&mut field, KeyCode::Enter));
        assert!(!press(&mut field, KeyCode::Esc));
        assert!(!field.handle_key(&KeyEvent::new(KeyCode::Char('s'), KeyModifiers::CONTROL)));
        assert_eq!(field.value, "x");
    }

    #[test]
    fn test_an_edit_clears_the_error() {
        let mut field = typed("bad name");
        field.error = Some("no spaces".to_string());
        press(&mut field, KeyCode::Backspace);
        assert_eq!(field.error, None);
    }

    #[test]
    fn test_field_line_reverses_the_cell_under_the_cursor() {
        let colors = ThemeColors::from_theme(Theme::Default);
        let mut field = typed("ab");
        press(&mut field, KeyCode::Left);
        let line = field_line("name", &field, true, &colors);
        let under = line
            .spans
            .iter()
            .find(|s| s.style.add_modifier.contains(Modifier::REVERSED))
            .unwrap();
        assert_eq!(under.content, "b");
    }

    #[test]
    fn test_field_line_puts_a_reversed_space_after_the_value_at_the_end() {
        let colors = ThemeColors::from_theme(Theme::Default);
        let line = field_line("name", &typed("ab"), true, &colors);
        let under = line
            .spans
            .iter()
            .find(|s| s.style.add_modifier.contains(Modifier::REVERSED))
            .unwrap();
        assert_eq!(under.content, " ");
    }

    #[test]
    fn test_label_and_value_are_separated_by_two_columns() {
        let colors = ThemeColors::from_theme(Theme::Default);
        let line = field_line("name", &typed("laptop"), false, &colors);
        let text: String = line.spans.iter().map(|s| s.content.as_ref()).collect();
        assert_eq!(text, "name  laptop");
    }

    #[test]
    fn test_unfocused_field_has_no_cursor_and_shows_the_error_in_err() {
        let colors = ThemeColors::from_theme(Theme::Default);
        let mut field = typed("ab");
        field.error = Some("taken".to_string());
        let line = field_line("name", &field, false, &colors);
        assert!(line
            .spans
            .iter()
            .all(|s| !s.style.add_modifier.contains(Modifier::REVERSED)));
        let error = line.spans.last().unwrap();
        assert_eq!(error.content, "  taken");
        assert_eq!(error.style.fg, Some(colors.err));
    }
}
