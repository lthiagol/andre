use std::path::Path;

use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::Style,
    text::{Line, Span},
    widgets::{List, ListItem, ListState},
    Frame,
};

use crate::components::settings::rows::{build_flat_items, SettingsRow};
use crate::components::settings::ConfigMessage;
use crate::components::settings::{GroupDialog, PickerState};
use crate::theme::ThemeColors;
use crate::ui::widgets::file_browser::FileBrowserWidget;
use crate::ui::widgets::scrollbar::render_scrollbar;
use andre_core::config::Config;

fn val_str(on: bool) -> &'static str {
    if on {
        "ON "
    } else {
        "OFF"
    }
}

#[allow(clippy::too_many_arguments)]
fn render_row(
    index: usize,
    cursor: usize,
    row: &SettingsRow,
    colors: &ThemeColors,
    verbosity: usize,
    dry_run: bool,
    no_folding: bool,
    adopt: bool,
    dotfiles: bool,
    action: &str,
    theme_name: &str,
    config_path: &Path,
    config_dir: &Path,
    config_dirty: bool,
    config_message: ConfigMessage,
    filename: &str,
) -> ListItem<'static> {
    let is_cursor = index == cursor;
    let is_interactive = !matches!(
        row,
        SettingsRow::ConfigPathInfo
            | SettingsRow::ConfigResolveInfo
            | SettingsRow::Spacer
            | SettingsRow::SavedMessage
    );
    let marker = if is_cursor && is_interactive {
        "--> "
    } else {
        "    "
    };

    let line = match row {
        SettingsRow::ConfigPathInfo => Line::from(Span::styled(
            format!("  Config: {}", config_path.display()),
            Style::default().fg(colors.muted),
        )),
        SettingsRow::ConfigResolveInfo => Line::from(Span::styled(
            format!("  Paths resolve relative to: {}", config_dir.display()),
            Style::default().fg(colors.muted),
        )),
        SettingsRow::Verbosity => Line::from(Span::styled(
            format!("{}verbosity:         {}", marker, verbosity),
            Style::default().fg(if is_cursor {
                colors.primary
            } else {
                colors.secondary
            }),
        )),
        SettingsRow::DryRun => {
            let val = val_str(dry_run);
            let fg = if dry_run {
                colors.success
            } else {
                colors.error
            };
            let parts = vec![
                Span::styled(
                    format!("{}dry_run:          ", marker),
                    Style::default().fg(if is_cursor {
                        colors.primary
                    } else {
                        colors.secondary
                    }),
                ),
                Span::styled(val, Style::default().fg(fg)),
            ];
            Line::from(parts)
        }
        SettingsRow::NoFolding => {
            let val = val_str(no_folding);
            let fg = if no_folding {
                colors.success
            } else {
                colors.error
            };
            let parts = vec![
                Span::styled(
                    format!("{}no_folding:       ", marker),
                    Style::default().fg(if is_cursor {
                        colors.primary
                    } else {
                        colors.secondary
                    }),
                ),
                Span::styled(val, Style::default().fg(fg)),
            ];
            Line::from(parts)
        }
        SettingsRow::Adopt => {
            let val = val_str(adopt);
            let fg = if adopt { colors.success } else { colors.error };
            let parts = vec![
                Span::styled(
                    format!("{}adopt:            ", marker),
                    Style::default().fg(if is_cursor {
                        colors.primary
                    } else {
                        colors.secondary
                    }),
                ),
                Span::styled(val, Style::default().fg(fg)),
            ];
            Line::from(parts)
        }
        SettingsRow::Dotfiles => {
            let val = val_str(dotfiles);
            let fg = if dotfiles {
                colors.success
            } else {
                colors.error
            };
            let parts = vec![
                Span::styled(
                    format!("{}dotfiles:         ", marker),
                    Style::default().fg(if is_cursor {
                        colors.primary
                    } else {
                        colors.secondary
                    }),
                ),
                Span::styled(val, Style::default().fg(fg)),
            ];
            Line::from(parts)
        }
        SettingsRow::Action => Line::from(Span::styled(
            format!("{}action:           {}", marker, action),
            Style::default().fg(if is_cursor {
                colors.primary
            } else {
                colors.secondary
            }),
        )),
        SettingsRow::ThemeName => Line::from(Span::styled(
            format!("{}theme:            {}", marker, theme_name),
            Style::default().fg(if is_cursor {
                colors.highlight
            } else {
                colors.secondary
            }),
        )),
        SettingsRow::IgnoreLine { count } => {
            let label = if *count == 1 { "pattern" } else { "patterns" };
            Line::from(vec![Span::styled(
                format!("{}ignore:          {} {}", marker, count, label),
                Style::default().fg(if is_cursor {
                    colors.primary
                } else {
                    colors.secondary
                }),
            )])
        }
        SettingsRow::Spacer => Line::from(Span::raw("")),
        SettingsRow::GroupLine { count } => {
            let label = if *count == 1 { "group" } else { "groups" };
            Line::from(vec![Span::styled(
                format!("{}group:           {} {}", marker, count, label),
                Style::default().fg(if is_cursor {
                    colors.primary
                } else {
                    colors.secondary
                }),
            )])
        }
        SettingsRow::SaveButton => {
            let label = if config_dirty {
                format!("[Update {}]", filename)
            } else {
                "[All saved]".to_string()
            };
            let style = if is_cursor {
                Style::default().fg(colors.success).bg(colors.background)
            } else if config_dirty {
                Style::default().fg(colors.highlight)
            } else {
                Style::default().fg(colors.muted)
            };
            Line::from(Span::styled(format!("{}    {}", marker, label), style))
        }
        SettingsRow::SavedMessage => {
            let (text, fg_color) = match config_message {
                ConfigMessage::ConfirmDiscard => (
                    "    Unsaved changes  y=save  n=discard  Esc=cancel".to_string(),
                    colors.warning,
                ),
                ConfigMessage::Saved => ("    Saved.".to_string(), colors.success),
                ConfigMessage::Hidden => ("".to_string(), colors.muted),
            };
            Line::from(Span::styled(text, Style::default().fg(fg_color)))
        }
    };

    ListItem::new(line)
}

