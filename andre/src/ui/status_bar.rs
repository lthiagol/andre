use ratatui::{
    layout::Rect,
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::Paragraph,
    Frame,
};
use unicode_width::UnicodeWidthStr;

use andre_core::{IconSet, StowFlags};

use crate::theme::ThemeColors;
use crate::ui::icons::Icons;

/// Columns between chips.
const GAP: usize = 2;

/// How much of each chip's label the top bar draws.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Labels {
    /// `✓ dry-run`: mark plus the whole label, hotkey letter coloured inside it.
    Full,
    /// `✓ d`: mark plus hotkey letter only. Used when the full labels do not fit.
    HotkeyOnly,
}

/// Short display label for a component stack id in the breadcrumb.
pub fn breadcrumb_label(id: &str) -> &str {
    match id {
        "MainMenu" => "Main",
        "GroupSelect" => "Groups",
        "PackageSelect" => "Packages",
        "Settings" => "Settings",
        "Status" => "Status",
        "Help" => "Help",
        "Confirm" => "Confirm",
        "Execute" => "Execute",
        "AdoptTargetSelect" => "Adopt",
        "AdoptFileSelect" => "Files",
        "AdoptConfirm" => "AdoptConfirm",
        "AdoptNamePrompt" => "Name",
        "AdoptPreview" => "Preview",
        "UnstowTargetSelect" => "Unstow",
        "UnstowBrowse" => "Browse",
        "UnstowConfirm" => "UnstowConfirm",
        other => other,
    }
}

/// Join stack ids into a breadcrumb string (`Main > Settings`).
pub fn format_breadcrumb(stack_ids: &[&str]) -> String {
    stack_ids
        .iter()
        .map(|id| breadcrumb_label(id))
        .collect::<Vec<_>>()
        .join(" > ")
}

pub fn render_status_bar(
    frame: &mut Frame,
    area: Rect,
    colors: &ThemeColors,
    flags: &StowFlags,
    icons: IconSet,
    debug: bool,
) {
    if area.width == 0 || area.height == 0 {
        return;
    }
    let line = top_bar_line(
        colors,
        flags,
        Icons::for_set(icons),
        debug,
        area.width as usize,
    );
    frame.render_widget(Paragraph::new(line), area);
}

/// The top bar as one line: the action pill, then a chip per toggle. The chips
/// show full labels when they fit, and hotkey letters only when they do not.
/// The full chips need up to 70 columns (`unstow`, every toggle on, debug), so
/// they fit the 76-column inner width at 80 columns. The hotkey-only chips need
/// up to 41 columns. Below that the line is simply clipped by the caller.
///
/// The chosen group (pill and chips together) is centred in `width`, with the
/// odd column, if any, on the right.
pub fn top_bar_line(
    colors: &ThemeColors,
    flags: &StowFlags,
    icons: &Icons,
    debug: bool,
    width: usize,
) -> Line<'static> {
    let full = left_chips(colors, flags, icons, debug, Labels::Full);
    let group = if spans_width(&full) <= width {
        full
    } else {
        left_chips(colors, flags, icons, debug, Labels::HotkeyOnly)
    };
    centred(group, width)
}

