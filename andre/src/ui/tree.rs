//! The grouped package tree shared by the packages browser, profile edit, status and review.
//! A group row has a fold glyph, a tri-state mark and a `marked/total` count; its packages
//! sit under it with a mark, a status glyph and the name. [`Tree`] owns the groups, the
//! folds and the marks and holds no terminal state. [`Tree::rows`] flattens it for a list
//! and [`tree_line`] paints one row.

use std::collections::HashSet;

use ratatui::{
    style::{Modifier, Style},
    text::{Line, Span},
};
use unicode_width::UnicodeWidthStr;

use andre_core::StowStatus;

use crate::theme::ThemeColors;
use crate::ui::elide;
use crate::ui::icons::Icons;
use crate::ui::selection;
use crate::ui::status_style::{status_color, status_word, STATUS_WORD_WIDTH};

/// The narrowest list pane that shows the status word column (§1.8).
const STATUS_WORD_MIN_PANE_WIDTH: u16 = 44;
/// The same threshold for `tree_line`'s `width`, which is the inner row (pane minus the
/// two border columns).
const STATUS_WORD_MIN_WIDTH: u16 = STATUS_WORD_MIN_PANE_WIDTH - 2;

/// A checkbox state. A package carries `On`, `Off` or `Inherited`. A group header takes
/// `On` when all its packages count as marked, `Off` when none do, and `Some` otherwise.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Mark {
    On,
    Off,
    Some,
    /// Marked by an included profile. It cannot be toggled here; the name is that profile.
    Inherited(String),
}

/// One row of the flattened tree, as [`tree_line`] draws it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TreeRow {
    Group {
        name: String,
        folded: bool,
        /// `None` when no package in the group has a checkbox.
        mark: Option<Mark>,
        /// `marked/total`, or empty when the group has no checkboxes.
        meta: String,
    },
    Package {
        group: String,
        name: String,
        /// `None` until the status is known; drawn as `·`.
        status: Option<StowStatus>,
        /// `None` when the list shows no checkboxes.
        mark: Option<Mark>,
        /// Right-hand text. An inherited package shows `via <profile>` instead.
        meta: String,
    },
}

/// A group and its packages, in config order.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TreeGroup {
    pub name: String,
    pub packages: Vec<TreePackage>,
}

/// One package. Only `On` and `Off` packages can be toggled; `Inherited` cannot, and a
/// `None` mark means the list has no checkbox at all.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TreePackage {
    pub name: String,
    pub status: Option<StowStatus>,
    pub mark: Option<Mark>,
    pub meta: String,
}

/// What [`Tree::toggle_mark`] did.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Toggle {
    Changed,
    /// The row is marked by the profile `via`. Nothing changed; the caller says why.
    Inherited {
        via: String,
    },
    /// The row has no checkbox, or a group has no packages of its own to mark.
    Unmarkable,
}

/// Groups, folds and marks. Folds are per group name; marks live on the packages.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Tree {
    groups: Vec<TreeGroup>,
    folded: HashSet<String>,
}

/// A row's place in the tree, before flattening.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum At {
    Group(usize),
    Package(usize, usize),
}

impl Tree {
    pub fn new(groups: Vec<TreeGroup>) -> Self {
        Self {
            groups,
            folded: HashSet::new(),
        }
    }

    /// Replaces the groups and packages, keeping the folds of groups that still exist.
    /// Use it to refresh a list from new data without unfolding it.
    pub fn replace_groups(&mut self, groups: Vec<TreeGroup>) {
        self.folded
            .retain(|name| groups.iter().any(|group| &group.name == name));
        self.groups = groups;
    }

    /// Sets one package's status, found by group and package name. Folds and marks are
    /// untouched. Returns false when no such package exists.
    pub fn set_status(&mut self, group: &str, package: &str, status: Option<StowStatus>) -> bool {
        let found = self
            .groups
            .iter_mut()
            .find(|g| g.name == group)
            .and_then(|g| g.packages.iter_mut().find(|p| p.name == package));
        match found {
            Some(package) => {
                package.status = status;
                true
            }
            None => false,
        }
    }

