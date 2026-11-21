//! What the packages browser shows beside and over its list: the detail of the cursor's row
//! and the membership dialog. Everything here is built from the app context, so `update`
//! builds it and paint only draws it.

use std::borrow::Cow;
use std::ops::Range;

use andre_core::stow::linkable_files;
use andre_core::{IgnoreFile, StowStatus};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::{
    layout::Rect,
    style::Style,
    text::{Line, Span},
    Frame,
};
use unicode_width::UnicodeWidthStr;

use super::Membership;
use crate::components::AppContext;
use crate::theme::ThemeColors;
use crate::ui::detail::{Detail, VALUE_WIDTH};
use crate::ui::dialog::{dialog_rect, render_dialog, Button, Dialog};
use crate::ui::icons::Icons;
use crate::ui::selection;
use crate::ui::status_style::{status_headline, SUMMARY_ORDER};
use crate::ui::{display_path, elide, plural};

/// Most file names the package detail lists. The rest are counted.
const FILES_SHOWN: usize = 8;
/// Widest a file name may be, so it stays on one line in the detail pane at 80 columns.
const NAME_WIDTH: usize = 34;

/// The list title: the packages, with the marked count once something is marked.
pub fn packages_title(marked: usize, total: usize) -> String {
    match marked {
        0 => format!("packages {total}"),
        _ => format!("packages {marked} of {total}"),
    }
}

/// The detail for one package: its status, where it links, the files it links and the
/// profiles that list it.
pub fn package_detail(
    ctx: &AppContext,
    group: &str,
    package: &str,
    status: Option<StowStatus>,
) -> Detail {
    let colors = ThemeColors::from_theme(ctx.core.theme);
    let icons = Icons::for_set(ctx.core.icons);
    let config = &ctx.core.config;
    let source = config.get_group_source(group, &ctx.config_dir, &ctx.home_dir);
    let target = config.get_group_target(group, &ctx.config_dir, &ctx.home_dir);
    let ignores = config
        .groups
        .get(group)
        .and_then(|g| g.ignore.clone())
        .unwrap_or_default();
    let global = andre_core::ignore::global_ignore_in(&ctx.home_dir);
    let dir = source.as_ref().map(|source| source.join(package));

    let files: Vec<String> = dir
        .as_ref()
        .map(|dir| {
            linkable_files(dir, &ignores, Some(&global))
                .unwrap_or_default()
                .iter()
                .map(|file| {
                    let name = file.strip_prefix(dir).unwrap_or(file.as_path());
                    elide(&name.display().to_string(), NAME_WIDTH)
                })
                .collect()
        })
        .unwrap_or_default();

    let reference = format!("{group}/{package}");
    let profiles: Vec<&str> = ctx
        .core
        .profiles
        .iter()
        .filter(|profile| profile.packages.contains(&reference))
        .map(|profile| profile.name.as_str())
        .collect();

    let mut facts = vec![
        fact(
            "group",
            match &target {
                Some(target) => format!("{group}  \u{2192}  {}", shown(target, ctx)),
                None => format!("{group}  \u{2192}  not found"),
            },
        ),
        fact(
            "source",
            dir.as_ref()
                .map_or("not found".to_string(), |dir| shown(dir, ctx)),
        ),
        fact("files", files.len().to_string()),
    ];
    if let Some(dir) = &dir {
        facts.push(fact(
            "ignore",
            format!(
                "{} .gitignore  {} stow",
                flag(IgnoreFile::GitIgnore.path_in(dir).is_file(), icons),
                flag(IgnoreFile::StowLocal.path_in(dir).is_file(), icons),
            ),
        ));
    }
    facts.push(fact(
        "profiles",
        match profiles.is_empty() {
            true => "none".to_string(),
            false => profiles.join(", "),
        },
    ));

    let mut sections: Vec<Line<'static>> = files
        .iter()
        .take(FILES_SHOWN)
        .map(|file| Line::styled(file.clone(), Style::default().fg(colors.text)))
        .collect();
    if files.len() > FILES_SHOWN {
        sections.push(Line::styled(
            format!("+ {} more", files.len() - FILES_SHOWN),
            Style::default().fg(colors.text_dim),
        ));
    }

    Detail {
        headline: vec![status_headline(status, icons, &colors)],
        facts,
        sections,
    }
}

