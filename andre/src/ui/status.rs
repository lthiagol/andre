use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::Style,
    widgets::{List, ListItem, ListState},
    Frame,
};

use crate::theme::ThemeColors;
use crate::ui::icons::Icons;
use crate::ui::plural;
use crate::ui::status_style::{render_legend, status_color, status_word, LEGEND_HEIGHT};
use crate::ui::widgets::scrollbar::{border_track, render_scrollbar};
use andre_core::StowStatus;

/// One line of the stow status list. Built by the status component in `update()`;
/// painting only turns these into styled list items.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StatusRow {
    Group(String),
    Package { name: String, status: StowStatus },
    Spacer,
    NoPackages,
}

/// Splits a status-style screen into its list and, when there is room, its legend.
/// Paint and `prepare` both call this, so the viewport is the list the user sees.
pub fn status_layout(area: Rect) -> (Rect, Option<Rect>) {
    if area.height > LEGEND_HEIGHT + 3 {
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Min(0), Constraint::Length(LEGEND_HEIGHT)])
            .split(area);
        (chunks[0], Some(chunks[1]))
    } else {
        (area, None)
    }
}

/// The panel title. The package count shows only when there is at least one package.
fn status_title(total_packages: usize) -> String {
    if total_packages > 0 {
        let noun = plural(total_packages, "package", "packages");
        format!(" stow status ({} {}) ", total_packages, noun)
    } else {
        " stow status ".to_string()
    }
}

pub fn render_status(
    frame: &mut Frame,
    area: Rect,
    colors: &ThemeColors,
    icons: &Icons,
    rows: &[StatusRow],
    scroll_index: usize,
    viewport_height: usize,
) {
    let total_packages = rows
        .iter()
        .filter(|row| matches!(row, StatusRow::Package { .. }))
        .count();
    let items: Vec<ListItem> = rows
        .iter()
        .map(|row| status_item(row, colors, icons))
        .collect();

    let title = status_title(total_packages);

    let (list_area, legend_area) = status_layout(area);

    let total = items.len();
    let list = List::new(items).block(crate::ui::themed_panel(colors).title(title));
    let scroll_at = scroll_index.min(total.saturating_sub(1));
    let mut state = ListState::default();
    state.select(Some(scroll_at));
    frame.render_stateful_widget(list, list_area, &mut state);
    render_scrollbar(
        frame,
        border_track(list_area),
        colors,
        total,
        viewport_height,
        scroll_at,
    );

    if let Some(legend_rect) = legend_area {
        render_legend(frame, legend_rect, colors, icons);
    }
}

fn status_item(row: &StatusRow, colors: &ThemeColors, icons: &Icons) -> ListItem<'static> {
    match row {
        StatusRow::Group(name) => {
            ListItem::new(name.clone()).style(Style::default().fg(colors.accent))
        }
        StatusRow::Package { name, status } => ListItem::new(format!(
            "  {} {}  {}",
            icons.status(*status),
            name,
            status_word(*status)
        ))
        .style(Style::default().fg(status_color(colors, *status))),
        StatusRow::Spacer => ListItem::new("").style(Style::default().fg(colors.accent)),
        StatusRow::NoPackages => ListItem::new("No packages found in any configured group.")
            .style(Style::default().fg(colors.text_dim)),
    }
}

#[cfg(test)]
mod tests {
    use super::status_title;

    #[test]
    fn test_status_title_counts_packages_with_the_right_noun() {
        assert_eq!(status_title(0), " stow status ");
        assert_eq!(status_title(1), " stow status (1 package) ");
        assert_eq!(status_title(2), " stow status (2 packages) ");
    }
}
