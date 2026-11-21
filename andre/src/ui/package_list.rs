//! The flat package list: one row per package, in name order, with no groups. A row has an
//! optional checkbox, a status glyph, the name and right-hand text. [`PackageList`] owns the
//! rows and their marks and holds no terminal state; [`package_line`] paints one row.

use ratatui::{
    style::Style,
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
/// The same threshold for `package_line`'s `width`, which is the inner row (pane minus the
/// two border columns).
const STATUS_WORD_MIN_WIDTH: u16 = STATUS_WORD_MIN_PANE_WIDTH - 2;

/// The checkbox column. The gutter is column 0, so the box is the first column after it.
const MARK_COL: u16 = 1;

/// A checkbox state. Only `On` and `Off` rows can be toggled; `Inherited` cannot.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Mark {
    On,
    Off,
    /// Marked by an included profile. It cannot be toggled here; the name is that profile.
    Inherited(String),
}

/// One package row.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PackageRow {
    pub name: String,
    /// `None` until the status is known; drawn as `·`.
    pub status: Option<StowStatus>,
    /// `None` when the list shows no checkbox for this row.
    pub mark: Option<Mark>,
    /// Right-hand text. An inherited row shows `via <profile>` instead.
    pub meta: String,
}

/// What [`PackageList::toggle_mark`] and [`PackageList::toggle_all`] did.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Toggle {
    Changed,
    /// The row is marked by the profile `via`. Nothing changed; the caller says why.
    Inherited {
        via: String,
    },
    /// The row has no checkbox, or the list has no row of its own to mark.
    Unmarkable,
}

/// The rows of a flat list and their marks.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PackageList {
    rows: Vec<PackageRow>,
}

impl PackageList {
    pub fn new(rows: Vec<PackageRow>) -> Self {
        Self { rows }
    }

    pub fn rows(&self) -> &[PackageRow] {
        &self.rows
    }

    /// Sets one row's status, found by name. Marks are untouched. Returns false when no row
    /// has that name.
    pub fn set_status(&mut self, name: &str, status: Option<StowStatus>) -> bool {
        match self.rows.iter_mut().find(|row| row.name == name) {
            Some(row) => {
                row.status = status;
                true
            }
            None => false,
        }
    }

    /// Toggles the mark on `row`. An `On` row becomes `Off` and the reverse. Inherited rows
    /// are left alone, and so are rows with no checkbox.
    pub fn toggle_mark(&mut self, row: usize) -> Toggle {
        let Some(row) = self.rows.get_mut(row) else {
            return Toggle::Unmarkable;
        };
        match &row.mark {
            Some(Mark::On) => row.mark = Some(Mark::Off),
            Some(Mark::Off) => row.mark = Some(Mark::On),
            Some(Mark::Inherited(via)) => return Toggle::Inherited { via: via.clone() },
            None => return Toggle::Unmarkable,
        }
        Toggle::Changed
    }

    /// `*`: marks every row the user can mark, or unmarks them all when they are all marked
    /// already. Inherited rows are never touched.
    pub fn toggle_all(&mut self) -> Toggle {
        if !self.rows.iter().any(is_own_checkbox) {
            return match self.rows.iter().find_map(inherited_via) {
                Some(via) => Toggle::Inherited { via },
                None => Toggle::Unmarkable,
            };
        }
        let all_marked = self
            .rows
            .iter()
            .filter(|row| is_own_checkbox(row))
            .all(|row| row.mark == Some(Mark::On));
        let next = if all_marked { Mark::Off } else { Mark::On };
        for row in self.rows.iter_mut().filter(|row| is_own_checkbox(row)) {
            row.mark = Some(next.clone());
        }
        Toggle::Changed
    }

    /// The names of the rows the user marked, in list order. Inherited rows are not listed.
    pub fn marked(&self) -> Vec<String> {
        self.rows
            .iter()
            .filter(|row| row.mark == Some(Mark::On))
            .map(|row| row.name.clone())
            .collect()
    }
}