/// The detail for one group: its size, where it links, a summary of its packages' statuses
/// and how many are marked. `statuses` holds one entry per package, `None` when unknown.
pub fn group_detail(
    ctx: &AppContext,
    group: &str,
    statuses: &[Option<StowStatus>],
    marked: usize,
) -> Detail {
    let colors = ThemeColors::from_theme(ctx.core.theme);
    let icons = Icons::for_set(ctx.core.icons);
    let config = &ctx.core.config;
    let source = config.get_group_source(group, &ctx.config_dir, &ctx.home_dir);
    let target = config.get_group_target(group, &ctx.config_dir, &ctx.home_dir);
    let count = statuses.len();

    let headline = Line::styled(
        format!("Group of {count} {}.", plural(count, "package", "packages")),
        Style::default().fg(colors.text),
    );
    let mut sections = Vec::new();
    if count > 0 {
        sections.push(Line::styled(
            "Space marks every package in the group; again unmarks them.",
            Style::default().fg(colors.text_dim),
        ));
    }
    Detail {
        headline: vec![headline],
        facts: vec![
            fact(
                "source",
                source.map_or("not found".to_string(), |path| shown(&path, ctx)),
            ),
            fact(
                "target",
                target.map_or("not found".to_string(), |path| shown(&path, ctx)),
            ),
            fact("status", status_summary(statuses, icons)),
            fact("marked", format!("{marked} of {count}")),
        ],
        sections,
    }
}

/// The detail when there are no groups: what the browser lists, and where groups come from.
pub fn empty_detail(ctx: &AppContext) -> Detail {
    let colors = ThemeColors::from_theme(ctx.core.theme);
    Detail {
        headline: vec![Line::styled(
            "No groups yet.",
            Style::default().fg(colors.text),
        )],
        facts: Vec::new(),
        sections: vec![Line::styled(
            "Groups are set in andre.yml. Each one has a source folder of packages and a target folder they link into.",
            Style::default().fg(colors.text_dim),
        )],
    }
}

/// Lines the membership dialog takes besides its profile rows: the two borders, the blank
/// row under the top border, the blank row and `Space toggle` above the buttons, and the
/// button row.
const MEMBERSHIP_CHROME_ROWS: u16 = 6;

/// Draws the membership dialog for the package, centred in `area`. Its profile rows are
/// windowed to the room `area` leaves, so a long profile list never runs off the screen.
pub fn render_membership(
    frame: &mut Frame,
    area: Rect,
    colors: &ThemeColors,
    icons: &Icons,
    membership: &Membership,
) {
    let shown = visible_profiles(area.height, membership.profiles.len(), membership.cursor);
    let dialog = membership_dialog(membership, shown, colors, icons);
    render_dialog(frame, dialog_rect(area, &dialog), colors, &dialog);
}

/// The profile rows that fit in a dialog `height` rows tall. The window ends at the cursor
/// once the cursor is past the first screen, so the cursor row is always drawn.
pub fn visible_profiles(height: u16, len: usize, cursor: usize) -> Range<usize> {
    let rows = usize::from(height.saturating_sub(MEMBERSHIP_CHROME_ROWS)).max(1);
    if len <= rows {
        return 0..len;
    }
    let start = (cursor + 1).saturating_sub(rows).min(len - rows);
    start..start + rows
}

