//! The stow status vocabulary shared by every screen that lists packages. The glyph
//! comes from `Icons::status`; this module supplies the colour, the word and the legend,
//! so the same status reads the same on the package select and the status screens.

use ratatui::{
    layout::Rect,
    style::{Color, Style},
    text::{Line, Span},
    widgets::{List, ListItem},
    Frame,
};

use andre_core::StowStatus;

use crate::theme::ThemeColors;
use crate::ui::icons::Icons;

/// Every status in legend order, with the meaning shown beside it.
const LEGEND: [(StowStatus, &str); 6] = [
    (StowStatus::Stowed, "all files symlinked correctly"),
    (StowStatus::Unstowed, "package exists but no symlinks"),
    (StowStatus::Partial, "some files symlinked, some not"),
    (
        StowStatus::Conflict,
        "real file at target, cannot overwrite",
    ),
    (StowStatus::Missing, "package source directory not found"),
    (StowStatus::Empty, "package contains no files"),
];

/// The legend's rows plus its two border rows.
pub const LEGEND_HEIGHT: u16 = LEGEND.len() as u16 + 2;

/// The width of the longest status word (`unstowed` and `conflict`). Checked against the
/// words by `test_status_word_width_is_the_longest_word`.
pub const STATUS_WORD_WIDTH: usize = 8;

/// The colour of a status's glyph and word.
pub fn status_color(colors: &ThemeColors, status: StowStatus) -> Color {
    match status {
        StowStatus::Stowed => colors.ok,
        StowStatus::Unstowed => colors.accent,
        StowStatus::Partial => colors.warn,
        StowStatus::Conflict => colors.err,
        StowStatus::Missing | StowStatus::Empty => colors.text_dim,
    }
}

/// The status word shown beside a package: the core label, in lowercase.
pub fn status_word(status: StowStatus) -> String {
    status.label().to_ascii_lowercase()
}

/// Draws the legend: one row per status, with its glyph, word and meaning.
pub fn render_legend(frame: &mut Frame, area: Rect, colors: &ThemeColors, icons: &Icons) {
    let items: Vec<ListItem> = LEGEND
        .iter()
        .map(|&(status, meaning)| {
            let fg = status_color(colors, status);
            ListItem::new(Line::from(vec![
                Span::styled(
                    format!("  {}  ", icons.status(status)),
                    Style::default().fg(fg),
                ),
                Span::styled(
                    format!("{:<8}  ", status_word(status)),
                    Style::default().fg(fg),
                ),
                Span::styled(meaning, Style::default().fg(colors.text_dim)),
            ]))
        })
        .collect();

    let list = List::new(items).block(crate::ui::themed_panel(colors).title(" legend "));
    frame.render_widget(list, area);
}

#[cfg(test)]
mod tests {
    use andre_core::Theme;
    use unicode_width::UnicodeWidthStr;

    use super::*;

    #[test]
    fn test_status_word_is_the_core_label_in_lowercase() {
        assert_eq!(status_word(StowStatus::Stowed), "stowed");
        assert_eq!(status_word(StowStatus::Unstowed), "unstowed");
        assert_eq!(status_word(StowStatus::Conflict), "conflict");
        assert_eq!(status_word(StowStatus::Empty), "empty");
    }

    #[test]
    fn test_status_colours_are_one_table_for_every_theme() {
        for theme in Theme::ALL {
            let colors = ThemeColors::from_theme(theme);
            assert_eq!(status_color(&colors, StowStatus::Stowed), colors.ok);
            assert_eq!(status_color(&colors, StowStatus::Unstowed), colors.accent);
            assert_eq!(status_color(&colors, StowStatus::Partial), colors.warn);
            assert_eq!(status_color(&colors, StowStatus::Conflict), colors.err);
            assert_eq!(status_color(&colors, StowStatus::Missing), colors.text_dim);
            assert_eq!(status_color(&colors, StowStatus::Empty), colors.text_dim);
        }
    }

    #[test]
    fn test_legend_lists_every_status_once_in_order() {
        let statuses: Vec<StowStatus> = LEGEND.iter().map(|(s, _)| *s).collect();
        assert_eq!(
            statuses,
            [
                StowStatus::Stowed,
                StowStatus::Unstowed,
                StowStatus::Partial,
                StowStatus::Conflict,
                StowStatus::Missing,
                StowStatus::Empty,
            ]
        );
    }

    #[test]
    fn test_legend_height_is_six_rows_and_two_borders() {
        assert_eq!(LEGEND_HEIGHT, 8);
    }

    #[test]
    fn test_status_word_width_is_the_longest_word() {
        let longest = LEGEND
            .iter()
            .map(|(status, _)| status_word(*status).width())
            .max()
            .unwrap();
        assert_eq!(STATUS_WORD_WIDTH, longest);
    }
}