    /// The visible rows in order: each group, then its packages unless it is folded.
    pub fn rows(&self) -> Vec<TreeRow> {
        self.places()
            .into_iter()
            .map(|at| match at {
                At::Group(g) => {
                    let group = &self.groups[g];
                    TreeRow::Group {
                        name: group.name.clone(),
                        folded: self.folded.contains(&group.name),
                        mark: group_mark(group),
                        meta: group_meta(group),
                    }
                }
                At::Package(g, p) => {
                    let group = &self.groups[g];
                    let package = &group.packages[p];
                    TreeRow::Package {
                        group: group.name.clone(),
                        name: package.name.clone(),
                        status: package.status,
                        mark: package.mark.clone(),
                        meta: package.meta.clone(),
                    }
                }
            })
            .collect()
    }

    /// Folds or unfolds the group of `row`. On a package row, that is its group.
    pub fn fold(&mut self, row: usize, folded: bool) {
        let Some(at) = self.at(row) else {
            return;
        };
        let g = match at {
            At::Group(g) | At::Package(g, _) => g,
        };
        let name = self.groups[g].name.clone();
        if folded {
            self.folded.insert(name);
        } else {
            self.folded.remove(&name);
        }
    }

    /// Toggles the mark on `row`. On a package row, that package flips. On a group row, the
    /// group's own packages all flip to the state its header does not show: marked unless
    /// every package already counts as marked. Inherited packages are left alone.
    pub fn toggle_mark(&mut self, row: usize) -> Toggle {
        let Some(at) = self.at(row) else {
            return Toggle::Unmarkable;
        };
        match at {
            At::Package(g, p) => {
                let package = &mut self.groups[g].packages[p];
                match &package.mark {
                    Some(Mark::On) => package.mark = Some(Mark::Off),
                    Some(Mark::Off) => package.mark = Some(Mark::On),
                    Some(Mark::Inherited(via)) => return Toggle::Inherited { via: via.clone() },
                    Some(Mark::Some) | None => return Toggle::Unmarkable,
                }
                Toggle::Changed
            }
            At::Group(g) => {
                let group = &mut self.groups[g];
                if !group.packages.iter().any(is_own_checkbox) {
                    return match group.packages.iter().find_map(inherited_via) {
                        Some(via) => Toggle::Inherited { via },
                        None => Toggle::Unmarkable,
                    };
                }
                let next = if group_mark(group) == Some(Mark::On) {
                    Mark::Off
                } else {
                    Mark::On
                };
                for package in group.packages.iter_mut().filter(|p| is_own_checkbox(p)) {
                    package.mark = Some(next.clone());
                }
                Toggle::Changed
            }
        }
    }

    /// The `(group, package)` pairs marked by the user, in config order. Inherited packages
    /// are not listed; folded groups are.
    pub fn marked(&self) -> Vec<(String, String)> {
        self.groups
            .iter()
            .flat_map(|group| {
                group
                    .packages
                    .iter()
                    .filter(|p| p.mark == Some(Mark::On))
                    .map(move |p| (group.name.clone(), p.name.clone()))
            })
            .collect()
    }

    fn at(&self, row: usize) -> Option<At> {
        self.places().get(row).copied()
    }

    fn places(&self) -> Vec<At> {
        let mut places = Vec::new();
        for (g, group) in self.groups.iter().enumerate() {
            places.push(At::Group(g));
            if !self.folded.contains(&group.name) {
                places.extend((0..group.packages.len()).map(|p| At::Package(g, p)));
            }
        }
        places
    }
}

/// A package whose mark the user can flip.
fn is_own_checkbox(package: &TreePackage) -> bool {
    matches!(package.mark, Some(Mark::On | Mark::Off))
}

fn inherited_via(package: &TreePackage) -> Option<String> {
    match &package.mark {
        Some(Mark::Inherited(via)) => Some(via.clone()),
        _ => None,
    }
}

/// How many of a group's checkboxed packages count as marked, and how many have one. An
/// inherited package counts as marked.
fn counts(group: &TreeGroup) -> (usize, usize) {
    let marks = group.packages.iter().filter_map(|p| p.mark.as_ref());
    marks.fold((0, 0), |(on, total), mark| {
        let counted = matches!(mark, Mark::On | Mark::Inherited(_));
        (on + usize::from(counted), total + 1)
    })
}

