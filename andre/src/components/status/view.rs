//! What the status screen shows beside and over its list: the profile chip with the status
//! counts, the profile dropdown and the detail of the cursor's row. Everything here is built
//! from the app context or from the component's state, so `update` builds it and paint only
//! draws it.

use std::borrow::Cow;

use andre_core::{file_links, ignore::global_ignore_in, FileLink, StowStatus};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind};
use ratatui::{
    layout::Rect,
    style::{Color, Style},
    text::{Line, Span},
    widgets::{Clear, Paragraph},
    Frame,
};
use unicode_width::UnicodeWidthStr;

use super::FilterMenu;
use crate::components::AppContext;
use crate::theme::ThemeColors;
use crate::ui::chips::{chip_rects, chip_span, chip_width, Chip};
use crate::ui::detail::{Detail, VALUE_WIDTH};
use crate::ui::icons::Icons;
use crate::ui::selection;
use crate::ui::status_style::{status_color, status_headline, SUMMARY_ORDER};
use crate::ui::themed_panel;
use crate::ui::{display_path, elide};

/// The chip row's lead and the label before the profile chip. The dropdown lines up under the chip.
const PROFILE_LABEL: &str = " profile ";
/// Where the status counts start, in columns from the row's left edge, so they line up across
/// profiles. Column 28 of the 80x24 mockup's frame.
const COUNTS_COLUMN: usize = 26;
/// The fewest spaces between a chip too long to reach `COUNTS_COLUMN` and the counts.
const COUNTS_GAP: usize = 3;
/// The fix line a conflict's, a missing package's or a missing target's detail shows.
const CONFLICT_FIX: &str = "Fix: move the real file out of the way, then stow again.";
const MISSING_FIX: &str = "Fix: restore the package folder, or take it out of its profiles.";
const TARGET_FIX: &str = "Fix: create the group's target folder, or fix its path in andre.yml.";
/// Most files the package detail lists. Files that block a link come first; the rest are
/// counted. Five keeps the fix line on screen at 80x24.
const FILES_SHOWN: usize = 5;
/// Widest a file name may be in the detail, so each file's line fits the pane at 80 columns.
const FILE_NAME_WIDTH: usize = 14;

/// How many of the visible packages hold each status, and how many are not read yet.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct Summary {
    /// Every visible package, marked or not.
    pub total: usize,
    /// The statuses that occur, in `SUMMARY_ORDER`, with their counts.
    pub counts: Vec<(StowStatus, usize)>,
    pub unknown: usize,
}

/// Counts `statuses`, one entry per visible package, `None` when the status is unread.
pub fn summarize(statuses: &[Option<StowStatus>]) -> Summary {
    let counts = SUMMARY_ORDER
        .iter()
        .filter_map(|status| {
            let count = statuses.iter().filter(|s| **s == Some(*status)).count();
            (count > 0).then_some((*status, count))
        })
        .collect();
    Summary {
        total: statuses.len(),
        counts,
        unknown: statuses.iter().filter(|s| s.is_none()).count(),
    }
}

/// The list title: `packages 9` for every package, or the profile's name and its count, as
/// `agent-work 5`.
pub fn list_title(profile: Option<&str>, total: usize) -> String {
    match profile {
        Some(name) => format!("{name} {total}"),
        None => format!("packages {total}"),
    }
}

/// What the list says when no package is shown. Short, so it fits the list pane at 80 columns.
pub fn empty_message(profile: Option<&str>) -> &'static str {
    match profile {
        Some(_) => "No packages on disk.",
        None => "No packages in any group yet.",
    }
}

/// The chip row's status counts, each glyph in its colour: `✓ 6  ◐ 1  → 1  ✗ 1`. Unread
/// packages are counted last with `·`.
pub fn summary_spans(summary: &Summary, icons: &Icons, colors: &ThemeColors) -> Vec<Span<'static>> {
    let mut parts: Vec<(String, Color)> = summary
        .counts
        .iter()
        .map(|(status, count)| {
            (
                format!("{} {count}", icons.status(*status)),
                status_color(colors, *status),
            )
        })
        .collect();
    if summary.unknown > 0 {
        parts.push((
            format!("{} {}", icons.unknown, summary.unknown),
            colors.text_dim,
        ));
    }
    let mut spans = Vec::new();
    for (i, (text, colour)) in parts.into_iter().enumerate() {
        if i > 0 {
            spans.push(Span::raw("  "));
        }
        spans.push(Span::styled(text, Style::default().fg(colour)));
    }
    spans
}

