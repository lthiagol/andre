//! Drawing for the profile editor: the packages pane title, the form pane (description,
//! includes, resolved facts) and the leave dialog. Everything here draws from the state it
//! is given. Nothing reads the disk.

use std::borrow::Cow;
use std::collections::BTreeMap;

use andre_core::profile::{split_package_ref, Origin, Profile, ProfileSet, ResolvedProfile};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::{
    layout::{Margin, Rect},
    style::Style,
    text::{Line, Span},
    widgets::Paragraph,
    Frame,
};
use unicode_width::UnicodeWidthStr;

use crate::theme::ThemeColors;
use crate::ui::dialog::{Button, Dialog};
use crate::ui::focus_panel;
use crate::ui::icons::Icons;
use crate::ui::selection::row_with_glyphs;
use crate::ui::text_field::{field_spans, TextField};
use crate::ui::{elide, plural};

/// Label column of the facts, as in the detail panes.
const LABEL_WIDTH: usize = 10;

/// What the form pane draws. Built by the editor from its state.
pub struct FormView<'a> {
    pub draft: &'a Profile,
    pub saved: &'a Profile,
    pub resolved: Option<&'a ResolvedProfile>,
    pub resolve_error: Option<&'a str>,
    /// The profiles the form offers as includes.
    pub includes: &'a [String],
    /// 0 is the description, `n` is `includes[n - 1]`.
    pub cursor: usize,
    pub focused: bool,
    /// The description being typed, if it is.
    pub editing: Option<&'a TextField>,
    pub profiles: &'a ProfileSet,
}

/// The packages pane title: how many packages are marked, out of all of them.
pub fn packages_title(marked: usize, total: usize) -> String {
    format!("packages {marked} of {total}")
}

/// Draws the form pane in `area`.
pub fn render_form(
    frame: &mut Frame,
    area: Rect,
    colors: &ThemeColors,
    icons: &Icons,
    form: &FormView,
) {
    let block = focus_panel(colors, form.focused).title(form_title(form, colors));
    let inner = block.inner(area).inner(Margin::new(1, 0));
    frame.render_widget(block, area);
    let (lines, cursor_line) = form_lines(form, colors, icons, inner.width);
    // Scroll just far enough that the cursor's row is the last one shown when the form is taller
    // than the pane. Nothing is stored: the offset is worked out from this frame's rows.
    let scroll = cursor_line.map_or(0, |line| {
        (line + 1).saturating_sub(usize::from(inner.height))
    });
    frame.render_widget(Paragraph::new(lines).scroll((scroll as u16, 0)), inner);
}

/// The profile's name, and `· unsaved ●` when the draft differs from the file.
fn form_title(form: &FormView, colors: &ThemeColors) -> Line<'static> {
    let text = Style::default().fg(colors.text);
    let mut spans = vec![Span::styled(format!(" {}", form.draft.name), text)];
    if form.draft != form.saved {
        spans.push(Span::styled(" · unsaved ", text));
        spans.push(Span::styled("●", Style::default().fg(colors.warn)));
    }
    spans.push(Span::raw(" "));
    Line::from(spans)
}

