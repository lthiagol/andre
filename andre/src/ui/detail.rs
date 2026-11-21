//! The detail pane: a headline, a column of facts and a short list of sections. Built in a
//! component's `update` and only read while painting.

use std::borrow::Cow;

use ratatui::{
    layout::{Margin, Rect},
    style::Style,
    text::{Line, Span},
    widgets::{Paragraph, Wrap},
    Frame,
};
use unicode_width::UnicodeWidthStr;

use crate::theme::ThemeColors;
use crate::ui::block::themed_panel;

/// Width of the label column in the facts.
const LABEL_WIDTH: usize = 10;
/// Widest a fact's value may be, so it stays on one line in the detail pane at 80
/// columns. Wider panes do not use the spare room; the detail wraps prose only.
pub const VALUE_WIDTH: usize = 27;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Detail {
    /// One or two lines under the title. A status headline starts with its glyph.
    pub headline: Vec<Line<'static>>,
    /// Label (`text_dim`, column 10) and value (`value`).
    pub facts: Vec<(Cow<'static, str>, String)>,
    /// Short lists below the facts: files, packages, patterns.
    pub sections: Vec<Line<'static>>,
}

/// The text area inside a detail pane drawn in `area`: inside the border, one column in.
pub fn detail_inner(area: Rect) -> Rect {
    // One row and column for the border, one more column of margin.
    area.inner(Margin::new(2, 1))
}

/// One fact as the detail draws it: the label in `text_dim`, the value in `value`. A
/// section that lists facts uses this too, so both read the same.
pub fn fact_line(label: &str, value: &str, colors: &ThemeColors) -> Line<'static> {
    Line::from(vec![
        Span::styled(
            format!("{label:<LABEL_WIDTH$}"),
            Style::default().fg(colors.text_dim),
        ),
        Span::styled(value.to_string(), Style::default().fg(colors.value)),
    ])
}

/// The rows `d` draws, before wrapping.
fn detail_lines(d: &Detail, colors: &ThemeColors) -> Vec<Line<'static>> {
    let mut lines: Vec<Line> = d.headline.clone();
    if !d.facts.is_empty() {
        if !lines.is_empty() {
            lines.push(Line::raw(""));
        }
        for (label, value) in &d.facts {
            lines.push(fact_line(label, value, colors));
        }
    }
    if !d.sections.is_empty() {
        if !lines.is_empty() {
            lines.push(Line::raw(""));
        }
        lines.extend(d.sections.iter().cloned());
    }
    lines
}

/// Draws `d` in a panel titled `title`. One blank line separates the headline, the facts
/// and the sections; text is inset one column from the border.
pub fn render_detail(frame: &mut Frame, area: Rect, colors: &ThemeColors, title: &str, d: &Detail) {
    render_detail_scrolled(frame, area, colors, title, d, 0);
}

/// `render_detail` with the text starting `scroll` rows down. Clamp `scroll` with
/// [`fit_scroll`] first; a value past the end draws blank rows.
pub fn render_detail_scrolled(
    frame: &mut Frame,
    area: Rect,
    colors: &ThemeColors,
    title: &str,
    d: &Detail,
    scroll: u16,
) {
    let block = themed_panel(colors).title(format!(" {title} "));
    let inner = detail_inner(area);
    frame.render_widget(block, area);

    let lines = detail_lines(d, colors);
    frame.render_widget(
        Paragraph::new(lines)
            .wrap(Wrap { trim: false })
            .scroll((scroll, 0)),
        inner,
    );
}

/// `scroll`, kept no further down than the last row of `d`'s text at the size of `area`.
/// The bottom of the text can be reached, and no blank rows are scrolled past.
pub fn fit_scroll(d: &Detail, area: Rect, colors: &ThemeColors, scroll: u16) -> u16 {
    let inner = detail_inner(area);
    let rows = wrapped_rows(&detail_lines(d, colors), inner.width);
    let last = rows.saturating_sub(usize::from(inner.height));
    scroll.min(u16::try_from(last).unwrap_or(u16::MAX))
}

/// The rows `lines` take when each breaks at spaces to fit `width` columns. A word wider
/// than a row breaks inside itself. This is the estimate `fit_scroll` clamps with, so it
/// follows the paragraph's greedy word wrap.
pub fn wrapped_rows(lines: &[Line], width: u16) -> usize {
    let width = usize::from(width).max(1);
    lines
        .iter()
        .map(|line| {
            let text: String = line.spans.iter().map(|s| s.content.as_ref()).collect();
            text_rows(&text, width)
        })
        .sum()
}

/// Rows one line of text takes at `width` columns.
fn text_rows(text: &str, width: usize) -> usize {
    let mut rows = 1;
    let mut used = 0;
    for word in text.split(' ') {
        let w = word.width();
        let gap = usize::from(used > 0);
        if used + gap + w <= width {
            used += gap + w;
            continue;
        }
        // The word starts a new row. One wider than a row takes several, the last one
        // partly filled.
        if used > 0 {
            rows += 1;
        }
        if w > width {
            let breaks = (w - 1) / width;
            rows += breaks;
            used = w - breaks * width;
        } else {
            used = w;
        }
    }
    rows
}

