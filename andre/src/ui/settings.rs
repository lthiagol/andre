use std::path::Path;

use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Clear, List, ListItem, ListState},
    Frame,
};

use crate::components::browser::Browser;
use crate::components::settings::rows::{build_flat_items, SettingsRow};
use crate::components::settings::ConfigMessage;
use crate::components::settings::{GroupDialog, PickerState};
use crate::theme::ThemeColors;
use crate::ui::icons::Icons;
use crate::ui::selection;
use crate::ui::widgets::file_browser::FileBrowserWidget;
use crate::ui::widgets::scrollbar::{border_track, render_scrollbar};
use andre_core::{config::Config, IconSet, StowFlags};

/// Width of the setting name column. Values start at the same column on every row.
const LABEL_WIDTH: usize = 18;

fn val_str(on: bool) -> &'static str {
    if on {
        "ON "
    } else {
        "OFF"
    }
}

/// A setting's name. Bold on the cursor row, like the labels elsewhere.
fn label(text: &str, colors: &ThemeColors, selected: bool) -> Span<'static> {
    let style = if selected {
        Style::default()
            .fg(colors.text)
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(colors.text)
    };
    Span::styled(format!("{text:<LABEL_WIDTH$}"), style)
}

fn value(text: String, fg: Color) -> Span<'static> {
    Span::styled(text, Style::default().fg(fg))
}

fn flag_value(on: bool, colors: &ThemeColors) -> Span<'static> {
    let fg = if on { colors.ok } else { colors.err };
    value(val_str(on).to_string(), fg)
}

#[allow(clippy::too_many_arguments)]
fn render_row(
    row: &SettingsRow,
    is_cursor: bool,
    colors: &ThemeColors,
    flags: &StowFlags,
    theme_name: &str,
    icons: IconSet,
    config_path: &Path,
    config_dir: &Path,
    config_dirty: bool,
    config_message: ConfigMessage,
    filename: &str,
    width: u16,
) -> ListItem<'static> {
    let StowFlags {
        action,
        verbosity,
        dry_run,
        no_folding,
        adopt,
        dotfiles,
    } = *flags;
    let selectable = !matches!(
        row,
        SettingsRow::ConfigPathInfo
            | SettingsRow::ConfigResolveInfo
            | SettingsRow::Spacer
            | SettingsRow::SavedMessage
    );
    let selected = is_cursor && selectable;

    let spans: Vec<Span<'static>> = match row {
        SettingsRow::ConfigPathInfo => vec![value(
            format!("Config: {}", config_path.display()),
            colors.text_dim,
        )],
        SettingsRow::ConfigResolveInfo => vec![value(
            format!("Paths resolve relative to: {}", config_dir.display()),
            colors.text_dim,
        )],
        SettingsRow::Verbosity => vec![
            label("verbosity:", colors, selected),
            value(verbosity.to_string(), colors.value),
        ],
        SettingsRow::DryRun => vec![
            label("dry_run:", colors, selected),
            flag_value(dry_run, colors),
        ],
        SettingsRow::NoFolding => vec![
            label("no_folding:", colors, selected),
            flag_value(no_folding, colors),
        ],
        SettingsRow::Adopt => vec![label("adopt:", colors, selected), flag_value(adopt, colors)],
        SettingsRow::Dotfiles => vec![
            label("dotfiles:", colors, selected),
            flag_value(dotfiles, colors),
        ],
        SettingsRow::Action => vec![
            label("action:", colors, selected),
            value(action.as_str().to_string(), colors.value),
        ],
        SettingsRow::ThemeName => vec![
            label("theme:", colors, selected),
            value(theme_name.to_string(), colors.value),
        ],
        SettingsRow::IconsName => vec![
            label("icons:", colors, selected),
            value(icons.name().to_string(), colors.value),
        ],
        SettingsRow::IgnoreLine { count } => {
            let noun = if *count == 1 { "pattern" } else { "patterns" };
            vec![
                label("ignore:", colors, selected),
                value(format!("{count} {noun}"), colors.value),
            ]
        }
        SettingsRow::Spacer => vec![Span::raw("")],
        SettingsRow::GroupLine { count } => {
            let noun = if *count == 1 { "group" } else { "groups" };
            vec![
                label("group:", colors, selected),
                value(format!("{count} {noun}"), colors.value),
            ]
        }
        SettingsRow::SaveButton => {
            let (text, fg) = if config_dirty {
                (format!("[Update {filename}]"), colors.accent)
            } else {
                ("[All saved]".to_string(), colors.text_dim)
            };
            vec![value(text, fg)]
        }
        SettingsRow::SavedMessage => {
            let (text, fg) = match config_message {
                ConfigMessage::ConfirmDiscard => (
                    "Unsaved changes  y=save  n=discard  Esc=cancel".to_string(),
                    colors.warn,
                ),
                ConfigMessage::Saved => ("Saved.".to_string(), colors.ok),
                ConfigMessage::Hidden => (String::new(), colors.text_dim),
            };
            vec![value(text, fg)]
        }
    };

    ListItem::new(selection::row(spans, selected, colors, width))
}