fn group_mark(group: &TreeGroup) -> Option<Mark> {
    match counts(group) {
        (_, 0) => None,
        (on, total) if on == total => Some(Mark::On),
        (0, _) => Some(Mark::Off),
        _ => Some(Mark::Some),
    }
}

fn group_meta(group: &TreeGroup) -> String {
    match counts(group) {
        (_, 0) => String::new(),
        (on, total) => format!("{on}/{total}"),
    }
}

/// The checkbox glyph. A row without a checkbox keeps its column blank, so names line up.
fn mark_span(mark: Option<&Mark>, icons: &Icons, colors: &ThemeColors) -> Span<'static> {
    let (glyph, colour) = match mark {
        Some(Mark::On) => (icons.checkbox_on, colors.accent),
        Some(Mark::Some) => (icons.checkbox_some, colors.accent),
        Some(Mark::Off) => (icons.checkbox_off, colors.text_dim),
        Some(Mark::Inherited(_)) => (icons.checkbox_on, colors.text_dim),
        None => return Span::raw(" "),
    };
    Span::styled(glyph, Style::default().fg(colour))
}

/// Paints one row `width` columns wide, with its checkbox column. Glyphs keep their colour
/// on the cursor row when they can (see `ui::selection`); the name is cut with `…` before
/// the right-hand text. Package rows at `STATUS_WORD_MIN_WIDTH` (a 44-col pane) or wider
/// also show the status word before the meta. The right-hand text ends one column before
/// the border.
pub fn tree_line(
    row: &TreeRow,
    selected: bool,
    icons: &Icons,
    colors: &ThemeColors,
    width: u16,
) -> Line<'static> {
    tree_row(row, selected, icons, colors, width, true)
}

/// `tree_line` for a list with no checkboxes at all, such as the review. It has no checkbox
/// column, so its names sit where a checkbox list's names would start, one column less.
pub fn tree_line_without_checkboxes(
    row: &TreeRow,
    selected: bool,
    icons: &Icons,
    colors: &ThemeColors,
    width: u16,
) -> Line<'static> {
    tree_row(row, selected, icons, colors, width, false)
}

fn tree_row(
    row: &TreeRow,
    selected: bool,
    icons: &Icons,
    colors: &ThemeColors,
    width: u16,
    checkboxes: bool,
) -> Line<'static> {
    let dim = Style::default().fg(colors.text_dim);
    // The checkbox and the gap after it, when the list has checkboxes.
    let box_cell = |mark: Option<&Mark>| -> Vec<Span<'static>> {
        if checkboxes {
            vec![mark_span(mark, icons, colors), Span::raw(" ")]
        } else {
            Vec::new()
        }
    };
    let (glyphs, name, name_style, word, meta, meta_style) = match row {
        TreeRow::Group {
            name,
            folded,
            mark,
            meta,
        } => {
            let fold = if *folded {
                icons.fold_closed
            } else {
                icons.fold_open
            };
            let mut glyphs = vec![Span::styled(fold, dim), Span::raw(" ")];
            glyphs.extend(box_cell(mark.as_ref()));
            let name_style = Style::default()
                .fg(colors.text)
                .add_modifier(Modifier::BOLD);
            let meta_style = Style::default().fg(colors.value);
            (
                glyphs,
                name.clone(),
                name_style,
                None,
                meta.clone(),
                meta_style,
            )
        }
        TreeRow::Package {
            name,
            status,
            mark,
            meta,
            ..
        } => {
            let (status_glyph, status_style) = match status {
                Some(status) => (
                    icons.status(*status),
                    Style::default().fg(status_color(colors, *status)),
                ),
                None => (icons.unknown, dim),
            };
            let mut glyphs = vec![Span::raw("    ")];
            glyphs.extend(box_cell(mark.as_ref()));
            glyphs.push(Span::styled(status_glyph, status_style));
            glyphs.push(Span::raw(" "));
            let meta = match mark {
                Some(Mark::Inherited(via)) => format!("via {via}"),
                _ => meta.clone(),
            };
            let name_style = Style::default().fg(colors.text);
            (glyphs, name.clone(), name_style, *status, meta, dim)
        }
    };

    // Right-hand side: the status word in a fixed cell (wide rows only), the meta, then one
    // column of margin before the border. The gutter comes first (`row_with_glyphs`).
    let mut right = Vec::new();
    if width >= STATUS_WORD_MIN_WIDTH {
        let (text, colour) = match word {
            Some(status) => (status_word(status), status_color(colors, status)),
            None => (String::new(), colors.text_dim),
        };
        let cell = format!("{text:>width$} ", width = STATUS_WORD_WIDTH);
        right.push(Span::styled(cell, Style::default().fg(colour)));
    }
    right.push(Span::styled(meta, meta_style));
    right.push(Span::raw(" "));
    let right_width: usize = right.iter().map(Span::width).sum();

    let room = usize::from(width).saturating_sub(1);
    let glyph_width: usize = glyphs.iter().map(Span::width).sum();
    // One column always separates the name from the right-hand side.
    let name_room = room.saturating_sub(glyph_width + right_width + 1);
    let name = elide(&name, name_room);
    let pad = room.saturating_sub(glyph_width + name.width() + right_width);
    let mut content = vec![Span::styled(name, name_style), Span::raw(" ".repeat(pad))];
    content.extend(right);
    selection::row_with_glyphs(glyphs, content, selected, colors, width)
}

