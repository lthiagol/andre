use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::Style,
    widgets::{Gauge, List, ListItem},
    Frame,
};

use crate::execute::{ExecutionResult, ExecutionState};
use crate::theme::ThemeColors;
use crate::ui::widgets::scrollbar::render_scrollbar;

const SPINNER_CHARS: &[char] = &[
    '\u{28FB}', '\u{28FC}', '\u{28FD}', '\u{28FE}', '\u{28FF}', '\u{28F7}', '\u{28EF}', '\u{28DF}',
    '\u{28BF}', '\u{287F}',
];

/// Rows reserved above the result list inside the Execution panel (border handled by block).
pub const GAUGE_ROWS: u16 = 1;

fn gauge_ratio(completed: usize, total: usize) -> f64 {
    if total == 0 {
        0.0
    } else {
        (completed as f64 / total as f64).clamp(0.0, 1.0)
    }
}

fn row_icon_style(success: bool, colors: &ThemeColors) -> (String, ratatui::style::Color) {
    if success {
        ("\u{2713}".to_string(), colors.ok)
    } else {
        ("\u{2717}".to_string(), colors.err)
    }
}

fn is_skipped(result: &ExecutionResult) -> bool {
    !result.success
        && result
            .error_message
            .as_deref()
            .is_some_and(|m| m.starts_with("Skipped"))
}

fn completion_summary(results: &[ExecutionResult]) -> String {
    let succeeded = results.iter().filter(|r| r.success).count();
    let skipped = results.iter().filter(|r| is_skipped(r)).count();
    let failed = results.len() - succeeded - skipped;
    if skipped == 0 {
        format!("{succeeded} succeeded, {failed} failed")
    } else {
        format!("{succeeded} succeeded, {failed} failed, {skipped} skipped")
    }
}

fn result_row_items(results: &[ExecutionResult], colors: &ThemeColors) -> Vec<ListItem<'static>> {
    results
        .iter()
        .map(|result| {
            let (icon, fg) = row_icon_style(result.success, colors);
            ListItem::new(format!("{} {} / {}", icon, result.group, result.package))
                .style(Style::default().fg(fg))
        })
        .collect()
}

/// Visible list line count for a given execute state (excludes the gauge row).
pub fn execute_list_len(state: ExecutionState, results: &[ExecutionResult]) -> usize {
    match state {
        // spinner line only (no dead Ctrl+C cancel hint)
        ExecutionState::Running => results.len() + 1,
        ExecutionState::Completed => results.len() + 4,
    }
}

#[allow(clippy::too_many_arguments)]
pub fn render_execute(
    frame: &mut Frame,
    area: Rect,
    colors: &ThemeColors,
    title: &str,
    state: ExecutionState,
    results: &[ExecutionResult],
    spinner_frame: usize,
    total: usize,
    scroll_index: Option<usize>,
) {
    let block = crate::ui::themed_panel(colors).title(title);
    let inner = block.inner(area);
    frame.render_widget(block, area);

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(GAUGE_ROWS), Constraint::Min(1)])
        .split(inner);

    let label = format!("{}/{}", results.len(), total);
    let gauge = Gauge::default()
        .ratio(gauge_ratio(results.len(), total))
        .label(label)
        .gauge_style(Style::default().fg(colors.accent).bg(colors.selection))
        .use_unicode(true);
    frame.render_widget(gauge, chunks[0]);

    let list_area = chunks[1];
    let items: Vec<ListItem> = match state {
        ExecutionState::Running => {
            let spinner = SPINNER_CHARS[spinner_frame % SPINNER_CHARS.len()];
            let mut items = result_row_items(results, colors);
            let progress = if total > 0 {
                let in_flight = (results.len() + 1).min(total);
                format!(" [{spinner}] {in_flight}/{total}")
            } else {
                format!(" [{spinner}] Executing...")
            };
            items.push(ListItem::new(progress).style(Style::default().fg(colors.accent)));
            items
        }
        ExecutionState::Completed => {
            let summary = completion_summary(results);
            let has_problems = results.iter().any(|r| !r.success);

            let mut items = Vec::new();
            items.push(
                ListItem::new("Results:".to_string()).style(Style::default().fg(colors.accent)),
            );

            for result in results {
                let (icon, fg) = row_icon_style(result.success, colors);
                let extra = if let Some(ref e) = result.error_message {
                    format!(" - {e}")
                } else {
                    String::new()
                };
                items.push(
                    ListItem::new(format!("{icon} {} {}{extra}", result.group, result.package))
                        .style(Style::default().fg(fg)),
                );
            }

            items.push(ListItem::new(String::new()).style(Style::default().fg(colors.accent)));
            items.push(
                ListItem::new(summary).style(Style::default().fg(if has_problems {
                    colors.warn
                } else {
                    colors.ok
                })),
            );
            items.push(
                ListItem::new("Press Enter to continue".to_string())
                    .style(Style::default().fg(colors.text_dim)),
            );
            items
        }
    };

    let content_len = items.len();
    debug_assert_eq!(
        content_len,
        execute_list_len(state, results),
        "execute_list_len must match rendered items"
    );

    let list =
        List::new(items).highlight_style(Style::default().bg(colors.selection).fg(colors.text));
    let page_size = list_area.height.max(1) as usize;

    let mut list_state = ratatui::widgets::ListState::default();
    list_state.select(scroll_index);
    frame.render_stateful_widget(list, list_area, &mut list_state);

    // The list has no border of its own: its right edge is the panel's right border.
    let track = Rect {
        x: list_area.right(),
        y: list_area.y,
        width: 1,
        height: list_area.height,
    };
    render_scrollbar(
        frame,
        track,
        colors,
        content_len,
        page_size,
        scroll_index.unwrap_or(0),
    );
}
