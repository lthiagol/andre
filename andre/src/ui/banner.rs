use std::sync::OnceLock;

use ratatui::{
    layout::{Alignment, Rect},
    style::{Modifier, Style},
    text::{Line, Span, Text},
    widgets::Paragraph,
    Frame,
};

use crate::theme::ThemeColors;

const LETTERS: [char; 5] = ['A', 'N', 'D', 'R', 'E'];

const TAGLINE: &str = "Another Neat Dotfile Repository Engine";

/// The name on the one-row title of screens other than the main menu.
const COMPACT_NAME: &str = "andre";

/// One letter's box on a row of the big title. `letter` indexes `LETTERS`
/// and `ThemeColors::banner`.
struct Glyph {
    col: usize,
    letter: usize,
    text: String,
}

/// Where every letter box sits. Colour is not part of it, so one cache serves
/// every theme.
struct Geometry {
    rows: Vec<Vec<Glyph>>,
    box_width: usize,
    width: usize,
}

/// Built once per process. The layout is fully static — rebuilding it on every
/// render frame was pure waste (5 `format!()` calls + Vec allocations per row ×
/// 4 rows × every redraw).
static GEOMETRY: OnceLock<Geometry> = OnceLock::new();

fn geometry() -> &'static Geometry {
    GEOMETRY.get_or_init(|| {
        let box_width = "\u{2554}\u{2550}\u{2550}\u{2550}\u{2550}\u{2557}"
            .chars()
            .count();
        let top_count = LETTERS.len().div_ceil(2);
        let stride =
            TAGLINE.chars().count().saturating_sub(box_width) / top_count.saturating_sub(1).max(1);
        let offset = stride / 2;
        let width = stride * top_count.saturating_sub(1) + box_width;

        let mut rows: Vec<Vec<Glyph>> = Vec::with_capacity(4);
        for r in 0..4usize {
            let mut glyphs: Vec<Glyph> = Vec::new();
            for (letter, &name) in LETTERS.iter().enumerate() {
                let base_row = letter % 2;
                let rib = r as isize - base_row as isize;
                if !(0..=2).contains(&rib) {
                    continue;
                }
                let col = (letter / 2) * stride + (letter % 2) * offset;
                let text = match rib {
                    0 => "\u{2554}\u{2550}\u{2550}\u{2550}\u{2550}\u{2557}".to_string(),
                    1 => format!("\u{2551} {name}. \u{2551}"),
                    _ => "\u{255a}\u{2550}\u{2550}\u{2550}\u{2550}\u{255d}".to_string(),
                };
                glyphs.push(Glyph { col, letter, text });
            }
            glyphs.sort_by_key(|g| g.col);
            rows.push(glyphs);
        }
        Geometry {
            rows,
            box_width,
            width,
        }
    })
}

/// The big title's four rows, each letter in its theme colour.
fn big_title(colors: &ThemeColors) -> Vec<Line<'static>> {
    let geometry = geometry();
    geometry
        .rows
        .iter()
        .map(|glyphs| {
            let mut spans: Vec<Span> = Vec::new();
            let mut cursor = 0usize;
            for glyph in glyphs {
                if glyph.col > cursor {
                    spans.push(Span::raw(" ".repeat(glyph.col - cursor)));
                }
                spans.push(Span::styled(
                    glyph.text.clone(),
                    Style::default().fg(colors.banner[glyph.letter]),
                ));
                cursor = glyph.col + geometry.box_width;
            }
            if cursor < geometry.width {
                spans.push(Span::raw(" ".repeat(geometry.width - cursor)));
            }
            Line::from(spans)
        })
        .collect()
}

/// One-row title for every screen except the main menu: the name in the
/// banner's colours, then the breadcrumb.
pub fn render_compact_banner(
    frame: &mut Frame,
    area: Rect,
    colors: &ThemeColors,
    breadcrumb: &str,
) {
    let mut spans: Vec<Span> = COMPACT_NAME
        .chars()
        .zip(colors.banner)
        .map(|(letter, color)| {
            Span::styled(
                letter.to_string(),
                Style::default().fg(color).add_modifier(Modifier::BOLD),
            )
        })
        .collect();
    spans.push(Span::styled(
        format!("  {breadcrumb}"),
        Style::default().fg(colors.text_dim),
    ));
    frame.render_widget(Paragraph::new(Line::from(spans)), area);
}

pub fn render_banner(frame: &mut Frame, area: Rect, colors: &ThemeColors) {
    if area.width < 60 {
        let text = Text::from(vec![
            Line::from(""),
            Line::from(vec![Span::styled(
                "andre",
                Style::default().fg(colors.accent),
            )]),
            Line::from(vec![Span::styled(
                TAGLINE,
                Style::default().fg(colors.text_dim),
            )]),
        ]);
        let paragraph = Paragraph::new(text).alignment(Alignment::Center);
        frame.render_widget(paragraph, area);
        return;
    }

    let title = big_title(colors);
    let mut rows: Vec<Line<'static>> = Vec::with_capacity(title.len() + 3);
    rows.push(Line::from(""));
    rows.extend(title);
    rows.push(Line::from(""));
    rows.push(Line::from(TAGLINE));

    let paragraph = Paragraph::new(rows)
        .alignment(Alignment::Center)
        .style(Style::default().fg(colors.text_dim));
    frame.render_widget(paragraph, area);
}