#[cfg(test)]
mod tests {
    use ratatui::{backend::TestBackend, buffer::Buffer, layout::Rect, Terminal};

    use super::*;
    use crate::ui::icons::UNICODE;
    use crate::ui::list_panel::{render_list_panel, ListView};
    use crate::ui::selection::ACCENT;
    use andre_core::Theme;

    fn colors() -> ThemeColors {
        ThemeColors::from_theme(Theme::Default)
    }

    fn pkg(name: &str, status: Option<StowStatus>, mark: Option<Mark>) -> TreePackage {
        TreePackage {
            name: name.to_string(),
            status,
            mark,
            meta: String::new(),
        }
    }

    fn group(name: &str, packages: Vec<TreePackage>) -> TreeGroup {
        TreeGroup {
            name: name.to_string(),
            packages,
        }
    }

    /// `home` with three packages: git marked, shell not, ssh inherited from `base`.
    fn home() -> TreeGroup {
        group(
            "home",
            vec![
                pkg("git", Some(StowStatus::Stowed), Some(Mark::On)),
                pkg("shell", Some(StowStatus::Partial), Some(Mark::Off)),
                pkg("ssh", None, Some(Mark::Inherited("base".into()))),
            ],
        )
    }

    fn header_mark(tree: &Tree, row: usize) -> Option<Mark> {
        match &tree.rows()[row] {
            TreeRow::Group { mark, .. } => mark.clone(),
            other => panic!("row {row} is not a group: {other:?}"),
        }
    }

    fn package_mark(tree: &Tree, row: usize) -> Option<Mark> {
        match &tree.rows()[row] {
            TreeRow::Package { mark, .. } => mark.clone(),
            other => panic!("row {row} is not a package: {other:?}"),
        }
    }

    fn text(line: &Line) -> String {
        line.spans.iter().map(|s| s.content.as_ref()).collect()
    }

    #[test]
    fn test_fold_hides_the_group_packages() {
        let mut tree = Tree::new(vec![home(), group("work", vec![pkg("vpn", None, None)])]);
        assert_eq!(tree.rows().len(), 6);
        tree.fold(0, true);
        let rows = tree.rows();
        assert_eq!(rows.len(), 3, "home's packages hidden, work's still shown");
        assert!(matches!(&rows[0], TreeRow::Group { folded: true, .. }));
        assert!(matches!(&rows[1], TreeRow::Group { folded: false, .. }));
    }

    #[test]
    fn test_fold_on_a_package_row_folds_its_group() {
        let mut tree = Tree::new(vec![home()]);
        tree.fold(2, true);
        assert_eq!(tree.rows().len(), 1);
        tree.fold(0, false);
        assert_eq!(tree.rows().len(), 4, "unfolding shows the packages again");
    }

