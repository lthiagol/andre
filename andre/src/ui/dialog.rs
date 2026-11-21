//! Overlay dialogs and the action bar. A dialog is a centred panel with body lines and a
//! row of buttons; the action bar is the same buttons on a row of their own (review
//! screens). Both lay their buttons out with [`button_rects`], so paint and mouse
//! hit-testing agree.

use std::borrow::Cow;

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::{
    layout::Rect,
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Clear, Paragraph},
    Frame,
};
use unicode_width::UnicodeWidthStr;

use crate::theme::ThemeColors;
use crate::ui::block::themed_panel;

/// Columns between two buttons.
const BUTTON_GAP: u16 = 2;
/// Dialog width bounds, in columns. The maximum also keeps a margin inside the content area.
const DIALOG_MIN: u16 = 30;
const DIALOG_MAX: u16 = 60;
/// Columns between a dialog's border and its content, on each side.
const DIALOG_MARGIN: u16 = 1;
/// Columns of frame and margin around a dialog's widest line: two borders, two margins.
const DIALOG_FRAME: u16 = 2 + 2 * DIALOG_MARGIN;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Button {
    pub key: KeyEvent,
    pub label: Cow<'static, str>,
    /// The main action: drawn filled and placed rightmost.
    pub primary: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Dialog {
    pub title: String,
    pub body: Vec<Line<'static>>,
    pub buttons: Vec<Button>,
}

/// A button for `code`, labelled with what it does. `primary` marks the main action.
pub fn key_button(code: KeyCode, label: impl Into<Cow<'static, str>>, primary: bool) -> Button {
    Button {
        key: KeyEvent::new(code, KeyModifiers::NONE),
        label: label.into(),
        primary,
    }
}

/// The key as a short label: `y`, `Enter`, `Esc`, `←`.
pub fn key_name(key: &KeyEvent) -> String {
    match key.code {
        KeyCode::Char(c) => c.to_string(),
        KeyCode::Enter => "Enter".to_string(),
        KeyCode::Esc => "Esc".to_string(),
        KeyCode::Tab => "Tab".to_string(),
        KeyCode::Backspace => "Backspace".to_string(),
        KeyCode::Delete => "Del".to_string(),
        KeyCode::Home => "Home".to_string(),
        KeyCode::End => "End".to_string(),
        KeyCode::Left => "\u{2190}".to_string(),
        KeyCode::Right => "\u{2192}".to_string(),
        KeyCode::Up => "\u{2191}".to_string(),
        KeyCode::Down => "\u{2193}".to_string(),
        _ => "?".to_string(),
    }
}

/// A button's spans: `[ key label ]`, the key in `key`. A primary button is one filled
/// span: `on_accent` text on the `accent` fill.
fn button_spans(button: &Button, colors: &ThemeColors) -> Vec<Span<'static>> {
    if button.primary {
        let style = Style::default()
            .fg(colors.on_accent)
            .bg(colors.accent)
            .add_modifier(Modifier::BOLD);
        return vec![Span::styled(
            format!("[ {} {} ]", key_name(&button.key), button.label),
            style,
        )];
    }
    vec![
        Span::styled("[ ", Style::default().fg(colors.text_dim)),
        Span::styled(key_name(&button.key), Style::default().fg(colors.key)),
        Span::styled(
            format!(" {} ]", button.label),
            Style::default().fg(colors.text),
        ),
    ]
}

fn button_width(button: &Button) -> u16 {
    format!("[ {} {} ]", key_name(&button.key), button.label).width() as u16
}

/// Width of the whole button row, gaps included.
fn row_width(buttons: &[Button]) -> u16 {
    let widths: u16 = buttons.iter().map(button_width).sum();
    widths + BUTTON_GAP * buttons.len().saturating_sub(1) as u16
}

/// The buttons in draw order, each with its rect on `area`'s first row. Secondary buttons
/// keep the caller's order and the primary comes last, so it is rightmost whatever order
/// the caller lists them in. The row is right-aligned. A button that overhangs the left
/// edge is clipped there, so the right end of the row, the primary, stays visible.
fn placed_buttons(area: Rect, buttons: &[Button]) -> Vec<(Rect, &Button)> {
    let draw_order = buttons
        .iter()
        .filter(|b| !b.primary)
        .chain(buttons.iter().filter(|b| b.primary));
    // Signed: the row can start left of `area`.
    let mut x = i32::from(area.right()) - i32::from(row_width(buttons));
    draw_order
        .map(|button| {
            let width = i32::from(button_width(button));
            let left = x.max(i32::from(area.x));
            let right = (x + width).min(i32::from(area.right()));
            x += width + i32::from(BUTTON_GAP);
            let rect = Rect {
                x: left as u16,
                y: area.y,
                width: (right - left).max(0) as u16,
                height: 1,
            };
            (rect, button)
        })
        .collect()
}