#[allow(clippy::too_many_arguments)]
pub fn render_settings(
    frame: &mut Frame,
    area: Rect,
    colors: &ThemeColors,
    config: &Config,
    config_path: &Path,
    config_dir: &Path,
    verbosity: usize,
    dry_run: bool,
    no_folding: bool,
    adopt: bool,
    dotfiles: bool,
    action: &str,
    theme_name: &str,
    cursor: usize,
    config_message: ConfigMessage,
    config_dirty: bool,
    filename: &str,
    picker: Option<&PickerState>,
    group_dialog: Option<&GroupDialog>,
) {
    let config_saved_message = config_message != ConfigMessage::Hidden;
    let mut flat = build_flat_items(config);
    if config_saved_message {
        flat.push(SettingsRow::SavedMessage);
    }
    let total = flat.len();
    let cursor = cursor.min(total.saturating_sub(1));

    let list_items: Vec<ListItem> = flat
        .iter()
        .enumerate()
        .map(|(i, row)| {
            render_row(
                i,
                cursor,
                row,
                colors,
                verbosity,
                dry_run,
                no_folding,
                adopt,
                dotfiles,
                action,
                theme_name,
                config_path,
                config_dir,
                config_dirty,
                config_message,
                filename,
            )
        })
        .collect();

    let total = flat.len();
    let list = List::new(list_items)
        .highlight_symbol(">> ")
        .highlight_style(Style::default().fg(colors.highlight))
        .block(crate::ui::themed_panel(colors).title("Config"));

    let viewport_height = area.height.saturating_sub(2).max(1) as usize;
    if total > viewport_height && area.width > 4 {
        let chunks = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Min(1), Constraint::Length(1)])
            .split(area);
        let mut list_state = ListState::default();
        list_state.select(Some(cursor));
        frame.render_stateful_widget(list, chunks[0], &mut list_state);
        render_scrollbar(frame, chunks[1], colors, total, cursor);
    } else {
        let mut list_state = ListState::default();
        list_state.select(Some(cursor));
        frame.render_stateful_widget(list, area, &mut list_state);
    }

    if let Some(picker) = picker {
        render_picker_popup(frame, area, colors, picker);
    }
    if let Some(dialog) = group_dialog {
        render_group_dialog(frame, area, colors, dialog, config);
    }
}