#[allow(clippy::too_many_arguments)]
pub fn render_settings(
    frame: &mut Frame,
    area: Rect,
    colors: &ThemeColors,
    title: &str,
    config: &Config,
    config_path: &Path,
    config_dir: &Path,
    flags: &StowFlags,
    theme_name: &str,
    icons: IconSet,
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

    let viewport_height = area.height.saturating_sub(2).max(1) as usize;
    // The panel borders take one column on each side.
    let width = area.width.saturating_sub(2);

    let list_items: Vec<ListItem> = flat
        .iter()
        .enumerate()
        .map(|(i, row)| {
            render_row(
                row,
                i == cursor,
                colors,
                flags,
                theme_name,
                icons,
                config_path,
                config_dir,
                config_dirty,
                config_message,
                filename,
                width,
            )
        })
        .collect();

    let list = List::new(list_items).block(crate::ui::themed_panel(colors).title(title));
    let mut list_state = ListState::default();
    list_state.select(Some(cursor));
    frame.render_stateful_widget(list, area, &mut list_state);
    render_scrollbar(
        frame,
        border_track(area),
        colors,
        total,
        viewport_height,
        cursor,
    );

    if let Some(picker) = picker {
        render_picker_popup(frame, area, colors, picker);
    }
    if let Some(dialog) = group_dialog {
        render_group_dialog(frame, area, colors, icons, dialog, config);
    }
}

/// One row of a popup list: the shared cursor style over plain text.
fn popup_row(text: &str, selected: bool, colors: &ThemeColors, width: u16) -> ListItem<'static> {
    let span = Span::styled(text.to_string(), Style::default().fg(colors.text));
    ListItem::new(selection::row(vec![span], selected, colors, width))
}

/// Clears the popup area and draws a themed panel with a list in it. `cursor` keeps
/// the cursor row in view when the list is taller than the popup.
fn render_popup_list(
    frame: &mut Frame,
    popup_area: Rect,
    colors: &ThemeColors,
    title: &str,
    items: Vec<ListItem>,
    cursor: usize,
) {
    let list = List::new(items).block(
        crate::ui::themed_panel(colors)
            .title(format!(" {} ", title))
            .style(Style::default().bg(colors.bg).fg(colors.border)),
    );
    let mut list_state = ListState::default();
    list_state.select(Some(cursor));

    frame.render_widget(Clear, popup_area);
    frame.render_stateful_widget(list, popup_area, &mut list_state);
}

fn render_picker_popup(frame: &mut Frame, area: Rect, colors: &ThemeColors, picker: &PickerState) {
    let popup_width = (picker.options.iter().map(String::len).max().unwrap_or(10) + 8)
        .min(area.width.saturating_sub(4) as usize) as u16;
    let popup_height =
        (picker.options.len() + 2).min(area.height.saturating_sub(4) as usize) as u16;

    let popup_area = centered_rect(popup_width, popup_height, area);
    let width = popup_area.width.saturating_sub(2);

    let items: Vec<ListItem> = picker
        .options
        .iter()
        .enumerate()
        .map(|(i, opt)| popup_row(opt, i == picker.selected_index, colors, width))
        .collect();

    render_popup_list(
        frame,
        popup_area,
        colors,
        &format!("pick {}", picker.title.to_lowercase()),
        items,
        picker.selected_index,
    );
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
    icons: IconSet,
    dialog: &GroupDialog,
    config: &Config,
) {
    match dialog {
        GroupDialog::Actions { cursor } => {
            render_group_actions_popup(frame, area, colors, *cursor);
        }
        GroupDialog::AddName { name } => {
            render_name_input_popup(frame, area, colors, name, "add group");
        }
        GroupDialog::AddBrowse {
            name: _,
            source: _,
            browser,
        } => {
            render_dir_browser_popup(frame, area, colors, icons, browser, "select directory");
        }
        GroupDialog::RemoveSelect { cursor } => {
            let names: Vec<&str> = config.groups.keys().map(String::as_str).collect();
            render_list_picker_popup(frame, area, colors, names, *cursor, "remove group");
        }
        GroupDialog::RemoveConfirm { name } => {
            render_confirm_popup(frame, area, colors, name, "remove group");
        }
        GroupDialog::EditSelect { cursor } => {
            let names: Vec<&str> = config.groups.keys().map(String::as_str).collect();
            render_list_picker_popup(frame, area, colors, names, *cursor, "edit group");
        }
        GroupDialog::EditField { group, cursor } => {
            let fields = vec!["Name", "Source", "Target", "Ignore"];
            render_edit_field_popup(frame, area, colors, group, fields, *cursor);
        }
        GroupDialog::EditName { name, .. } => {
            render_name_input_popup(frame, area, colors, name, "edit group name");
        }
        GroupDialog::EditBrowse {
            group: _,
            field: _,
            browser,
        } => {
            render_dir_browser_popup(frame, area, colors, icons, browser, "select directory");
        }
        GroupDialog::IgnoreManage { group, cursor } => {
            let patterns: Vec<String> = match group {
                Some(g) => config
                    .get_group_ignore(g)
                    .map(<[String]>::to_vec)
                    .unwrap_or_default(),
                None => config.global_ignore(),
            };
            render_ignore_manage_popup(frame, area, colors, &patterns, *cursor);
        }
        GroupDialog::IgnoreAdd { group: _, pattern } => {
            render_name_input_popup(frame, area, colors, pattern, "add ignore pattern");
        }
    }
}

