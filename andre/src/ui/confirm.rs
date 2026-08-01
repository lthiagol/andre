use std::collections::HashMap;

use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::Style,
    widgets::{List, ListItem, ListState},
    Frame,
};

use crate::theme::ThemeColors;
use crate::ui::elide;
use crate::ui::widgets::scrollbar::render_scrollbar;
use andre_core::{build_stow_args, config::Config};

#[allow(clippy::too_many_arguments)]
pub fn render_confirm(
    frame: &mut Frame,
    area: Rect,
    colors: &ThemeColors,
    title: &str,
    selected_packages: &HashMap<String, Vec<String>>,
    action: &str,
    config: &Config,
    config_dir: &std::path::Path,
    home_dir: &std::path::Path,
    verbosity: usize,
    dry_run: bool,
    no_folding: bool,
    adopt: bool,
    dotfiles: bool,
    scroll_index: usize,
    viewport_height: usize,
) {
    let action_upper = action.to_uppercase();

    let mut items: Vec<ListItem> = Vec::new();

    items.push(
        ListItem::new(format!("Action: {}", action_upper))
            .style(Style::default().fg(colors.primary)),
    );

    items.push(ListItem::new("".to_string()).style(Style::default().fg(colors.primary)));

    for (group_name, packages) in selected_packages {
        items.push(
            ListItem::new(format!("Group: {}", group_name))
                .style(Style::default().fg(colors.success)),
        );

        let source = config.get_group_source(group_name, config_dir, home_dir);
        let target = config.get_group_target(group_name, config_dir, home_dir);

        for pkg in packages {
            // Engine-aware preview: native (default) does not shell out, so do not
            // show a `stow ...` command that would mislead users. Only the stow
            // engine renders its actual command line.
            let cmd_str = if config.effective_engine() == "native" {
                match (&source, &target) {
                    (Some(_), Some(_)) => format!("native: {}  (relative symlink)", pkg),
                    _ => format!("native: {} (path unresolved)", pkg),
                }
            } else if let (Some(src), Some(tgt)) = (&source, &target) {
                let ignores = config.effective_ignores(group_name);
                let cmd = build_stow_args(
                    src,
                    tgt,
                    pkg,
                    action,
                    verbosity as u8,
                    dry_run,
                    no_folding,
                    adopt,
                    dotfiles,
                    &ignores,
                );
                let args: Vec<String> = cmd
                    .get_args()
                    .map(|a| a.to_string_lossy().to_string())
                    .collect();
                let prog = cmd.get_program().to_string_lossy();
                format!("{} {}", prog, args.join(" "))
            } else {
                format!("stow {} (path unresolved)", pkg)
            };

            items.push(
                ListItem::new(format!("  {}", elide(&cmd_str, 80)))
                    .style(Style::default().fg(colors.secondary)),
            );
        }

        items.push(ListItem::new("".to_string()).style(Style::default().fg(colors.primary)));
    }

    items.push(
        ListItem::new("Press 'y' to confirm, 'n' to cancel".to_string())
            .style(Style::default().fg(colors.muted)),
    );

    let total = items.len();
    let list = List::new(items).block(crate::ui::themed_panel(colors).title(title));

    if total > viewport_height && area.width > 4 {
        let chunks = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Min(1), Constraint::Length(1)])
            .split(area);
        let scroll_at = scroll_index.min(total.saturating_sub(1));
        let mut list_state = ListState::default();
        list_state.select(Some(scroll_at));
        frame.render_stateful_widget(list, chunks[0], &mut list_state);
        render_scrollbar(frame, chunks[1], colors, total, scroll_index);
    } else {
        let mut list_state = ListState::default();
        list_state.select(Some(scroll_index.min(total.saturating_sub(1))));
        frame.render_stateful_widget(list, area, &mut list_state);
    }
}
