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

/// Draws `d` in a panel titled `title`. One blank line separates the headline, the facts
/// and the sections; text is inset one column from the border.
pub fn render_detail(frame: &mut Frame, area: Rect, colors: &ThemeColors, title: &str, d: &Detail) {
    let block = themed_panel(colors).title(format!(" {title} "));
    let inner = block.inner(area).inner(Margin::new(1, 0));
    frame.render_widget(block, area);

    let mut lines: Vec<Line> = d.headline.clone();
    if !d.facts.is_empty() {
        if !lines.is_empty() {
            lines.push(Line::raw(""));
        }
        for (label, value) in &d.facts {
            lines.push(Line::from(vec![
                Span::styled(
                    format!("{label:<LABEL_WIDTH$}"),
                    Style::default().fg(colors.text_dim),
                ),
                Span::styled(value.clone(), Style::default().fg(colors.value)),
            ]));
        }
    }
    if !d.sections.is_empty() {
        if !lines.is_empty() {
            lines.push(Line::raw(""));
        }
        lines.extend(d.sections.iter().cloned());
    }

    frame.render_widget(Paragraph::new(lines).wrap(Wrap { trim: false }), inner);
}

#[cfg(test)]
mod tests {
    use ratatui::{backend::TestBackend, buffer::Buffer, text::Line, Terminal};

    use super::*;
    use andre_core::Theme;

    fn draw(d: &Detail) -> Buffer {
        let c = ThemeColors::from_theme(Theme::Default);
        let mut terminal = Terminal::new(TestBackend::new(40, 8)).unwrap();
        terminal
            .draw(|f| render_detail(f, Rect::new(0, 0, 40, 8), &c, "title", d))
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
}
