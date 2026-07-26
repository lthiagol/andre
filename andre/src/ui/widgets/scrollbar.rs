use ratatui::{
    layout::Rect,
    style::Style,
    widgets::{Scrollbar, ScrollbarOrientation, ScrollbarState},
    Frame,
};

use crate::theme::ThemeColors;

const SCROLLBAR_WIDTH: u16 = 1;

pub fn render_scrollbar(
    frame: &mut Frame,
    area: Rect,
    colors: &ThemeColors,
    content_len: usize,
    position: usize,
) {
    if content_len <= 1 || area.width < SCROLLBAR_WIDTH + 1 || area.height < 2 {
        return;
    }

    let scrollbar = Scrollbar::new(ScrollbarOrientation::VerticalRight)
        .begin_symbol(Some("↑"))
        .end_symbol(Some("↓"))
        .track_symbol(Some("│"))
        .thumb_style(Style::default().fg(colors.indicator))
        .track_style(Style::default().fg(colors.muted));

    let viewport = area.height.saturating_sub(2).max(1) as usize;
    let mut state = ScrollbarState::new(content_len)
        .position(position)
        .viewport_content_length(viewport);

    frame.render_stateful_widget(scrollbar, area, &mut state);
}