fn render_picker_popup(frame: &mut Frame, area: Rect, colors: &ThemeColors, picker: &PickerState) {
    let popup_width = (picker.options.iter().map(|o| o.len()).max().unwrap_or(10) + 8)
        .min(area.width.saturating_sub(4) as usize) as u16;
    let popup_height =
        (picker.options.len() + 2).min(area.height.saturating_sub(4) as usize) as u16;

    let popup_area = centered_rect(popup_width, popup_height, area);

    let items: Vec<ListItem> = picker
        .options
        .iter()
        .enumerate()
        .map(|(i, opt)| {
            let marker = if i == picker.selected_index {
                Span::styled("> ", Style::default().fg(colors.highlight))
            } else {
                Span::raw("  ")
            };
            let text = Span::styled(
                opt.clone(),
                if i == picker.selected_index {
                    Style::default().fg(colors.primary)
                } else {
                    Style::default().fg(colors.secondary)
                },
            );
            ListItem::new(Line::from(vec![marker, text]))
        })
        .collect();

    let list = List::new(items)
        .highlight_symbol(">> ")
        .highlight_style(Style::default().fg(colors.highlight))
        .block(
            crate::ui::themed_panel(colors)
                .title(format!("Pick {}", picker.title))
                .style(Style::default().bg(colors.background).fg(colors.border)),
        );

    let mut list_state = ListState::default();
    list_state.select(Some(picker.selected_index));

    frame.render_widget(ratatui::widgets::Clear, popup_area);
    frame.render_stateful_widget(list, popup_area, &mut list_state);
}

fn centered_rect(percent_x: u16, percent_y: u16, r: Rect) -> Rect {
    let popup_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length((r.height.saturating_sub(percent_y)) / 2),
            Constraint::Length(percent_y),
            Constraint::Length((r.height.saturating_sub(percent_y)) / 2),
        ])
        .split(r);

    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Length((r.width.saturating_sub(percent_x)) / 2),
            Constraint::Length(percent_x),
            Constraint::Length((r.width.saturating_sub(percent_x)) / 2),
        ])
        .split(popup_layout[1])[1]
}

fn render_group_dialog(
    frame: &mut Frame,
    area: Rect,
    colors: &ThemeColors,
    dialog: &GroupDialog,
    config: &Config,
) {
    match dialog {
        GroupDialog::Actions { cursor } => {
            render_group_actions_popup(frame, area, colors, *cursor);
        }
        GroupDialog::AddName { name } => {
            render_name_input_popup(frame, area, colors, name, "Add Group");
        }
        GroupDialog::AddBrowse {
            name: _,
            source: _,
            path,
            cursor,
        } => {
            render_dir_browser_popup(frame, area, colors, path, *cursor, "Select Directory");
        }
        GroupDialog::RemoveSelect { cursor } => {
            let names: Vec<&str> = config.groups.keys().map(|s| s.as_str()).collect();
            render_list_picker_popup(frame, area, colors, names, *cursor, "Remove Group");
        }
        GroupDialog::RemoveConfirm { name } => {
            render_confirm_popup(frame, area, colors, name, "Remove group");
        }
        GroupDialog::EditSelect { cursor } => {
            let names: Vec<&str> = config.groups.keys().map(|s| s.as_str()).collect();
            render_list_picker_popup(frame, area, colors, names, *cursor, "Edit Group");
        }
        GroupDialog::EditField { group, cursor } => {
            let fields = vec!["Name", "Source", "Target", "Ignore"];
            render_edit_field_popup(frame, area, colors, group, fields, *cursor);
        }
        GroupDialog::EditName { group } => {
            render_name_input_popup(frame, area, colors, group, "Edit Group Name");
        }
        GroupDialog::EditBrowse {
            group: _,
            field: _,
            path,
            cursor,
        } => {
            render_dir_browser_popup(frame, area, colors, path, *cursor, "Select Directory");
        }
        GroupDialog::IgnoreManage { group, cursor } => {
            let patterns: Vec<String> = match group {
                Some(g) => config
                    .get_group_ignore(g)
                    .map(|v| v.to_vec())
                    .unwrap_or_default(),
                None => config.global_ignore(),
            };
            render_ignore_manage_popup(frame, area, colors, &patterns, *cursor);
        }
        GroupDialog::IgnoreAdd { group: _, pattern } => {
            render_name_input_popup(frame, area, colors, pattern, "Add Ignore Pattern");
        }
    }
}