/// One line per line of `text`, so an engine's multi-line message keeps its rows.
/// `Line::styled` would join them into one line.
pub fn text_lines(text: &str, style: Style) -> Vec<Line<'static>> {
    text.lines()
        .map(|line| Line::styled(line.to_string(), style))
        .collect()
}

#[cfg(test)]
mod tests {
    use ratatui::{backend::TestBackend, buffer::Buffer, text::Line, Terminal};

    use super::*;
    use andre_core::Theme;

    fn draw(d: &Detail) -> Buffer {
        draw_scrolled(d, 0)
    }

    fn draw_scrolled(d: &Detail, scroll: u16) -> Buffer {
        let c = ThemeColors::from_theme(Theme::Default);
        let mut terminal = Terminal::new(TestBackend::new(40, 8)).unwrap();
        terminal
            .draw(|f| render_detail_scrolled(f, Rect::new(0, 0, 40, 8), &c, "title", d, scroll))
            .unwrap();
        terminal.backend().buffer().clone()
    }

    /// The text inside the panel border on row `y`.
    fn row_text(buf: &Buffer, y: u16) -> String {
        (1..buf.area().width - 1)
            .map(|x| buf[(x, y)].symbol())
            .collect::<String>()
            .trim_end()
            .to_string()
    }

    #[test]
    fn test_headline_facts_and_sections_are_separated_by_one_blank_line() {
        let d = Detail {
            headline: vec![Line::raw("headline")],
            facts: vec![(Cow::Borrowed("packages"), "9".to_string())],
            sections: vec![Line::raw("section")],
        };
        let buf = draw(&d);
        assert!(row_text(&buf, 1).contains("headline"));
        assert_eq!(row_text(&buf, 2), "", "blank after the headline");
        assert!(row_text(&buf, 3).contains("packages"));
        assert_eq!(row_text(&buf, 4), "", "blank before the sections");
        assert!(row_text(&buf, 5).contains("section"));
    }

    #[test]
    fn test_fact_label_takes_ten_columns_then_the_value() {
        let d = Detail {
            facts: vec![(Cow::Borrowed("stowed"), "1 of 2".to_string())],
            ..Detail::default()
        };
        let buf = draw(&d);
        // Inset: one margin column inside the border. Label padded to 10, then the value.
        let row = row_text(&buf, 1);
        assert_eq!(row, " stowed    1 of 2", "{row:?}");
    }

    #[test]
    fn test_no_blank_first_row_when_there_is_no_headline() {
        let d = Detail {
            facts: vec![(Cow::Borrowed("targets"), "~".to_string())],
            ..Detail::default()
        };
        let buf = draw(&d);
        assert!(row_text(&buf, 1).contains("targets"));
    }

    #[test]
    fn test_empty_detail_draws_only_the_panel() {
        let buf = draw(&Detail::default());
        assert_eq!(row_text(&buf, 2), "");
    }

    fn lines_of(texts: &[&str]) -> Detail {
        Detail {
            sections: texts.iter().map(|t| Line::raw(t.to_string())).collect(),
            ..Detail::default()
        }
    }

    #[test]
    fn test_scroll_starts_the_text_that_many_rows_down() {
        let d = lines_of(&["row0", "row1", "row2", "row3", "row4"]);
        let buf = draw_scrolled(&d, 2);
        assert!(
            row_text(&buf, 1).contains("row2"),
            "{:?}",
            row_text(&buf, 1)
        );
    }

    #[test]
    fn test_fit_scroll_stops_when_the_last_row_is_at_the_bottom() {
        // 40x8 pane: six rows inside the border, ten rows of text.
        let area = Rect::new(0, 0, 40, 8);
        let colors = ThemeColors::from_theme(Theme::Default);
        let d = lines_of(&["r0", "r1", "r2", "r3", "r4", "r5", "r6", "r7", "r8", "r9"]);
        assert_eq!(
            fit_scroll(&d, area, &colors, 99),
            4,
            "ten rows in six leave four"
        );
        assert_eq!(fit_scroll(&d, area, &colors, 1), 1, "in range, unchanged");
    }

    #[test]
    fn test_fit_scroll_leaves_short_text_at_the_top() {
        let area = Rect::new(0, 0, 40, 8);
        let colors = ThemeColors::from_theme(Theme::Default);
        let d = lines_of(&["r0", "r1"]);
        assert_eq!(fit_scroll(&d, area, &colors, 3), 0);
    }

    #[test]
    fn test_wrapped_rows_breaks_at_spaces_then_inside_long_words() {
        assert_eq!(text_rows("aaa bbb", 4), 2, "each word fills a row");
        assert_eq!(
            text_rows("a b c", 3),
            2,
            "a space and c do not fit together"
        );
        assert_eq!(text_rows("abcdefghij", 4), 3, "4 + 4 + 2 columns");
        assert_eq!(text_rows("", 4), 1, "an empty line still takes a row");
    }

    #[test]
    fn test_text_lines_keep_one_row_per_line() {
        let lines = text_lines("a: x\nAll operations aborted.", Style::default());
        let texts: Vec<String> = lines
            .iter()
            .map(|l| l.spans.iter().map(|s| s.content.as_ref()).collect())
            .collect();
        assert_eq!(texts, ["a: x", "All operations aborted."]);
    }
}
