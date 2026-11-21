//! Paint for the settings screen: the list of rows and the dialogs on top of it. Every
//! dialog is built from the overlay's state and the pending settings, so painting reads
//! them and changes nothing.

use andre_core::{IconSet, StowAction, Theme};
use crossterm::event::KeyCode;
use ratatui::{
    layout::Rect,
    style::Style,
    text::{Line, Span},
    Frame,
};

use crate::components::settings::build::pattern_count;
use crate::components::settings::pending::PendingSettings;
use crate::components::settings::{IgnoreScope, Item, NameTarget, Overlay, Pick, Row, GROUP_DONE};
use crate::components::AppContext;
use crate::theme::ThemeColors;
use crate::ui::dialog::{dialog_rect, key_button, leave_dialog, render_dialog, Dialog};
use crate::ui::list_panel::{self, render_titled_list_panel, ListView};
use crate::ui::selection;
use crate::ui::text_field::{field_line, TextField};
use crate::ui::{display_path, elide};

/// Width of every dialog row. Dialogs size to their widest line, so this sets their width.
const DIALOG_ROW: u16 = 44;
/// Rows of a browser or a pattern list that a dialog shows at once.
const VISIBLE: usize = 8;
/// Width of a label column inside a dialog or the list.
const LABEL: usize = 14;

/// Paints the settings rows in the list pane. The selected row takes the cursor fill. With
/// unsaved edits the title ends in `· unsaved ●`, the dot in `warn`.
pub fn render_list(
    frame: &mut Frame,
    area: Rect,
    colors: &ThemeColors,
    dirty: bool,
    items: &[Item],
    view: &ListView,
) {
    let width = list_panel::inner(area).width;
    let rows = items
        .iter()
        .enumerate()
        .map(|(i, item)| list_line(item, i == view.cursor, colors, width))
        .collect();
    render_titled_list_panel(
        frame,
        area,
        colors,
        false,
        list_title(dirty, colors),
        rows,
        view,
        None,
    );
}

/// The list's title: `settings`, and `· unsaved ●` while there are unsaved edits.
fn list_title(dirty: bool, colors: &ThemeColors) -> Line<'static> {
    let mut spans = vec![Span::raw(" settings")];
    if dirty {
        spans.push(Span::raw(" \u{b7} unsaved "));
        spans.push(Span::styled("\u{25cf}", Style::default().fg(colors.warn)));
    }
    spans.push(Span::raw(" "));
    Line::from(spans)
}

/// One row: a header is a dim label with its meta in `accent`; a setting or a group is a
/// label column and its value.
pub fn list_line(item: &Item, selected: bool, colors: &ThemeColors, width: u16) -> Line<'static> {
    if item.row == Row::Header {
        let mut content = vec![Span::styled(
            item.label.clone(),
            Style::default().fg(colors.text_dim),
        )];
        if !item.value.is_empty() {
            content.push(Span::raw("  "));
            content.push(Span::styled(
                item.value.clone(),
                Style::default().fg(colors.accent),
            ));
        }
        return selection::row(content, false, colors, width);
    }
    let label_style = Style::default().fg(colors.text);
    let value_style = Style::default().fg(colors.value);
    let content = vec![
        Span::raw("  "),
        Span::styled(format!("{:<LABEL$}", item.label), label_style),
        Span::styled(item.value.clone(), value_style),
    ];
    selection::row(content, selected, colors, width)
}

/// Paints the dialog for the top overlay, centred in `area`.
pub fn render_overlay(
    frame: &mut Frame,
    area: Rect,
    colors: &ThemeColors,
    overlay: &Overlay,
    pending: &PendingSettings,
    ctx: &AppContext,
) {
    let dialog = overlay_dialog(overlay, pending, ctx, colors);
    render_dialog(frame, dialog_rect(area, &dialog), colors, &dialog);
}

/// The dialog `overlay` shows, built from the pending settings. Painting and mouse input
/// both read it, so a click lands on the buttons and rows drawn.
pub fn overlay_dialog(
    overlay: &Overlay,
    pending: &PendingSettings,
    ctx: &AppContext,
    colors: &ThemeColors,
) -> Dialog {
    match overlay {
        Overlay::Action { cursor } => {
            let options = StowAction::ALL
                .iter()
                .map(|action| action.as_str().to_string())
                .collect();
            picker("action", options, *cursor, "cancel", colors)
        }
        Overlay::Theme { cursor, .. } => {
            let options = Theme::ALL.iter().map(|t| t.name().to_string()).collect();
            picker("theme", options, *cursor, "revert", colors)
        }
        Overlay::Icons { cursor, .. } => {
            let options = IconSet::ALL.iter().map(|i| i.name().to_string()).collect();
            picker("icons", options, *cursor, "revert", colors)
        }
        Overlay::Group { group, cursor, .. } => group_menu(group, *cursor, pending, ctx, colors),
        Overlay::Name { target, field } => name_dialog(target, field, colors),
        Overlay::Browse { pick, browser } => browse_dialog(pick, browser, ctx, colors),
        Overlay::Remove { group, refs } => remove_dialog(group, *refs, colors),
        Overlay::Ignore {
            scope,
            cursor,
            edit,
        } => ignore_dialog(scope, *cursor, edit.as_ref(), pending, colors),
        Overlay::Leave => leave_dialog("andre.yml", colors),
    }
}

