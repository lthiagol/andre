//! A scrolling list in a themed panel. The caller builds the rows, each already styled by
//! `ui::selection`; [`ListView`] owns the cursor, the window onto the rows and the mouse
//! hit test. Its `prepare` runs before paint, so `render_list_panel` only reads it.

use crossterm::event::{MouseButton, MouseEvent, MouseEventKind};
use ratatui::{
    layout::{Alignment, Margin, Rect},
    style::Style,
    text::{Line, Span},
    widgets::Paragraph,
    Frame,
};
use unicode_width::UnicodeWidthStr;

use crate::input::step_cursor;
use crate::theme::ThemeColors;
use crate::ui::block::focus_panel;
use crate::ui::widgets::scrollbar;

/// The cursor and the scroll window of one list.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct ListView {
    pub cursor: usize,
    /// Index of the first row drawn.
    pub offset: usize,
    /// The rows inside the panel's border, as of the last `prepare`.
    inner: Rect,
    /// Row count as of the last `prepare`.
    len: usize,
}

impl ListView {
    /// Keeps the cursor inside the window for `len` rows shown in `panel`. `panel` is the
    /// bordered rect the list is drawn in.
    pub fn prepare(&mut self, panel: Rect, len: usize) {
        self.inner = inner(panel);
        self.len = len;
        self.cursor = self.cursor.min(len.saturating_sub(1));
        let rows = usize::from(self.inner.height);
        if rows == 0 {
            self.offset = 0;
            return;
        }
        self.offset = self.offset.min(len.saturating_sub(rows));
        if self.cursor < self.offset {
            self.offset = self.cursor;
        } else if self.cursor >= self.offset + rows {
            self.offset = self.cursor + 1 - rows;
        }
    }

    /// Whether a mouse position is inside the rows (the panel's border is not).
    pub fn contains(&self, x: u16, y: u16) -> bool {
        x >= self.inner.x && x < self.inner.right() && y >= self.inner.y && y < self.inner.bottom()
    }

    /// The row under a mouse position, or `None` outside the rows.
    pub fn row_at(&self, x: u16, y: u16) -> Option<usize> {
        let row = self.offset + usize::from(y.checked_sub(self.inner.y)?);
        (self.contains(x, y) && row < self.len).then_some(row)
    }

    /// Moves the cursor one row, wrapping at either end.
    pub fn step(&mut self, down: bool, len: usize) {
        self.cursor = step_cursor(self.cursor, len, down);
    }

    /// Scrolls the window by `lines` (negative is up) without leaving the cursor out of
    /// view: the cursor moves to the nearest visible row when the window passes it.
    pub fn scroll(&mut self, lines: i16, len: usize) {
        let rows = usize::from(self.inner.height);
        if len == 0 || rows == 0 {
            return;
        }
        let max_offset = len.saturating_sub(rows);
        let target = self.offset as i64 + i64::from(lines);
        self.offset = target.clamp(0, max_offset as i64) as usize;
        self.cursor = self
            .cursor
            .clamp(self.offset, (self.offset + rows - 1).min(len - 1));
    }
}

/// The rows inside a bordered list panel. Rows are built at this width.
pub fn inner(panel: Rect) -> Rect {
    panel.inner(Margin::new(1, 1))
}

/// What one mouse event did to a list. `row` is the row under the pointer; a wheel notch
/// reports the cursor's new row. `col` counts from the list's first inner column, so a
/// caller can tell a row's glyph cells apart. `activate` is set when a click landed on the
/// row the cursor already was on: that is the row's primary verb (D9).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ListClick {
    pub row: usize,
    pub col: u16,
    pub activate: bool,
}

/// Applies a left click or a wheel notch to `view`, a list of `len` rows. A click moves the
/// cursor onto the row under the pointer. A wheel notch moves it one row, as ↑↓ does. Other
/// events, and points outside the rows, return `None` and leave `view` alone.
pub fn click_list(view: &mut ListView, ev: MouseEvent, len: usize) -> Option<ListClick> {
    match ev.kind {
        MouseEventKind::ScrollUp | MouseEventKind::ScrollDown => {
            if len == 0 || !view.contains(ev.column, ev.row) {
                return None;
            }
            view.cursor = step_cursor(view.cursor, len, ev.kind == MouseEventKind::ScrollDown);
            Some(ListClick {
                row: view.cursor,
                col: 0,
                activate: false,
            })
        }
        MouseEventKind::Down(MouseButton::Left) => {
            let row = view.row_at(ev.column, ev.row).filter(|&row| row < len)?;
            let activate = row == view.cursor;
            view.cursor = row;
            Some(ListClick {
                row,
                col: ev.column - view.inner.x,
                activate,
            })
        }
        _ => None,
    }
}

