//! The cursor style shared by every list screen. The cursor row is filled edge to edge
//! with the theme's `selection` colour and starts with a `▌` accent in the primary
//! colour, which sits on the normal background. Unselected rows get the same one-column
//! gutter, so text lines up down the list. No screen draws its own cursor marker.
//!
//! On the selected row, text takes the label colour (secondary), so it reads in every
//! theme. A single-glyph mark (a checkbox or a status icon) keeps its own colour when
//! that colour reaches 3:1 against the fill, the WCAG 1.4.11 minimum for non-text
//! graphics. Where it does not, the glyph falls back to the label colour and the glyph
//! shape alone tells the states apart.

use ratatui::{
    style::Style,
    text::{Line, Span},
};

use crate::theme::{contrast_ratio, ThemeColors};

/// The accent bar that marks the cursor row.
pub const ACCENT: &str = "\u{258c}";

/// WCAG 1.4.11 minimum contrast for a glyph against its background.
pub const MIN_GLYPH_CONTRAST: f64 = 3.0;

/// Builds one list row from its content. The row is padded to `width` columns so the
/// selected fill reaches the right edge.
pub fn row(
    content: Vec<Span<'static>>,
    selected: bool,
    colors: &ThemeColors,
    width: u16,
) -> Line<'static> {
    row_with_glyphs(Vec::new(), content, selected, colors, width)
}

/// Like [`row`], with single-glyph marks drawn before `content`. Glyph spans keep their
/// colour on the cursor row when it meets [`MIN_GLYPH_CONTRAST`] against the fill.
pub fn row_with_glyphs(
    glyphs: Vec<Span<'static>>,
    content: Vec<Span<'static>>,
    selected: bool,
    colors: &ThemeColors,
    width: u16,
) -> Line<'static> {
    let mut spans = Vec::with_capacity(glyphs.len() + content.len() + 2);
    if selected {
        let fill = Style::default().bg(colors.selection);
        spans.push(Span::styled(ACCENT, Style::default().fg(colors.primary)));
        for glyph in glyphs {
            spans.push(glyph_on_fill(glyph, colors).patch_style(fill));
        }
        for span in content {
            spans.push(text_on_fill(span, colors).patch_style(fill));
        }
    } else {
        spans.push(Span::raw(" "));
        spans.extend(glyphs);
        spans.extend(content);
    }

    let used: usize = spans.iter().map(Span::width).sum();
    let pad = " ".repeat((width as usize).saturating_sub(used));
    if selected {
        spans.push(Span::styled(pad, Style::default().bg(colors.selection)));
    } else {
        spans.push(Span::raw(pad));
    }
    Line::from(spans)
}