/// The form's rows, top to bottom: the resolve error, the description, the includes, then
/// the facts of the resolved profile. Also returns the index of the row under the cursor,
/// when the form has the keys.
fn form_lines(
    form: &FormView,
    colors: &ThemeColors,
    icons: &Icons,
    width: u16,
) -> (Vec<Line<'static>>, Option<usize>) {
    let dim = Style::default().fg(colors.text_dim);
    let mut lines = Vec::new();
    let mut cursor_line = None;
    if let Some(error) = form.resolve_error {
        lines.push(Line::styled(
            elide(error, usize::from(width)),
            Style::default().fg(colors.err),
        ));
        lines.push(Line::raw(""));
    }

    lines.push(Line::styled("description", dim));
    let on_row = |i: usize| form.focused && form.cursor == i;
    if on_row(0) {
        cursor_line = Some(lines.len());
    }
    let content = match form.editing {
        Some(field) => field_spans(field, true, colors),
        None => match form.draft.description.as_deref() {
            Some(text) => vec![Span::styled(
                text.to_string(),
                Style::default().fg(colors.text),
            )],
            None => vec![Span::styled("no description", dim)],
        },
    };
    lines.push(row_with_glyphs(
        Vec::new(),
        content,
        on_row(0),
        colors,
        width,
    ));
    lines.push(Line::raw(""));

    lines.push(Line::styled("includes", dim));
    for (i, name) in form.includes.iter().enumerate() {
        if on_row(i + 1) {
            cursor_line = Some(lines.len());
        }
        let checked = form.draft.includes.contains(name);
        let exists = form.profiles.get(name).is_some();
        let glyph = Style::default().fg(if checked {
            colors.accent
        } else {
            colors.text_dim
        });
        let glyphs = vec![Span::styled(icons.checkbox(checked), glyph), Span::raw(" ")];
        let mut content = vec![Span::styled(
            name.clone(),
            Style::default().fg(if exists { colors.text } else { colors.text_dim }),
        )];
        if !exists {
            content.push(Span::styled("  missing", dim));
        }
        lines.push(row_with_glyphs(
            glyphs,
            content,
            on_row(i + 1),
            colors,
            width,
        ));
    }

    if let Some(resolved) = form.resolved {
        lines.push(Line::raw(""));
        lines.extend(fact_lines(resolved, width, colors));
    }
    (lines, cursor_line)
}

/// The resolved facts as rows: a label column, then the value. The label column fits the
/// longest label with a space after it, up to half the pane, so a label never runs into its
/// value. A label wider than that is elided.
fn fact_lines(resolved: &ResolvedProfile, width: u16, colors: &ThemeColors) -> Vec<Line<'static>> {
    let dim = Style::default().fg(colors.text_dim);
    let facts = facts(resolved);
    let width = usize::from(width);
    let label_width = facts
        .iter()
        .map(|(label, _)| label.width() + 1)
        .max()
        .unwrap_or(0)
        .max(LABEL_WIDTH)
        .min(width / 2);
    let room = width.saturating_sub(label_width);
    facts
        .iter()
        .map(|(label, value)| {
            Line::from(vec![
                Span::styled(
                    format!(
                        "{:<label_width$}",
                        elide(label, label_width.saturating_sub(1))
                    ),
                    dim,
                ),
                Span::styled(elide(value, room), Style::default().fg(colors.value)),
            ])
        })
        .collect()
}

/// The resolved facts: the total, the profile's own packages, and the packages each include
/// brings in. Package names are shown without their group.
fn facts(resolved: &ResolvedProfile) -> Vec<(String, String)> {
    let mut own = Vec::new();
    let mut via: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for entry in &resolved.entries {
        let name = split_package_ref(&entry.package)
            .map_or(entry.package.as_str(), |(_, package)| package)
            .to_string();
        match &entry.origin {
            Origin::Own => own.push(name),
            Origin::Included { include, .. } => {
                via.entry(include.clone()).or_default().push(name);
            }
        }
    }

    let total = resolved.entries.len();
    let mut facts = vec![(
        "resolved".to_string(),
        format!("{total} {}", plural(total, "package", "packages")),
    )];
    facts.push(("own".to_string(), names_fact(&own)));
    for (include, names) in via {
        facts.push((format!("via {include}"), names_fact(&names)));
    }
    facts
}

/// `2  git, shell`, or `0` for none.
fn names_fact(names: &[String]) -> String {
    match names.len() {
        0 => "0".to_string(),
        n => format!("{n}  {}", names.join(", ")),
    }
}

/// The dialog that asks what to do with unsaved edits. Primary is `y save`, on the right.
pub fn leave_dialog(name: &str, colors: &ThemeColors) -> Dialog {
    Dialog {
        title: "unsaved changes".to_string(),
        body: vec![
            Line::raw(""),
            Line::styled(
                format!("Save changes to profile `{name}`?"),
                Style::default().fg(colors.text),
            ),
            Line::raw(""),
        ],
        buttons: vec![
            button(KeyCode::Esc, "stay", false),
            button(KeyCode::Char('n'), "discard", false),
            button(KeyCode::Char('y'), "save", true),
        ],
    }
}

