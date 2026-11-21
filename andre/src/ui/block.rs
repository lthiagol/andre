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

/// The panel frame, with the border in `accent` when `focused`: the pane that takes keys.
pub fn focus_panel(colors: &ThemeColors, focused: bool) -> Block<'static> {
    let border = if focused {
        colors.accent
    } else {
        colors.border
    };
    Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(border))
}
