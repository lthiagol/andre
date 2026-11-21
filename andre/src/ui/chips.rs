//! The chip row above a list or the panes: filters and tabs. The active chip is drawn in
//! brackets in `accent`, the others in `text_dim`. Right-hand spans sit at the right edge.

use crossterm::event::{KeyEvent, MouseButton, MouseEvent, MouseEventKind};
use ratatui::{
    layout::{Position, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
};
use unicode_width::UnicodeWidthStr;

use crate::theme::ThemeColors;

/// Columns between two chips.
const CHIP_GAP: u16 = 1;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Chip {
    pub label: String,
    pub active: bool,
    /// The key that selects this chip.
    pub key: KeyEvent,
}

/// The text of a chip: `[label]` when active, ` label ` otherwise, so the two keep the
/// same footprint and switching does not move the row.
fn chip_text(chip: &Chip) -> String {
    if chip.active {
        format!("[{}]", chip.label)
    } else {
        format!(" {} ", chip.label)
    }
}

/// The chip as one span, in its style.
pub fn chip_span(chip: &Chip, colors: &ThemeColors) -> Span<'static> {
    Span::styled(chip_text(chip), chip_style(chip, colors))
}

/// The columns the chip takes.
pub fn chip_width(chip: &Chip) -> usize {
    chip_text(chip).width()
}

fn chip_style(chip: &Chip, colors: &ThemeColors) -> Style {
    if chip.active {
        Style::default()
            .fg(colors.accent)
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(colors.text_dim)
    }
}

/// The row as one line `width` columns wide: the chips from the left, `right` at the edge.
pub fn chip_line(
    chips: &[Chip],
    right: Vec<Span<'static>>,
    colors: &ThemeColors,
    width: u16,
) -> Line<'static> {
    let mut spans = Vec::new();
    for (i, chip) in chips.iter().enumerate() {
        if i > 0 {
            spans.push(Span::raw(" ".repeat(usize::from(CHIP_GAP))));
        }
        spans.push(chip_span(chip, colors));
    }
    let used: usize = spans.iter().map(|s| s.content.width()).sum();
    let right_width: usize = right.iter().map(|s| s.content.width()).sum();
    let gap = usize::from(width).saturating_sub(used + right_width);
    spans.push(Span::raw(" ".repeat(gap)));
    spans.extend(right);
    Line::from(spans)
}

/// The rect of each chip on `area`'s first row, paired with the chip's index in `chips`. A
/// chip cut by the edge is clipped; chips past it are left out.
pub fn chip_rects(area: Rect, chips: &[Chip]) -> Vec<(Rect, usize)> {
    let mut x = area.x;
    let mut out = Vec::new();
    for (i, chip) in chips.iter().enumerate() {
        if i > 0 {
            x = x.saturating_add(CHIP_GAP);
        }
        let width = chip_text(chip).width() as u16;
        let rect = Rect::new(x, area.y, width, 1).intersection(area);
        if rect.width > 0 {
            out.push((rect, i));
        }
        x = x.saturating_add(width);
    }
    out
}

/// The index of the chip a left click at `ev` lands on, from the rects [`chip_rects`] gave.
pub fn chip_click(hits: &[(Rect, usize)], ev: MouseEvent) -> Option<usize> {
    if ev.kind != MouseEventKind::Down(MouseButton::Left) {
        return None;
    }
    let at = Position::new(ev.column, ev.row);
    hits.iter()
        .find(|(rect, _)| rect.contains(at))
        .map(|(_, index)| *index)
}

#[cfg(test)]
mod tests {
    use crossterm::event::{KeyCode, KeyModifiers, MouseButton, MouseEventKind};
    use ratatui::text::Span;

    use super::*;
    use andre_core::Theme;

    fn chip(label: &str, active: bool, key: char) -> Chip {
        Chip {
            label: label.to_string(),
            active,
            key: KeyEvent::new(KeyCode::Char(key), KeyModifiers::NONE),
        }
    }

    fn colors() -> ThemeColors {
        ThemeColors::from_theme(Theme::Default)
    }

    fn text(line: &Line) -> String {
        line.spans.iter().map(|s| s.content.as_ref()).collect()
    }

    #[test]
    fn test_active_chip_is_bracketed_and_the_others_are_padded_to_the_same_footprint() {
        let chips = [chip("all", true, 'a'), chip("home", false, 'h')];
        let line = chip_line(&chips, Vec::new(), &colors(), 30);
        assert!(text(&line).starts_with("[all]  home "), "{:?}", text(&line));
    }

    #[test]
    fn test_active_chip_is_accent_and_bold_inactive_is_text_dim() {
        let c = colors();
        let chips = [chip("all", true, 'a'), chip("home", false, 'h')];
        let line = chip_line(&chips, Vec::new(), &c, 30);
        let active = line.spans.iter().find(|s| s.content == "[all]").unwrap();
        assert_eq!(active.style.fg, Some(c.accent));
        assert!(active.style.add_modifier.contains(Modifier::BOLD));
        let inactive = line.spans.iter().find(|s| s.content == " home ").unwrap();
        assert_eq!(inactive.style.fg, Some(c.text_dim));
    }

    #[test]
    fn test_right_spans_sit_at_the_right_edge() {
        let chips = [chip("all", true, 'a')];
        let right = vec![Span::raw("3 of 9")];
        let line = chip_line(&chips, right, &colors(), 20);
        assert_eq!(line.width(), 20);
        assert!(text(&line).ends_with("3 of 9"));
    }

    #[test]
    fn test_chip_rects_follow_the_row_and_carry_each_index() {
        let chips = [chip("all", true, 'a'), chip("home", false, 'h')];
        let rects = chip_rects(Rect::new(0, 0, 30, 1), &chips);
        assert_eq!(
            rects,
            vec![(Rect::new(0, 0, 5, 1), 0), (Rect::new(6, 0, 6, 1), 1)]
        );
    }

    #[test]
    fn test_chip_rects_leave_out_chips_past_the_edge() {
        let chips = [chip("all", true, 'a'), chip("home", false, 'h')];
        // "[all]" fills columns 0..5; the second chip starts at column 6, past a 6-column row.
        let rects = chip_rects(Rect::new(0, 0, 6, 1), &chips);
        assert_eq!(rects.len(), 1);
        assert!(rects.iter().all(|(r, _)| r.right() <= 6));
    }

    fn press(x: u16, y: u16) -> MouseEvent {
        MouseEvent {
            kind: MouseEventKind::Down(MouseButton::Left),
            column: x,
            row: y,
            modifiers: KeyModifiers::NONE,
        }
    }

    #[test]
    fn test_chip_click_gives_the_index_of_the_chip_under_the_press() {
        let chips = [chip("all", true, 'a'), chip("home", false, 'h')];
        let hits = chip_rects(Rect::new(0, 0, 30, 1), &chips);
        assert_eq!(chip_click(&hits, press(1, 0)), Some(0));
        assert_eq!(chip_click(&hits, press(7, 0)), Some(1));
        assert_eq!(
            chip_click(&hits, press(5, 0)),
            None,
            "the gap between chips"
        );
        assert_eq!(chip_click(&hits, press(1, 1)), None, "the row below");
    }

    #[test]
    fn test_chip_click_ignores_the_wheel() {
        let chips = [chip("all", true, 'a')];
        let hits = chip_rects(Rect::new(0, 0, 30, 1), &chips);
        let wheel = MouseEvent {
            kind: MouseEventKind::ScrollUp,
            ..press(1, 0)
        };
        assert_eq!(chip_click(&hits, wheel), None);
    }
}
