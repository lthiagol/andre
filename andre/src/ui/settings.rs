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

use crate::components::browser::Browser;
use crate::components::settings::pending::PendingSettings;
use crate::components::settings::{Item, LineEdit, Overlay, Row};
use crate::components::AppContext;
use crate::theme::ThemeColors;
use crate::ui::dialog::{dialog_rect, key_button, leave_dialog, render_dialog, Dialog};
use crate::ui::list_panel::{self, render_titled_list_panel, ListView};
use crate::ui::selection;
use crate::ui::text_field::field_line;
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

/// One row: a header is a dim label with its meta in `accent`; a setting is a label column
/// and its value, which is elided to the width the row has left.
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
    let room = usize::from(width).saturating_sub(2 + LABEL + 1);
    let content = vec![
        Span::raw("  "),
        Span::styled(format!("{:<LABEL$}", item.label), label_style),
        Span::styled(elide(&item.value, room), value_style),
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
        Overlay::Browse { browser } => packages_dialog(browser, ctx, colors),
        Overlay::Ignore { cursor, edit } => ignore_dialog(*cursor, edit.as_ref(), pending, colors),
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

/// The folder being picked, then the folders in it, a window of them with the cursor
/// shown. Enter chooses the highlighted folder.
fn packages_dialog(browser: &Browser, ctx: &AppContext, colors: &ThemeColors) -> Dialog {
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
        title: "packages folder".to_string(),
        body,
        buttons: vec![
            key_button(KeyCode::Esc, "cancel", false),
            key_button(KeyCode::Enter, choose, true),
        ],
    }
}

/// A pattern list: one row per pattern, then the add row. An open edit shows its field in
/// place of its row.
fn ignore_dialog(
    cursor: usize,
    edit: Option<&LineEdit>,
    pending: &PendingSettings,
    colors: &ThemeColors,
) -> Dialog {
    let patterns = pending.config.global_ignore();
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
        title: "global ignore".to_string(),
        body,
        buttons,
    }
}