fn button(code: KeyCode, label: &'static str, primary: bool) -> Button {
    Button {
        key: KeyEvent::new(code, KeyModifiers::NONE),
        label: Cow::Borrowed(label),
        primary,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_packages_title_counts_marked_out_of_all() {
        assert_eq!(packages_title(3, 9), "packages 3 of 9");
    }

    #[test]
    fn test_facts_split_own_from_each_include() {
        let resolved = ResolvedProfile {
            name: "personal".into(),
            entries: vec![
                entry("home/ssh", Origin::Own),
                entry(
                    "home/git",
                    Origin::Included {
                        include: "base".into(),
                        declared_in: "base".into(),
                    },
                ),
                entry(
                    "home/shell",
                    Origin::Included {
                        include: "base".into(),
                        declared_in: "base".into(),
                    },
                ),
            ],
        };
        assert_eq!(
            facts(&resolved),
            vec![
                ("resolved".to_string(), "3 packages".to_string()),
                ("own".to_string(), "1  ssh".to_string()),
                ("via base".to_string(), "2  git, shell".to_string()),
            ]
        );
    }

    #[test]
    fn test_facts_keep_a_gap_after_a_long_include_label() {
        let colors = ThemeColors::from_theme(andre_core::Theme::Default);
        let resolved = ResolvedProfile {
            name: "personal".into(),
            entries: vec![entry(
                "home/ssh",
                Origin::Included {
                    include: "agent-work".into(),
                    declared_in: "agent-work".into(),
                },
            )],
        };
        let rows: Vec<String> = fact_lines(&resolved, 40, &colors)
            .iter()
            .map(ToString::to_string)
            .collect();
        assert_eq!(rows[2], "via agent-work 1  ssh");
    }

    #[test]
    fn test_facts_elide_a_label_too_wide_for_half_the_pane() {
        let colors = ThemeColors::from_theme(andre_core::Theme::Default);
        let resolved = ResolvedProfile {
            name: "personal".into(),
            entries: vec![entry(
                "home/ssh",
                Origin::Included {
                    include: "agent-work".into(),
                    declared_in: "agent-work".into(),
                },
            )],
        };
        // 20 columns: the label column is capped at 10, so `via agent-work` is elided to fit.
        let rows: Vec<String> = fact_lines(&resolved, 20, &colors)
            .iter()
            .map(ToString::to_string)
            .collect();
        assert_eq!(rows[2], "via agen… 1  ssh");
    }

    #[test]
    fn test_facts_say_zero_when_there_is_nothing_of_a_kind() {
        let resolved = ResolvedProfile {
            name: "empty".into(),
            entries: Vec::new(),
        };
        assert_eq!(
            facts(&resolved)[0],
            ("resolved".to_string(), "0 packages".to_string())
        );
        assert_eq!(facts(&resolved)[1], ("own".to_string(), "0".to_string()));
    }

    #[test]
    fn test_leave_dialog_puts_save_on_the_right_as_the_primary_button() {
        let colors = ThemeColors::from_theme(andre_core::Theme::Default);
        let dialog = leave_dialog("personal", &colors);
        let labels: Vec<&str> = dialog.buttons.iter().map(|b| b.label.as_ref()).collect();
        assert_eq!(labels, vec!["stay", "discard", "save"]);
        assert!(dialog.buttons.last().unwrap().primary);
        assert!(dialog
            .body
            .iter()
            .any(|l| l.to_string().contains("`personal`?")));
    }

    fn entry(package: &str, origin: Origin) -> andre_core::profile::ResolvedEntry {
        andre_core::profile::ResolvedEntry {
            package: package.into(),
            origin,
            missing: false,
        }
    }
}