    #[test]
    fn test_fold_keeps_marks_of_hidden_packages() {
        let mut tree = Tree::new(vec![home()]);
        tree.fold(0, true);
        assert_eq!(tree.marked(), vec![("home".into(), "git".into())]);
    }

    #[test]
    fn test_header_mark_is_some_with_one_of_three_marked() {
        let tree = Tree::new(vec![group(
            "home",
            vec![
                pkg("git", None, Some(Mark::On)),
                pkg("shell", None, Some(Mark::Off)),
                pkg("ssh", None, Some(Mark::Off)),
            ],
        )]);
        assert_eq!(header_mark(&tree, 0), Some(Mark::Some));
        assert!(matches!(&tree.rows()[0], TreeRow::Group { meta, .. } if meta == "1/3"));
    }

    #[test]
    fn test_header_mark_is_on_when_every_package_is_marked_or_inherited() {
        let tree = Tree::new(vec![home()]);
        let mut all = Tree::new(vec![group(
            "home",
            vec![
                pkg("git", None, Some(Mark::On)),
                pkg("ssh", None, Some(Mark::Inherited("base".into()))),
            ],
        )]);
        assert_eq!(header_mark(&all, 0), Some(Mark::On));
        assert_eq!(header_mark(&tree, 0), Some(Mark::Some));
        all.toggle_mark(0);
        assert_eq!(
            header_mark(&all, 0),
            Some(Mark::Some),
            "own package off, the inherited one still on"
        );
    }

    #[test]
    fn test_header_without_checkboxes_has_no_mark_and_no_count() {
        let tree = Tree::new(vec![group("home", vec![pkg("git", None, None)])]);
        assert_eq!(header_mark(&tree, 0), None);
        assert!(matches!(&tree.rows()[0], TreeRow::Group { meta, .. } if meta.is_empty()));
    }

    #[test]
    fn test_space_on_header_marks_all_own_packages_and_keeps_inherited() {
        let mut tree = Tree::new(vec![home()]);
        assert_eq!(tree.toggle_mark(0), Toggle::Changed);
        assert_eq!(package_mark(&tree, 1), Some(Mark::On));
        assert_eq!(package_mark(&tree, 2), Some(Mark::On));
        assert_eq!(package_mark(&tree, 3), Some(Mark::Inherited("base".into())));
        assert_eq!(header_mark(&tree, 0), Some(Mark::On));
    }

    #[test]
    fn test_space_on_marked_header_unmarks_own_packages_only() {
        let mut tree = Tree::new(vec![group(
            "home",
            vec![
                pkg("git", None, Some(Mark::On)),
                pkg("ssh", None, Some(Mark::Inherited("base".into()))),
                pkg("shell", None, Some(Mark::On)),
            ],
        )]);
        assert_eq!(tree.toggle_mark(0), Toggle::Changed);
        assert_eq!(package_mark(&tree, 1), Some(Mark::Off));
        assert_eq!(package_mark(&tree, 2), Some(Mark::Inherited("base".into())));
        assert_eq!(package_mark(&tree, 3), Some(Mark::Off));
        assert_eq!(
            header_mark(&tree, 0),
            Some(Mark::Some),
            "only the profile's ssh"
        );
    }

    #[test]
    fn test_package_toggle_flips_its_mark() {
        let mut tree = Tree::new(vec![home()]);
        assert_eq!(tree.toggle_mark(2), Toggle::Changed);
        assert_eq!(package_mark(&tree, 2), Some(Mark::On));
        assert_eq!(tree.toggle_mark(2), Toggle::Changed);
        assert_eq!(package_mark(&tree, 2), Some(Mark::Off));
    }

    #[test]
    fn test_inherited_row_refuses_toggle() {
        let mut tree = Tree::new(vec![home()]);
        assert_eq!(
            tree.toggle_mark(3),
            Toggle::Inherited { via: "base".into() }
        );
        assert_eq!(package_mark(&tree, 3), Some(Mark::Inherited("base".into())));
    }

    #[test]
    fn test_header_of_a_group_marked_only_by_a_profile_refuses_toggle() {
        let mut tree = Tree::new(vec![group(
            "home",
            vec![pkg("ssh", None, Some(Mark::Inherited("base".into())))],
        )]);
        assert_eq!(
            tree.toggle_mark(0),
            Toggle::Inherited { via: "base".into() }
        );
        assert_eq!(header_mark(&tree, 0), Some(Mark::On));
    }

