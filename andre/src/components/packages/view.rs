//! What the packages browser shows beside and over its list: the detail of the cursor's
//! package, the empty state and the membership dialog. Everything here is built from the app
//! context, so `update` builds it and paint only draws it.

use std::borrow::Cow;
use std::ops::Range;
use std::path::Path;

use andre_core::ignore::global_ignore_in;
use andre_core::{file_links, FileLink, IgnoreFile, PackageEntry, StowStatus};
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
use crate::ui::status_style::{status_color, status_headline};
use crate::ui::{display_path, elide};

/// Most file names the package detail lists. The rest are counted.
const FILES_SHOWN: usize = 8;
/// Widest a file name may be, so it stays on one line in the detail pane at 80 columns.
const NAME_WIDTH: usize = 34;

/// The screen an empty list's hint opens when the user clicks it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Fix {
    /// `6 settings`: where the packages folder is set.
    Settings,
    /// `3 new package`: where the first package is made.
    NewPackage,
}

impl Fix {
    /// The key and label the empty state draws under its message.
    pub fn hint(self) -> (&'static str, &'static str) {
        match self {
            Fix::Settings => ("6", "settings"),
            Fix::NewPackage => ("3", "new package"),
        }
    }
}

/// What an empty list says, and the fix its hint opens.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Blank {
    pub message: String,
    pub fix: Fix,
}

/// The empty state of a list with no packages: no packages folder is set, or the folder that
/// is set holds none.
pub fn blank(ctx: &AppContext) -> Blank {
    match ctx.core.config.packages_dir(&ctx.config_dir, &ctx.home_dir) {
        Some(dir) => Blank {
            message: format!("no packages in {}", shown(&dir, ctx)),
            fix: Fix::NewPackage,
        },
        None => Blank {
            message: "no packages folder".to_string(),
            fix: Fix::Settings,
        },
    }
}

/// The list title: the packages, with the marked count once something is marked.
pub fn packages_title(marked: usize, total: usize) -> String {
    match marked {
        0 => format!("packages {total}"),
        _ => format!("packages {marked} of {total}"),
    }
}

/// The detail for one package: its status, its folder and where it links, the files it links
/// and the profiles that list it. Each file is drawn with its link's status.
pub fn package_detail(
    ctx: &AppContext,
    entry: &PackageEntry,
    status: Option<StowStatus>,
) -> Detail {
    let colors = ThemeColors::from_theme(ctx.core.theme);
    let icons = Icons::for_set(ctx.core.icons);
    let global = global_ignore_in(&ctx.home_dir);
    let links = file_links(
        &entry.source,
        &entry.target,
        &entry.folder_name(),
        &entry.ignores,
        Some(&global),
    )
    .unwrap_or_default();
    let linked = links
        .iter()
        .filter(|(_, link)| *link == FileLink::Linked)
        .count();

    let profiles: Vec<&str> = ctx
        .core
        .profiles
        .iter()
        .filter(|profile| profile.packages.contains(&entry.name))
        .map(|profile| profile.name.as_str())
        .collect();

    let files = match links.len() {
        0 => "0".to_string(),
        count => format!("{count}  ({linked} linked)"),
    };
    let facts = vec![
        fact("folder", shown(&entry.dir, ctx)),
        fact("links to", shown(&entry.target, ctx)),
        fact("files", files),
        fact(
            "ignore",
            format!(
                "{} .gitignore  {} stow",
                flag(IgnoreFile::GitIgnore.path_in(&entry.dir).is_file(), icons),
                flag(IgnoreFile::StowLocal.path_in(&entry.dir).is_file(), icons),
            ),
        ),
        fact(
            "profiles",
            match profiles.is_empty() {
                true => "none".to_string(),
                false => profiles.join(", "),
            },
        ),
    ];

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

    Detail {
        headline: vec![status_headline(status, icons, &colors)],
        facts,
        sections,
    }
}

/// One file of a package: its link as a status glyph, then its name. A file that is not
/// linked shows the unlinked glyph; a real file or a wrong link shows the conflict glyph.
fn file_line(file: &Path, link: FileLink, icons: &Icons, colors: &ThemeColors) -> Line<'static> {
    let status = match link {
        FileLink::Linked => StowStatus::Stowed,
        FileLink::Unlinked => StowStatus::Unstowed,
        FileLink::RealFile | FileLink::WrongLink => StowStatus::Conflict,
    };
    let name = elide(&file.display().to_string(), NAME_WIDTH);
    Line::from(vec![
        Span::styled(
            icons.status(status),
            Style::default().fg(status_color(colors, status)),
        ),
        Span::styled(format!(" {name}"), Style::default().fg(colors.text)),
    ])
}

/// The detail when the list is empty. With no packages folder set, it says so first; either
/// way it explains what a package is.
pub fn empty_detail(ctx: &AppContext) -> Detail {
    let colors = ThemeColors::from_theme(ctx.core.theme);
    let text = |s: &'static str| Line::styled(s, Style::default().fg(colors.text));
    let explain = [
        text("A package is a folder inside the packages folder. Its files mirror your home folder: shell/.bashrc links to ~/.bashrc."),
        Line::raw(""),
    ];

    match ctx.core.config.packages {
        None => Detail {
            headline: vec![Line::from(vec![
                Span::styled("! ", Style::default().fg(colors.warn)),
                Span::styled(
                    "andre.yml sets no `packages:`",
                    Style::default().fg(colors.text),
                ),
            ])],
            facts: Vec::new(),
            sections: [
                explain.to_vec(),
                vec![text(
                    "Set the folder in settings, or add `packages: ~/dotfiles` to andre.yml.",
                )],
            ]
            .concat(),
        },
        Some(_) => Detail {
            headline: Vec::new(),
            facts: Vec::new(),
            sections: [
                explain.to_vec(),
                vec![text(
                    "Make one with new package, or add a folder to the packages folder.",
                )],
            ]
            .concat(),
        },
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
fn shown(path: &Path, ctx: &AppContext) -> String {
    display_path(path, &ctx.home_dir)
}

#[cfg(test)]
mod tests {
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
    fn test_blank_names_the_fix_for_each_empty_state() {
        let mut ctx = AppContext::for_tests(
            andre_core::config::Config::default_empty(),
            Path::new("/home/u/.config/andre"),
        );
        assert_eq!(
            blank(&ctx),
            Blank {
                message: "no packages folder".to_string(),
                fix: Fix::Settings,
            }
        );
        assert_eq!(Fix::Settings.hint(), ("6", "settings"));

        ctx.core
            .config
            .set_packages(Some("/home/u/dotfiles".into()));
        let empty = blank(&ctx);
        assert_eq!(empty.fix, Fix::NewPackage);
        assert_eq!(empty.message, "no packages in /home/u/dotfiles");
        assert_eq!(Fix::NewPackage.hint(), ("3", "new package"));
    }

    #[test]
    fn test_packages_hints_name_the_run_and_offer_mark_all_on_a_package_only() {
        let keys =
            |hints: &[Hint]| -> Vec<String> { hints.iter().map(|h| h.key.to_string()).collect() };
        let on_package = packages_hints(Some("stow 2".into()), true);
        assert_eq!(keys(&on_package), ["Enter", "Space", "*", "i", "p", "Esc"]);
        assert_eq!(on_package[0].label, "stow 2");

        let empty = packages_hints(None, false);
        assert_eq!(keys(&empty), ["Esc"]);
    }
}
