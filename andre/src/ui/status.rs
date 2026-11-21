use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::Style,
    text::Line,
    widgets::{List, ListItem, ListState},
    Frame,
};

use crate::theme::ThemeColors;
use crate::ui::widgets::scrollbar::render_scrollbar;
use andre_core::StowStatus;

const LEGEND_HEIGHT: u16 = 9;

/// One line of the stow status list. Built by the status component in `update()`;
/// painting only turns these into styled list items.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StatusRow {
    Group(String),
    Package { name: String, status: StowStatus },
    Spacer,
    NoPackages,
}

pub fn render_status(
    frame: &mut Frame,
    area: Rect,
    colors: &ThemeColors,
    breadcrumb: &str,
    rows: &[StatusRow],
    scroll_index: usize,
    viewport_height: usize,
) {
    let total_packages = rows
        .iter()
        .filter(|row| matches!(row, StatusRow::Package { .. }))
        .count();
    let items: Vec<ListItem> = rows.iter().map(|row| status_item(row, colors)).collect();

    let title = if total_packages > 0 {
        format!(
            " {} > Stow Status ({} packages) ",
            breadcrumb, total_packages
        )
    } else {
        format!(" {} > Stow Status ", breadcrumb)
    };

    let area_has_room = area.height > LEGEND_HEIGHT + 3;
    let (list_area, legend_area) = if area_has_room {
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Min(0), Constraint::Length(LEGEND_HEIGHT)])
            .split(area);
        (chunks[0], Some(chunks[1]))
    } else {
        (area, None)
    };

    let total = items.len();
    let list = List::new(items).block(crate::ui::themed_panel(colors).title(title));

    if total > viewport_height && list_area.width > 4 {
        let chunks = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Min(1), Constraint::Length(1)])
            .split(list_area);
        let scroll_at = scroll_index.min(total.saturating_sub(1));
        let mut state = ListState::default();
        state.select(Some(scroll_at));
        frame.render_stateful_widget(list, chunks[0], &mut state);
        render_scrollbar(frame, chunks[1], colors, total, scroll_index);
    } else {
        let mut state = ListState::default();
        state.select(Some(scroll_index.min(total.saturating_sub(1))));
        frame.render_stateful_widget(list, list_area, &mut state);
    }

    if let Some(legend_rect) = legend_area {
        render_status_legend(frame, legend_rect, colors);
    }
}

fn status_item(row: &StatusRow, colors: &ThemeColors) -> ListItem<'static> {
    match row {
        StatusRow::Group(name) => {
            ListItem::new(name.clone()).style(Style::default().fg(colors.primary))
        }
        StatusRow::Package { name, status } => {
            let fg = match status {
                StowStatus::Stowed => colors.success,
                StowStatus::Unstowed => colors.muted,
                StowStatus::Partial => colors.warning,
                StowStatus::Conflict => colors.error,
                StowStatus::Missing | StowStatus::Empty => colors.muted,
            };
            ListItem::new(format!("  {} {}  {}", status.icon(), name, status.label()))
                .style(Style::default().fg(fg))
        }
        StatusRow::Spacer => ListItem::new("").style(Style::default().fg(colors.primary)),
        StatusRow::NoPackages => ListItem::new("No packages found in any configured group.")
            .style(Style::default().fg(colors.muted)),
    }
}

fn render_status_legend(frame: &mut Frame, area: Rect, colors: &ThemeColors) {
    let legend_items: Vec<ListItem> = vec![
        ListItem::new(Line::from("Legend:")).style(Style::default().fg(colors.primary)),
        ListItem::new(Line::from(format!(
            "  {} Stowed    all files symlinked correctly",
            StowStatus::Stowed.icon()
        )))
        .style(Style::default().fg(colors.success)),
        ListItem::new(Line::from(format!(
            "  {} Unstowed  package exists but no symlinks",
            StowStatus::Unstowed.icon()
        )))
        .style(Style::default().fg(colors.muted)),
        ListItem::new(Line::from(format!(
            "  {} Partial   some files symlinked, some not",
            StowStatus::Partial.icon()
        )))
        .style(Style::default().fg(colors.warning)),
        ListItem::new(Line::from(format!(
            "  {} Conflict  real file at target, cannot overwrite",
            StowStatus::Conflict.icon()
        )))
        .style(Style::default().fg(colors.error)),
        ListItem::new(Line::from(format!(
            "  {} Missing   package source directory not found",
            StowStatus::Missing.icon()
        )))
        .style(Style::default().fg(colors.muted)),
        ListItem::new(Line::from(format!(
            "  {} Empty     package contains no files",
            StowStatus::Empty.icon()
        )))
        .style(Style::default().fg(colors.muted)),
    ];

    let legend = List::new(legend_items).block(crate::ui::themed_panel(colors));
    frame.render_widget(legend, area);
}
