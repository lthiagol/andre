//! What the profiles screen shows: the list rows, the selected profile's detail and the
//! dialogs. Everything here is a pure function of the app context, so `update` builds it and
//! paint only draws it.

use std::borrow::Cow;
use std::collections::BTreeSet;

use andre_core::profile::{
    is_valid_name, Origin, Profile, ProfileSet, ResolvedEntry, ResolvedProfile,
};
use andre_core::StowStatus;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::{
    layout::Rect,
    style::{Modifier, Style},
    text::{Line, Span},
    Frame,
};
use unicode_width::UnicodeWidthStr;

use super::{NameCheck, NameField, Popup};
use crate::components::AppContext;
use crate::theme::ThemeColors;
use crate::ui::detail::Detail;
use crate::ui::dialog::{dialog_rect, render_dialog, Button, Dialog};
use crate::ui::icons::Icons;
use crate::ui::selection;
use crate::ui::status_style::status_color;
use crate::ui::text_field::field_line;
use crate::ui::{display_path, elide};

/// The label of the name field, and the rule a profile name must follow, shown under it.
const NAME_LABEL: &str = "name";
const NAME_RULE: &str = "a-z 0-9 _ -, starts a-z/0-9";

/// One profile's list row. `count` is the number of resolved packages, or `None` when the
/// profile does not resolve (a cycle or an unknown include).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Row {
    pub name: String,
    pub count: Option<usize>,
}

/// One row per profile, in name order, as the list shows them.
pub fn rows(profiles: &ProfileSet, known: &BTreeSet<String>) -> Vec<Row> {
    profiles
        .iter()
        .map(|profile| Row {
            name: profile.name.clone(),
            count: profiles
                .resolve(&profile.name, known)
                .ok()
                .map(|resolved| resolved.entries.len()),
        })
        .collect()
}

/// A profile row: the profile glyph, the name, and the package count at the right.
pub fn list_row(
    row: &Row,
    selected: bool,
    glyph: &'static str,
    colors: &ThemeColors,
    width: u16,
) -> Line<'static> {
    let (count, count_colour) = match row.count {
        Some(n) => (n.to_string(), colors.value),
        None => ("err".to_string(), colors.err),
    };
    // The accent bar, the glyph, a space and the count with its margin take the fixed columns.
    let fixed = 1 + glyph.width() + 1 + count.width() + 1;
    let name = elide(&row.name, usize::from(width).saturating_sub(fixed).max(1));
    let gap = usize::from(width).saturating_sub(fixed + name.width());
    let label_style = if selected {
        Style::default()
            .fg(colors.text)
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(colors.text)
    };

    selection::row_with_glyphs(
        vec![Span::styled(glyph, Style::default().fg(colors.accent))],
        vec![
            Span::raw(" "),
            Span::styled(name, label_style),
            Span::raw(" ".repeat(gap)),
            Span::styled(count, Style::default().fg(count_colour)),
            Span::raw(" "),
        ],
        selected,
        colors,
        width,
    )
}

/// The detail for `profile`. `resolved` is the profile resolved against the packages that
/// exist: a resolution error becomes the headline, in `err`.
pub fn profile_detail(
    ctx: &AppContext,
    profile: &Profile,
    resolved: &andre_core::Result<ResolvedProfile>,
) -> Detail {
    let colors = ThemeColors::from_theme(ctx.core.theme);
    let icons = Icons::for_set(ctx.core.icons);
    let includes = if profile.includes.is_empty() {
        "none".to_string()
    } else {
        profile.includes.join(", ")
    };

    let resolved = match resolved {
        Ok(resolved) => resolved,
        Err(err) => {
            return Detail {
                headline: vec![Line::styled(
                    err.to_string(),
                    Style::default().fg(colors.err),
                )],
                facts: vec![fact("includes", includes)],
                sections: Vec::new(),
            };
        }
    };

    let headline = match profile.description.as_deref().map(str::trim) {
        Some(text) if !text.is_empty() => {
            Line::styled(text.to_string(), Style::default().fg(colors.text))
        }
        _ => Line::styled(
            "no description".to_string(),
            Style::default().fg(colors.text_dim),
        ),
    };
    Detail {
        headline: vec![headline],
        facts: vec![
            fact("includes", includes),
            fact("packages", packages_fact(resolved)),
            fact("status", status_summary(ctx, resolved, icons)),
        ],
        sections: resolved
            .entries
            .iter()
            .map(|entry| entry_line(ctx, entry, icons, &colors))
            .collect(),
    }
}

