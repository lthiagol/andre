//! The help topics: the names in the list on the left of the help screen, and the text each
//! one shows on the right.

use std::borrow::Cow;

use andre_core::StowStatus;
use ratatui::{
    style::{Color, Modifier, Style},
    text::{Line, Span},
};
use unicode_width::UnicodeWidthStr;

use crate::components::AppContext;
use crate::theme::ThemeColors;
use crate::ui::detail::Detail;
use crate::ui::display_path;
use crate::ui::icons::Icons;
use crate::ui::status_style::{status_color, status_meaning};

/// Columns the key column takes in the keys topic, after its one-column indent.
const KEY_WIDTH: usize = 11;

/// Columns the glyph cell takes in the mark, radio and kind rows (`■ □` and `/ ~ •` are the
/// widest). Those rows pad their glyphs to this width, so their meanings line up in one column.
const MARK_CELL: usize = 5;

/// Every stow status, in the order the glyph topic lists them.
const STATUSES: [StowStatus; 7] = [
    StowStatus::Stowed,
    StowStatus::Unstowed,
    StowStatus::Partial,
    StowStatus::Conflict,
    StowStatus::Missing,
    StowStatus::Empty,
    StowStatus::Invalid,
];

/// The keys every screen takes, and what each does.
const EVERYWHERE: [(&str, &str); 8] = [
    ("\u{2191}\u{2193} / jk", "move"),
    ("Enter", "open / run"),
    ("Space", "mark"),
    ("Esc", "back"),
    ("s", "cycle the action"),
    ("v d n a o", "stow options"),
    ("t", "theme"),
    ("q", "quit"),
];

/// The keys that work on lists.
const LISTS: [(&str, &str); 3] = [
    ("*", "mark / unmark all"),
    ("+ e r x", "new, edit, rename, delete"),
    ("w", "save edits"),
];

/// The help topics, in list order.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Topic {
    Keys,
    Mouse,
    StatusGlyphs,
    Profiles,
    Packages,
    NewPackage,
    IgnoreFiles,
    About,
}

impl Topic {
    pub const ALL: [Topic; 8] = [
        Topic::Keys,
        Topic::Mouse,
        Topic::StatusGlyphs,
        Topic::Profiles,
        Topic::Packages,
        Topic::NewPackage,
        Topic::IgnoreFiles,
        Topic::About,
    ];

    /// The name in the list, and the detail's title.
    pub fn label(self) -> &'static str {
        match self {
            Topic::Keys => "keys",
            Topic::Mouse => "mouse",
            Topic::StatusGlyphs => "status glyphs",
            Topic::Profiles => "profiles",
            Topic::Packages => "packages",
            Topic::NewPackage => "new package",
            Topic::IgnoreFiles => "ignore files",
            Topic::About => "about",
        }
    }

    /// The text the topic shows, read from the app's state.
    pub fn detail(self, ctx: &AppContext) -> Detail {
        let colors = ThemeColors::from_theme(ctx.core.theme);
        match self {
            Topic::Keys => keys(&colors),
            Topic::Mouse => Detail {
                headline: vec![prose(
                    "A click does what the key on that control does. Clicking a selected row opens it, as Enter does.",
                    &colors,
                )],
                sections: vec![
                    prose("The wheel moves a list one row, and a detail pane three rows.", &colors),
                    prose("Click a dialog or action-bar button to press it. Click a file tab or a filter chip to switch to it, and a finished step to go back to it.", &colors),
                    prose("Mouse capture is always on. Hold Shift to select text in the terminal.", &colors),
                ],
                ..Detail::default()
            },
            Topic::StatusGlyphs => status_glyphs(Icons::for_set(ctx.core.icons), &colors),
            Topic::Profiles => Detail {
                headline: vec![prose(
                    "A profile is a named set of packages, saved as profiles/<name>.yml in the config folder.",
                    &colors,
                )],
                sections: vec![
                    prose("Enter runs the current action on the whole profile.", &colors),
                    prose("+ new, e edit, r rename, x delete, c check status.", &colors),
                    prose(
                        "A profile can include other profiles. A cycle of includes is an error.",
                        &colors,
                    ),
                ],
                ..Detail::default()
            },
            Topic::Packages => Detail {
                headline: vec![prose(
                    "A package is a folder inside the packages folder. Its files mirror the target, so shell/.bashrc links to .bashrc in the target.",
                    &colors,
                )],
                sections: vec![
                    prose(
                        "Space marks a package. * marks them all, or unmarks them when all are marked.",
                        &colors,
                    ),
                    prose(
                        "Enter runs the action on the marks, or on the cursor's package when none are marked.",
                        &colors,
                    ),
                    prose("i edits the package's ignore files. p sets which profiles hold it.", &colors),
                ],
                ..Detail::default()
            },
            Topic::NewPackage => Detail {
                headline: vec![prose(
                    "Create a package from files that already sit in a target, or start it empty.",
                    &colors,
                )],
                sections: vec![
                    prose(
                        "Pick a group, then a name: a new package, or an existing one to add files to.",
                        &colors,
                    ),
                    prose(
                        "Live files are moved into the package and linked back, so the programs that use them see no change.",
                        &colors,
                    ),
                    prose(
                        "Then choose ignore files and profiles. The moves and links are reviewed before anything runs.",
                        &colors,
                    ),
                ],
                ..Detail::default()
            },
            Topic::IgnoreFiles => Detail {
                headline: vec![prose(
                    ".gitignore keeps files out of git. .stow-local-ignore keeps files from being linked.",
                    &colors,
                )],
                sections: vec![
                    prose(
                        ".stow-local-ignore takes one regex per line; # starts a comment. Paths are relative to the package root, and ^/ anchors a pattern to its top. When the file is present it replaces the global ignore list for that package.",
                        &colors,
                    ),
                    prose(".gitignore is never read for linking.", &colors),
                    prose(
                        "Ignore files are never linked. i opens the editor for a package: + adds a line, e edits it, x deletes it, w saves.",
                        &colors,
                    ),
                ],
                ..Detail::default()
            },
            Topic::About => Detail {
                headline: vec![prose(
                    "A terminal UI for dotfiles, on the GNU Stow model.",
                    &colors,
                )],
                facts: vec![
                    (
                        Cow::Borrowed("version"),
                        env!("CARGO_PKG_VERSION").to_string(),
                    ),
                    (
                        Cow::Borrowed("config"),
                        display_path(&ctx.config_path, &ctx.home_dir),
                    ),
                ],
                ..Detail::default()
            },
        }
    }
}