/// What a list shows when it has no rows: a message and an optional key hint, centred.
pub struct Empty<'a> {
    pub message: &'a str,
    /// The key and what it does, e.g. `("+", "new profile")`.
    pub hint: Option<(&'a str, &'a str)>,
}

/// Draws the panel with `title` and the rows in `view`'s window. A scrollbar sits on the
/// right border when the rows overflow. With no rows, `empty` is drawn instead.
pub fn render_list_panel(
    frame: &mut Frame,
    area: Rect,
    colors: &ThemeColors,
    title: &str,
    rows: Vec<Line<'static>>,
    view: &ListView,
    empty: Option<Empty>,
) {
    render_focused_list_panel(frame, area, colors, false, title, rows, view, empty);
}

/// [`render_list_panel`] with the border in `accent` when `focused`. Of two panes side by
/// side, the one that takes keys is the focused one.
#[allow(clippy::too_many_arguments)]
pub fn render_focused_list_panel(
    frame: &mut Frame,
    area: Rect,
    colors: &ThemeColors,
    focused: bool,
    title: &str,
    rows: Vec<Line<'static>>,
    view: &ListView,
    empty: Option<Empty>,
) {
    render_titled_list_panel(
        frame,
        area,
        colors,
        focused,
        Line::raw(format!(" {title} ")),
        rows,
        view,
        empty,
    );
}

/// [`render_focused_list_panel`] with a styled title line, for a title that marks a state
/// in its own colour, such as `· unsaved ●`. The line is drawn as given, so it carries its
/// own padding.
#[allow(clippy::too_many_arguments)]
pub fn render_titled_list_panel(
    frame: &mut Frame,
    area: Rect,
    colors: &ThemeColors,
    focused: bool,
    title: Line<'static>,
    rows: Vec<Line<'static>>,
    view: &ListView,
    empty: Option<Empty>,
) {
    let block = focus_panel(colors, focused).title(title);
    frame.render_widget(block, area);
    let body = inner(area);

    if rows.is_empty() {
        if let Some(empty) = empty {
            render_empty(frame, body, colors, &empty);
        }
        return;
    }

    let total = rows.len();
    let window = usize::from(body.height);
    let visible: Vec<Line> = rows.into_iter().skip(view.offset).take(window).collect();
    frame.render_widget(Paragraph::new(visible), body);
    scrollbar::render_scrollbar(
        frame,
        scrollbar::border_track(area),
        colors,
        total,
        window,
        view.offset,
    );
}

/// Rows the empty state takes: the message, and with a hint a blank row and the hint.
fn empty_rows(empty: &Empty) -> usize {
    if empty.hint.is_some() {
        3
    } else {
        1
    }
}

/// Blank rows above the empty state, which is centred vertically in `height` rows.
fn empty_top(height: u16, rows: usize) -> usize {
    usize::from(height).saturating_sub(rows) / 2
}

/// The row of an empty list's key hint inside `body`, centred as `render_empty` draws it.
/// `None` when the list has no hint or the row does not fit.
pub fn empty_hint_rect(body: Rect, empty: &Empty) -> Option<Rect> {
    let (key, label) = empty.hint?;
    let row = empty_top(body.height, empty_rows(empty)) + 2;
    if row >= usize::from(body.height) {
        return None;
    }
    let width = (key.width() + 1 + label.width()).min(usize::from(body.width)) as u16;
    let x = body.x + (body.width - width) / 2;
    Some(Rect::new(x, body.y + row as u16, width, 1))
}

fn render_empty(frame: &mut Frame, area: Rect, colors: &ThemeColors, empty: &Empty) {
    let mut lines = vec![Line::from(Span::styled(
        empty.message.to_string(),
        Style::default().fg(colors.text_dim),
    ))];
    if let Some((key, label)) = empty.hint {
        lines.push(Line::raw(""));
        lines.push(Line::from(vec![
            Span::styled(key.to_string(), Style::default().fg(colors.key)),
            Span::styled(format!(" {label}"), Style::default().fg(colors.text_dim)),
        ]));
    }
    let top = empty_top(area.height, lines.len());
    let mut padded = vec![Line::raw(""); top];
    padded.extend(lines);
    frame.render_widget(Paragraph::new(padded).alignment(Alignment::Center), area);
}

#[cfg(test)]
mod tests {
    use ratatui::{
        backend::TestBackend,
        buffer::Buffer,
        style::Style,
        text::{Line, Span},
        Terminal,
    };

    use super::*;
    use crate::theme::ThemeColors;
    use andre_core::Theme;
    use crossterm::event::KeyModifiers;

    /// A 76x16 panel at the origin.
    fn panel() -> Rect {
        Rect::new(0, 0, 76, 16)
    }

    fn colors() -> ThemeColors {
        ThemeColors::from_theme(Theme::Default)
    }