/// The rect and key of each button, in draw order (see [`placed_buttons`]). Mouse
/// hit-testing uses these rects, and [`render_action_bar`] paints into them.
pub fn button_rects(area: Rect, buttons: &[Button]) -> Vec<(Rect, KeyEvent)> {
    placed_buttons(area, buttons)
        .into_iter()
        .map(|(rect, button)| (rect, button.key))
        .collect()
}

/// Draws `buttons` right-aligned on the first row of `area`, each into its
/// [`button_rects`] rect.
pub fn render_action_bar(frame: &mut Frame, area: Rect, colors: &ThemeColors, buttons: &[Button]) {
    for (rect, button) in placed_buttons(area, buttons) {
        // A button clipped on the left starts this many columns into its text.
        let skip = button_width(button) - rect.width;
        frame.render_widget(
            Paragraph::new(Line::from(button_spans(button, colors))).scroll((0, skip)),
            rect,
        );
    }
}

/// The area inside a dialog's border and margins: its first row is the blank row under the
/// top border, its last row holds the buttons.
fn dialog_inner(rect: Rect) -> Rect {
    Rect {
        x: rect.x + 1 + DIALOG_MARGIN,
        y: rect.y + 1,
        width: rect.width.saturating_sub(DIALOG_FRAME),
        height: rect.height.saturating_sub(2),
    }
}

/// The row a dialog's buttons sit on: the last row inside its border.
fn button_row(inner: Rect) -> Rect {
    Rect {
        y: inner.bottom() - 1,
        height: 1,
        ..inner
    }
}

/// The rect of a dialog centred in `content`. Its width fits the widest line (body,
/// buttons or title) plus the frame, clamped to 30..=60 columns and to the content width.
/// Its height is a blank row, the body, the button row and the two borders.
pub fn dialog_rect(content: Rect, d: &Dialog) -> Rect {
    let body = d.body.iter().map(Line::width).max().unwrap_or(0) as u16;
    let widest = body
        .max(row_width(&d.buttons))
        .max(d.title.width() as u16 + 2);
    let max = DIALOG_MAX.min(content.width.saturating_sub(DIALOG_FRAME));
    let width = (widest + DIALOG_FRAME).clamp(DIALOG_MIN.min(max), max);
    let height = ((d.body.len() + 4) as u16).min(content.height);
    Rect::new(
        content.x + (content.width - width) / 2,
        content.y + (content.height - height) / 2,
        width,
        height,
    )
}

/// Draws a dialog at `rect`, which [`dialog_rect`] sized. The panel is cleared first, so
/// nothing under it shows through.
pub fn render_dialog(frame: &mut Frame, rect: Rect, colors: &ThemeColors, d: &Dialog) {
    frame.render_widget(Clear, rect);
    frame.render_widget(Block::default().style(Style::default().bg(colors.bg)), rect);
    frame.render_widget(themed_panel(colors).title(format!(" {} ", d.title)), rect);
    let inner = dialog_inner(rect);
    if inner.height == 0 {
        return;
    }

    // The body starts under the blank row; the buttons take the last row.
    let body_rows = (d.body.len() as u16).min(inner.height.saturating_sub(2));
    let body = Rect {
        y: inner.y + 1,
        height: body_rows,
        ..inner
    };
    frame.render_widget(
        Paragraph::new(d.body.clone()).style(Style::default().fg(colors.text)),
        body,
    );
    render_action_bar(frame, button_row(inner), colors, &d.buttons);
}

/// The dialog that asks what to do with unsaved edits to `subject`, e.g. the profile
/// `work`. Primary is `y save`, on the right.
pub fn leave_dialog(subject: &str, colors: &ThemeColors) -> Dialog {
    Dialog {
        title: "unsaved changes".to_string(),
        body: vec![
            Line::raw(""),
            Line::styled(
                format!("Save changes to {subject}?"),
                Style::default().fg(colors.text),
            ),
            Line::raw(""),
        ],
        buttons: vec![
            leave_button(KeyCode::Esc, "stay", false),
            leave_button(KeyCode::Char('n'), "discard", false),
            leave_button(KeyCode::Char('y'), "save", true),
        ],
    }
}

fn leave_button(code: KeyCode, label: &'static str, primary: bool) -> Button {
    Button {
        key: KeyEvent::new(code, KeyModifiers::NONE),
        label: Cow::Borrowed(label),
        primary,
    }
}

#[cfg(test)]
mod tests {
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
    use ratatui::{backend::TestBackend, Terminal};

    use super::*;
    use andre_core::Theme;

