use ratatui::{
    layout::Rect,
    style::Style,
    widgets::{Scrollbar, ScrollbarOrientation, ScrollbarState},
    Frame,
};

use crate::theme::ThemeColors;

const SCROLLBAR_WIDTH: u16 = 1;

/// The column a scrollbar draws in for a bordered panel: the panel's right border,
/// between its top and bottom borders. The bar sits on the border, so the list keeps
/// the full panel width and lines up with its neighbours.
pub fn border_track(panel: Rect) -> Rect {
    let width = if panel.width > 1 { SCROLLBAR_WIDTH } else { 0 };
    Rect {
        x: panel.right().saturating_sub(SCROLLBAR_WIDTH),
        y: panel.y.saturating_add(1),
        width,
        height: panel.height.saturating_sub(2),
    }
}

/// Draws a vertical scrollbar in `track` when `content_len` rows do not fit in
/// `viewport_len`. Nothing is drawn when everything fits.
pub fn render_scrollbar(
    frame: &mut Frame,
    track: Rect,
    colors: &ThemeColors,
    content_len: usize,
    viewport_len: usize,
    position: usize,
) {
    if content_len <= viewport_len || track.width < SCROLLBAR_WIDTH || track.height < 1 {
        return;
    }

    let scrollbar = Scrollbar::new(ScrollbarOrientation::VerticalRight)
        .begin_symbol(None)
        .end_symbol(None)
        .track_symbol(Some("│"))
        .thumb_style(Style::default().fg(colors.indicator))
        .track_style(Style::default().fg(colors.border));

    let mut state = ScrollbarState::new(content_len)
        .position(position)
        .viewport_content_length(viewport_len.max(1));

    frame.render_stateful_widget(scrollbar, track, &mut state);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_border_track_is_the_right_border_between_the_corners() {
        let panel = Rect::new(3, 5, 20, 10);
        assert_eq!(border_track(panel), Rect::new(22, 6, 1, 8));
    }

    #[test]
    fn test_border_track_is_empty_when_the_panel_has_no_room_for_a_border() {
        assert_eq!(border_track(Rect::new(0, 0, 1, 10)).width, 0);
        assert_eq!(border_track(Rect::new(0, 0, 0, 10)).width, 0);
    }

    #[test]
    fn test_border_track_has_no_rows_on_a_two_row_panel() {
        assert_eq!(border_track(Rect::new(0, 0, 10, 2)).height, 0);
    }
}
