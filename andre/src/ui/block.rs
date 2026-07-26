use ratatui::{
    style::Style,
    widgets::{Block, BorderType, Borders},
};

use crate::theme::ThemeColors;

pub fn themed_panel(colors: &ThemeColors) -> Block<'static> {
    Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(colors.border))
}