/// Text on the cursor row keeps the accent colour and takes the label colour otherwise.
/// Modifiers such as bold are kept.
fn text_on_fill(span: Span<'static>, colors: &ThemeColors) -> Span<'static> {
    if span.style.fg == Some(colors.primary) {
        span
    } else {
        Span::styled(span.content, span.style.fg(colors.secondary))
    }
}

/// A glyph keeps its colour on the cursor row when the colour reads at
/// [`MIN_GLYPH_CONTRAST`] against the fill, and takes the label colour otherwise.
fn glyph_on_fill(span: Span<'static>, colors: &ThemeColors) -> Span<'static> {
    let readable = span.style.fg.is_some_and(|fg| {
        contrast_ratio(fg, colors.selection).is_some_and(|ratio| ratio >= MIN_GLYPH_CONTRAST)
    });
    if readable {
        span
    } else {
        Span::styled(span.content, span.style.fg(colors.secondary))
    }
}

#[cfg(test)]
mod tests {
    use andre_core::Theme;
    use ratatui::style::Color;

    use super::*;

    fn colors() -> ThemeColors {
        ThemeColors::from_theme(Theme::Nord)
    }

    fn text(line: &Line) -> String {
        line.spans.iter().map(|s| s.content.as_ref()).collect()
    }

    /// The glyph colours a list row can draw: checkbox on and off, and the status colours.
    fn glyph_colours(colors: &ThemeColors) -> [(&'static str, Color); 5] {
        [
            ("success", colors.success),
            ("primary", colors.primary),
            ("warning", colors.warning),
            ("error", colors.error),
            ("muted", colors.muted),
        ]
    }

    #[test]
    fn test_selected_row_starts_with_primary_accent_on_normal_background() {
        let colors = colors();
        let line = row(vec![Span::raw("alpha")], true, &colors, 10);
        let accent = &line.spans[0];
        assert_eq!(accent.content, ACCENT);
        assert_eq!(accent.style.fg, Some(colors.primary));
        assert_eq!(accent.style.bg, None, "accent is not filled");
    }

    #[test]
    fn test_selected_row_fills_every_column_with_selection_colour() {
        let colors = colors();
        let line = row(vec![Span::raw("alpha")], true, &colors, 12);
        assert_eq!(line.width(), 12);
        for span in &line.spans[1..] {
            assert_eq!(span.style.bg, Some(colors.selection), "{:?}", span.content);
        }
    }

    #[test]
    fn test_unselected_row_has_gutter_and_no_fill() {
        let colors = colors();
        let line = row(vec![Span::raw("alpha")], false, &colors, 12);
        assert_eq!(line.width(), 12);
        assert!(text(&line).starts_with(" alpha"));
        assert!(line.spans.iter().all(|s| s.style.bg.is_none()));
    }

    #[test]
    fn test_selected_row_uses_the_label_colour_for_other_text() {
        let colors = colors();
        for fg in [colors.muted, colors.success, colors.error, colors.highlight] {
            let span = Span::styled("value", Style::default().fg(fg));
            let line = row(vec![span], true, &colors, 20);
            assert_eq!(line.spans[1].style.fg, Some(colors.secondary));
            assert_eq!(line.spans[1].style.bg, Some(colors.selection));
        }
    }

    #[test]
    fn test_selected_row_keeps_the_accent_colour_on_text() {
        let colors = colors();
        let icon = Span::styled("i", Style::default().fg(colors.primary));
        let line = row(vec![icon], true, &colors, 10);
        assert_eq!(line.spans[1].style.fg, Some(colors.primary));
    }

    #[test]
    fn test_selected_row_keeps_modifiers_when_it_lifts_the_colour() {
        let colors = colors();
        let bold = Span::styled(
            "label",
            Style::default()
                .fg(colors.muted)
                .add_modifier(ratatui::style::Modifier::BOLD),
        );
        let line = row(vec![bold], true, &colors, 10);
        assert!(line.spans[1]
            .style
            .add_modifier
            .contains(ratatui::style::Modifier::BOLD));
    }

    #[test]
    fn test_unselected_row_keeps_every_span_colour() {
        let colors = colors();
        let ok = Span::styled("ON", Style::default().fg(colors.success));
        let line = row(vec![ok], false, &colors, 10);
        assert_eq!(line.spans[1].style.fg, Some(colors.success));
    }

    #[test]
    fn test_row_wider_than_width_is_not_padded() {
        let colors = colors();
        let line = row(vec![Span::raw("a long label")], true, &colors, 4);
        assert_eq!(text(&line), format!("{ACCENT}a long label"));
    }

    #[test]
    fn test_selected_row_glyph_keeps_its_colour_when_it_reads_at_3_to_1() {
        // Nord: success, primary and warning reach 3:1 on the fill and keep their colour.
        // Error and muted do not, so they fall back (see FALLBACKS).
        let colors = colors();
        for (name, fg, keeps) in [
            ("success", colors.success, true),
            ("primary", colors.primary, true),
            ("warning", colors.warning, true),
            ("error", colors.error, false),
            ("muted", colors.muted, false),
        ] {
            let glyph = Span::styled("\u{f00c}", Style::default().fg(fg));
            let line = row_with_glyphs(vec![glyph], vec![Span::raw("x")], true, &colors, 10);
            let expected = if keeps { fg } else { colors.secondary };
            assert_eq!(line.spans[1].style.fg, Some(expected), "nord {name}");
            assert_eq!(
                line.spans[1].style.bg,
                Some(colors.selection),
                "nord {name}"
            );
        }
    }

    #[test]
    fn test_selected_row_glyph_falls_back_to_label_colour_below_3_to_1() {
        // A glyph colour that cannot reach 3:1 on the fill becomes the label colour.
        let colors = colors();
        let glyph = Span::styled("x", Style::default().fg(colors.selection));
        let line = row_with_glyphs(vec![glyph], Vec::new(), true, &colors, 4);
        assert_eq!(line.spans[1].style.fg, Some(colors.secondary));
    }

    #[test]
    fn test_selected_row_glyph_without_rgb_colour_falls_back() {
        let colors = colors();
        let glyph = Span::raw("x");
        let line = row_with_glyphs(vec![glyph], Vec::new(), true, &colors, 4);
        assert_eq!(line.spans[1].style.fg, Some(colors.secondary));
    }

    #[test]
    fn test_unselected_row_glyph_keeps_every_colour() {
        let colors = colors();
        let glyph = Span::styled("x", Style::default().fg(colors.error));
        let line = row_with_glyphs(vec![glyph], Vec::new(), false, &colors, 4);
        assert_eq!(line.spans[1].style.fg, Some(colors.error));
        assert_eq!(line.spans[1].style.bg, None);
    }

    #[test]
    fn test_row_with_glyphs_places_glyphs_before_content_after_the_accent() {
        let colors = colors();
        let line = row_with_glyphs(
            vec![Span::raw("\u{25a1}")],
            vec![Span::raw("name")],
            true,
            &colors,
            10,
        );
        assert_eq!(text(&line), format!("{ACCENT}\u{25a1}name    "));
    }

    /// Every theme and glyph colour where the glyph falls back to the label colour on the
    /// cursor row. Each one reads below 3:1 against that theme's `selection` fill, so the
    /// glyph shape alone carries the state there. Listed in theme order. Default has none.
    const FALLBACKS: &[(Theme, &str)] = &[
        (Theme::Dracula, "error"),
        (Theme::Dracula, "muted"),
        (Theme::CatppuccinMocha, "muted"),
        (Theme::CatppuccinLatte, "success"),
        (Theme::CatppuccinLatte, "warning"),
        (Theme::CatppuccinLatte, "muted"),
        (Theme::CatppuccinFrappe, "error"),
        (Theme::CatppuccinFrappe, "muted"),
        (Theme::CatppuccinMacchiato, "muted"),
        (Theme::Nord, "error"),
        (Theme::Nord, "muted"),
        (Theme::Gruvbox, "success"),
        (Theme::Gruvbox, "warning"),
        (Theme::Gruvbox, "error"),
        (Theme::Gruvbox, "muted"),
    ];

    #[test]
    fn test_glyph_fallbacks_on_the_cursor_row_match_the_documented_list() {
        let mut found = Vec::new();
        for theme in Theme::ALL {
            let colors = ThemeColors::from_theme(theme);
            for (name, fg) in glyph_colours(&colors) {
                let ratio = contrast_ratio(fg, colors.selection).expect("RGB theme");
                if ratio < MIN_GLYPH_CONTRAST {
                    found.push((theme, name));
                }
            }
        }
        assert_eq!(
            found, FALLBACKS,
            "update FALLBACKS and the CHANGELOG to match"
        );
    }
}
