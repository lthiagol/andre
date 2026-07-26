use ratatui::{
    layout::{Alignment, Rect},
    style::{Color, Style},
    text::{Line, Span, Text},
    widgets::Paragraph,
    Frame,
};

use crate::theme::ThemeColors;

const BIG_COLORS: [Color; 5] = [
    Color::Rgb(255, 80, 80),
    Color::Rgb(255, 160, 40),
    Color::Rgb(255, 215, 0),
    Color::Rgb(80, 200, 80),
    Color::Rgb(0, 180, 255),
];

const LETTERS: [char; 5] = ['A', 'N', 'D', 'R', 'E'];

const TAGLINE: &str = "Another Neat Dotfile Repository Engine";

fn big_title() -> Vec<Line<'static>> {
    let bw = "\u{2554}\u{2550}\u{2550}\u{2550}\u{2550}\u{2557}"
        .chars()
        .count();
    let n = LETTERS.len();
    let top_count = n.div_ceil(2);
    let stride = TAGLINE.chars().count().saturating_sub(bw) / top_count.saturating_sub(1).max(1);
    let offset = stride / 2;
    let total_width = stride * top_count.saturating_sub(1) + bw;

    let mut lines: Vec<Line<'static>> = Vec::with_capacity(4);
    for r in 0..4usize {
        let mut items: Vec<(usize, Color, String)> = Vec::new();
        for (i, &color) in BIG_COLORS.iter().enumerate() {
            let base_row = i % 2;
            let rib = r as isize - base_row as isize;
            if !(0..=2).contains(&rib) {
                continue;
            }
            let col = (i / 2) * stride + (i % 2) * offset;
            let text = match rib {
                0 => "\u{2554}\u{2550}\u{2550}\u{2550}\u{2550}\u{2557}".to_string(),
                1 => format!("\u{2551} {}. \u{2551}", LETTERS[i]),
                _ => "\u{255a}\u{2550}\u{2550}\u{2550}\u{2550}\u{255d}".to_string(),
            };
            items.push((col, color, text));
        }
        items.sort_by_key(|(c, _, _)| *c);
        let mut spans: Vec<Span> = Vec::new();
        let mut cursor = 0usize;
        for (col, color, text) in items {
            if col > cursor {
                spans.push(Span::raw(" ".repeat(col - cursor)));
            }
            spans.push(Span::styled(text, Style::default().fg(color)));
            cursor = col + bw;
        }
        if cursor < total_width {
            spans.push(Span::raw(" ".repeat(total_width - cursor)));
        }
        lines.push(Line::from(spans));
    }
    lines
}

pub fn render_banner(frame: &mut Frame, area: Rect, colors: &ThemeColors) {
    if area.width < 60 {
        let text = Text::from(vec![
            Line::from(""),
            Line::from(vec![Span::styled(
                "andre",
                Style::default().fg(colors.primary),
            )]),
            Line::from(vec![Span::styled(
                TAGLINE,
                Style::default().fg(colors.secondary),
            )]),
        ]);
        let paragraph = Paragraph::new(text).alignment(Alignment::Center);
        frame.render_widget(paragraph, area);
        return;
    }

    let title = big_title();
    let mut rows = vec![Line::from("")];
    rows.extend(title);
    rows.push(Line::from(""));
    rows.push(Line::from(TAGLINE));

    let paragraph = Paragraph::new(rows)
        .alignment(Alignment::Center)
        .style(Style::default().fg(colors.primary));
    frame.render_widget(paragraph, area);
}
