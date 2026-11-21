use ratatui::{
    layout::Rect,
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::Paragraph,
    Frame,
};
use unicode_width::{UnicodeWidthChar, UnicodeWidthStr};

use crate::theme::ThemeColors;

/// Columns between the hints and the right-hand side, and between the theme
/// chip and `q quit`.
const GAP: usize = 2;

fn key_span(s: &str, colors: &ThemeColors) -> Span<'static> {
    Span::styled(s.to_string(), Style::default().fg(colors.indicator))
}

/// The bottom bar: the current screen's key hints on the left, then the theme
/// chip and `q quit` at the right edge. One row; the global toggles live in the
/// top bar.
pub fn render_footer(
    frame: &mut Frame,
    area: Rect,
    colors: &ThemeColors,
    screen_id: &str,
    theme_name: &str,
) {
    if area.width == 0 || area.height == 0 {
        return;
    }
    let line = footer_line(colors, screen_id, theme_name, area.width as usize);
    frame.render_widget(
        Paragraph::new(line).style(Style::default().fg(colors.muted)),
        area,
    );
}

/// The bottom bar as one line, `width` columns wide. The theme chip shows its
/// full name when the hints leave room for it, then just `theme`, and is
/// dropped when not even that fits. `q quit` is always kept: the hints are
/// clipped to make room for it.
pub fn footer_line(
    colors: &ThemeColors,
    screen_id: &str,
    theme_name: &str,
    width: usize,
) -> Line<'static> {
    let hints = screen_hints(screen_id, colors);
    let quit = quit_spans(colors);
    let hints_width = spans_width(&hints);

    for theme in [
        theme_chip(colors, Some(theme_name)),
        theme_chip(colors, None),
    ] {
        let right_width = spans_width(&theme) + GAP + spans_width(&quit);
        if hints_width + GAP + right_width <= width {
            let mut right = theme;
            right.push(Span::raw(" ".repeat(GAP)));
            right.extend(quit);
            return join(hints, right, width);
        }
    }

    let room = width.saturating_sub(GAP + spans_width(&quit));
    join(clip(hints, room), quit, width)
}

/// `left`, then spaces, then `right`, filling `width` columns.
fn join(mut left: Vec<Span<'static>>, right: Vec<Span<'static>>, width: usize) -> Line<'static> {
    let gap = width.saturating_sub(spans_width(&left) + spans_width(&right));
    left.push(Span::raw(" ".repeat(gap)));
    left.extend(right);
    Line::from(left)
}

/// `t` in the indicator colour and bold, `heme` muted, then the theme name in
/// the highlight colour when there is room for it.
fn theme_chip(colors: &ThemeColors, name: Option<&str>) -> Vec<Span<'static>> {
    let mut spans = vec![
        Span::styled(
            "t",
            Style::default()
                .fg(colors.indicator)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled("heme", Style::default().fg(colors.muted)),
    ];
    if let Some(name) = name {
        spans.push(Span::raw(" "));
        spans.push(Span::styled(
            name.to_string(),
            Style::default().fg(colors.highlight),
        ));
    }
    spans
}

fn quit_spans(colors: &ThemeColors) -> Vec<Span<'static>> {
    vec![
        key_span("q", colors),
        Span::styled(" quit", Style::default().fg(colors.muted)),
    ]
}