fn render_group_actions_popup(frame: &mut Frame, area: Rect, colors: &ThemeColors, cursor: usize) {
    let options = ["Add", "Remove", "Edit"];
    let popup_width = 24u16.min(area.width.saturating_sub(4));
    let popup_height = (options.len() + 2) as u16;
    let popup_area = centered_rect(popup_width, popup_height, area);

    let items: Vec<ListItem> = options
        .iter()
        .enumerate()
        .map(|(i, opt)| {
            let marker = if i == cursor { "> " } else { "  " };
            ListItem::new(Line::from(Span::styled(
                format!("{}{}", marker, opt),
                Style::default().fg(if i == cursor {
                    colors.highlight
                } else {
                    colors.secondary
                }),
            )))
        })
        .collect();

    let list = List::new(items).block(
        crate::ui::themed_panel(colors)
            .title(" Group Actions ")
            .style(Style::default().bg(colors.background).fg(colors.border)),
    );

    frame.render_widget(ratatui::widgets::Clear, popup_area);
    frame.render_widget(list, popup_area);
}

fn render_name_input_popup(
    frame: &mut Frame,
    area: Rect,
    colors: &ThemeColors,
    current: &str,
    title: &str,
) {
    let popup_width = 40u16.min(area.width.saturating_sub(4));
    let popup_height = 5u16.min(area.height.saturating_sub(4));
    let popup_area = centered_rect(popup_width, popup_height, area);

    let cursor_visible = "_".to_string();
    let display_name = if current.is_empty() {
        cursor_visible
    } else {
        current.to_string()
    };

    let items: Vec<ListItem> = vec![
        ListItem::new(Line::from(vec![
            Span::styled(" Name: ", Style::default().fg(colors.muted)),
            Span::styled(display_name, Style::default().fg(colors.highlight)),
        ])),
        ListItem::new(Line::from(Span::styled(
            "",
            Style::default().fg(colors.muted),
        ))),
        ListItem::new(Line::from(vec![
            Span::styled("Enter", Style::default().fg(colors.indicator)),
            Span::styled(" confirm  ", Style::default().fg(colors.muted)),
            Span::styled("Esc", Style::default().fg(colors.indicator)),
            Span::styled(" cancel", Style::default().fg(colors.muted)),
        ])),
    ];

    let list = List::new(items).block(
        crate::ui::themed_panel(colors)
            .title(format!(" {} ", title))
            .style(Style::default().bg(colors.background).fg(colors.border)),
    );

    frame.render_widget(ratatui::widgets::Clear, popup_area);
    frame.render_widget(list, popup_area);
}

fn render_confirm_popup(
    frame: &mut Frame,
    area: Rect,
    colors: &ThemeColors,
    name: &str,
    title: &str,
) {
    let popup_width = 36u16.min(area.width.saturating_sub(4));
    let popup_height = 5u16.min(area.height.saturating_sub(4));
    let popup_area = centered_rect(popup_width, popup_height, area);

    let items: Vec<ListItem> = vec![
        ListItem::new(Line::from(Span::styled(
            format!(" {} {}?", title, name),
            Style::default().fg(colors.secondary),
        ))),
        ListItem::new(Line::from(vec![
            Span::styled("y", Style::default().fg(colors.indicator)),
            Span::styled("es  ", Style::default().fg(colors.muted)),
            Span::styled("n", Style::default().fg(colors.indicator)),
            Span::styled("o  ", Style::default().fg(colors.muted)),
            Span::styled("Esc", Style::default().fg(colors.indicator)),
            Span::styled(" cancel", Style::default().fg(colors.muted)),
        ])),
    ];

    let list = List::new(items).block(
        crate::ui::themed_panel(colors)
            .title(format!(" {} ", title))
            .style(Style::default().bg(colors.background).fg(colors.border)),
    );

    frame.render_widget(ratatui::widgets::Clear, popup_area);
    frame.render_widget(list, popup_area);
}

fn render_list_picker_popup(
    frame: &mut Frame,
    area: Rect,
    colors: &ThemeColors,
    options: Vec<&str>,
    cursor: usize,
    title: &str,
) {
    let max_len = options.iter().map(|o| o.len()).max().unwrap_or(10);
    let popup_width = (max_len + 8).min(area.width.saturating_sub(4) as usize) as u16;
    let popup_height = (options.len() + 2).min(area.height.saturating_sub(4) as usize) as u16;
    let popup_area = centered_rect(popup_width, popup_height, area);

    let items: Vec<ListItem> = options
        .iter()
        .enumerate()
        .map(|(i, opt)| {
            let marker = if i == cursor { "> " } else { "  " };
            ListItem::new(Line::from(Span::styled(
                format!("{}{}", marker, opt),
                Style::default().fg(if i == cursor {
                    colors.highlight
                } else {
                    colors.secondary
                }),
            )))
        })
        .collect();

    let list = List::new(items).block(
        crate::ui::themed_panel(colors)
            .title(format!(" {} ", title))
            .style(Style::default().bg(colors.background).fg(colors.border)),
    );

    frame.render_widget(ratatui::widgets::Clear, popup_area);
    frame.render_widget(list, popup_area);
}