fn render_group_actions_popup(frame: &mut Frame, area: Rect, colors: &ThemeColors, cursor: usize) {
    let options = ["Add", "Remove", "Edit"];
    let popup_width = 24u16.min(area.width.saturating_sub(4));
    let popup_height = (options.len() + 2) as u16;
    let popup_area = centered_rect(popup_width, popup_height, area);
    let width = popup_area.width.saturating_sub(2);

    let items: Vec<ListItem> = options
        .iter()
        .enumerate()
        .map(|(i, opt)| popup_row(opt, i == cursor, colors, width))
        .collect();

    render_popup_list(frame, popup_area, colors, "group actions", items, cursor);
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
            Span::styled(" Name: ", Style::default().fg(colors.text_dim)),
            Span::styled(display_name, Style::default().fg(colors.value)),
        ])),
        ListItem::new(Line::from(Span::styled(
            "",
            Style::default().fg(colors.text_dim),
        ))),
        ListItem::new(Line::from(vec![
            Span::styled("Enter", Style::default().fg(colors.key)),
            Span::styled(" confirm  ", Style::default().fg(colors.text_dim)),
            Span::styled("Esc", Style::default().fg(colors.key)),
            Span::styled(" cancel", Style::default().fg(colors.text_dim)),
        ])),
    ];

    let list = List::new(items).block(
        crate::ui::themed_panel(colors)
            .title(format!(" {} ", title))
            .style(Style::default().bg(colors.bg).fg(colors.border)),
    );

    frame.render_widget(Clear, popup_area);
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
            Style::default().fg(colors.text),
        ))),
        ListItem::new(Line::from(vec![
            Span::styled("y", Style::default().fg(colors.key)),
            Span::styled("es  ", Style::default().fg(colors.text_dim)),
            Span::styled("n", Style::default().fg(colors.key)),
            Span::styled("o  ", Style::default().fg(colors.text_dim)),
            Span::styled("Esc", Style::default().fg(colors.key)),
            Span::styled(" cancel", Style::default().fg(colors.text_dim)),
        ])),
    ];

    let list = List::new(items).block(
        crate::ui::themed_panel(colors)
            .title(format!(" {} ", title))
            .style(Style::default().bg(colors.bg).fg(colors.border)),
    );

    frame.render_widget(Clear, popup_area);
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
    let width = popup_area.width.saturating_sub(2);

    let items: Vec<ListItem> = options
        .iter()
        .enumerate()
        .map(|(i, opt)| popup_row(opt, i == cursor, colors, width))
        .collect();

    render_popup_list(frame, popup_area, colors, title, items, cursor);
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
    let width = popup_area.width.saturating_sub(2);

    let items: Vec<ListItem> = fields
        .iter()
        .enumerate()
        .map(|(i, field)| popup_row(field, i == cursor, colors, width))
        .collect();

    render_popup_list(
        frame,
        popup_area,
        colors,
        &format!("edit: {}", group),
        items,
        cursor,
    );
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
    let width = popup_area.width.saturating_sub(2);

    let mut items: Vec<ListItem> = patterns
        .iter()
        .enumerate()
        .map(|(i, pat)| {
            let selected = i == cursor;
            let remove_fg = if selected {
                colors.err
            } else {
                colors.text_dim
            };
            let spans = vec![
                Span::styled(pat.clone(), Style::default().fg(colors.text)),
                Span::raw("  "),
                Span::styled("[r]", Style::default().fg(remove_fg)),
            ];
            ListItem::new(selection::row(spans, selected, colors, width))
        })
        .collect();

    let rule = Span::styled("-".repeat(max_len), Style::default().fg(colors.border));
    items.push(ListItem::new(selection::row(
        vec![rule],
        false,
        colors,
        width,
    )));
    items.push(popup_row(
        "[+] Add Pattern",
        cursor >= patterns.len(),
        colors,
        width,
    ));

    render_popup_list(frame, popup_area, colors, "ignore patterns", items, cursor);
}

fn render_dir_browser_popup(
    frame: &mut Frame,
    area: Rect,
    colors: &ThemeColors,
    icons: IconSet,
    browser: &Browser,
    title: &str,
) {
    let popup_width = area.width.saturating_sub(6).max(30);
    let popup_height = area.height.saturating_sub(6).max(10);
    let popup_area = centered_rect(popup_width, popup_height, area);

    let mut list_state = ListState::default();
    let widget = FileBrowserWidget::new(
        colors,
        Icons::for_set(icons),
        &browser.path,
        browser.cursor,
        title,
    )
    .with_entries(browser.entries());

    frame.render_widget(Clear, popup_area);
    frame.render_stateful_widget(widget, popup_area, &mut list_state);
}
