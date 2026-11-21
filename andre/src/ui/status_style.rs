//! The stow status vocabulary shared by every screen that lists packages. The glyph
//! comes from `Icons::status`; this module supplies the colour, the word, the meaning and
//! the detail headline, so the same status reads the same on the packages browser and the
//! status screen.

use ratatui::{
    style::{Color, Style},
    text::Line,
};

use andre_core::StowStatus;

use crate::theme::ThemeColors;
use crate::ui::icons::Icons;

/// The width of the longest status word (`unstowed` and `conflict`). Checked against the
/// words by `test_status_word_width_is_the_longest_word`.
pub const STATUS_WORD_WIDTH: usize = 8;

/// The statuses in the order a summary lists them: linked first, then the ones that need
/// a fix.
pub const SUMMARY_ORDER: [StowStatus; 7] = [
    StowStatus::Stowed,
    StowStatus::Partial,
    StowStatus::Unstowed,
    StowStatus::Conflict,
    StowStatus::Missing,
    StowStatus::Empty,
    StowStatus::Invalid,
];

/// The colour of a status's glyph and word.
pub fn status_color(colors: &ThemeColors, status: StowStatus) -> Color {
    match status {
        StowStatus::Stowed => colors.ok,
        StowStatus::Unstowed => colors.text,
        StowStatus::Partial => colors.warn,
        StowStatus::Conflict => colors.err,
        StowStatus::Missing | StowStatus::Empty => colors.text_dim,
        StowStatus::Invalid => colors.err,
    }
}

/// The status word shown beside a package: the core label, in lowercase.
pub fn status_word(status: StowStatus) -> String {
    status.label().to_ascii_lowercase()
}

/// What a status means, in a few words.
pub fn status_meaning(status: StowStatus) -> &'static str {
    match status {
        StowStatus::Stowed => "stowed \u{2014} every file is linked",
        StowStatus::Unstowed => "not stowed \u{2014} nothing is linked",
        StowStatus::Partial => "partial \u{2014} some files linked",
        StowStatus::Conflict => "conflict \u{2014} a real file is in place",
        StowStatus::Missing => "missing \u{2014} no such package folder",
        StowStatus::Empty => "empty \u{2014} the package has no files",
        StowStatus::Invalid => "invalid \u{2014} ignore list unreadable",
    }
}

/// The headline of a package's detail: its status glyph, then what the status means. An
/// unread status shows `·` and says so.
pub fn status_headline(
    status: Option<StowStatus>,
    icons: &Icons,
    colors: &ThemeColors,
) -> Line<'static> {
    let (glyph, text, colour) = match status {
        Some(status) => (
            icons.status(status),
            status_meaning(status),
            status_color(colors, status),
        ),
        None => (icons.unknown, "status not read yet", colors.text_dim),
    };
    Line::styled(format!("{glyph} {text}"), Style::default().fg(colour))
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
        assert_eq!(status_word(StowStatus::Invalid), "invalid");
    }

    #[test]
    fn test_status_colours_are_one_table_for_every_theme() {
        for theme in Theme::ALL {
            let colors = ThemeColors::from_theme(theme);
            assert_eq!(status_color(&colors, StowStatus::Stowed), colors.ok);
            assert_eq!(status_color(&colors, StowStatus::Unstowed), colors.text);
            assert_eq!(status_color(&colors, StowStatus::Partial), colors.warn);
            assert_eq!(status_color(&colors, StowStatus::Conflict), colors.err);
            assert_eq!(status_color(&colors, StowStatus::Missing), colors.text_dim);
            assert_eq!(status_color(&colors, StowStatus::Empty), colors.text_dim);
            assert_eq!(status_color(&colors, StowStatus::Invalid), colors.err);
        }
    }

    #[test]
    fn test_unstowed_and_partial_are_distinct_colours_in_every_theme() {
        // Text and warn can share a luminance (Mocha: about 1.1:1), so the colours
        // must differ in hue as well as brightness.
        for theme in Theme::ALL {
            let colors = ThemeColors::from_theme(theme);
            assert_ne!(
                status_color(&colors, StowStatus::Unstowed),
                status_color(&colors, StowStatus::Partial),
                "{}",
                theme.name()
            );
        }
    }

    #[test]
    fn test_summary_order_lists_every_status_once() {
        let mut statuses = SUMMARY_ORDER.to_vec();
        statuses.sort_by_key(|status| status.label());
        statuses.dedup();
        assert_eq!(statuses.len(), 7);
    }

    #[test]
    fn test_status_word_width_is_the_longest_word() {
        let longest = SUMMARY_ORDER
            .iter()
            .map(|status| status_word(*status).width())
            .max()
            .unwrap();
        assert_eq!(STATUS_WORD_WIDTH, longest);
    }
}