/// A list of options with the cursor on one. Enter keeps the cursor's option.
fn picker(
    title: &str,
    options: Vec<String>,
    cursor: usize,
    back: &'static str,
    colors: &ThemeColors,
) -> Dialog {
    let body = options
        .into_iter()
        .enumerate()
        .map(|(i, option)| {
            let content = vec![Span::styled(option, Style::default().fg(colors.text))];
            selection::row(content, i == cursor, colors, DIALOG_ROW)
        })
        .collect();
    Dialog {
        title: title.to_string(),
        body,
        buttons: vec![
            key_button(KeyCode::Esc, back, false),
            key_button(KeyCode::Enter, "keep", true),
        ],
    }
}

/// A group's fields. Enter on a field opens its dialog; the last row is done.
fn group_menu(
    group: &str,
    cursor: usize,
    pending: &PendingSettings,
    ctx: &AppContext,
    colors: &ThemeColors,
) -> Dialog {
    let (source, target) =
        crate::components::settings::build::group_paths(pending, group, ctx).unwrap_or_default();
    let own_ignore = pending
        .config
        .get_group_ignore(group)
        .map_or(0, <[String]>::len);
    let fields = [
        ("name", group.to_string(), "rename \u{25b8}"),
        ("source", elide(&source, 24), "browse \u{25b8}"),
        ("target", elide(&target, 24), "browse \u{25b8}"),
        ("ignore", pattern_count(own_ignore), "edit \u{25b8}"),
    ];
    let mut body: Vec<Line<'static>> = fields
        .into_iter()
        .enumerate()
        .map(|(i, (label, value, action))| {
            let content = vec![
                Span::styled(
                    format!("{label:<LABEL$}"),
                    Style::default().fg(colors.text_dim),
                ),
                Span::styled(format!("{value:<24}"), Style::default().fg(colors.value)),
                Span::styled(action, Style::default().fg(colors.text_dim)),
            ];
            selection::row(content, i == cursor, colors, DIALOG_ROW)
        })
        .collect();
    body.push(selection::row(
        vec![Span::styled(
            "done \u{25b8}",
            Style::default().fg(colors.text),
        )],
        cursor == GROUP_DONE,
        colors,
        DIALOG_ROW,
    ));
    Dialog {
        title: format!("edit group {group}"),
        body,
        buttons: vec![key_button(KeyCode::Esc, "cancel", false)],
    }
}

fn name_dialog(target: &NameTarget, field: &TextField, colors: &ThemeColors) -> Dialog {
    let (title, verb) = match target {
        NameTarget::New => ("new group", "create"),
        NameTarget::Rename { .. } => ("rename group", "rename"),
    };
    let mut body = vec![field_line("name", field, true, colors)];
    if let Some(error) = &field.error {
        body.push(Line::styled(error.clone(), Style::default().fg(colors.err)));
    }
    Dialog {
        title: title.to_string(),
        body,
        buttons: vec![
            key_button(KeyCode::Esc, "cancel", false),
            key_button(KeyCode::Enter, verb, true),
        ],
    }
}