fn spans_width(spans: &[Span<'static>]) -> usize {
    spans
        .iter()
        .map(|span| UnicodeWidthStr::width(span.content.as_ref()))
        .sum()
}

/// The leading `max` columns of `spans`, cutting the span that crosses the edge.
fn clip(spans: Vec<Span<'static>>, max: usize) -> Vec<Span<'static>> {
    let mut out = Vec::new();
    let mut used = 0;
    for span in spans {
        let width = UnicodeWidthStr::width(span.content.as_ref());
        if used + width <= max {
            used += width;
            out.push(span);
            continue;
        }
        let mut kept = String::new();
        for c in span.content.chars() {
            let char_width = UnicodeWidthChar::width(c).unwrap_or(0);
            if used + char_width > max {
                break;
            }
            used += char_width;
            kept.push(c);
        }
        if !kept.is_empty() {
            out.push(Span::styled(kept, span.style));
        }
        break;
    }
    out
}

/// Key hints for one screen, as spans. Keys are drawn in the indicator colour.
pub fn screen_hints(screen_id: &str, colors: &ThemeColors) -> Vec<Span<'static>> {
    let arrows = key_span("\u{2191}\u{2193}", colors);
    let enter = key_span("Enter", colors);
    let space = key_span("Space", colors);
    let esc = key_span("Esc", colors);
    let ky = key_span("y", colors);
    let kn = key_span("n", colors);

    match screen_id {
        "MainMenu" => vec![
            arrows,
            Span::raw(" navigate  "),
            key_span("1\u{2013}7", colors),
            Span::raw(" jump  "),
            enter,
            Span::raw(" select"),
        ],
        "GroupSelect" | "PackageSelect" => vec![
            arrows,
            Span::raw(" navigate  "),
            space,
            Span::raw(" toggle  "),
            enter,
            Span::raw(" confirm  "),
            esc,
            Span::raw(" cancel"),
        ],
        "Status" | "Execute" => vec![enter, Span::raw(" continue")],
        "Help" => vec![esc, Span::raw(" back")],
        "Settings" => vec![
            arrows,
            Span::raw(" navigate  "),
            enter,
            Span::raw(" group menu  "),
            esc,
            Span::raw(" back"),
        ],
        "Confirm" => vec![ky, Span::raw(" confirm  "), kn, Span::raw(" cancel")],
        "AdoptTargetSelect" | "UnstowTargetSelect" => vec![
            arrows,
            Span::raw(" navigate  "),
            enter,
            Span::raw(" select  "),
            esc,
            Span::raw(" back"),
        ],
        "AdoptFileSelect" => vec![
            arrows,
            Span::raw(" navigate  "),
            space,
            Span::raw(" toggle  "),
            key_span("\u{2192}", colors),
            Span::raw(" into  "),
            enter,
            Span::raw(" confirm  "),
            key_span("\u{2190}", colors),
            Span::raw(" up  "),
            esc,
            Span::raw(" back"),
        ],
        "AdoptConfirm" => vec![ky, Span::raw(" proceed  "), kn, Span::raw(" back")],
        "AdoptNamePrompt" => vec![enter, Span::raw(" confirm  "), esc, Span::raw(" back")],
        "AdoptPreview" => vec![ky, Span::raw(" execute  "), kn, Span::raw(" cancel")],
        "UnstowBrowse" => vec![
            arrows,
            Span::raw(" navigate  "),
            key_span("\u{2192}", colors),
            Span::raw(" into  "),
            enter,
            Span::raw(" unlink  "),
            key_span("\u{2190}", colors),
            Span::raw(" up  "),
            esc,
            Span::raw(" back"),
        ],
        "UnstowConfirm" => vec![ky, Span::raw(" remove  "), kn, Span::raw(" cancel")],
        _ => vec![Span::raw("")],
    }
}

#[cfg(test)]
mod tests {
    use super::{footer_line, screen_hints};
    use crate::theme::ThemeColors;
    use andre_core::Theme;
    use ratatui::{
        style::Modifier,
        text::{Line, Span},
    };
    use unicode_width::UnicodeWidthStr;

    /// Inner width at the 80-column minimum: border and margin take four.
    const INNER_WIDTH: usize = 76;

    const SCREENS: [&str; 16] = [
        "MainMenu",
        "GroupSelect",
        "PackageSelect",
        "Settings",
        "Status",
        "Execute",
        "Help",
        "Confirm",
        "AdoptTargetSelect",
        "AdoptFileSelect",
        "AdoptConfirm",
        "AdoptNamePrompt",
        "AdoptPreview",
        "UnstowTargetSelect",
        "UnstowBrowse",
        "UnstowConfirm",
    ];

    fn colors() -> ThemeColors {
        ThemeColors::from_theme(Theme::Default)
    }

    fn text(spans: &[Span]) -> String {
        spans.iter().map(|s| s.content.as_ref()).collect()
    }

    fn line_text(line: &Line) -> String {
        text(&line.spans)
    }

    fn line_width(line: &Line) -> usize {
        line.spans
            .iter()
            .map(|s| UnicodeWidthStr::width(s.content.as_ref()))
            .sum()
    }

    /// The span whose text is exactly `content`.
    fn span<'a>(line: &'a Line<'a>, content: &str) -> &'a Span<'a> {
        line.spans
            .iter()
            .find(|s| s.content.as_ref() == content)
            .unwrap_or_else(|| panic!("no span {content:?} in {:?}", line_text(line)))
    }

    /// Columns the hints take on `screen`.
    fn hints_width(screen: &str) -> usize {
        text(&screen_hints(screen, &colors())).width()
    }

    /// Smallest width at which the full `theme <name>` chip fits on `screen`.
    fn full_chip_width(screen: &str, theme: Theme) -> usize {
        // hints, GAP, "theme <name>", GAP, "q quit"
        hints_width(screen) + 2 + format!("theme {}", theme.name()).width() + 2 + "q quit".width()
    }

    /// Smallest width at which the bare `theme` word fits on `screen`.
    fn stub_chip_width(screen: &str) -> usize {
        hints_width(screen) + 2 + "theme".width() + 2 + "q quit".width()
    }

    #[test]
    fn test_main_menu_hints_mention_jump_and_select() {
        let hints = text(&screen_hints("MainMenu", &colors()));
        assert_eq!(
            hints,
            "\u{2191}\u{2193} navigate  1\u{2013}7 jump  Enter select"
        );
    }

    #[test]
    fn test_key_names_use_indicator_colour() {
        let spans = screen_hints("Settings", &colors());
        let enter = spans.iter().find(|s| s.content == "Enter").unwrap();
        assert_eq!(enter.style.fg, Some(colors().indicator));
    }

    #[test]
    fn test_every_screen_hint_fits_at_80_columns_with_quit_and_gap() {
        // The inner width less `q quit` and the two-column gap before it.
        let budget = INNER_WIDTH - "q quit".len() - 2;
        for screen in SCREENS {
            let hints = hints_width(screen);
            assert!(
                hints <= budget,
                "{screen} hints are {hints} columns, budget is {budget}"
            );
        }
    }

    #[test]
    fn test_unknown_screen_has_no_hints() {
        assert_eq!(text(&screen_hints("Nowhere", &colors())), "");
    }

    #[test]
    fn test_main_menu_shows_theme_chip_for_every_theme_at_80_columns() {
        for theme in Theme::ALL {
            let line = footer_line(
                &ThemeColors::from_theme(theme),
                "MainMenu",
                theme.name(),
                INNER_WIDTH,
            );
            let t = line_text(&line);
            assert!(
                t.contains(&format!("theme {}", theme.name())),
                "{}: theme chip missing: {t:?}",
                theme.name()
            );
            assert!(t.ends_with("q quit"), "{}: {t:?}", theme.name());
            assert_eq!(line_width(&line), INNER_WIDTH, "{}: {t:?}", theme.name());
        }
    }

    #[test]
    fn test_theme_chip_colours_are_pinned() {
        let line = footer_line(&colors(), "MainMenu", "nord", INNER_WIDTH);
        let t = span(&line, "t");
        assert_eq!(t.style.fg, Some(colors().indicator));
        assert!(t.style.add_modifier.contains(Modifier::BOLD));
        assert_eq!(span(&line, "heme").style.fg, Some(colors().muted));
        assert_eq!(span(&line, "nord").style.fg, Some(colors().highlight));
    }

    #[test]
    fn test_quit_key_is_indicator_and_label_is_muted() {
        let line = footer_line(&colors(), "MainMenu", "nord", INNER_WIDTH);
        assert_eq!(span(&line, "q").style.fg, Some(colors().indicator));
        assert_eq!(span(&line, " quit").style.fg, Some(colors().muted));
    }

    #[test]
    fn test_theme_chip_gap_is_exactly_two_columns_when_tight() {
        let screen = "MainMenu";
        let width = full_chip_width(screen, Theme::CatppuccinMacchiato);
        let line = footer_line(
            &ThemeColors::from_theme(Theme::CatppuccinMacchiato),
            screen,
            "catppuccin-macchiato",
            width,
        );
        assert_eq!(
            line_text(&line),
            "\u{2191}\u{2193} navigate  1\u{2013}7 jump  Enter select  theme catppuccin-macchiato  q quit"
        );
    }

    #[test]
    fn test_theme_name_drops_to_the_word_theme_when_the_name_does_not_fit() {
        // UnstowBrowse is long enough that the full name does not fit at 76
        // columns, but the bare word does.
        let screen = "UnstowBrowse";
        let theme = Theme::CatppuccinMacchiato;
        let colors = ThemeColors::from_theme(theme);
        assert!(full_chip_width(screen, theme) > INNER_WIDTH);
        assert!(stub_chip_width(screen) <= INNER_WIDTH);

        let line = footer_line(&colors, screen, theme.name(), INNER_WIDTH);
        let t = line_text(&line);
        assert!(t.contains("theme"), "word stub expected: {t:?}");
        assert!(!t.contains("catppuccin"), "name must be dropped: {t:?}");
        assert_eq!(span(&line, "t").style.fg, Some(colors.indicator));
        assert_eq!(span(&line, "heme").style.fg, Some(colors.muted));

        // At exactly the width the full chip needs, the name is back; one
        // column less drops it.
        let wide = footer_line(
            &colors,
            screen,
            theme.name(),
            full_chip_width(screen, theme),
        );
        assert!(line_text(&wide).contains("theme catppuccin-macchiato"));
        let short = footer_line(
            &colors,
            screen,
            theme.name(),
            full_chip_width(screen, theme) - 1,
        );
        assert!(!line_text(&short).contains("catppuccin"));
    }

    #[test]
    fn test_longest_hint_screen_drops_theme_chip_at_80_columns() {
        // AdoptFileSelect has the longest hints. Neither the name nor the word
        // fits beside them at 76 columns, so the chip goes and `q quit` stays.
        let longest = SCREENS
            .into_iter()
            .max_by_key(|screen| hints_width(screen))
            .unwrap();
        assert_eq!(longest, "AdoptFileSelect");
        assert!(stub_chip_width(longest) > INNER_WIDTH);
        for theme in Theme::ALL {
            let line = footer_line(
                &ThemeColors::from_theme(theme),
                longest,
                theme.name(),
                INNER_WIDTH,
            );
            let t = line_text(&line);
            assert!(
                !t.contains("theme"),
                "{}: chip must be dropped: {t:?}",
                theme.name()
            );
            assert!(t.ends_with("q quit"), "{}: {t:?}", theme.name());
            assert_eq!(line_width(&line), INNER_WIDTH);
        }
    }

    #[test]
    fn test_every_screen_keeps_quit_at_the_right_edge_for_every_theme() {
        for screen in SCREENS {
            for theme in Theme::ALL {
                let line = footer_line(
                    &ThemeColors::from_theme(theme),
                    screen,
                    theme.name(),
                    INNER_WIDTH,
                );
                assert!(
                    line_text(&line).ends_with("q quit"),
                    "{screen}/{}: {:?}",
                    theme.name(),
                    line_text(&line)
                );
            }
        }
    }

    #[test]
    fn test_footer_clips_hints_so_quit_is_still_shown_when_very_narrow() {
        for width in [0, 1, 5, 6, 8, 20, 30] {
            let line = footer_line(&colors(), "AdoptFileSelect", "default", width);
            assert!(
                line_width(&line) <= width.max(6),
                "width {width}: {:?}",
                line_text(&line)
            );
        }
        let line = footer_line(&colors(), "AdoptFileSelect", "default", 30);
        let t = line_text(&line);
        assert!(t.ends_with("q quit"), "{t:?}");
        assert_eq!(line_width(&line), 30);
        assert!(!t.contains("theme"), "no room for the chip: {t:?}");
    }
}