    #[test]
    fn test_rows_without_checkboxes_are_unmarkable() {
        let mut tree = Tree::new(vec![group("home", vec![pkg("git", None, None)])]);
        assert_eq!(tree.toggle_mark(0), Toggle::Unmarkable);
        assert_eq!(tree.toggle_mark(1), Toggle::Unmarkable);
        assert_eq!(tree.marked(), Vec::<(String, String)>::new());
    }

    #[test]
    fn test_toggle_past_the_last_row_is_unmarkable() {
        let mut tree = Tree::new(vec![home()]);
        assert_eq!(tree.toggle_mark(99), Toggle::Unmarkable);
        tree.fold(99, true);
        assert_eq!(tree.rows().len(), 4, "fold past the end changes nothing");
    }

    #[test]
    fn test_marked_lists_own_marks_in_config_order() {
        let mut tree = Tree::new(vec![
            group(
                "home",
                vec![
                    pkg("zsh", None, Some(Mark::On)),
                    pkg("git", None, Some(Mark::Off)),
                    pkg("ssh", None, Some(Mark::Inherited("base".into()))),
                ],
            ),
            group(
                "config",
                vec![
                    pkg("nvim", None, Some(Mark::On)),
                    pkg("kitty", None, Some(Mark::On)),
                ],
            ),
        ]);
        tree.fold(0, true);
        assert_eq!(
            tree.marked(),
            vec![
                ("home".into(), "zsh".into()),
                ("config".into(), "nvim".into()),
                ("config".into(), "kitty".into()),
            ]
        );
        tree.toggle_mark(0);
        assert_eq!(
            tree.marked(),
            vec![
                ("home".into(), "zsh".into()),
                ("home".into(), "git".into()),
                ("config".into(), "nvim".into()),
                ("config".into(), "kitty".into()),
            ],
            "a header toggle marks in config order"
        );
    }

    #[test]
    fn test_group_row_draws_fold_mark_name_and_count() {
        let tree = Tree::new(vec![home()]);
        let row = &tree.rows()[0];
        let line = tree_line(row, false, &UNICODE, &colors(), 24);
        assert_eq!(line.width(), 24);
        assert_eq!(
            text(&line),
            format!(" \u{25be} \u{25a3} home{}2/3 ", " ".repeat(11))
        );
    }

    #[test]
    fn test_folded_group_row_draws_a_closed_fold() {
        let mut tree = Tree::new(vec![home()]);
        tree.fold(0, true);
        let line = tree_line(&tree.rows()[0], false, &UNICODE, &colors(), 24);
        assert!(text(&line).starts_with(" \u{25b8} "));
    }

    #[test]
    fn test_package_row_draws_mark_status_and_name_under_the_group() {
        let tree = Tree::new(vec![home()]);
        let line = tree_line(&tree.rows()[1], false, &UNICODE, &colors(), 24);
        assert_eq!(line.width(), 24);
        assert!(
            text(&line).starts_with("     \u{25a0} \u{2713} git"),
            "{:?}",
            text(&line)
        );
    }

    #[test]
    fn test_unknown_status_draws_a_dot_and_inherited_shows_its_profile() {
        let tree = Tree::new(vec![home()]);
        let line = tree_line(&tree.rows()[3], false, &UNICODE, &colors(), 30);
        assert_eq!(line.width(), 30);
        assert_eq!(
            text(&line).trim_end(),
            format!("     \u{25a0} \u{b7} ssh{}via base", " ".repeat(9))
        );
    }

    #[test]
    fn test_package_without_checkbox_keeps_its_column() {
        let tree = Tree::new(vec![group(
            "home",
            vec![pkg("git", Some(StowStatus::Stowed), None)],
        )]);
        let line = tree_line(&tree.rows()[1], false, &UNICODE, &colors(), 20);
        assert_eq!(text(&line).trim_end(), "       \u{2713} git");
    }

