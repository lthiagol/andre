use ratatui::{
    layout::Rect,
    style::{Modifier, Style},
    text::Span,
    widgets::{List, ListItem, ListState, Paragraph},
    Frame,
};
use unicode_width::UnicodeWidthStr;

use andre_core::StowStatus;

use crate::theme::ThemeColors;
use crate::ui::elide;
use crate::ui::icons::Icons;
use crate::ui::selection;
use crate::ui::status::status_layout;
use crate::ui::status_style::{render_legend, status_color, status_word, STATUS_WORD_WIDTH};
use crate::ui::widgets::scrollbar::{border_track, render_scrollbar};

const SCANNING_DURATION_MS: u128 = 1000;
const SCANNER_SPINNER: &[char] = &[
    '\u{280B}', '\u{2819}', '\u{2839}', '\u{2838}', '\u{283C}', '\u{2834}', '\u{2826}', '\u{2827}',
    '\u{2807}', '\u{280F}',
];

/// Widest the package column may be. Longer names are cut with `…`.
pub const PKG_COL_MAX: usize = 32;
/// Widest the group column may be. Longer names are cut with `…`.
pub const GROUP_COL_MAX: usize = 16;

pub struct FlatItem {
    pub group_name: String,
    pub package_name: String,
    pub selected: bool,
    pub status: StowStatus,
    pub preview: Vec<String>,
    /// The package name as drawn, cut to the package column. Set by `align_columns`.
    pub package_display: String,
    /// The group name as drawn, cut to the group column. Set by `align_columns`.
    pub group_display: String,
    /// Spaces after the package name so the group column lines up. Set by `align_columns`.
    pub package_pad: usize,
    /// Spaces after the group name so the status column lines up. Set by `align_columns`.
    pub group_pad: usize,
}

/// A column's width: its longest name, up to `max`.
fn column_width<'a>(names: impl Iterator<Item = &'a str>, max: usize) -> usize {
    names.map(|name| name.width()).max().unwrap_or(0).min(max)
}

/// Sizes the package and group columns, cuts over-wide names with `…`, and pads each row
/// so every column starts at the same place. Called when the rows are built, not on paint.
pub fn align_columns(items: &mut [FlatItem]) {
    let package_col = column_width(items.iter().map(|i| i.package_name.as_str()), PKG_COL_MAX);
    let group_col = column_width(items.iter().map(|i| i.group_name.as_str()), GROUP_COL_MAX);
    for item in items.iter_mut() {
        item.package_display = elide(&item.package_name, package_col);
        item.group_display = elide(&item.group_name, group_col);
        item.package_pad = package_col - item.package_display.width();
        item.group_pad = group_col - item.group_display.width();
    }
}

/// Columns left for the detail on a row `width` columns wide. The detail follows the
/// gutter, the glyphs, the package and group columns and the status word.
fn detail_room(item: &FlatItem, glyph_width: usize, width: usize) -> usize {
    let package_col = item.package_display.width() + item.package_pad;
    let group_col = item.group_display.width() + item.group_pad;
    let used = 1 + glyph_width + package_col + 2 + group_col + 2 + STATUS_WORD_WIDTH + 2;
    width.saturating_sub(used)
}