    fn key(c: char) -> KeyEvent {
        KeyEvent::new(KeyCode::Char(c), KeyModifiers::NONE)
    }

    fn button(k: char, label: &'static str, primary: bool) -> Button {
        Button {
            key: key(k),
            label: Cow::Borrowed(label),
            primary,
        }
    }

    fn colors() -> ThemeColors {
        ThemeColors::from_theme(Theme::Default)
    }

    fn dialog(body: &[&str], buttons: Vec<Button>) -> Dialog {
        Dialog {
            title: "confirm".to_string(),
            body: body.iter().map(|s| Line::raw(s.to_string())).collect(),
            buttons,
        }
    }

    #[test]
    fn test_key_names_cover_letters_and_named_keys() {
        assert_eq!(key_name(&key('y')), "y");
        assert_eq!(
            key_name(&KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE)),
            "Enter"
        );
        assert_eq!(
            key_name(&KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE)),
            "Esc"
        );
        assert_eq!(
            key_name(&KeyEvent::new(KeyCode::Left, KeyModifiers::NONE)),
            "\u{2190}"
        );
    }

    #[test]
    fn test_dialog_is_centred_in_the_content_area() {
        let content = Rect::new(0, 0, 76, 16);
        let d = dialog(&["short"], vec![button('n', "cancel", false)]);
        let rect = dialog_rect(content, &d);
        assert_eq!(rect.x, (76 - rect.width) / 2);
        assert_eq!(rect.y, (16 - rect.height) / 2);
    }

    #[test]
    fn test_dialog_width_is_clamped_to_30_through_60() {
        let content = Rect::new(0, 0, 120, 40);
        let narrow = dialog(&["x"], vec![button('n', "no", false)]);
        assert_eq!(dialog_rect(content, &narrow).width, 30);

        let long = "x".repeat(100);
        let wide = dialog(&[long.as_str()], vec![button('n', "no", false)]);
        assert_eq!(dialog_rect(content, &wide).width, 60);
    }

    #[test]
    fn test_dialog_width_fits_its_widest_body_line_plus_the_frame() {
        let content = Rect::new(0, 0, 120, 40);
        let d = dialog(
            &["a line of forty characters, give or take a few"],
            Vec::new(),
        );
        let width = d.body[0].width() as u16;
        assert_eq!(dialog_rect(content, &d).width, (width + 4).clamp(30, 60));
    }

    #[test]
    fn test_dialog_never_grows_past_a_narrow_content_area() {
        let content = Rect::new(0, 0, 20, 10);
        let d = dialog(&["body"], vec![button('n', "no", false)]);
        let rect = dialog_rect(content, &d);
        assert!(
            rect.width <= content.width,
            "{rect:?} overflows {content:?}"
        );
        assert!(rect.height <= content.height);
    }

    #[test]
    fn test_dialog_height_is_body_plus_blank_buttons_and_borders() {
        let content = Rect::new(0, 0, 76, 16);
        let d = dialog(&["one", "two", "three"], vec![button('y', "ok", true)]);
        assert_eq!(dialog_rect(content, &d).height, 3 + 4);
    }

    #[test]
    fn test_primary_button_is_rightmost_and_filled_accent() {
        let c = colors();
        // The primary is listed first: it must still be drawn rightmost.
        let d = dialog(
            &["Remove the link?"],
            vec![button('y', "remove", true), button('n', "cancel", false)],
        );
        let rect = dialog_rect(Rect::new(0, 0, 76, 16), &d);
        let mut terminal = Terminal::new(TestBackend::new(76, 16)).unwrap();
        terminal.draw(|f| render_dialog(f, rect, &c, &d)).unwrap();
        let buf = terminal.backend().buffer().clone();

        let rects = button_rects(button_row(dialog_inner(rect)), &d.buttons);
        let (primary_rect, primary_key) = rects[1];
        assert_eq!(primary_key, key('y'));
        assert!(rects[0].0.x < rects[1].0.x, "primary is rightmost");
        let cell = &buf[(primary_rect.x + 1, primary_rect.y)];
        assert_eq!(cell.bg, c.accent, "primary fill");
        assert_eq!(cell.fg, c.on_accent, "primary text");
        let other = &buf[(rects[0].0.x + 1, rects[0].0.y)];
        assert_ne!(other.bg, c.accent, "secondary is not filled");
    }

    #[test]
    fn test_button_rects_are_right_aligned_with_a_two_column_gap() {
        let area = Rect::new(0, 0, 40, 1);
        let buttons = [button('n', "cancel", false), button('y', "save", true)];
        let rects = button_rects(area, &buttons);
        let save = rects[1].0;
        assert_eq!(save.right(), 40, "rightmost button ends at the edge");
        assert_eq!(save.width, "[ y save ]".len() as u16);
        assert_eq!(rects[0].0.right() + BUTTON_GAP, save.x);
    }

    #[test]
    fn test_button_rects_are_clipped_to_the_area() {
        let area = Rect::new(0, 0, 6, 1);
        let rects = button_rects(area, &[button('y', "save", true)]);
        assert!(rects[0].0.right() <= area.right());
    }

    #[test]
    fn test_overflowing_row_keeps_the_primary_whole() {
        let c = colors();
        let area = Rect::new(0, 0, 12, 1);
        let buttons = [button('n', "cancel", false), button('y', "save", true)];
        let rects = button_rects(area, &buttons);
        assert_eq!(rects[1].0, Rect::new(2, 0, 10, 1));
        assert_eq!(rects[0].0.width, 0, "the secondary is cut off entirely");

        let mut terminal = Terminal::new(TestBackend::new(12, 1)).unwrap();
        terminal
            .draw(|f| render_action_bar(f, area, &c, &buttons))
            .unwrap();
        let buf = terminal.backend().buffer().clone();
        let row: String = (0..12).map(|x| buf[(x, 0)].symbol()).collect();
        assert_eq!(row, "  [ y save ]");
    }

    #[test]
    fn test_action_bar_paints_into_the_rects_when_the_button_overhangs_the_left() {
        let c = colors();
        // A 10-column button in a 6-column area that starts at x=10: four columns are cut
        // from its left, so the paint and the rect both cover the last six.
        let area = Rect::new(10, 0, 6, 1);
        let buttons = [button('y', "save", true)];
        let mut terminal = Terminal::new(TestBackend::new(20, 1)).unwrap();
        terminal
            .draw(|f| render_action_bar(f, area, &c, &buttons))
            .unwrap();
        let buf = terminal.backend().buffer().clone();
        let (rect, _) = button_rects(area, &buttons)[0];
        assert_eq!(rect, Rect::new(10, 0, 6, 1));
        let painted: String = (rect.x..rect.right())
            .map(|x| buf[(x, 0)].symbol())
            .collect();
        assert_eq!(painted, "save ]");
        assert_eq!(
            buf[(9, 0)].symbol(),
            " ",
            "nothing is painted left of the area"
        );
    }

    #[test]
    fn test_dialog_body_and_buttons_are_inset_from_the_border() {
        let c = colors();
        let d = dialog(
            &["Remove the link?"],
            vec![button('n', "cancel", false), button('y', "remove", true)],
        );
        let rect = dialog_rect(Rect::new(0, 0, 76, 16), &d);
        let mut terminal = Terminal::new(TestBackend::new(76, 16)).unwrap();
        terminal.draw(|f| render_dialog(f, rect, &c, &d)).unwrap();
        let buf = terminal.backend().buffer().clone();

        assert_eq!(
            buf[(rect.x + 2, rect.y + 1)].symbol(),
            " ",
            "blank row under the top border"
        );
        let body_row = rect.y + 2;
        assert_eq!(buf[(rect.x + 1, body_row)].symbol(), " ", "left margin");
        assert_eq!(
            buf[(rect.x + 2, body_row)].symbol(),
            "R",
            "body starts after the margin"
        );

        let row = button_row(dialog_inner(rect));
        assert_eq!(
            buf[(row.right(), row.y)].symbol(),
            " ",
            "right margin after the primary"
        );
        assert_eq!(
            buf[(row.right() - 1, row.y)].symbol(),
            "]",
            "primary ends before the margin"
        );
    }

    #[test]
    fn test_action_bar_draws_the_buttons_where_their_rects_say() {
        let c = colors();
        let buttons = [button('n', "back", false), button('y', "go", true)];
        let area = Rect::new(0, 0, 30, 1);
        let mut terminal = Terminal::new(TestBackend::new(30, 1)).unwrap();
        terminal
            .draw(|f| render_action_bar(f, area, &c, &buttons))
            .unwrap();
        let buf = terminal.backend().buffer().clone();
        let row: String = (0..30).map(|x| buf[(x, 0)].symbol()).collect();
        assert!(row.trim_end().ends_with("[ y go ]"), "{row:?}");
        for (rect, _) in button_rects(area, &buttons) {
            assert_ne!(buf[(rect.x, 0)].symbol(), " ", "button at {rect:?}");
        }
    }

    #[test]
    fn test_secondary_button_draws_the_key_in_the_key_colour() {
        let c = colors();
        let spans = button_spans(&button('n', "cancel", false), &c);
        assert_eq!(spans[1].content, "n");
        assert_eq!(spans[1].style.fg, Some(c.key));
        assert_eq!(spans[2].style.fg, Some(c.text));
    }
}