    #[test]
    fn test_checkbox_less_rows_drop_the_checkbox_column() {
        let tree = Tree::new(vec![group(
            "home",
            vec![pkg("shell", Some(StowStatus::Stowed), None)],
        )]);
        let rows = tree.rows();
        let header = tree_line_without_checkboxes(&rows[0], false, &UNICODE, &colors(), 24);
        let package = tree_line_without_checkboxes(&rows[1], false, &UNICODE, &colors(), 24);
        // The gutter is one column, as on every row.
        assert_eq!(text(&header).trim_end(), " \u{25be} home");
        assert_eq!(text(&package).trim_end(), "     \u{2713} shell");
    }

    #[test]
    fn test_long_name_is_cut_before_the_right_hand_text() {
        let tree = Tree::new(vec![group(
            "home",
            vec![TreePackage {
                meta: "3 files".into(),
                ..pkg("a-very-long-package-name", None, Some(Mark::Off))
            }],
        )]);
        let line = tree_line(&tree.rows()[1], false, &UNICODE, &colors(), 28);
        let text = text(&line);
        assert_eq!(line.width(), 28);
        assert!(text.contains('\u{2026}'), "{text:?}");
        assert!(text.ends_with("3 files "), "{text:?}");
    }

    #[test]
    fn test_status_word_column_shows_from_44_cols() {
        let tree = Tree::new(vec![group(
            "home",
            vec![pkg("git", Some(StowStatus::Stowed), Some(Mark::On))],
        )]);
        let row = &tree.rows()[1];
        let word_at = |width| text(&tree_line(row, false, &UNICODE, &colors(), width));
        // `width` is the inner row, so a 44-col pane (42-col row) shows the word.
        assert!(!word_at(32).contains("stowed"), "{:?}", word_at(32));
        assert!(!word_at(41).contains("stowed"), "{:?}", word_at(41));
        let wide = word_at(42);
        assert!(wide.contains("stowed"), "{wide:?}");
        let line = tree_line(row, false, &UNICODE, &colors(), 46);
        assert_eq!(line.width(), 46);
        assert!(text(&line).ends_with("  stowed  "), "{:?}", text(&line));
    }

    #[test]
    fn test_status_word_column_lines_up_across_rows() {
        let tree = Tree::new(vec![group(
            "home",
            vec![
                pkg("git", Some(StowStatus::Unstowed), Some(Mark::On)),
                pkg("shell", Some(StowStatus::Stowed), Some(Mark::Off)),
                pkg("vpn", None, None),
            ],
        )]);
        // Column (in chars) where a row's status word ends.
        let word_end = |row: usize, word: &str| {
            let line = text(&tree_line(
                &tree.rows()[row],
                false,
                &UNICODE,
                &colors(),
                50,
            ));
            let at = line.find(word).unwrap() + word.len();
            line[..at].chars().count()
        };
        assert_eq!(word_end(1, "unstowed"), word_end(2, "stowed"));
    }

    #[test]
    fn test_set_status_keeps_folds_and_marks_and_draws_the_new_glyph() {
        let mut tree = Tree::new(vec![
            home(),
            group("config", vec![pkg("andre", None, Some(Mark::Off))]),
        ]);
        tree.fold(0, true);
        let marked = tree.marked();

        assert!(tree.set_status("config", "andre", Some(StowStatus::Conflict)));
        assert!(!tree.set_status("config", "missing", None));
        assert!(!tree.set_status("missing", "andre", None));

        let rows = tree.rows();
        assert_eq!(rows.len(), 3, "home is still folded");
        assert!(matches!(&rows[0], TreeRow::Group { folded: true, .. }));
        assert_eq!(tree.marked(), marked);
        let line = tree_line(&rows[2], false, &UNICODE, &colors(), 30);
        assert!(
            text(&line).contains(UNICODE.status(StowStatus::Conflict)),
            "{:?}",
            text(&line)
        );
    }