/// The detail when no profile is selected: what a profile is, and where they are saved.
pub fn empty_detail(ctx: &AppContext) -> Detail {
    let colors = ThemeColors::from_theme(ctx.core.theme);
    let text = |s: &str| Line::styled(s.to_string(), Style::default().fg(colors.text));
    Detail {
        headline: Vec::new(),
        facts: Vec::new(),
        sections: vec![
            text("A profile is a named set of packages, e.g. `personal` or `agent-work`."),
            Line::raw(""),
            text("Profiles can include other profiles."),
            Line::raw(""),
            Line::styled(
                format!(
                    "Saved in {}/.",
                    display_path(ctx.core.profiles.dir(), &ctx.home_dir)
                ),
                Style::default().fg(colors.text_dim),
            ),
        ],
    }
}

/// The profiles that include `name`, in name order. A profile with includers cannot be
/// deleted.
pub fn included_by(profiles: &ProfileSet, name: &str) -> Vec<String> {
    profiles
        .iter()
        .filter(|p| p.name != name && p.includes.iter().any(|inc| inc == name))
        .map(|p| p.name.clone())
        .collect()
}

/// The rows the sections of `detail` may take in `height` rows: what the headline and facts
/// leave, less the blank row before the sections.
fn section_room(detail: &Detail, height: usize) -> usize {
    let mut used = detail.headline.len();
    if !detail.facts.is_empty() {
        used += usize::from(used > 0) + detail.facts.len();
    }
    height.saturating_sub(used + usize::from(used > 0))
}

/// `detail` cut to `height` rows, laid out as `render_detail` lays it out. The sections start
/// `scroll` sections down the list, as the wheel leaves them. When the rest do not fit, the
/// last row says how many are left: `+ N more`.
pub fn fit(detail: &Detail, height: usize, scroll: usize, colors: &ThemeColors) -> Detail {
    let scroll = scroll.min(max_section_scroll(detail, height));
    let rest = &detail.sections[scroll..];
    let room = section_room(detail, height);
    let mut shown = detail.clone();
    if rest.len() <= room {
        shown.sections = rest.to_vec();
        return shown;
    }

    shown.sections.clear();
    if room > 0 {
        let kept = room - 1;
        shown.sections.extend(rest[..kept].iter().cloned());
        let more = rest.len() - kept;
        shown.sections.push(Line::styled(
            format!("+ {more} more"),
            Style::default().fg(colors.text_dim),
        ));
    }
    shown
}

/// The furthest `fit` lets the sections of `detail` scroll in `height` rows: the last section
/// on the last row.
pub fn max_section_scroll(detail: &Detail, height: usize) -> usize {
    detail
        .sections
        .len()
        .saturating_sub(section_room(detail, height))
}

/// Draws the dialog for `popup`, centred in `content`. Reads only what `popup` holds: the
/// name check was made when the name changed.
pub fn render_popup(frame: &mut Frame, content: Rect, colors: &ThemeColors, popup: &Popup) {
    let dialog = popup_dialog(popup, colors);
    let rect = dialog_rect(content, &dialog);
    render_dialog(frame, rect, colors, &dialog);
}

/// The dialog `popup` shows. Its rect and buttons are the ones [`render_popup`] paints.
pub fn popup_dialog(popup: &Popup, colors: &ThemeColors) -> Dialog {
    match popup {
        Popup::New(name) => name_dialog("new profile", "create", name, colors),
        Popup::Rename { name, .. } => name_dialog("rename profile", "rename", name, colors),
        Popup::Delete { name, included_by } => delete_dialog(name, included_by, colors),
    }
}

/// A dialog with one name field. The create or rename button appears only for a valid name.
/// The rule sits under the value, and a blank row ends the body above the buttons.
fn name_dialog(title: &str, verb: &'static str, name: &NameField, colors: &ThemeColors) -> Dialog {
    let indent = " ".repeat(format!("{NAME_LABEL}  ").width());
    let rule =
        |text: &str, colour| Line::styled(format!("{indent}{text}"), Style::default().fg(colour));
    let rule = match name.check {
        NameCheck::Exists => rule("already exists", colors.warn),
        NameCheck::Invalid => rule(NAME_RULE, colors.warn),
        NameCheck::Empty | NameCheck::Valid => rule(NAME_RULE, colors.text_dim),
    };
    let mut buttons = vec![button(KeyCode::Esc, "cancel", false)];
    if name.check == NameCheck::Valid {
        buttons.push(button(KeyCode::Enter, verb, true));
    }
    Dialog {
        title: title.to_string(),
        body: vec![
            field_line(NAME_LABEL, &name.field, true, colors),
            rule,
            Line::raw(""),
        ],
        buttons,
    }
}