/// The folder being picked, then the folders in it, a window of them with the cursor
/// shown. Enter chooses the highlighted folder.
fn browse_dialog(
    pick: &Pick,
    browser: &crate::components::browser::Browser,
    ctx: &AppContext,
    colors: &ThemeColors,
) -> Dialog {
    let title = match pick {
        Pick::NewSource { .. } => "source for new group".to_string(),
        Pick::NewTarget { name, .. } => format!("target for {name}"),
        Pick::Source { group } => format!("source for {group}"),
        Pick::Target { group } => format!("target for {group}"),
    };
    let width = DIALOG_ROW as usize;
    let mut body = vec![Line::styled(
        elide(&display_path(&browser.path, &ctx.home_dir), width),
        Style::default().fg(colors.text_dim),
    )];
    body.push(Line::raw(""));
    let entries = browser.entries();
    if entries.is_empty() {
        body.push(Line::styled(
            "no folders here",
            Style::default().fg(colors.text_dim),
        ));
    }
    let start = browser.cursor.saturating_sub(VISIBLE - 1);
    for (i, entry) in entries.iter().enumerate().skip(start).take(VISIBLE) {
        let content = vec![
            Span::styled("/ ", Style::default().fg(colors.text_dim)),
            Span::styled(entry.name.clone(), Style::default().fg(colors.text)),
        ];
        body.push(selection::row(
            content,
            i == browser.cursor,
            colors,
            DIALOG_ROW,
        ));
    }
    let choose = match browser.current() {
        Some(entry) => format!("choose {}", entry.name),
        None => "choose".to_string(),
    };
    Dialog {
        title,
        body,
        buttons: vec![
            key_button(KeyCode::Esc, "cancel", false),
            key_button(KeyCode::Enter, choose, true),
        ],
    }
}

fn remove_dialog(group: &str, refs: usize, colors: &ThemeColors) -> Dialog {
    let text = Style::default().fg(colors.text);
    let mut body = vec![Line::styled(
        format!("Remove group `{group}` from andre.yml?"),
        text,
    )];
    if refs > 0 {
        let (noun, verb) = if refs == 1 {
            ("profile ref", "is")
        } else {
            ("profile refs", "are")
        };
        body.push(Line::styled(
            format!("{refs} {noun} to {group}/* {verb} removed."),
            text,
        ));
    }
    body.push(Line::styled("No files are touched.", text));
    Dialog {
        title: "remove group".to_string(),
        body,
        buttons: vec![
            key_button(KeyCode::Char('n'), "keep", false),
            key_button(KeyCode::Char('y'), "remove", true),
        ],
    }
}

/// A pattern list: one row per pattern, then the add row. An open edit shows its field in
/// place of its row.
fn ignore_dialog(
    scope: &IgnoreScope,
    cursor: usize,
    edit: Option<&crate::components::settings::LineEdit>,
    pending: &PendingSettings,
    colors: &ThemeColors,
) -> Dialog {
    let (title, patterns) = match scope {
        IgnoreScope::Global => ("global ignore".to_string(), pending.config.global_ignore()),
        IgnoreScope::Group(group) => (
            format!("ignore \u{b7} {group}"),
            pending
                .config
                .get_group_ignore(group)
                .map(<[String]>::to_vec)
                .unwrap_or_default(),
        ),
    };
    let rows = patterns.len() + 1;
    let start = cursor.saturating_sub(VISIBLE - 1);
    let body = (start..rows)
        .take(VISIBLE)
        .map(|i| {
            let editing = edit.filter(|line| line.index.unwrap_or(patterns.len()) == i);
            if let Some(line) = editing {
                return field_line("", &line.field, true, colors);
            }
            if i == patterns.len() {
                let content = vec![Span::styled(
                    "+ add a pattern",
                    Style::default().fg(colors.text_dim),
                )];
                return selection::row(content, i == cursor, colors, DIALOG_ROW);
            }
            let content = vec![Span::styled(
                elide(&patterns[i], DIALOG_ROW as usize - 4),
                Style::default().fg(colors.text),
            )];
            selection::row(content, i == cursor, colors, DIALOG_ROW)
        })
        .collect();
    let buttons = if edit.is_some() {
        vec![
            key_button(KeyCode::Esc, "cancel", false),
            key_button(KeyCode::Enter, "done", true),
        ]
    } else {
        vec![key_button(KeyCode::Esc, "back", false)]
    };
    Dialog {
        title,
        body,
        buttons,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn body_text(dialog: &Dialog) -> Vec<String> {
        dialog
            .body
            .iter()
            .map(|line| {
                line.spans
                    .iter()
                    .map(|span| span.content.as_ref())
                    .collect()
            })
            .collect()
    }

    #[test]
    fn test_remove_dialog_says_the_profile_refs_are_removed() {
        let colors = ThemeColors::from_theme(Theme::Default);
        let text = body_text(&remove_dialog("work", 2, &colors));
        assert!(text.contains(&"2 profile refs to work/* are removed.".to_string()));

        let text = body_text(&remove_dialog("work", 1, &colors));
        assert!(text.contains(&"1 profile ref to work/* is removed.".to_string()));
    }

    #[test]
    fn test_remove_dialog_without_refs_says_nothing_about_profiles() {
        let colors = ThemeColors::from_theme(Theme::Default);
        let text = body_text(&remove_dialog("work", 0, &colors));
        assert!(text.iter().all(|line| !line.contains("profile")));
    }
}