/// Pads `group` with spaces so it sits in the middle of `width` columns.
fn centred(group: Vec<Span<'static>>, width: usize) -> Line<'static> {
    let free = width.saturating_sub(spans_width(&group));
    let left = free / 2;
    let right = free - left;
    let mut spans = Vec::with_capacity(group.len() + 2);
    spans.push(Span::raw(" ".repeat(left)));
    spans.extend(group);
    spans.push(Span::raw(" ".repeat(right)));
    Line::from(spans)
}

fn left_chips(
    colors: &ThemeColors,
    flags: &StowFlags,
    icons: &Icons,
    debug: bool,
    labels: Labels,
) -> Vec<Span<'static>> {
    let mut chips = vec![
        verbose_chip(colors, icons, flags.verbosity, labels),
        toggle_chip(colors, icons, "dry-run", 0, flags.dry_run, labels),
        toggle_chip(colors, icons, "no-fold", 0, flags.no_folding, labels),
        toggle_chip(colors, icons, "adopt", 0, flags.adopt, labels),
        toggle_chip(colors, icons, "dotfiles", 1, flags.dotfiles, labels),
    ];
    if debug {
        chips.push(vec![Span::styled(
            "debug",
            Style::default()
                .fg(colors.warning)
                .add_modifier(Modifier::BOLD),
        )]);
    }

    let mut spans = vec![action_pill(colors, flags.action.as_str()), Span::raw(" ")];
    for (i, chip) in chips.into_iter().enumerate() {
        if i > 0 {
            spans.push(Span::raw(" ".repeat(GAP)));
        }
        spans.extend(chip);
    }
    spans
}

/// The current action, lowercase, as a filled pill.
fn action_pill(colors: &ThemeColors, action: &str) -> Span<'static> {
    Span::styled(
        format!(" {action} "),
        Style::default().fg(colors.background).bg(colors.primary),
    )
}

fn verbose_chip(
    colors: &ThemeColors,
    icons: &Icons,
    verbosity: u8,
    labels: Labels,
) -> Vec<Span<'static>> {
    let on = verbosity > 0;
    let mut spans = icon_span(colors, icons, on);
    spans.extend(label_spans(colors, "verbose", 0, on, labels));
    spans.push(Span::raw(" "));
    spans.push(Span::styled(
        verbosity.to_string(),
        Style::default().fg(colors.primary),
    ));
    spans
}

fn toggle_chip(
    colors: &ThemeColors,
    icons: &Icons,
    label: &str,
    hotkey: usize,
    on: bool,
    labels: Labels,
) -> Vec<Span<'static>> {
    let mut spans = icon_span(colors, icons, on);
    spans.extend(label_spans(colors, label, hotkey, on, labels));
    spans
}

/// A chip's mark: green when on, red when off. The label's own colours are
/// separate (see `label_spans`).
fn icon_span(colors: &ThemeColors, icons: &Icons, on: bool) -> Vec<Span<'static>> {
    let (glyph, fg) = if on {
        (icons.flag_on, colors.success)
    } else {
        (icons.flag_off, colors.error)
    };
    vec![Span::styled(format!("{glyph} "), Style::default().fg(fg))]
}

/// A label with its hotkey letter (`hotkey` is a byte index into an ASCII label)
/// drawn in the indicator colour. On labels are bright, off labels muted.
fn label_spans(
    colors: &ThemeColors,
    label: &str,
    hotkey: usize,
    on: bool,
    labels: Labels,
) -> Vec<Span<'static>> {
    let (before, rest) = label.split_at(hotkey);
    let (key, after) = rest.split_at(1);
    let key_span = Span::styled(
        key.to_string(),
        Style::default()
            .fg(colors.indicator)
            .add_modifier(Modifier::BOLD),
    );
    if labels == Labels::HotkeyOnly {
        return vec![key_span];
    }
    let word = if on {
        Style::default()
            .fg(colors.primary)
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(colors.muted)
    };
    vec![
        Span::styled(before.to_string(), word),
        key_span,
        Span::styled(after.to_string(), word),
    ]
}