/// The meta of a folded group's row: the worst status among its packages and how many have
/// it, as `✓ 2`. A group whose statuses are all unread shows `·` and its count.
pub fn folded_meta(statuses: &[Option<StowStatus>], icons: &Icons) -> String {
    match statuses
        .iter()
        .flatten()
        .copied()
        .max_by_key(|status| severity(*status))
    {
        Some(worst) => {
            let count = statuses.iter().filter(|s| **s == Some(worst)).count();
            format!("{} {count}", icons.status(worst))
        }
        None => format!("{} {}", icons.unknown, statuses.len()),
    }
}

/// How much a status needs attention, for picking a folded group's worst status. Higher is
/// worse.
fn severity(status: StowStatus) -> u8 {
    match status {
        StowStatus::Conflict => 6,
        StowStatus::Invalid => 5,
        StowStatus::Missing => 4,
        StowStatus::Partial => 3,
        StowStatus::Unstowed => 2,
        StowStatus::Empty => 1,
        StowStatus::Stowed => 0,
    }
}

/// The profile chip: the profile the rows show, with the fold glyph that says it opens a menu.
fn profile_chip(profile: Option<&str>, icons: &Icons) -> Chip {
    Chip {
        label: format!("{} {}", profile.unwrap_or("all"), icons.fold_open),
        active: true,
        key: KeyEvent::new(KeyCode::Char('f'), KeyModifiers::NONE),
    }
}

/// The rect of the profile chip on the chip row `chip_row`, where a click opens the dropdown.
pub fn profile_chip_rect(chip_row: Rect, profile: Option<&str>, icons: &Icons) -> Option<Rect> {
    let x = chip_row.x.saturating_add(PROFILE_LABEL.width() as u16);
    let area = Rect::new(x, chip_row.y, chip_row.right().saturating_sub(x), 1);
    chip_rects(area, &[profile_chip(profile, icons)])
        .first()
        .map(|(rect, _)| *rect)
}

/// Draws the chip row: ` profile [all ▾]`, then the status counts from `COUNTS_COLUMN` on.
/// A chip too long to leave that room pushes the counts `COUNTS_GAP` columns on.
pub fn render_chip_row(
    frame: &mut Frame,
    area: Rect,
    colors: &ThemeColors,
    icons: &Icons,
    profile: Option<&str>,
    summary: &Summary,
) {
    let chip = profile_chip(profile, icons);
    let chip_end = PROFILE_LABEL.width() + chip_width(&chip);
    let gap = COUNTS_COLUMN.saturating_sub(chip_end).max(COUNTS_GAP);
    let mut spans = vec![
        Span::styled(PROFILE_LABEL, Style::default().fg(colors.text_dim)),
        chip_span(&chip, colors),
        Span::raw(" ".repeat(gap)),
    ];
    spans.extend(summary_spans(summary, icons, colors));
    frame.render_widget(Paragraph::new(Line::from(spans)), area);
}

/// The dropdown's rect: under the profile chip, clipped to `area`.
pub fn filter_rect(area: Rect, chip_row: Rect, menu: &FilterMenu) -> Rect {
    let longest = menu
        .options
        .iter()
        .map(|o| option_label(o).width())
        .max()
        .unwrap_or(0);
    // The cursor bar (or a gap) comes before each name, and one column of air after it.
    let inner_width = longest + 2;
    let height = menu.options.len() as u16 + 2;
    let x = chip_row.x + PROFILE_LABEL.width() as u16;
    Rect::new(x, chip_row.y + 1, inner_width as u16 + 2, height).intersection(area)
}

/// The index of the dropdown option a left click at `ev` lands on, when the dropdown is at
/// `rect`. Its options are the rows inside its border.
pub fn option_at(rect: Rect, count: usize, ev: MouseEvent) -> Option<usize> {
    if ev.kind != MouseEventKind::Down(MouseButton::Left) {
        return None;
    }
    let inside = ev.column > rect.x
        && ev.column + 1 < rect.right()
        && ev.row > rect.y
        && ev.row + 1 < rect.bottom();
    let index = usize::from(ev.row.saturating_sub(rect.y + 1));
    (inside && index < count).then_some(index)
}

/// Draws the profile dropdown under the chip row's profile chip, clipped to `area`.
pub fn render_filter(
    frame: &mut Frame,
    area: Rect,
    chip_row: Rect,
    colors: &ThemeColors,
    menu: &FilterMenu,
) {
    let rect = filter_rect(area, chip_row, menu);
    frame.render_widget(Clear, rect);
    let panel = themed_panel(colors);
    let inner = panel.inner(rect);
    frame.render_widget(panel, rect);
    let rows: Vec<Line> = menu
        .options
        .iter()
        .enumerate()
        .map(|(i, option)| {
            selection::row(
                vec![Span::styled(
                    option_label(option).to_string(),
                    Style::default().fg(colors.text),
                )],
                i == menu.cursor,
                colors,
                inner.width,
            )
        })
        .collect();
    frame.render_widget(Paragraph::new(rows), inner);
}

