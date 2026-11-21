use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::Style,
    text::{Line, Span},
    widgets::{List, ListItem, ListState, Paragraph},
    Frame,
};

use andre_core::StowStatus;

use crate::theme::ThemeColors;
use crate::ui::elide;

const SCANNING_DURATION_MS: u128 = 1000;
const SCANNER_SPINNER: &[char] = &[
    '\u{280B}', '\u{2819}', '\u{2839}', '\u{2838}', '\u{283C}', '\u{2834}', '\u{2826}', '\u{2827}',
    '\u{2807}', '\u{280F}',
];
const LEGEND_HEIGHT: u16 = 8;

pub struct FlatItem {
    pub group_name: String,
    pub package_name: String,
    pub selected: bool,
    pub status: StowStatus,
    pub preview: Vec<String>,
}

#[allow(clippy::too_many_arguments)]
pub fn render_package_select(
    frame: &mut Frame,
    area: Rect,
    colors: &ThemeColors,
    title: &str,
    flat_items: &[FlatItem],
    cursor: usize,
    entered_at: Option<std::time::Instant>,
    spinner_frame: usize,
) {
    let is_scanning = entered_at
        .map(|t| t.elapsed().as_millis() < SCANNING_DURATION_MS)
        .unwrap_or(false);

    if is_scanning {
        let ch = SCANNER_SPINNER[spinner_frame % SCANNER_SPINNER.len()];
        let text = format!("{} Scanning packages...", ch);
        let paragraph = Paragraph::new(text).style(Style::default().fg(colors.primary));
        frame.render_widget(paragraph, area);
        return;
    }

    let total = flat_items.len();

    if total == 0 {
        let paragraph = Paragraph::new("No packages found in selected groups.")
            .style(Style::default().fg(colors.muted));
        frame.render_widget(paragraph, area);
        return;
    }

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

    let cursor = cursor.min(total.saturating_sub(1));

    let items: Vec<ListItem> = flat_items
        .iter()
        .enumerate()
        .map(|(i, item)| {
            let is_cursor = i == cursor;

            let status_fg = match item.status {
                StowStatus::Stowed => colors.success,
                StowStatus::Unstowed => colors.primary,
                StowStatus::Partial => colors.warning,
                StowStatus::Conflict => colors.error,
                StowStatus::Missing | StowStatus::Empty => colors.muted,
            };

            let (icon_str, label_str) = match item.status {
                StowStatus::Stowed => ("\u{2713}", "Installed"),
                StowStatus::Unstowed => ("\u{2717}", "Ready"),
                StowStatus::Partial => ("\u{25D0}", "Partial"),
                StowStatus::Conflict => ("\u{26A0}", "Blocked"),
                StowStatus::Missing => ("?", "Not found"),
                StowStatus::Empty => ("\u{2205}", "Empty"),
            };

            let mark = if item.selected { "[x]" } else { "[ ]" };
            let mark_fg = if item.selected {
                colors.success
            } else {
                colors.muted
            };
            let name_fg = if is_cursor {
                colors.primary
            } else {
                colors.secondary
            };
            let preview_str = if item.preview.is_empty() {
                String::new()
            } else {
                format!(" ({})", item.preview.join(", "))
            };

            let mut spans: Vec<Span> = Vec::new();

            let cursor_marker = if is_cursor {
                Span::styled("--> ", Style::default().fg(colors.highlight))
            } else {
                Span::raw("    ")
            };
            spans.push(cursor_marker);

            spans.push(Span::styled(
                format!("{} ", mark),
                Style::default().fg(mark_fg),
            ));

            spans.push(Span::styled(
                format!("{} ", icon_str),
                Style::default().fg(status_fg),
            ));

            let name_str = format!("{} {} ", item.group_name, item.package_name);
            spans.push(Span::styled(
                elide(&name_str, 50),
                Style::default().fg(name_fg),
            ));

            spans.push(Span::styled(
                label_str.to_string(),
                Style::default().fg(status_fg),
            ));

            if !preview_str.is_empty() {
                spans.push(Span::styled(preview_str, Style::default().fg(colors.muted)));
            }

            ListItem::new(Line::from(spans))
        })
        .collect();

    let list = List::new(items).block(crate::ui::themed_panel(colors).title(title));

    let page_size = list_area.height.saturating_sub(2).max(1) as usize;

    if total > page_size && list_area.width > 4 {
        let chunks = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Min(1), Constraint::Length(1)])
            .split(list_area);
        let mut list_state = ListState::default();
        list_state.select(Some(cursor));
        frame.render_stateful_widget(list, chunks[0], &mut list_state);
        crate::ui::widgets::scrollbar::render_scrollbar(frame, chunks[1], colors, total, cursor);
    } else {
        let mut list_state = ListState::default();
        list_state.select(Some(cursor));
        frame.render_stateful_widget(list, list_area, &mut list_state);
    }

    if let Some(legend_rect) = legend_area {
        render_legend(frame, legend_rect, colors);
    }
}

fn render_legend(frame: &mut Frame, area: Rect, colors: &ThemeColors) {
    let items: Vec<ListItem> = vec![
        ListItem::new(Line::from("Legend:")).style(Style::default().fg(colors.primary)),
        ListItem::new(Line::from(vec![
            Span::styled(
                format!("  {} Installed ", StowStatus::Stowed.icon()),
                Style::default().fg(colors.success),
            ),
            Span::styled("all symlinks in place", Style::default().fg(colors.muted)),
        ])),
        ListItem::new(Line::from(vec![
            Span::styled(
                format!("  {} Ready     ", StowStatus::Unstowed.icon()),
                Style::default().fg(colors.primary),
            ),
            Span::styled(
                "package ready to be linked",
                Style::default().fg(colors.muted),
            ),
        ])),
        ListItem::new(Line::from(vec![
            Span::styled(
                format!("  {} Partial   ", StowStatus::Partial.icon()),
                Style::default().fg(colors.warning),
            ),
            Span::styled(
                "some symlinks, will complete",
                Style::default().fg(colors.muted),
            ),
        ])),
        ListItem::new(Line::from(vec![
            Span::styled(
                format!("  {} Blocked   ", StowStatus::Conflict.icon()),
                Style::default().fg(colors.error),
            ),
            Span::styled(
                "real file at target, cannot overwrite",
                Style::default().fg(colors.muted),
            ),
        ])),
        ListItem::new(Line::from(vec![
            Span::styled(
                format!("  {} Not found ", StowStatus::Missing.icon()),
                Style::default().fg(colors.muted),
            ),
            Span::styled(
                "package source directory not found",
                Style::default().fg(colors.muted),
            ),
        ])),
        ListItem::new(Line::from(vec![
            Span::styled(
                format!("  {} Empty     ", StowStatus::Empty.icon()),
                Style::default().fg(colors.muted),
            ),
            Span::styled(
                "package has no files to link",
                Style::default().fg(colors.muted),
            ),
        ])),
    ];

    let list = List::new(items).block(crate::ui::themed_panel(colors));

    frame.render_widget(list, area);
}