    #[test]
    fn test_replace_groups_keeps_folds_of_groups_that_remain() {
        let mut tree = Tree::new(vec![home(), group("work", vec![pkg("vpn", None, None)])]);
        tree.fold(0, true);
        tree.fold(1, true);

        tree.replace_groups(vec![
            group(
                "home",
                vec![
                    pkg("git", Some(StowStatus::Conflict), Some(Mark::On)),
                    pkg("zsh", Some(StowStatus::Stowed), Some(Mark::Off)),
                ],
            ),
            group("config", vec![pkg("kitty", None, None)]),
        ]);
        let rows = tree.rows();
        assert_eq!(rows.len(), 3, "home stays folded, config is open");
        assert!(matches!(&rows[0], TreeRow::Group { folded: true, .. }));
        assert!(matches!(&rows[1], TreeRow::Group { folded: false, .. }));

        tree.replace_groups(vec![group("work", vec![pkg("vpn", None, None)])]);
        assert!(
            matches!(&tree.rows()[0], TreeRow::Group { folded: false, .. }),
            "a group that left the tree loses its fold"
        );
    }

    #[test]
    fn test_selected_row_starts_with_the_cursor_bar() {
        let tree = Tree::new(vec![home()]);
        let line = tree_line(&tree.rows()[1], true, &UNICODE, &colors(), 24);
        assert_eq!(line.spans[0].content, ACCENT);
    }

    #[test]
    fn test_checked_mark_is_accent_and_unchecked_is_dim() {
        let tree = Tree::new(vec![home()]);
        let colors = colors();
        let on = tree_line(&tree.rows()[1], false, &UNICODE, &colors, 24);
        let off = tree_line(&tree.rows()[2], false, &UNICODE, &colors, 24);
        let glyph = |line: &Line, glyph: &str| {
            line.spans
                .iter()
                .find(|s| s.content == glyph)
                .and_then(|s| s.style.fg)
        };
        assert_eq!(glyph(&on, UNICODE.checkbox_on), Some(colors.accent));
        assert_eq!(glyph(&off, UNICODE.checkbox_off), Some(colors.text_dim));
    }

    /// Draws the tree in a bordered panel `width` x `height`, as a list screen does.
    fn drawn(tree: &Tree, width: u16, height: u16) -> Buffer {
        let rows = tree.rows();
        let c = colors();
        let mut view = ListView::default();
        let area = Rect::new(0, 0, width, height);
        view.prepare(area, rows.len());
        let lines: Vec<Line<'static>> = rows
            .iter()
            .enumerate()
            .map(|(i, row)| tree_line(row, i == view.cursor, &UNICODE, &c, width - 2))
            .collect();
        let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
        terminal
            .draw(|f| render_list_panel(f, area, &c, "packages", lines, &view, None))
            .unwrap();
        terminal.backend().buffer().clone()
    }

    fn buffer_text(buf: &Buffer) -> Vec<String> {
        (0..buf.area().height)
            .map(|y| {
                (0..buf.area().width)
                    .map(|x| buf[(x, y)].symbol())
                    .collect()
            })
            .collect()
    }

    fn sample_tree() -> Tree {
        Tree::new(vec![
            home(),
            group(
                "config",
                vec![
                    pkg("andre", Some(StowStatus::Conflict), Some(Mark::Off)),
                    pkg("kitty", Some(StowStatus::Unstowed), Some(Mark::Off)),
                ],
            ),
            group("work", vec![pkg("vpn", None, Some(Mark::Off))]),
        ])
    }

    #[test]
    fn test_tree_panel_draws_at_80x24_content_size() {
        let lines = buffer_text(&drawn(&sample_tree(), 76, 16));
        assert_eq!(lines.len(), 16);
        assert!(lines[0].starts_with('╭'), "{:?}", lines[0]);
        assert!(lines[1].contains("▾ ▣ home"), "{:?}", lines[1]);
        assert!(lines[1].contains("2/3"), "{:?}", lines[1]);
        // The right-hand text stops one column before the border.
        assert!(lines[1].contains("2/3 │"), "{:?}", lines[1]);
        assert!(lines[2].contains("■ ✓ git"), "{:?}", lines[2]);
    }

    #[test]
    fn test_tree_panel_renders_at_120x40_and_200x50_without_panic() {
        for (width, height) in [(120, 40), (200, 50)] {
            let buf = drawn(&sample_tree(), width, height);
            assert_eq!(buf.area().width, width);
            assert_eq!(buf.area().height, height);
        }
    }
}