/// The dropdown's name for a choice: every package, or a profile's name.
fn option_label(option: &Option<String>) -> &str {
    option.as_deref().unwrap_or("all packages")
}

/// The detail for one package: its status, the group's target, how each file of the package
/// sits in the target (files that block a link first) and the fix for what is wrong.
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
    let global = global_ignore_in(&ctx.home_dir);

    let mut links = match (&source, &target) {
        (Some(source), Some(target)) => {
            file_links(source, target, package, &ignores, Some(&global)).unwrap_or_default()
        }
        _ => Vec::new(),
    };
    links.sort_by_key(|(_, link)| attention(*link));

    let fix = match (status, &target) {
        (_, None) => Some(TARGET_FIX),
        (Some(StowStatus::Conflict), _) => Some(CONFLICT_FIX),
        (Some(StowStatus::Missing), _) => Some(MISSING_FIX),
        _ => None,
    };

    let mut sections: Vec<Line<'static>> = links
        .iter()
        .take(FILES_SHOWN)
        .map(|(file, link)| file_line(file, *link, icons, &colors))
        .collect();
    if links.len() > FILES_SHOWN {
        sections.push(Line::styled(
            format!("+ {} more", links.len() - FILES_SHOWN),
            Style::default().fg(colors.text_dim),
        ));
    }
    if let Some(text) = fix {
        if !sections.is_empty() {
            sections.push(Line::raw(""));
        }
        sections.push(Line::styled(text, Style::default().fg(colors.text_dim)));
    }

    Detail {
        headline: vec![status_headline(status, icons, &colors)],
        facts: vec![fact(
            "target",
            target.map_or("not found".to_string(), |path| shown(&path, ctx)),
        )],
        sections,
    }
}

/// One file of a package, as its status glyph, its name and what the link is: `✗ andre.yml
/// real file`.
fn file_line(
    file: &std::path::Path,
    link: FileLink,
    icons: &Icons,
    colors: &ThemeColors,
) -> Line<'static> {
    let (status, word) = match link {
        FileLink::Linked => (StowStatus::Stowed, "linked"),
        FileLink::Unlinked => (StowStatus::Unstowed, "not linked"),
        FileLink::RealFile => (StowStatus::Conflict, "real file"),
        FileLink::WrongLink => (StowStatus::Conflict, "wrong link"),
    };
    let name = elide(&file.display().to_string(), FILE_NAME_WIDTH);
    let pad = " ".repeat(FILE_NAME_WIDTH.saturating_sub(name.width()));
    Line::from(vec![
        Span::styled(
            icons.status(status),
            Style::default().fg(status_color(colors, status)),
        ),
        Span::styled(
            format!(" {name}{pad}  {word}"),
            Style::default().fg(colors.text),
        ),
    ])
}

/// The order files are listed in: what blocks a link, then what is unlinked, then what is
/// linked. Lower comes first.
fn attention(link: FileLink) -> u8 {
    match link {
        FileLink::RealFile | FileLink::WrongLink => 0,
        FileLink::Unlinked => 1,
        FileLink::Linked => 2,
    }
}

/// The detail for one group: where it links, and a summary of its packages' statuses.
pub fn group_detail(ctx: &AppContext, group: &str, statuses: &[Option<StowStatus>]) -> Detail {
    let colors = ThemeColors::from_theme(ctx.core.theme);
    let icons = Icons::for_set(ctx.core.icons);
    let target = ctx
        .core
        .config
        .get_group_target(group, &ctx.config_dir, &ctx.home_dir);

    Detail {
        headline: vec![Line::from(summary_spans(
            &summarize(statuses),
            icons,
            &colors,
        ))],
        facts: vec![
            fact(
                "target",
                target.map_or("not found".to_string(), |path| shown(&path, ctx)),
            ),
            fact("packages", statuses.len().to_string()),
        ],
        sections: Vec::new(),
    }
}

/// The key hint under the empty list's message. A filtered profile with no packages on disk
/// is one `f` away from every package.
pub fn empty_hint(profile: Option<&str>) -> Option<(&'static str, &'static str)> {
    profile.map(|_| ("f", "show every package"))
}

/// The detail when no package is shown: why, and what to do about it.
pub fn empty_detail(ctx: &AppContext, profile: Option<&str>) -> Detail {
    let colors = ThemeColors::from_theme(ctx.core.theme);
    let (headline, hint) = match profile {
        Some(name) => (
            format!("Profile {name} has no packages on disk."),
            "The rows are this profile's packages that are on disk.",
        ),
        None => (
            "No packages found in any configured group.".to_string(),
            "Groups come from andre.yml. Each one has a source folder of packages.",
        ),
    };
    Detail {
        headline: vec![Line::styled(headline, Style::default().fg(colors.text))],
        facts: Vec::new(),
        sections: vec![Line::styled(hint, Style::default().fg(colors.text_dim))],
    }
}