#[allow(clippy::too_many_arguments)]
pub fn render_package_select(
    frame: &mut Frame,
    area: Rect,
    colors: &ThemeColors,
    icons: &Icons,
    title: &str,
    flat_items: &[FlatItem],
    cursor: usize,
    entered_at: Option<std::time::Instant>,
    spinner_frame: usize,
) {
    let is_scanning = entered_at
        .map(|t| t.elapsed().as_millis() < SCANNING_DURATION_MS)
        .unwrap_or(false);

    if is_scanning {
        let ch = SCANNER_SPINNER[spinner_frame % SCANNER_SPINNER.len()];
        let text = format!("{} Scanning packages...", ch);
        let paragraph = Paragraph::new(text).style(Style::default().fg(colors.accent));
        frame.render_widget(paragraph, area);
        return;
    }

    let total = flat_items.len();

    if total == 0 {
        let paragraph = Paragraph::new("No packages found in selected groups.")
            .style(Style::default().fg(colors.text_dim));
        frame.render_widget(paragraph, area);
        return;
    }

    let (list_area, legend_area) = status_layout(area);

    let cursor = cursor.min(total.saturating_sub(1));

    let page_size = list_area.height.saturating_sub(2).max(1) as usize;
    // The panel borders take one column on each side.
    let width = list_area.width.saturating_sub(2);

    let items: Vec<ListItem> = flat_items
        .iter()
        .enumerate()
        .map(|(i, item)| {
            let is_cursor = i == cursor;
            let status_fg = status_color(colors, item.status);
            let mark_fg = if item.selected {
                colors.ok
            } else {
                colors.text_dim
            };

            // Checkbox and status icon are glyphs: they keep their colour on the cursor row.
            let glyphs = vec![
                Span::styled(
                    format!("{} ", icons.checkbox(item.selected)),
                    Style::default().fg(mark_fg),
                ),
                Span::styled(
                    format!("{} ", icons.status(item.status)),
                    Style::default().fg(status_fg),
                ),
            ];

            let package_style = if is_cursor {
                Style::default()
                    .fg(colors.text)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(colors.text)
            };
            let glyph_width: usize = glyphs.iter().map(Span::width).sum();
            let word = status_word(item.status);
            let mut content = vec![
                Span::styled(
                    format!("{}{}  ", item.package_display, " ".repeat(item.package_pad)),
                    package_style,
                ),
                Span::styled(
                    format!("{}{}  ", item.group_display, " ".repeat(item.group_pad)),
                    Style::default().fg(colors.text_dim),
                ),
                Span::styled(word.clone(), Style::default().fg(status_fg)),
            ];
            let room = detail_room(item, glyph_width, width as usize);
            if !item.preview.is_empty() && room >= 2 {
                // Pad the word to the longest one so the detail column lines up.
                let word_pad = STATUS_WORD_WIDTH.saturating_sub(word.width());
                let detail = elide(&format!("({})", item.preview.join(", ")), room);
                content.push(Span::styled(
                    format!("{}  {detail}", " ".repeat(word_pad)),
                    Style::default().fg(colors.text_dim),
                ));
            }

            ListItem::new(selection::row_with_glyphs(
                glyphs, content, is_cursor, colors, width,
            ))
        })
        .collect();

    let list = List::new(items).block(crate::ui::themed_panel(colors).title(title));
    let mut list_state = ListState::default();
    list_state.select(Some(cursor));
    frame.render_stateful_widget(list, list_area, &mut list_state);

    render_scrollbar(
        frame,
        border_track(list_area),
        colors,
        total,
        page_size,
        cursor,
    );

    if let Some(legend_rect) = legend_area {
        render_legend(frame, legend_rect, colors, icons);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn item(group: &str, package: &str) -> FlatItem {
        FlatItem {
            group_name: group.to_string(),
            package_name: package.to_string(),
            selected: false,
            status: StowStatus::Unstowed,
            preview: Vec::new(),
            package_display: String::new(),
            group_display: String::new(),
            package_pad: 0,
            group_pad: 0,
        }
    }

    #[test]
    fn test_align_columns_pads_each_column_to_its_longest_name() {
        let mut items = vec![item("home", "test"), item("workstation", "dotfiles-extra")];
        align_columns(&mut items);
        // Packages are 4 and 14 columns wide, groups 4 and 11.
        assert_eq!(items[0].package_pad, 10);
        assert_eq!(items[0].group_pad, 7);
        assert_eq!(items[1].package_pad, 0);
        assert_eq!(items[1].group_pad, 0);
        assert_eq!(items[0].package_display, "test");
        assert_eq!(items[1].group_display, "workstation");
    }

    #[test]
    fn test_align_columns_cuts_a_60_column_package_to_the_cap() {
        let long = "a".repeat(60);
        let mut items = vec![item("home", &long), item("home", "test")];
        align_columns(&mut items);
        assert_eq!(
            items[0].package_display,
            format!("{}…", "a".repeat(PKG_COL_MAX - 1))
        );
        assert_eq!(items[0].package_display.width(), PKG_COL_MAX);
        assert_eq!(items[0].package_pad, 0);
        // The short row is padded to the capped width, not to the 60-column name.
        assert_eq!(items[1].package_display, "test");
        assert_eq!(items[1].package_pad, PKG_COL_MAX - 4);
    }

    #[test]
    fn test_align_columns_cuts_a_long_group_to_its_cap() {
        let long = "g".repeat(20);
        let mut items = vec![item(&long, "x"), item("home", "y")];
        align_columns(&mut items);
        assert_eq!(items[0].group_display.width(), GROUP_COL_MAX);
        assert!(items[0].group_display.ends_with('…'));
        assert_eq!(items[1].group_pad, GROUP_COL_MAX - 4);
    }

    #[test]
    fn test_align_columns_counts_wide_characters_by_column() {
        // "漢字" is two wide characters, four columns.
        let mut items = vec![item("home", "漢字"), item("home", "ab")];
        align_columns(&mut items);
        assert_eq!(items[0].package_pad, 0);
        assert_eq!(items[1].package_pad, 2);
    }

    #[test]
    fn test_align_columns_never_splits_a_wide_character_at_the_cap() {
        // Twenty wide characters are forty columns. Cut to 32 with `…` needs 31 columns:
        // fifteen characters (30 columns) and the ellipsis. The pad makes up the last one.
        let cjk = "漢".repeat(20);
        let mut items = vec![item("home", &cjk), item("home", "ab")];
        align_columns(&mut items);
        assert_eq!(items[0].package_display, format!("{}…", "漢".repeat(15)));
        assert_eq!(items[0].package_display.width(), 31);
        assert_eq!(items[0].package_pad, 1);
        for row in &items {
            assert_eq!(row.package_display.width() + row.package_pad, PKG_COL_MAX);
        }
    }

    #[test]
    fn test_align_columns_accepts_no_rows() {
        let mut items: Vec<FlatItem> = Vec::new();
        align_columns(&mut items);
        assert!(items.is_empty());
    }

    #[test]
    fn test_detail_room_is_what_the_columns_leave_on_the_row() {
        // Glyphs are the checkbox and the status icon, two columns each with its space.
        let mut items = vec![item("home", "test"), item("workstation", "dotfiles-extra")];
        align_columns(&mut items);
        // Gutter 1 + glyphs 4 + package 14 + 2 + group 11 + 2 + status word 8 + 2 = 44.
        assert_eq!(detail_room(&items[1], 4, 80), 36);
        // The short package's row has the same detail column, so the same room.
        assert_eq!(detail_room(&items[0], 4, 80), 36);
    }

    #[test]
    fn test_detail_room_saturates_on_a_narrow_row() {
        let mut items = vec![item("home", "test")];
        align_columns(&mut items);
        assert_eq!(detail_room(&items[0], 4, 10), 0);
    }
}