    fn row_text(buf: &Buffer, y: u16) -> String {
        (0..buf.area().width)
            .map(|x| buf[(x, y)].symbol())
            .collect()
    }

    fn drawn(rows: Vec<Line<'static>>, view: &ListView, empty: Option<Empty>) -> Buffer {
        let backend = TestBackend::new(76, 16);
        let mut terminal = Terminal::new(backend).unwrap();
        let c = colors();
        terminal
            .draw(|f| render_list_panel(f, panel(), &c, "items", rows, view, empty))
            .unwrap();
        terminal.backend().buffer().clone()
    }

    fn numbered(n: usize) -> Vec<Line<'static>> {
        (0..n).map(|i| Line::raw(format!("row {i}"))).collect()
    }

    #[test]
    fn test_cursor_stays_visible_after_30_steps_on_40_rows() {
        let mut view = ListView::default();
        view.prepare(panel(), 40);
        for _ in 0..30 {
            view.step(true, 40);
            view.prepare(panel(), 40);
        }
        assert_eq!(view.cursor, 30);
        let rows = usize::from(inner(panel()).height);
        assert!(view.cursor >= view.offset && view.cursor < view.offset + rows);
    }

    #[test]
    fn test_prepare_scrolls_down_just_far_enough_to_show_the_cursor() {
        let mut view = ListView {
            cursor: 20,
            ..ListView::default()
        };
        view.prepare(panel(), 40);
        // 14 inner rows: the cursor is the last visible row.
        assert_eq!(view.offset, 7);
    }

    #[test]
    fn test_step_wraps_at_both_ends() {
        let mut view = ListView::default();
        view.step(false, 5);
        assert_eq!(view.cursor, 4);
        view.step(true, 5);
        assert_eq!(view.cursor, 0);
    }

    #[test]
    fn test_prepare_clamps_a_cursor_left_past_a_shorter_list() {
        let mut view = ListView {
            cursor: 39,
            offset: 30,
            ..ListView::default()
        };
        view.prepare(panel(), 5);
        assert_eq!(view.cursor, 4);
        assert_eq!(view.offset, 0);
    }

    #[test]
    fn test_row_at_maps_the_first_inner_row_to_the_offset() {
        let mut view = ListView {
            cursor: 20,
            ..ListView::default()
        };
        view.prepare(panel(), 40);
        let offset = view.offset;
        assert!(offset > 0, "the window must have scrolled");
        // Inner rows start at (1, 1): the border takes the outer ring.
        assert_eq!(view.row_at(1, 1), Some(offset));
        assert_eq!(view.row_at(1, 2), Some(offset + 1));
        assert_eq!(view.row_at(0, 1), None, "left border is not a row");
        assert_eq!(view.row_at(1, 0), None, "top border is not a row");
        assert_eq!(view.row_at(1, 15), None, "bottom border is not a row");
        assert_eq!(view.row_at(75, 1), None, "right border is not a row");
    }

    #[test]
    fn test_row_at_is_none_past_the_last_row() {
        let mut view = ListView::default();
        view.prepare(panel(), 3);
        assert_eq!(view.row_at(1, 3), Some(2));
        assert_eq!(view.row_at(1, 4), None);
    }

    #[test]
    fn test_scroll_moves_the_window_and_pulls_the_cursor_into_it() {
        let mut view = ListView {
            cursor: 0,
            ..ListView::default()
        };
        view.prepare(panel(), 40);
        view.scroll(10, 40);
        assert_eq!(view.offset, 10);
        assert_eq!(view.cursor, 10, "cursor moves to the top visible row");
        view.scroll(-50, 40);
        assert_eq!(view.offset, 0);
        view.scroll(100, 40);
        assert_eq!(view.offset, 40 - 14, "window stops at the last full page");
    }

    #[test]
    fn test_empty_state_draws_message_and_hint_centred() {
        let empty = Empty {
            message: "no profiles yet",
            hint: Some(("+", "new profile")),
        };
        let buf = drawn(Vec::new(), &ListView::default(), Some(empty));
        let text: Vec<String> = (0..16).map(|y| row_text(&buf, y)).collect();
        let message = text.iter().find(|r| r.contains("no profiles yet")).unwrap();
        let hint = text.iter().find(|r| r.contains("new profile")).unwrap();
        assert!(hint.contains("+ new profile"));
        assert!(
            message.find("no profiles").unwrap() > 1,
            "centred, not at the edge"
        );
    }

    #[test]
    fn test_empty_state_message_uses_text_dim_and_the_hint_key_uses_key() {
        let c = colors();
        let backend = TestBackend::new(76, 16);
        let mut terminal = Terminal::new(backend).unwrap();
        let empty = Empty {
            message: "none",
            hint: Some(("+", "add")),
        };
        terminal
            .draw(|f| {
                render_list_panel(
                    f,
                    panel(),
                    &c,
                    "x",
                    Vec::new(),
                    &ListView::default(),
                    Some(empty),
                )
            })
            .unwrap();
        let buf = terminal.backend().buffer().clone();
        let (mut message_fg, mut key_fg) = (None, None);
        for y in 0..16 {
            for x in 0..76 {
                let cell = &buf[(x, y)];
                match cell.symbol() {
                    "n" if message_fg.is_none() => message_fg = Some(cell.fg),
                    "+" => key_fg = Some(cell.fg),
                    _ => {}
                }
            }
        }
        assert_eq!(message_fg, Some(c.text_dim));
        assert_eq!(key_fg, Some(c.key));
    }

    #[test]
    fn test_rows_are_drawn_from_the_window_offset() {
        let mut view = ListView {
            cursor: 20,
            ..ListView::default()
        };
        view.prepare(panel(), 40);
        let buf = drawn(numbered(40), &view, None);
        let first = row_text(&buf, 1);
        assert!(first.contains(&format!("row {}", view.offset)), "{first:?}");
    }

    #[test]
    fn test_overflowing_rows_draw_a_scrollbar_on_the_border() {
        let view = ListView::default();
        let buf = drawn(numbered(40), &view, None);
        let track: String = (1..15).map(|y| buf[(75, y)].symbol().to_string()).collect();
        assert!(
            track.contains('█') || track.contains('┃'),
            "no thumb: {track:?}"
        );
    }

    #[test]
    fn test_selected_row_style_survives_the_panel() {
        let c = colors();
        let selected = Line::from(Span::styled("picked", Style::default().bg(c.selection)));
        let buf = drawn(vec![selected], &ListView::default(), None);
        assert_eq!(buf[(1, 1)].bg, c.selection);
    }

    fn mouse(kind: MouseEventKind, column: u16, row: u16) -> MouseEvent {
        MouseEvent {
            kind,
            column,
            row,
            modifiers: KeyModifiers::NONE,
        }
    }

    #[test]
    fn test_click_selects_the_row_under_the_pointer_then_activates_it() {
        let mut view = ListView::default();
        view.prepare(panel(), 40);
        let down = MouseEventKind::Down(MouseButton::Left);
        // Row y = 3 is the third inner row (inner starts at y = 1); x = 5 is column 4 of it.
        let first = click_list(&mut view, mouse(down, 5, 3), 40);
        assert_eq!(
            first,
            Some(ListClick {
                row: 2,
                col: 4,
                activate: false
            })
        );
        assert_eq!(view.cursor, 2);
        let second = click_list(&mut view, mouse(down, 5, 3), 40);
        assert_eq!(second.map(|c| c.activate), Some(true));
    }

    #[test]
    fn test_click_outside_the_rows_leaves_the_list_alone() {
        let mut view = ListView::default();
        view.prepare(panel(), 3);
        let down = MouseEventKind::Down(MouseButton::Left);
        assert_eq!(
            click_list(&mut view, mouse(down, 0, 1), 3),
            None,
            "left border"
        );
        assert_eq!(
            click_list(&mut view, mouse(down, 1, 0), 3),
            None,
            "top border"
        );
        assert_eq!(
            click_list(&mut view, mouse(down, 1, 4), 3),
            None,
            "past the last row"
        );
        assert_eq!(view.cursor, 0);
    }

    #[test]
    fn test_wheel_moves_the_cursor_one_row_and_wraps_like_the_arrows() {
        let mut view = ListView::default();
        view.prepare(panel(), 5);
        let up = MouseEventKind::ScrollUp;
        let down = MouseEventKind::ScrollDown;
        let click = click_list(&mut view, mouse(down, 5, 5), 5);
        assert_eq!(
            click,
            Some(ListClick {
                row: 1,
                col: 0,
                activate: false
            })
        );
        assert_eq!(
            click_list(&mut view, mouse(up, 5, 5), 5).map(|c| c.row),
            Some(0)
        );
        assert_eq!(
            click_list(&mut view, mouse(up, 5, 5), 5).map(|c| c.row),
            Some(4)
        );
        assert_eq!(
            click_list(&mut view, mouse(up, 0, 0), 5),
            None,
            "outside the panel"
        );
    }

    #[test]
    fn test_other_mouse_events_do_nothing() {
        let mut view = ListView::default();
        view.prepare(panel(), 5);
        let moved = mouse(MouseEventKind::Moved, 5, 2);
        let right = mouse(MouseEventKind::Down(MouseButton::Right), 5, 2);
        assert_eq!(click_list(&mut view, moved, 5), None);
        assert_eq!(click_list(&mut view, right, 5), None);
        assert_eq!(view.cursor, 0);
    }
}