/// A row whose mark the user can flip.
fn is_own_checkbox(row: &PackageRow) -> bool {
    matches!(row.mark, Some(Mark::On | Mark::Off))
}

fn inherited_via(row: &PackageRow) -> Option<String> {
    match &row.mark {
        Some(Mark::Inherited(via)) => Some(via.clone()),
        _ => None,
    }
}

/// The checkbox glyph. A row without a checkbox keeps its column blank, so names line up.
fn mark_span(mark: Option<&Mark>, icons: &Icons, colors: &ThemeColors) -> Span<'static> {
    let (glyph, colour) = match mark {
        Some(Mark::On) => (icons.checkbox_on, colors.accent),
        Some(Mark::Off) => (icons.checkbox_off, colors.text_dim),
        Some(Mark::Inherited(_)) => (icons.checkbox_on, colors.text_dim),
        None => return Span::raw(" "),
    };
    Span::styled(glyph, Style::default().fg(colour))
}

/// Whether `col` is the checkbox cell of `row`, so a click there toggles its mark. A row with
/// no checkbox has no such cell.
pub fn mark_hit(row: &PackageRow, col: u16, checkboxes: bool) -> bool {
    checkboxes && row.mark.is_some() && col == MARK_COL
}

/// Paints one row `width` columns wide. `checkboxes` says whether the list has a checkbox
/// column; a list without one (the review) starts the status glyph at column 1. Glyphs keep
/// their colour on the cursor row when they can (see `ui::selection`); the name is cut with
/// `…` before the right-hand text. Rows at `STATUS_WORD_MIN_WIDTH` (a 44-col pane) or wider
/// also show the status word before the meta. The right-hand text ends one column before the
/// border.
pub fn package_line(
    row: &PackageRow,
    selected: bool,
    checkboxes: bool,
    icons: &Icons,
    colors: &ThemeColors,
    width: u16,
) -> Line<'static> {
    let dim = Style::default().fg(colors.text_dim);
    let (status_glyph, status_style) = match row.status {
        Some(status) => (
            icons.status(status),
            Style::default().fg(status_color(colors, status)),
        ),
        None => (icons.unknown, dim),
    };
    let mut glyphs = Vec::new();
    if checkboxes {
        glyphs.push(mark_span(row.mark.as_ref(), icons, colors));
        glyphs.push(Span::raw(" "));
    }
    glyphs.push(Span::styled(status_glyph, status_style));
    glyphs.push(Span::raw(" "));

    let meta = match &row.mark {
        Some(Mark::Inherited(via)) => format!("via {via}"),
        _ => row.meta.clone(),
    };
    let name_style = Style::default().fg(colors.text);

    // Right-hand side: the status word in a fixed cell (wide rows only), the meta, then one
    // column of margin before the border. The gutter comes first (`row_with_glyphs`).
    let mut right = Vec::new();
    if width >= STATUS_WORD_MIN_WIDTH {
        let (text, colour) = match row.status {
            Some(status) => (status_word(status), status_color(colors, status)),
            None => (String::new(), colors.text_dim),
        };
        let cell = format!("{text:>width$} ", width = STATUS_WORD_WIDTH);
        right.push(Span::styled(cell, Style::default().fg(colour)));
    }
    right.push(Span::styled(meta, dim));
    right.push(Span::raw(" "));
    let right_width: usize = right.iter().map(Span::width).sum();

    let room = usize::from(width).saturating_sub(1);
    let glyph_width: usize = glyphs.iter().map(Span::width).sum();
    // One column always separates the name from the right-hand side.
    let name_room = room.saturating_sub(glyph_width + right_width + 1);
    let name = elide(&row.name, name_room);
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

    fn row(name: &str, status: Option<StowStatus>, mark: Option<Mark>) -> PackageRow {
        PackageRow {
            name: name.to_string(),
            status,
            mark,
            meta: String::new(),
        }
    }

    /// `git` marked, `shell` not, `ssh` inherited from `base`.
    fn sample() -> PackageList {
        PackageList::new(vec![
            row("git", Some(StowStatus::Stowed), Some(Mark::On)),
            row("shell", Some(StowStatus::Partial), Some(Mark::Off)),
            row("ssh", None, Some(Mark::Inherited("base".into()))),
        ])
    }

    fn mark_of(list: &PackageList, index: usize) -> Option<Mark> {
        list.rows()[index].mark.clone()
    }

    fn text(line: &Line) -> String {
        line.spans.iter().map(|s| s.content.as_ref()).collect()
    }

    #[test]
    fn test_toggle_mark_flips_its_row() {
        let mut list = sample();
        assert_eq!(list.toggle_mark(1), Toggle::Changed);
        assert_eq!(mark_of(&list, 1), Some(Mark::On));
        assert_eq!(list.toggle_mark(1), Toggle::Changed);
        assert_eq!(mark_of(&list, 1), Some(Mark::Off));
    }

    #[test]
    fn test_inherited_row_refuses_toggle() {
        let mut list = sample();
        assert_eq!(
            list.toggle_mark(2),
            Toggle::Inherited { via: "base".into() }
        );
        assert_eq!(mark_of(&list, 2), Some(Mark::Inherited("base".into())));
    }

    #[test]
    fn test_rows_without_checkboxes_and_rows_past_the_end_are_unmarkable() {
        let mut list = PackageList::new(vec![row("git", None, None)]);
        assert_eq!(list.toggle_mark(0), Toggle::Unmarkable);
        assert_eq!(list.toggle_mark(99), Toggle::Unmarkable);
        assert_eq!(list.marked(), Vec::<String>::new());
    }

    #[test]
    fn test_toggle_all_marks_every_own_row_then_unmarks_them_all() {
        let mut list = sample();
        assert_eq!(list.toggle_all(), Toggle::Changed);
        assert_eq!(mark_of(&list, 0), Some(Mark::On));
        assert_eq!(mark_of(&list, 1), Some(Mark::On));
        assert_eq!(list.toggle_all(), Toggle::Changed);
        assert_eq!(mark_of(&list, 0), Some(Mark::Off));
        assert_eq!(mark_of(&list, 1), Some(Mark::Off));
    }

    #[test]
    fn test_toggle_all_never_touches_inherited_rows() {
        let mut list = sample();
        list.toggle_all();
        assert_eq!(mark_of(&list, 2), Some(Mark::Inherited("base".into())));
        list.toggle_all();
        assert_eq!(mark_of(&list, 2), Some(Mark::Inherited("base".into())));
    }

    #[test]
    fn test_toggle_all_marks_the_rest_when_only_some_are_marked() {
        // `git` is marked and `shell` is not: `*` marks both.
        let mut list = sample();
        assert_eq!(list.toggle_all(), Toggle::Changed);
        assert_eq!(mark_of(&list, 0), Some(Mark::On));
        assert_eq!(mark_of(&list, 1), Some(Mark::On));
    }

    #[test]
    fn test_toggle_all_with_only_inherited_rows_names_the_profile() {
        let mut list =
            PackageList::new(vec![row("ssh", None, Some(Mark::Inherited("base".into())))]);
        assert_eq!(list.toggle_all(), Toggle::Inherited { via: "base".into() });
        let mut bare = PackageList::new(vec![row("git", None, None)]);
        assert_eq!(bare.toggle_all(), Toggle::Unmarkable);
    }

    #[test]
    fn test_marked_lists_marked_names_in_list_order() {
        let mut list = PackageList::new(vec![
            row("zsh", None, Some(Mark::On)),
            row("git", None, Some(Mark::Off)),
            row("ssh", None, Some(Mark::Inherited("base".into()))),
            row("nvim", None, Some(Mark::On)),
        ]);
        assert_eq!(list.marked(), ["zsh", "nvim"]);
        list.toggle_mark(1);
        assert_eq!(list.marked(), ["zsh", "git", "nvim"]);
    }

    #[test]
    fn test_set_status_keeps_marks_and_draws_the_new_glyph() {
        let mut list = sample();
        let marked = list.marked();

        assert!(list.set_status("shell", Some(StowStatus::Conflict)));
        assert!(!list.set_status("missing", None));

        assert_eq!(list.marked(), marked);
        let line = package_line(&list.rows()[1], false, true, &UNICODE, &colors(), 30);
        assert!(
            text(&line).contains(UNICODE.status(StowStatus::Conflict)),
            "{:?}",
            text(&line)
        );
    }

    #[test]
    fn test_mark_hit_is_the_checkbox_cell_only() {
        let marked = row("git", None, Some(Mark::On));
        let unmarkable = row("git", None, None);
        assert!(mark_hit(&marked, 1, true));
        assert!(!mark_hit(&marked, 0, true));
        assert!(
            !mark_hit(&marked, 3, true),
            "the status glyph is not the box"
        );
        assert!(
            !mark_hit(&marked, 1, false),
            "a list without checkboxes has none"
        );
        assert!(
            !mark_hit(&unmarkable, 1, true),
            "a row without a checkbox has none"
        );
    }

    #[test]
    fn test_package_row_draws_mark_status_and_name() {
        let list = sample();
        let line = package_line(&list.rows()[0], false, true, &UNICODE, &colors(), 24);
        assert_eq!(line.width(), 24);
        assert!(
            text(&line).starts_with(" \u{25a0} \u{2713} git"),
            "{:?}",
            text(&line)
        );
    }

    #[test]
    fn test_unknown_status_draws_a_dot_and_inherited_shows_its_profile() {
        let list = sample();
        let line = package_line(&list.rows()[2], false, true, &UNICODE, &colors(), 30);
        assert_eq!(line.width(), 30);
        assert_eq!(
            text(&line).trim_end(),
            format!(" \u{25a0} \u{b7} ssh{}via base", " ".repeat(13))
        );
    }

    #[test]
    fn test_review_rows_have_no_checkbox_column() {
        let list = PackageList::new(vec![row("shell", Some(StowStatus::Stowed), None)]);
        let line = package_line(&list.rows()[0], false, false, &UNICODE, &colors(), 24);
        assert_eq!(text(&line).trim_end(), " \u{2713} shell");
    }

    #[test]
    fn test_package_without_checkbox_keeps_its_status_column() {
        let list = PackageList::new(vec![row("git", Some(StowStatus::Stowed), None)]);
        let line = package_line(&list.rows()[0], false, false, &UNICODE, &colors(), 20);
        assert_eq!(text(&line).trim_end(), " \u{2713} git");
    }

    #[test]
    fn test_long_name_is_cut_before_the_right_hand_text() {
        let list = PackageList::new(vec![PackageRow {
            meta: "3 files".into(),
            ..row("a-very-long-package-name", None, Some(Mark::Off))
        }]);
        let line = package_line(&list.rows()[0], false, true, &UNICODE, &colors(), 28);
        let text = text(&line);
        assert_eq!(line.width(), 28);
        assert!(text.contains('\u{2026}'), "{text:?}");
        assert!(text.ends_with("3 files "), "{text:?}");
    }

    #[test]
    fn test_status_word_column_shows_from_44_cols() {
        let list = PackageList::new(vec![row("git", Some(StowStatus::Stowed), Some(Mark::On))]);
        let row = &list.rows()[0];
        let word_at = |width| text(&package_line(row, false, true, &UNICODE, &colors(), width));
        // `width` is the inner row, so a 44-col pane (42-col row) shows the word.
        assert!(!word_at(32).contains("stowed"), "{:?}", word_at(32));
        assert!(!word_at(41).contains("stowed"), "{:?}", word_at(41));
        let wide = word_at(42);
        assert!(wide.contains("stowed"), "{wide:?}");
        let line = package_line(row, false, true, &UNICODE, &colors(), 46);
        assert_eq!(line.width(), 46);
        assert!(text(&line).ends_with("  stowed  "), "{:?}", text(&line));
    }

    #[test]
    fn test_status_word_column_lines_up_across_rows() {
        let list = PackageList::new(vec![
            row("git", Some(StowStatus::Unstowed), Some(Mark::On)),
            row("shell", Some(StowStatus::Stowed), Some(Mark::Off)),
            row("vpn", None, None),
        ]);
        // Column (in chars) where a row's status word ends.
        let word_end = |index: usize, word: &str| {
            let line = text(&package_line(
                &list.rows()[index],
                false,
                true,
                &UNICODE,
                &colors(),
                50,
            ));
            let at = line.find(word).unwrap() + word.len();
            line[..at].chars().count()
        };
        assert_eq!(word_end(0, "unstowed"), word_end(1, "stowed"));
    }

    #[test]
    fn test_selected_row_starts_with_the_cursor_bar() {
        let list = sample();
        let line = package_line(&list.rows()[0], true, true, &UNICODE, &colors(), 24);
        assert_eq!(line.spans[0].content, ACCENT);
    }

    #[test]
    fn test_checked_mark_is_accent_and_unchecked_is_dim() {
        let list = sample();
        let colors = colors();
        let on = package_line(&list.rows()[0], false, true, &UNICODE, &colors, 24);
        let off = package_line(&list.rows()[1], false, true, &UNICODE, &colors, 24);
        let glyph = |line: &Line, glyph: &str| {
            line.spans
                .iter()
                .find(|s| s.content == glyph)
                .and_then(|s| s.style.fg)
        };
        assert_eq!(glyph(&on, UNICODE.checkbox_on), Some(colors.accent));
        assert_eq!(glyph(&off, UNICODE.checkbox_off), Some(colors.text_dim));
    }

    #[test]
    fn test_status_glyph_sits_after_the_box_only_when_there_is_one() {
        let list = PackageList::new(vec![row("git", Some(StowStatus::Stowed), Some(Mark::On))]);
        let boxed = text(&package_line(
            &list.rows()[0],
            false,
            true,
            &UNICODE,
            &colors(),
            24,
        ));
        let bare = text(&package_line(
            &list.rows()[0],
            false,
            false,
            &UNICODE,
            &colors(),
            24,
        ));
        assert_eq!(
            boxed.chars().nth(3),
            UNICODE.status(StowStatus::Stowed).chars().next()
        );
        assert_eq!(
            bare.chars().nth(1),
            UNICODE.status(StowStatus::Stowed).chars().next()
        );
    }

    /// Draws the list in a bordered panel `width` x `height`, as a list screen does.
    fn drawn(list: &PackageList, width: u16, height: u16) -> Buffer {
        let c = colors();
        let mut view = ListView::default();
        let area = Rect::new(0, 0, width, height);
        view.prepare(area, list.rows().len());
        let lines: Vec<Line<'static>> = list
            .rows()
            .iter()
            .enumerate()
            .map(|(i, row)| package_line(row, i == view.cursor, true, &UNICODE, &c, width - 2))
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

    #[test]
    fn test_package_panel_draws_at_80x24_content_size() {
        let lines = buffer_text(&drawn(&sample(), 76, 16));
        assert_eq!(lines.len(), 16);
        assert!(lines[0].starts_with('╭'), "{:?}", lines[0]);
        assert!(lines[1].contains("■ ✓ git"), "{:?}", lines[1]);
        assert!(lines[2].contains("□ ◐ shell"), "{:?}", lines[2]);
    }

    #[test]
    fn test_package_panel_renders_at_120x40_and_200x50_without_panic() {
        for (width, height) in [(120, 40), (200, 50)] {
            let buf = drawn(&sample(), width, height);
            assert_eq!(buf.area().width, width);
            assert_eq!(buf.area().height, height);
        }
    }
}
