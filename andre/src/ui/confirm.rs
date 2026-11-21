use ratatui::{
    layout::Rect,
    style::Style,
    widgets::{List, ListItem, ListState},
    Frame,
};

use crate::theme::ThemeColors;
use crate::ui::widgets::scrollbar::{border_track, render_scrollbar};

/// One line of the review list. The component builds these before paint, so
/// rendering only draws text.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConfirmRow {
    /// `Action: STOW` and the like.
    Action(String),
    /// `Group: name`.
    Group(String),
    /// The command preview for one package.
    Command(String),
    /// Spacing between sections.
    Blank,
}

pub fn render_confirm(
    frame: &mut Frame,
    area: Rect,
    colors: &ThemeColors,
    title: &str,
    rows: &[ConfirmRow],
    scroll_index: usize,
    viewport_height: usize,
) {
    let mut items: Vec<ListItem> = rows
        .iter()
        .map(|row| match row {
            ConfirmRow::Action(text) => {
                ListItem::new(text.as_str()).style(Style::default().fg(colors.primary))
            }
            ConfirmRow::Group(text) => {
                ListItem::new(text.as_str()).style(Style::default().fg(colors.success))
            }
            ConfirmRow::Command(text) => {
                ListItem::new(text.as_str()).style(Style::default().fg(colors.secondary))
            }
            ConfirmRow::Blank => ListItem::new("").style(Style::default().fg(colors.primary)),
        })
        .collect();

    items.push(
        ListItem::new("Press 'y' to confirm, 'n' to cancel")
            .style(Style::default().fg(colors.muted)),
    );

    let total = items.len();
    let list = List::new(items).block(crate::ui::themed_panel(colors).title(title));

    let scroll_at = scroll_index.min(total.saturating_sub(1));
    let mut list_state = ListState::default();
    list_state.select(Some(scroll_at));
    frame.render_stateful_widget(list, area, &mut list_state);
    render_scrollbar(
        frame,
        border_track(area),
        colors,
        total,
        viewport_height,
        scroll_at,
    );
}