fn fact(label: &'static str, value: String) -> (Cow<'static, str>, String) {
    (Cow::Borrowed(label), elide(&value, VALUE_WIDTH))
}

/// A path with `~` for home, as the detail shows it.
fn shown(path: &std::path::Path, ctx: &AppContext) -> String {
    display_path(path, &ctx.home_dir)
}

#[cfg(test)]
mod tests {
    use andre_core::IconSet;

    use crossterm::event::{MouseButton, MouseEventKind};

    use super::*;

    #[test]
    fn test_list_title_names_the_profile_once_one_is_filtered_to() {
        assert_eq!(list_title(None, 9), "packages 9");
        assert_eq!(list_title(Some("agent-work"), 5), "agent-work 5");
    }

    #[test]
    fn test_summarize_counts_each_status_in_summary_order_and_the_unread_ones() {
        let summary = summarize(&[
            Some(StowStatus::Conflict),
            Some(StowStatus::Stowed),
            None,
            Some(StowStatus::Stowed),
        ]);
        assert_eq!(summary.total, 4);
        assert_eq!(
            summary.counts,
            vec![(StowStatus::Stowed, 2), (StowStatus::Conflict, 1)]
        );
        assert_eq!(summary.unknown, 1);
    }

    #[test]
    fn test_folded_meta_is_the_worst_status_and_how_many_have_it() {
        let icons = Icons::for_set(IconSet::Unicode);
        let stowed = Some(StowStatus::Stowed);
        assert_eq!(
            folded_meta(&[stowed, stowed], icons),
            format!("{} 2", icons.status(StowStatus::Stowed))
        );
        assert_eq!(
            folded_meta(&[stowed, Some(StowStatus::Partial), stowed], icons),
            format!("{} 1", icons.status(StowStatus::Partial)),
            "partial outranks stowed"
        );
        assert_eq!(
            folded_meta(
                &[Some(StowStatus::Partial), Some(StowStatus::Conflict)],
                icons
            ),
            format!("{} 1", icons.status(StowStatus::Conflict)),
            "conflict outranks partial"
        );
        assert_eq!(
            folded_meta(&[None, None], icons),
            format!("{} 2", icons.unknown),
            "unread statuses only"
        );
    }

    #[test]
    fn test_summary_spans_lead_with_the_linked_count_and_end_with_unread() {
        let icons = Icons::for_set(IconSet::Unicode);
        let colors = ThemeColors::from_theme(andre_core::Theme::Default);
        let summary = summarize(&[Some(StowStatus::Stowed), None]);
        let text: String = summary_spans(&summary, icons, &colors)
            .iter()
            .map(|span| span.content.as_ref())
            .collect();
        assert_eq!(
            text,
            format!(
                "{} 1  {} 1",
                icons.status(StowStatus::Stowed),
                icons.unknown
            )
        );
    }

    #[test]
    fn test_empty_message_fits_the_list_pane_at_80_columns() {
        // The list pane's inner width at 80 columns.
        for profile in [None, Some("a-very-long-profile-name")] {
            let message = empty_message(profile);
            assert!(message.width() <= 32, "{message:?} is too wide");
        }
    }

    #[test]
    fn test_option_labels_put_every_package_first_as_the_name_all() {
        assert_eq!(option_label(&None), "all packages");
        assert_eq!(option_label(&Some("work".into())), "work");
    }

    #[test]
    fn test_option_at_picks_the_row_inside_the_dropdown_border() {
        let rect = Rect::new(9, 1, 12, 4);
        let press = |x: u16, y: u16| MouseEvent {
            kind: MouseEventKind::Down(MouseButton::Left),
            column: x,
            row: y,
            modifiers: KeyModifiers::NONE,
        };
        assert_eq!(
            option_at(rect, 2, press(10, 2)),
            Some(0),
            "first option row"
        );
        assert_eq!(
            option_at(rect, 2, press(10, 3)),
            Some(1),
            "second option row"
        );
        assert_eq!(
            option_at(rect, 2, press(10, 4)),
            None,
            "past the last option"
        );
        assert_eq!(option_at(rect, 2, press(9, 2)), None, "on the left border");
        assert_eq!(option_at(rect, 2, press(10, 1)), None, "on the top border");
        assert_eq!(option_at(rect, 2, press(30, 2)), None, "outside");
    }
}