fn render_edit_field_popup(
    frame: &mut Frame,
    area: Rect,
    colors: &ThemeColors,
    group: &str,
    fields: Vec<&str>,
    cursor: usize,
) {
    let max_len = fields.iter().map(|f| f.len()).max().unwrap_or(10);
    let popup_width =
        ((max_len + 8).max(group.len() + 8)).min(area.width.saturating_sub(4) as usize) as u16;
    let popup_height = (fields.len() + 2).min(area.height.saturating_sub(4) as usize) as u16;
    let popup_area = centered_rect(popup_width, popup_height, area);

    let items: Vec<ListItem> = fields
        .iter()
        .enumerate()
        .map(|(i, field)| {
            let marker = if i == cursor { "> " } else { "  " };
            ListItem::new(Line::from(Span::styled(
                format!("{}{}", marker, field),
                Style::default().fg(if i == cursor {
                    colors.highlight
                } else {
                    colors.secondary
                }),
            )))
        })
        .collect();

    let list = List::new(items).block(
        crate::ui::themed_panel(colors)
            .title(format!(" Edit: {} ", group))
            .style(Style::default().bg(colors.background).fg(colors.border)),
    );

    frame.render_widget(ratatui::widgets::Clear, popup_area);
    frame.render_widget(list, popup_area);
}

fn render_ignore_manage_popup(
    frame: &mut Frame,
    area: Rect,
    colors: &ThemeColors,
    patterns: &[String],
    cursor: usize,
) {
    let max_len = patterns
        .iter()
        .map(|p| p.len() + 6)
        .max()
        .unwrap_or(20)
        .max(20);
    let popup_width = (max_len + 6).min(area.width.saturating_sub(4) as usize) as u16;
    let popup_height = (patterns.len() + 4).min(area.height.saturating_sub(4) as usize) as u16;
    let popup_area = centered_rect(popup_width, popup_height, area);

    let mut items: Vec<ListItem> = patterns
        .iter()
        .enumerate()
        .map(|(i, pat)| {
            let marker = if i == cursor { "> " } else { "  " };
            ListItem::new(Line::from(vec![
                Span::styled(
                    format!("{}{}", marker, pat),
                    Style::default().fg(if i == cursor {
                        colors.highlight
                    } else {
                        colors.secondary
                    }),
                ),
                Span::raw("  "),
                Span::styled(
                    "[r]",
                    Style::default().fg(if i == cursor {
                        colors.error
                    } else {
                        colors.muted
                    }),
                ),
            ]))
        })
        .collect();

    items.push(ListItem::new(Line::from(Span::styled(
        "-".repeat(max_len),
        Style::default().fg(colors.muted),
    ))));
    let add_marker = if cursor >= patterns.len() { "> " } else { "  " };
    items.push(ListItem::new(Line::from(Span::styled(
        format!("{}[+] Add Pattern", add_marker),
        Style::default().fg(if cursor >= patterns.len() {
            colors.highlight
        } else {
            colors.secondary
        }),
    ))));

    let list = List::new(items).block(
        crate::ui::themed_panel(colors)
            .title(" Ignore Patterns ")
            .style(Style::default().bg(colors.background).fg(colors.border)),
    );

    frame.render_widget(ratatui::widgets::Clear, popup_area);
    frame.render_widget(list, popup_area);
}

fn render_dir_browser_popup(
    frame: &mut Frame,
    area: Rect,
    colors: &ThemeColors,
    browse_path: &Path,
    cursor: usize,
    title: &str,
) {
    let popup_width = area.width.saturating_sub(6).max(30);
    let popup_height = area.height.saturating_sub(6).max(10);
    let popup_area = centered_rect(popup_width, popup_height, area);

    let mut list_state = ListState::default();
    let widget = FileBrowserWidget::new(colors, browse_path, cursor, title).dirs_only();

    frame.render_widget(ratatui::widgets::Clear, popup_area);
    frame.render_stateful_widget(widget, popup_area, &mut list_state);
}