fn spans_width(spans: &[Span<'static>]) -> usize {
    spans
        .iter()
        .map(|span| UnicodeWidthStr::width(span.content.as_ref()))
        .sum()
}

#[cfg(test)]
mod tests {
    use super::{breadcrumb_label, format_breadcrumb, top_bar_line};
    use crate::theme::ThemeColors;
    use crate::ui::icons::{Icons, NERD, UNICODE};
    use andre_core::{StowAction, StowFlags, Theme};
    use ratatui::{
        style::Modifier,
        text::{Line, Span},
    };
    use unicode_width::UnicodeWidthStr;

    fn colors() -> ThemeColors {
        ThemeColors::from_theme(Theme::Default)
    }

    fn text(line: &Line) -> String {
        line.spans.iter().map(|s| s.content.as_ref()).collect()
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
            .unwrap_or_else(|| panic!("no span {content:?} in {:?}", text(line)))
    }

    fn all_off() -> StowFlags {
        StowFlags::default()
    }

    fn all_on(action: StowAction) -> StowFlags {
        StowFlags {
            action,
            verbosity: 3,
            dry_run: true,
            no_folding: true,
            adopt: true,
            dotfiles: true,
        }
    }

    fn bar(flags: &StowFlags, icons: &Icons, debug: bool, width: usize) -> Line<'static> {
        top_bar_line(&colors(), flags, icons, debug, width)
    }

    #[test]
    fn test_format_breadcrumb_single() {
        assert_eq!(format_breadcrumb(&["MainMenu"]), "Main");
    }

    #[test]
    fn test_format_breadcrumb_stack() {
        assert_eq!(
            format_breadcrumb(&["MainMenu", "Settings"]),
            "Main > Settings"
        );
    }

    #[test]
    fn test_breadcrumb_label_unknown_passthrough() {
        assert_eq!(breadcrumb_label("CustomScreen"), "CustomScreen");
    }

    #[test]
    fn test_top_bar_is_lowercase_with_labelled_chips() {
        let line = bar(&all_off(), &NERD, false, 76);
        let t = text(&line);
        assert!(
            t.trim_start().starts_with("stow "),
            "action pill first: {t:?}"
        );
        for label in ["verbose 0", "dry-run", "no-fold", "adopt", "dotfiles"] {
            assert!(t.contains(label), "missing {label:?} in {t:?}");
        }
        assert!(
            t.trim_end().ends_with("dotfiles"),
            "dotfiles chip last: {t:?}"
        );
        assert!(
            !t.chars().any(char::is_uppercase),
            "top bar must be all lowercase: {t:?}"
        );
        assert!(
            !t.contains("ON") && !t.contains("OFF"),
            "no ON/OFF text: {t:?}"
        );
    }

    #[test]
    fn test_top_bar_has_no_theme_chip_for_any_theme() {
        for theme in Theme::ALL {
            let line = top_bar_line(
                &ThemeColors::from_theme(theme),
                &all_on(StowAction::Unstow),
                &NERD,
                true,
                120,
            );
            let t = text(&line);
            assert!(!t.contains("theme"), "no theme chip in top bar: {t:?}");
            assert!(
                !t.contains(theme.name()),
                "theme name {:?} must not be in top bar: {t:?}",
                theme.name()
            );
        }
    }

    #[test]
    fn test_top_bar_action_pill_shows_current_action_on_primary() {
        let flags = StowFlags {
            action: StowAction::Unstow,
            ..all_off()
        };
        let line = bar(&flags, &NERD, false, 76);
        let pill = span(&line, " unstow ");
        assert_eq!(pill.style.bg, Some(colors().primary));
        assert_eq!(pill.style.fg, Some(colors().background));
    }

    #[test]
    fn test_top_bar_full_labels_at_80_columns_for_every_theme_action_and_debug() {
        // 80 columns less the outer border and the one-column margin each side.
        let inner_width = 76;
        for theme in Theme::ALL {
            let colors = ThemeColors::from_theme(theme);
            for action in StowAction::ALL {
                for debug in [false, true] {
                    let flags = all_on(action);
                    let line = top_bar_line(&colors, &flags, &NERD, debug, inner_width);
                    let t = text(&line);
                    let context = format!("{} / {} / debug={debug}", theme.name(), action.as_str());
                    assert!(
                        t.trim_start().starts_with(&format!("{} ", action.as_str())),
                        "{context}: action pill missing: {t:?}"
                    );
                    for label in ["verbose 3", "dry-run", "no-fold", "adopt", "dotfiles"] {
                        assert!(t.contains(label), "{context}: {label:?} missing: {t:?}");
                    }
                    assert_eq!(t.contains("debug"), debug, "{context}: {t:?}");
                    assert_eq!(
                        line_width(&line),
                        inner_width,
                        "{context}: group must fill the width: {t:?}"
                    );
                }
            }
        }
    }

    #[test]
    fn test_mark_is_success_when_on_and_error_when_off() {
        let flags = StowFlags {
            dry_run: true,
            ..all_off()
        };
        let line = bar(&flags, &NERD, false, 76);
        assert_eq!(span(&line, "\u{f00c} ").style.fg, Some(colors().success));
        assert_eq!(span(&line, "\u{f00d} ").style.fg, Some(colors().error));
    }

    #[test]
    fn test_marks_are_error_when_off_in_unicode_set_too() {
        let line = bar(&all_off(), &UNICODE, false, 76);
        assert_eq!(span(&line, "\u{2717} ").style.fg, Some(colors().error));
        let on = bar(
            &StowFlags {
                adopt: true,
                ..all_off()
            },
            &UNICODE,
            false,
            76,
        );
        assert_eq!(span(&on, "\u{2713} ").style.fg, Some(colors().success));
    }

    #[test]
    fn test_on_label_is_bright_bold_and_off_label_is_muted() {
        let flags = StowFlags {
            dry_run: true,
            ..all_off()
        };
        let line = bar(&flags, &NERD, false, 76);
        let on_word = span(&line, "ry-run");
        assert_eq!(on_word.style.fg, Some(colors().primary));
        assert!(on_word.style.add_modifier.contains(Modifier::BOLD));

        let off_word = span(&line, "o-fold");
        assert_eq!(off_word.style.fg, Some(colors().muted));
    }

    #[test]
    fn test_hotkey_letter_is_indicator_bold_inside_the_label() {
        let line = bar(&all_off(), &NERD, false, 76);
        let key = span(&line, "d");
        assert_eq!(key.style.fg, Some(colors().indicator));
        assert!(key.style.add_modifier.contains(Modifier::BOLD));

        // The letter is part of the label, not a separate key chip: it is
        // followed directly by the rest of the word.
        let idx = line
            .spans
            .iter()
            .position(|s| s.content.as_ref() == "d")
            .unwrap();
        assert_eq!(line.spans[idx + 1].content.as_ref(), "ry-run");
    }

    #[test]
    fn test_hotkey_letters_match_their_keys() {
        let line = bar(&all_off(), &NERD, false, 76);
        // Verbose `v`, dry-run `d`, no-fold `n`, adopt `a`, dotfiles `o` (d-o-tfiles).
        for key in ["v", "d", "n", "a", "o"] {
            assert_eq!(span(&line, key).style.fg, Some(colors().indicator), "{key}");
        }
        assert_eq!(span(&line, "erbose").style.fg, Some(colors().muted));
        assert_eq!(span(&line, "o-fold").style.fg, Some(colors().muted));
        assert_eq!(span(&line, "dopt").style.fg, Some(colors().muted));
        assert_eq!(span(&line, "tfiles").style.fg, Some(colors().muted));
    }

    #[test]
    fn test_verbosity_chip_is_on_styled_above_zero() {
        let on = bar(
            &StowFlags {
                verbosity: 2,
                ..all_off()
            },
            &NERD,
            false,
            76,
        );
        assert!(text(&on).contains("verbose 2"), "{:?}", text(&on));
        assert_eq!(span(&on, "\u{f00c} ").style.fg, Some(colors().success));
        assert_eq!(span(&on, "2").style.fg, Some(colors().primary));

        let off = bar(&all_off(), &NERD, false, 76);
        assert_eq!(span(&off, "\u{f00d} ").style.fg, Some(colors().error));
    }

    #[test]
    fn test_debug_chip_appears_only_in_debug_mode_in_warning_colour() {
        let plain = bar(&all_off(), &NERD, false, 76);
        assert!(!text(&plain).contains("debug"));

        let debug = bar(&all_off(), &NERD, true, 76);
        assert_eq!(span(&debug, "debug").style.fg, Some(colors().warning));
    }

    #[test]
    fn test_unicode_icon_set_uses_plain_symbols() {
        let line = bar(&all_off(), &UNICODE, false, 76);
        let t = text(&line);
        assert!(t.contains("✗ "), "off marks use ✗: {t:?}");
        assert!(!t.chars().any(|c| ('\u{e000}'..='\u{f8ff}').contains(&c)));

        let on = bar(
            &StowFlags {
                adopt: true,
                ..all_off()
            },
            &UNICODE,
            false,
            76,
        );
        assert!(text(&on).contains("✓ "), "on marks use ✓");
    }

    #[test]
    fn test_nerd_icon_set_uses_nerd_glyphs() {
        let line = bar(&all_off(), &NERD, false, 76);
        assert!(text(&line).contains("\u{f00d} "));
    }

    #[test]
    fn test_top_bar_group_is_centred_with_odd_column_on_the_right() {
        // Mixed marks: dry-run and dotfiles on, the rest off.
        for width in [76, 116] {
            for action in [StowAction::Stow, StowAction::Unstow] {
                let flags = StowFlags {
                    action,
                    verbosity: 1,
                    dry_run: true,
                    dotfiles: true,
                    ..all_off()
                };
                let line = bar(&flags, &NERD, false, width);
                let t = text(&line);
                // The pill's own leading space is part of the group, not padding.
                let left = t.len() - t.trim_start().len() - 1;
                let right = t.len() - t.trim_end().len();
                let context = format!("{} at {width} columns: {t:?}", action.as_str());
                assert_eq!(line_width(&line), width, "{context}");
                assert!(
                    right == left || right == left + 1,
                    "left pad {left}, right pad {right}: {context}"
                );
            }
        }
    }

    #[test]
    fn test_narrow_width_drops_labels_to_hotkey_letters() {
        let flags = StowFlags {
            verbosity: 1,
            ..all_off()
        };
        let line = bar(&flags, &NERD, false, 50);
        let t = text(&line);
        assert!(!t.contains("dry-run"), "labels should shrink: {t:?}");
        assert!(!t.contains("verbose"), "labels should shrink: {t:?}");
        assert!(
            t.trim_start().starts_with("stow "),
            "action pill kept: {t:?}"
        );
        assert!(t.contains("v 1"), "hotkey letters kept: {t:?}");
        // The fallback is picked on the unpadded width, then centred.
        assert_eq!(line_width(&line), 50, "{t:?}");
        let left = t.len() - t.trim_start().len() - 1;
        let right = t.len() - t.trim_end().len();
        assert!(
            right == left || right == left + 1,
            "left pad {left}, right pad {right}: {t:?}"
        );
    }

    #[test]
    fn test_top_bar_never_exceeds_requested_width_from_41_columns_up() {
        for width in 41..=120 {
            for theme in Theme::ALL {
                for debug in [false, true] {
                    for action in StowAction::ALL {
                        let line = top_bar_line(
                            &ThemeColors::from_theme(theme),
                            &all_on(action),
                            &NERD,
                            debug,
                            width,
                        );
                        assert!(
                            line_width(&line) <= width,
                            "{} wide at width {width} ({:?})",
                            line_width(&line),
                            text(&line)
                        );
                    }
                }
            }
        }
    }

    #[test]
    fn test_top_bar_degenerate_widths_do_not_panic() {
        for width in 0..=3 {
            let _ = bar(&all_on(StowAction::Unstow), &NERD, true, width);
        }
    }
}