fn prose(text: &str, colors: &ThemeColors) -> Line<'static> {
    Line::styled(text.to_string(), Style::default().fg(colors.text))
}

fn heading(text: &str, colors: &ThemeColors) -> Line<'static> {
    Line::styled(
        text.to_string(),
        Style::default()
            .fg(colors.accent)
            .add_modifier(Modifier::BOLD),
    )
}

/// A key (in the key colour) in its column, one column in from the heading, then what it does.
fn key_line(keys: &str, what: &str, colors: &ThemeColors) -> Line<'static> {
    Line::from(vec![
        Span::styled(
            format!(" {:<width$}", keys, width = KEY_WIDTH),
            Style::default().fg(colors.key),
        ),
        Span::styled(what.to_string(), Style::default().fg(colors.text)),
    ])
}

/// One or more glyphs, each in its colour, padded to `cell` columns, then two columns and what
/// they mean.
fn glyph_line(
    glyphs: &[(&'static str, Color)],
    cell: usize,
    meaning: &str,
    colors: &ThemeColors,
) -> Line<'static> {
    let mut spans = Vec::with_capacity(glyphs.len() * 2 + 3);
    let mut used = 0;
    for (index, (glyph, color)) in glyphs.iter().enumerate() {
        if index > 0 {
            spans.push(Span::raw(" "));
            used += 1;
        }
        spans.push(Span::styled(*glyph, Style::default().fg(*color)));
        used += glyph.width();
    }
    spans.push(Span::raw(" ".repeat(cell.saturating_sub(used) + 2)));
    spans.push(Span::styled(
        meaning.to_string(),
        Style::default().fg(colors.text),
    ));
    Line::from(spans)
}

fn keys(colors: &ThemeColors) -> Detail {
    let mut sections = vec![heading("everywhere", colors)];
    sections.extend(
        EVERYWHERE
            .iter()
            .map(|(keys, what)| key_line(keys, what, colors)),
    );
    sections.push(Line::raw(""));
    sections.push(heading("lists", colors));
    sections.extend(
        LISTS
            .iter()
            .map(|(keys, what)| key_line(keys, what, colors)),
    );
    Detail {
        sections,
        ..Detail::default()
    }
}

/// Every status glyph in its colour, then the unknown glyph, then the marks and kinds the
/// lists draw.
fn status_glyphs(icons: &Icons, colors: &ThemeColors) -> Detail {
    let mut sections: Vec<Line<'static>> = STATUSES
        .iter()
        .map(|&status| {
            // No cell: padding these to the mark cell would wrap `conflict — a real file is in
            // place` in the 80x24 pane and push the rest of the legend out of view.
            glyph_line(
                &[(icons.status(status), status_color(colors, status))],
                0,
                status_meaning(status),
                colors,
            )
        })
        .collect();
    sections.push(glyph_line(
        &[(icons.unknown, colors.text_dim)],
        0,
        "unknown \u{2014} not scanned yet",
        colors,
    ));
    sections.push(Line::raw(""));
    sections.push(glyph_line(
        &[
            (icons.checkbox_on, colors.accent),
            (icons.checkbox_off, colors.text_dim),
        ],
        MARK_CELL,
        "marked, unmarked",
        colors,
    ));
    sections.push(glyph_line(
        &[
            (icons.radio_on, colors.accent),
            (icons.radio_off, colors.text_dim),
        ],
        MARK_CELL,
        "chosen, not chosen",
        colors,
    ));
    // The kinds are the file browser's own glyphs, the same in both icon sets.
    sections.push(glyph_line(
        &[
            ("/", colors.text_dim),
            ("~", colors.text_dim),
            ("\u{2022}", colors.text_dim),
        ],
        MARK_CELL,
        "folder, symlink, file",
        colors,
    ));
    Detail {
        sections,
        ..Detail::default()
    }
}
