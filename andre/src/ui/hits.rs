//! Clickable pieces of the top bar and the footer. Each bar builds its line from [`Piece`]s,
//! and its hit rects come from the same pieces, so a click lands on the chip that is drawn.

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::{layout::Rect, text::Span};
use unicode_width::UnicodeWidthStr;

/// A run of spans on a bar. A keyed piece is clickable: a click sends its key.
#[derive(Debug, Clone)]
pub struct Piece {
    pub spans: Vec<Span<'static>>,
    pub key: Option<KeyEvent>,
}

impl Piece {
    /// Spans that do nothing when clicked, such as gaps and dividers.
    pub fn plain(spans: Vec<Span<'static>>) -> Self {
        Self { spans, key: None }
    }

    /// Spans that send `code` when clicked.
    pub fn keyed(spans: Vec<Span<'static>>, code: KeyCode) -> Self {
        Self {
            spans,
            key: Some(KeyEvent::new(code, KeyModifiers::NONE)),
        }
    }

    pub fn width(&self) -> usize {
        self.spans
            .iter()
            .map(|span| UnicodeWidthStr::width(span.content.as_ref()))
            .sum()
    }
}

/// The width of `pieces` together.
pub fn pieces_width(pieces: &[Piece]) -> usize {
    pieces.iter().map(Piece::width).sum()
}

/// The spans of `pieces`, in order.
pub fn flatten(pieces: Vec<Piece>) -> Vec<Span<'static>> {
    pieces.into_iter().flat_map(|piece| piece.spans).collect()
}

/// Where a bar's keyed pieces are drawn: one row of `area`, the pieces laid out from column
/// `start` on. A piece cut off at the right edge keeps only the columns that show.
pub fn hit_rects(area: Rect, pieces: &[Piece], start: usize) -> Vec<(Rect, KeyEvent)> {
    let edge = usize::from(area.width);
    let mut x = start;
    let mut hits = Vec::new();
    for piece in pieces {
        let width = piece.width();
        if let Some(key) = piece.key {
            if x < edge && width > 0 {
                let shown = width.min(edge - x);
                let rect = Rect::new(area.x + x as u16, area.y, shown as u16, 1);
                hits.push((rect, key));
            }
        }
        x += width;
    }
    hits
}

/// The clickable parts of the top bar and the footer, each as its rect and the key a click
/// sends. Rebuilt on every frame by `ui::render`.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct ChromeHits {
    pub top_bar: Vec<(Rect, KeyEvent)>,
    pub footer: Vec<(Rect, KeyEvent)>,
}

/// The key under a mouse position, if a hit rect covers it.
pub fn key_at(hits: &[(Rect, KeyEvent)], x: u16, y: u16) -> Option<KeyEvent> {
    hits.iter()
        .find(|(rect, _)| x >= rect.x && x < rect.right() && y == rect.y)
        .map(|(_, key)| *key)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn area() -> Rect {
        Rect::new(10, 3, 20, 1)
    }

    #[test]
    fn test_hit_rects_place_keyed_pieces_by_width_from_start() {
        let pieces = vec![
            Piece::plain(vec![Span::raw("  ")]),
            Piece::keyed(vec![Span::raw("abc")], KeyCode::Char('a')),
            Piece::plain(vec![Span::raw(" ")]),
            Piece::keyed(vec![Span::raw("de")], KeyCode::Char('d')),
        ];
        let hits = hit_rects(area(), &pieces, 1);
        assert_eq!(hits.len(), 2);
        assert_eq!(hits[0].0, Rect::new(13, 3, 3, 1));
        assert_eq!(hits[0].1.code, KeyCode::Char('a'));
        assert_eq!(hits[1].0, Rect::new(17, 3, 2, 1));
        assert_eq!(hits[1].1.code, KeyCode::Char('d'));
    }

    #[test]
    fn test_hit_rects_clip_a_piece_at_the_right_edge() {
        let pieces = vec![Piece::keyed(vec![Span::raw("wide")], KeyCode::Char('w'))];
        let hits = hit_rects(Rect::new(0, 0, 3, 1), &pieces, 1);
        assert_eq!(
            hits,
            vec![(
                Rect::new(1, 0, 2, 1),
                KeyEvent::new(KeyCode::Char('w'), KeyModifiers::NONE)
            )]
        );
    }

    #[test]
    fn test_key_at_needs_the_row_and_a_column_inside_the_rect() {
        let hits = hit_rects(
            area(),
            &[Piece::keyed(vec![Span::raw("ab")], KeyCode::Esc)],
            0,
        );
        assert_eq!(key_at(&hits, 10, 3).map(|k| k.code), Some(KeyCode::Esc));
        assert_eq!(key_at(&hits, 11, 3).map(|k| k.code), Some(KeyCode::Esc));
        assert_eq!(key_at(&hits, 12, 3), None, "past the piece");
        assert_eq!(key_at(&hits, 10, 4), None, "other row");
    }
}