/// One checkbox row per profile in `shown`, the cursor's row filled. Save and cancel are the
/// buttons.
pub fn membership_dialog(
    membership: &Membership,
    shown: Range<usize>,
    colors: &ThemeColors,
    icons: &Icons,
) -> Dialog {
    let longest = membership
        .profiles
        .iter()
        .map(|row| row.name.width())
        .max()
        .unwrap_or(0);
    // The cursor's accent bar, the checkbox and a space come before the name.
    let width = (longest + 3) as u16;

    let mut body: Vec<Line<'static>> = membership
        .profiles
        .iter()
        .enumerate()
        .skip(shown.start)
        .take(shown.len())
        .map(|(i, row)| {
            let (box_colour, name_colour) = match row.now {
                true => (colors.accent, colors.text),
                false => (colors.text_dim, colors.text),
            };
            selection::row_with_glyphs(
                vec![Span::styled(
                    icons.checkbox(row.now),
                    Style::default().fg(box_colour),
                )],
                vec![
                    Span::raw(" "),
                    Span::styled(row.name.clone(), Style::default().fg(name_colour)),
                ],
                i == membership.cursor,
                colors,
                width,
            )
        })
        .collect();
    if membership.profiles.is_empty() {
        body.push(Line::styled(
            "No profiles yet. Make one on the profiles screen.",
            Style::default().fg(colors.text_dim),
        ));
        body.push(Line::raw(""));
    } else {
        body.push(Line::raw(""));
        body.push(Line::styled(
            "Space toggle",
            Style::default().fg(colors.text_dim),
        ));
    }

    Dialog {
        title: format!("{} in profiles", membership.package),
        body,
        buttons: vec![
            button(KeyCode::Esc, "cancel", false),
            button(KeyCode::Enter, "save", true),
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
    (Cow::Borrowed(label), elide(&value, VALUE_WIDTH))
}

/// `✓` or `✗` for a yes or no, in the active icon set.
fn flag(on: bool, icons: &Icons) -> &'static str {
    match on {
        true => icons.flag_on,
        false => icons.flag_off,
    }
}

/// A path with `~` for home, as the detail shows it.
fn shown(path: &std::path::Path, ctx: &AppContext) -> String {
    display_path(path, &ctx.home_dir)
}

/// Each status a group holds, with its glyph and count: `✓ 2  ◐ 1`. `—` when the group has
/// no packages, and unknown statuses are counted with `·`.
fn status_summary(statuses: &[Option<StowStatus>], icons: &Icons) -> String {
    let mut parts: Vec<String> = SUMMARY_ORDER
        .iter()
        .filter_map(|status| {
            let count = statuses.iter().filter(|s| **s == Some(*status)).count();
            (count > 0).then(|| format!("{} {count}", icons.status(*status)))
        })
        .collect();
    let unknown = statuses.iter().filter(|s| s.is_none()).count();
    if unknown > 0 {
        parts.push(format!("{} {unknown}", icons.unknown));
    }
    match parts.is_empty() {
        true => "\u{2014}".to_string(),
        false => parts.join("  "),
    }
}

#[cfg(test)]
mod tests {
    use andre_core::IconSet;

    use super::*;
    use crate::ui::footer::{packages_hints, Hint};
    use crate::ui::status_style::status_meaning;

    #[test]
    fn test_title_counts_packages_and_the_marks_once_something_is_marked() {
        assert_eq!(packages_title(0, 9), "packages 9");
        assert_eq!(packages_title(2, 9), "packages 2 of 9");
    }

    #[test]
    fn test_membership_window_keeps_the_cursor_row_inside_the_dialog() {
        // 12 rows leave six for profiles.
        assert_eq!(visible_profiles(12, 3, 2), 0..3, "short lists show all");
        assert_eq!(visible_profiles(12, 20, 0), 0..6);
        assert_eq!(visible_profiles(12, 20, 5), 0..6);
        assert_eq!(visible_profiles(12, 20, 6), 1..7);
        assert_eq!(
            visible_profiles(12, 20, 19),
            14..20,
            "the last row is reachable"
        );
        // A dialog with no room still shows one row.
        assert_eq!(visible_profiles(3, 5, 4), 4..5);
    }

    #[test]
    fn test_every_status_headline_fits_the_detail_pane_at_80_columns() {
        // The detail pane holds 37 columns at 80x24, and the glyph and a space come first.
        let all = [
            StowStatus::Stowed,
            StowStatus::Unstowed,
            StowStatus::Partial,
            StowStatus::Conflict,
            StowStatus::Missing,
            StowStatus::Empty,
            StowStatus::Invalid,
        ];
        for status in all {
            let width = status_meaning(status).width() + 2;
            assert!(width <= 37, "{status:?} headline is {width} columns");
        }
    }

    #[test]
    fn test_status_summary_counts_each_status_and_the_unknown_ones() {
        let icons = Icons::for_set(IconSet::Unicode);
        assert_eq!(status_summary(&[], icons), "\u{2014}");

        let statuses = [
            Some(StowStatus::Stowed),
            Some(StowStatus::Stowed),
            Some(StowStatus::Partial),
            Some(StowStatus::Unstowed),
            None,
        ];
        let expected = format!(
            "{} 2  {} 1  {} 1  {} 1",
            icons.status(StowStatus::Stowed),
            icons.status(StowStatus::Partial),
            icons.status(StowStatus::Unstowed),
            icons.unknown,
        );
        assert_eq!(status_summary(&statuses, icons), expected);
    }

    #[test]
    fn test_packages_hints_name_the_run_and_offer_p_and_i_only_on_a_package() {
        let keys =
            |hints: &[Hint]| -> Vec<String> { hints.iter().map(|h| h.key.to_string()).collect() };
        let on_package = packages_hints(Some("stow 2".into()), true, true);
        assert_eq!(
            keys(&on_package),
            ["Enter", "Space", "i", "p", "\u{2190}\u{2192}", "Esc"]
        );
        assert_eq!(on_package[0].label, "stow 2");

        let on_group = packages_hints(None, true, false);
        assert_eq!(keys(&on_group), ["Space", "\u{2190}\u{2192}", "Esc"]);

        let empty = packages_hints(None, false, false);
        assert_eq!(keys(&empty), ["Esc"]);
    }
}
