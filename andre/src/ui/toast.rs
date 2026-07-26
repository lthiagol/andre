use ratatui::{
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Paragraph},
    Frame,
};

use crate::components::{Toast, ToastKind};
use crate::theme::ThemeColors;
use crate::ui::elide;

/// Draw a non-blocking toast near the bottom of `area` (does not own input).
pub fn render_toast(frame: &mut Frame, area: Rect, colors: &ThemeColors, toast: &Toast) {
    if area.width < 8 || area.height < 3 {
        return;
    }

    let max_inner = (area.width as usize).saturating_sub(4).max(1);
    let msg = elide(&toast.message, max_inner);
    let width = (unicode_width::UnicodeWidthStr::width(msg.as_str()) as u16)
        .saturating_add(4)
        .min(area.width)
        .max(10)
        .min(area.width);

    let height: u16 = 3;
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Min(0),
            Constraint::Length(height),
            Constraint::Length(1),
        ])
        .split(area);
    let row = chunks[1];
    let h = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Min(0),
            Constraint::Length(width),
            Constraint::Min(0),
        ])
        .split(row);
    let toast_area = h[1];

    let fg = match toast.kind {
        ToastKind::Success => colors.success,
        ToastKind::Error => colors.error,
    };
    let border = match toast.kind {
        ToastKind::Success => colors.highlight,
        ToastKind::Error => colors.error,
    };

    frame.render_widget(Clear, toast_area);
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(border))
        .style(Style::default().bg(colors.background).fg(colors.primary));
    let paragraph = Paragraph::new(Line::from(Span::styled(
        msg,
        Style::default().fg(fg).add_modifier(Modifier::BOLD),
    )))
    .alignment(Alignment::Center)
    .block(block);
    frame.render_widget(paragraph, toast_area);
}