/// The delete confirmation. When other profiles include this one, the dialog names them
/// and says their includes go with it. The delete button is always there.
fn delete_dialog(name: &str, included_by: &[String], colors: &ThemeColors) -> Dialog {
    let text = |s: String| Line::styled(s, Style::default().fg(colors.text));
    let mut body = vec![text(format!("Delete profile `{name}`?"))];
    if !included_by.is_empty() {
        body.push(text(elide(
            &format!("It is included by: {}.", included_by.join(", ")),
            52,
        )));
        body.push(text("Those includes are removed too.".to_string()));
    }
    body.push(Line::styled(
        "No files are unlinked.",
        Style::default().fg(colors.text_dim),
    ));
    body.push(Line::raw(""));
    Dialog {
        title: "delete profile".to_string(),
        body,
        buttons: vec![
            button(KeyCode::Char('n'), "keep", false),
            button(KeyCode::Char('y'), "delete", true),
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

fn fact(label: &'static str, value: String) -> (Cow<'static, str>, String) {
    (Cow::Borrowed(label), value)
}

/// `total`, and when some packages come via an include, the split: `5  (2 own, 3 via base)`.
fn packages_fact(resolved: &ResolvedProfile) -> String {
    let total = resolved.entries.len();
    let own = resolved
        .entries
        .iter()
        .filter(|e| e.origin == Origin::Own)
        .count();
    let via: BTreeSet<&str> = resolved
        .entries
        .iter()
        .filter_map(|e| match &e.origin {
            Origin::Included { include, .. } => Some(include.as_str()),
            Origin::Own => None,
        })
        .collect();
    if via.is_empty() {
        return total.to_string();
    }
    let names = via.into_iter().collect::<Vec<_>>().join(", ");
    format!("{total}  ({own} own, {} via {names})", total - own)
}

/// The stow status of a resolved package, from the cache. `None` when it has not been read.
fn entry_status(ctx: &AppContext, entry: &ResolvedEntry) -> Option<StowStatus> {
    ctx.status_cache.borrow().get(&entry.package)
}

/// The statuses of the packages that exist, counted: `✓ 3  → 1  ✗ 1`. Unread ones count as
/// unknown.
fn status_summary(ctx: &AppContext, resolved: &ResolvedProfile, icons: &Icons) -> String {
    const ORDER: [StowStatus; 7] = [
        StowStatus::Stowed,
        StowStatus::Unstowed,
        StowStatus::Partial,
        StowStatus::Conflict,
        StowStatus::Missing,
        StowStatus::Empty,
        StowStatus::Invalid,
    ];
    let statuses: Vec<Option<StowStatus>> = resolved
        .entries
        .iter()
        .filter(|e| !e.missing)
        .map(|e| entry_status(ctx, e))
        .collect();

    let mut parts: Vec<String> = ORDER
        .iter()
        .filter_map(|&status| {
            let n = statuses.iter().filter(|s| **s == Some(status)).count();
            (n > 0).then(|| format!("{} {n}", icons.status(status)))
        })
        .collect();
    let unknown = statuses.iter().filter(|s| s.is_none()).count();
    if unknown > 0 {
        parts.push(format!("{} {unknown}", icons.unknown));
    }
    if parts.is_empty() {
        "\u{2013}".to_string()
    } else {
        parts.join("  ")
    }
}

/// One resolved package in the detail: its status glyph, its name, then where it comes
/// from. A reference that names no package is `missing`.
fn entry_line(
    ctx: &AppContext,
    entry: &ResolvedEntry,
    icons: &Icons,
    colors: &ThemeColors,
) -> Line<'static> {
    let (glyph, colour) = if entry.missing {
        (icons.missing, colors.text_dim)
    } else {
        match entry_status(ctx, entry) {
            Some(status) => (icons.status(status), status_color(colors, status)),
            None => (icons.unknown, colors.text_dim),
        }
    };
    let mut spans = vec![
        Span::styled(glyph, Style::default().fg(colour)),
        Span::styled(
            format!(" {}", entry.package),
            Style::default().fg(colors.text),
        ),
    ];
    if let Origin::Included { include, .. } = &entry.origin {
        spans.push(Span::styled(
            format!("  via {include}"),
            Style::default().fg(colors.text_dim),
        ));
    }
    if entry.missing {
        spans.push(Span::styled("  missing", Style::default().fg(colors.warn)));
    }
    Line::from(spans)
}

/// Whether `name` is free for a profile, and why not.
pub fn name_check(name: &str, profiles: &ProfileSet, keep: Option<&str>) -> NameCheck {
    if name.is_empty() {
        NameCheck::Empty
    } else if !is_valid_name(name) {
        NameCheck::Invalid
    } else if Some(name) != keep
        && (profiles.get(name).is_some() || profiles.file_path(name).exists())
    {
        NameCheck::Exists
    } else {
        NameCheck::Valid
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_fit_keeps_everything_that_fits() {
        let colors = ThemeColors::from_theme(andre_core::Theme::Default);
        let detail = Detail {
            headline: vec![Line::raw("head")],
            facts: vec![fact("packages", "2".to_string())],
            sections: vec![Line::raw("a"), Line::raw("b")],
        };
        // headline, blank, one fact, blank, two sections: seven rows.
        assert_eq!(fit(&detail, 7, 0, &colors), detail);
    }

    #[test]
    fn test_fit_ends_the_sections_with_a_count_when_they_overflow() {
        let colors = ThemeColors::from_theme(andre_core::Theme::Default);
        let detail = Detail {
            headline: vec![Line::raw("head")],
            facts: Vec::new(),
            sections: (0..6).map(|i| Line::raw(format!("pkg{i}"))).collect(),
        };
        // headline and blank take two rows, so four rows are left: three packages and `+ 3 more`.
        let shown = fit(&detail, 6, 0, &colors);
        let text: Vec<String> = shown.sections.iter().map(line_text).collect();
        assert_eq!(text, ["pkg0", "pkg1", "pkg2", "+ 3 more"]);
    }

    #[test]
    fn test_fit_starts_the_sections_at_the_scroll_and_counts_what_is_left() {
        let colors = ThemeColors::from_theme(andre_core::Theme::Default);
        let detail = Detail {
            headline: vec![Line::raw("head")],
            facts: Vec::new(),
            sections: (0..6).map(|i| Line::raw(format!("pkg{i}"))).collect(),
        };
        let text = |scroll| -> Vec<String> {
            fit(&detail, 6, scroll, &colors)
                .sections
                .iter()
                .map(line_text)
                .collect()
        };
        assert_eq!(text(1), ["pkg1", "pkg2", "pkg3", "+ 2 more"]);
        // The last sections fill the four rows, so nothing is left to count.
        assert_eq!(text(2), ["pkg2", "pkg3", "pkg4", "pkg5"]);
        assert_eq!(
            text(99),
            ["pkg2", "pkg3", "pkg4", "pkg5"],
            "past the end clamps"
        );
    }

    #[test]
    fn test_max_section_scroll_puts_the_last_section_on_the_last_row() {
        let detail = Detail {
            headline: vec![Line::raw("head")],
            facts: Vec::new(),
            sections: (0..6).map(|i| Line::raw(format!("pkg{i}"))).collect(),
        };
        assert_eq!(max_section_scroll(&detail, 6), 2);
        assert_eq!(max_section_scroll(&detail, 20), 0, "everything fits");
    }

    #[test]
    fn test_fit_drops_the_sections_when_no_row_is_left() {
        let colors = ThemeColors::from_theme(andre_core::Theme::Default);
        let detail = Detail {
            headline: vec![Line::raw("head")],
            facts: Vec::new(),
            sections: vec![Line::raw("a")],
        };
        assert!(fit(&detail, 1, 0, &colors).sections.is_empty());
    }

    #[test]
    fn test_packages_fact_splits_own_from_included() {
        let entry = |package: &str, origin| ResolvedEntry {
            package: package.to_string(),
            origin,
            missing: false,
        };
        let via = |include: &str| Origin::Included {
            include: include.to_string(),
            declared_in: include.to_string(),
        };
        let resolved = ResolvedProfile {
            name: "agent".to_string(),
            entries: vec![
                entry("work/aws", Origin::Own),
                entry("home/git", via("base")),
                entry("home/shell", via("base")),
            ],
        };
        assert_eq!(packages_fact(&resolved), "3  (1 own, 2 via base)");

        let own_only = ResolvedProfile {
            name: "solo".to_string(),
            entries: vec![entry("home/git", Origin::Own)],
        };
        assert_eq!(packages_fact(&own_only), "1");
    }

    #[test]
    fn test_name_check_follows_the_rule_and_existing_files() {
        let dir = tempfile::TempDir::new().unwrap();
        let profiles = ProfileSet::load(&dir.path().join("profiles")).unwrap();
        assert_eq!(name_check("", &profiles, None), NameCheck::Empty);
        assert_eq!(name_check("Work", &profiles, None), NameCheck::Invalid);
        assert_eq!(name_check("laptop", &profiles, None), NameCheck::Valid);
    }

    fn line_text(line: &Line<'_>) -> String {
        line.spans.iter().map(|s| s.content.as_ref()).collect()
    }
}
