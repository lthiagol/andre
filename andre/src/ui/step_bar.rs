//! The step bar of a multi-step flow: done steps in `ok` with a check, the current step in
//! `accent` in brackets, the steps still to come in `text_dim`. Steps are numbered from 1,
//! and the done ones are not, as in `✓ group › [2 name] › 3 source`.

use crossterm::event::{MouseButton, MouseEvent, MouseEventKind};
use ratatui::{
    layout::{Position, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
};
use unicode_width::UnicodeWidthStr;

use crate::theme::ThemeColors;
use crate::ui::icons::Icons;

/// Between two steps, drawn in `text_dim`.
const SEPARATOR: &str = " \u{203a} ";

pub struct StepBar {
    pub steps: &'static [&'static str],
    /// Index of the step in progress.
    pub current: usize,
}

/// The text of each step: `✓ name` when done, `[n name]` when current, `n name` otherwise.
fn step_texts(s: &StepBar, icons: &Icons) -> Vec<String> {
    s.steps
        .iter()
        .enumerate()
        .map(|(i, name)| {
            if i < s.current {
                format!("{} {name}", icons.check)
            } else if i == s.current {
                format!("[{} {name}]", i + 1)
            } else {
                format!("{} {name}", i + 1)
            }
        })
        .collect()
}

/// The bar as one line. Steps are separated by [`SEPARATOR`].
pub fn step_line(s: &StepBar, icons: &Icons, colors: &ThemeColors) -> Line<'static> {
    let mut spans = Vec::new();
    for (i, text) in step_texts(s, icons).into_iter().enumerate() {
        if i > 0 {
            spans.push(Span::styled(
                SEPARATOR,
                Style::default().fg(colors.text_dim),
            ));
        }
        let style = if i < s.current {
            Style::default().fg(colors.ok)
        } else if i == s.current {
            Style::default()
                .fg(colors.accent)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(colors.text_dim)
        };
        spans.push(Span::styled(text, style));
    }
    Line::from(spans)
}

/// The rect of each step's text on `area`'s first row, paired with the step index. A step
/// cut by the edge is clipped to the area; steps past it are left out.
pub fn step_rects(area: Rect, s: &StepBar, icons: &Icons) -> Vec<(Rect, usize)> {
    let gap = SEPARATOR.width() as u16;
    let mut x = area.x;
    let mut out = Vec::new();
    for (i, text) in step_texts(s, icons).iter().enumerate() {
        let width = text.width() as u16;
        let rect = Rect::new(x, area.y, width, 1).intersection(area);
        if rect.width > 0 {
            out.push((rect, i));
        }
        x = x.saturating_add(width + gap);
    }
    out
}

/// The index of the step a left click at `ev` lands on, from the rects [`step_rects`] gave.
pub fn step_click(hits: &[(Rect, usize)], ev: MouseEvent) -> Option<usize> {
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
    use super::*;
    use crossterm::event::{MouseButton, MouseEvent, MouseEventKind};

    use crate::ui::icons::UNICODE;
    use andre_core::Theme;

    const STEPS: &[&str] = &["source", "name", "files", "ignore", "review"];

    fn colors() -> ThemeColors {
        ThemeColors::from_theme(Theme::Default)
    }

    fn text(line: &Line) -> String {
        line.spans.iter().map(|s| s.content.as_ref()).collect()
    }

    #[test]
    fn test_done_current_and_next_steps_are_marked() {
        let bar = StepBar {
            steps: STEPS,
            current: 2,
        };
        assert_eq!(
            text(&step_line(&bar, &UNICODE, &colors())),
            "\u{2713} source \u{203a} \u{2713} name \u{203a} [3 files] \u{203a} 4 ignore \u{203a} 5 review"
        );
    }

    #[test]
    fn test_done_steps_are_ok_current_is_accent_bold_and_next_is_text_dim() {
        let c = colors();
        let bar = StepBar {
            steps: STEPS,
            current: 2,
        };
        let line = step_line(&bar, &UNICODE, &c);
        let by_text = |t: &str| line.spans.iter().find(|s| s.content.contains(t)).unwrap();
        assert_eq!(by_text("source").style.fg, Some(c.ok));
        assert_eq!(by_text("[3 files]").style.fg, Some(c.accent));
        assert!(by_text("[3 files]")
            .style
            .add_modifier
            .contains(Modifier::BOLD));
        assert_eq!(by_text("4 ignore").style.fg, Some(c.text_dim));
        let separator = line.spans.iter().find(|s| s.content == SEPARATOR).unwrap();
        assert_eq!(separator.style.fg, Some(c.text_dim));
    }

    #[test]
    fn test_first_step_has_no_done_steps_before_it() {
        let bar = StepBar {
            steps: STEPS,
            current: 0,
        };
        assert!(text(&step_line(&bar, &UNICODE, &colors())).starts_with("[1 source]"));
    }

    #[test]
    fn test_step_rects_cover_each_step_text_in_order() {
        let bar = StepBar {
            steps: STEPS,
            current: 1,
        };
        let area = Rect::new(0, 0, 76, 1);
        let rects = step_rects(area, &bar, &UNICODE);
        assert_eq!(rects.len(), 5);
        assert_eq!(
            rects.iter().map(|(_, i)| *i).collect::<Vec<_>>(),
            vec![0, 1, 2, 3, 4]
        );
        // "✓ source" is 8 columns, then the 3-column separator.
        assert_eq!(rects[0].0, Rect::new(0, 0, 8, 1));
        assert_eq!(rects[1].0.x, 11);
        let drawn = text(&step_line(&bar, &UNICODE, &colors())).width();
        assert_eq!(usize::from(rects[4].0.right()), drawn);
    }

    #[test]
    fn test_step_rects_drop_steps_past_the_edge() {
        let bar = StepBar {
            steps: STEPS,
            current: 0,
        };
        let rects = step_rects(Rect::new(0, 0, 12, 1), &bar, &UNICODE);
        assert!(rects.len() < 5);
        assert!(rects.iter().all(|(r, _)| r.right() <= 12));
    }

    #[test]
    fn test_step_marks_use_the_icon_set_for_done_steps() {
        let bar = StepBar {
            steps: &["a", "b"],
            current: 1,
        };
        let nerd = text(&step_line(&bar, &crate::ui::icons::NERD, &colors()));
        assert!(nerd.starts_with("\u{f00c} a"), "{nerd:?}");
    }

    #[test]
    fn test_step_click_gives_the_index_of_the_step_under_the_press() {
        let bar = StepBar {
            steps: &["group", "name", "source"],
            current: 2,
        };
        let hits = step_rects(Rect::new(0, 0, 60, 1), &bar, &UNICODE);
        let at = |x| MouseEvent {
            kind: MouseEventKind::Down(MouseButton::Left),
            column: x,
            row: 0,
            modifiers: crossterm::event::KeyModifiers::NONE,
        };
        assert_eq!(step_click(&hits, at(0)), Some(0), "the done group step");
        assert_eq!(step_click(&hits, at(3)), Some(0), "inside the done step");
        assert_eq!(step_click(&hits, at(60)), None, "past the last step");
    }
}
