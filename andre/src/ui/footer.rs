use std::borrow::Cow;

use ratatui::{
    layout::Rect,
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::Paragraph,
    Frame,
};
use unicode_width::UnicodeWidthStr;

use crate::theme::ThemeColors;

/// Columns between the hints and the right-hand side, and between the theme
/// chip and `q quit`.
const GAP: usize = 2;
/// Columns between two hints.
const HINT_GAP: usize = 2;

/// Priorities for [`Hint::prio`]. When the row is too narrow the lowest go first.
pub const PRIO_NAV: u8 = 1;
pub const PRIO_BACK: u8 = 2;
pub const PRIO_JUMP: u8 = 2;
pub const PRIO_VERB: u8 = 3;
pub const PRIO_PRIMARY: u8 = 4;

/// One key hint on the footer: the key as shown, what it does, and how much it matters
/// when the row is narrow. The footer drops the lowest-priority hints whole, never a
/// part of one.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Hint {
    /// The key as drawn: `y`, `Enter`, `↑↓`, `1–7`. A key name, not a `KeyEvent`, since
    /// a hint can stand for several keys.
    pub key: Cow<'static, str>,
    pub label: Cow<'static, str>,
    pub prio: u8,
}

impl Hint {
    pub fn new(key: &'static str, label: impl Into<Cow<'static, str>>, prio: u8) -> Self {
        Self {
            key: Cow::Borrowed(key),
            label: label.into(),
            prio,
        }
    }

    /// Columns the hint takes: the key, a space, the label.
    fn width(&self) -> usize {
        self.key.width() + 1 + self.label.width()
    }
}

/// Columns the hints take together, gaps included.
fn hints_width(hints: &[Hint]) -> usize {
    let text: usize = hints.iter().map(Hint::width).sum();
    text + HINT_GAP * hints.len().saturating_sub(1)
}

/// The hints that fit in `room` columns. The lowest-priority hint goes first, whole; among
/// equal priorities the later one goes first. Order is kept.
fn fit(mut hints: Vec<Hint>, room: usize) -> Vec<Hint> {
    while !hints.is_empty() && hints_width(&hints) > room {
        let (drop, _) = hints
            .iter()
            .enumerate()
            .min_by_key(|(i, h)| (h.prio, std::cmp::Reverse(*i)))
            .expect("hints is not empty");
        hints.remove(drop);
    }
    hints
}

fn key_span(s: &str, colors: &ThemeColors) -> Span<'static> {
    Span::styled(s.to_string(), Style::default().fg(colors.key))
}

/// The hints as spans: each key in `key`, each label in `text_dim`, two columns between.
fn hint_spans(hints: &[Hint], colors: &ThemeColors) -> Vec<Span<'static>> {
    let mut spans = Vec::with_capacity(hints.len() * 3);
    for (i, hint) in hints.iter().enumerate() {
        if i > 0 {
            spans.push(Span::raw(" ".repeat(HINT_GAP)));
        }
        spans.push(key_span(&hint.key, colors));
        spans.push(Span::styled(
            format!(" {}", hint.label),
            Style::default().fg(colors.text_dim),
        ));
    }
    spans
}

/// The bottom bar: the current screen's key hints on the left, then the theme
/// chip and `q quit` at the right edge. One row; the global toggles live in the
/// top bar.
pub fn render_footer(
    frame: &mut Frame,
    area: Rect,
    colors: &ThemeColors,
    hints: &[Hint],
    theme_name: &str,
) {
    if area.width == 0 || area.height == 0 {
        return;
    }
    let line = footer_line(colors, hints, theme_name, area.width as usize);
    frame.render_widget(
        Paragraph::new(line).style(Style::default().fg(colors.text_dim)),
        area,
    );
}

/// The bottom bar as one line, `width` columns wide. The theme chip shows its
/// full name when the hints leave room for it, then just `theme`, and is
/// dropped when not even that fits. Then the lowest-priority hints are dropped
/// whole. `q quit` is always kept.
pub fn footer_line(
    colors: &ThemeColors,
    hints: &[Hint],
    theme_name: &str,
    width: usize,
) -> Line<'static> {
    let quit = quit_spans(colors);
    let hints_len = hints_width(hints);

    for theme in [
        theme_chip(colors, Some(theme_name)),
        theme_chip(colors, None),
    ] {
        let right_width = spans_width(&theme) + GAP + spans_width(&quit);
        if hints_len + GAP + right_width <= width {
            let mut right = theme;
            right.push(Span::raw(" ".repeat(GAP)));
            right.extend(quit);
            return join(hint_spans(hints, colors), right, width);
        }
    }

    let room = width.saturating_sub(GAP + spans_width(&quit));
    let kept = fit(hints.to_vec(), room);
    join(hint_spans(&kept, colors), quit, width)
}

/// `left`, then spaces, then `right`, filling `width` columns.
fn join(mut left: Vec<Span<'static>>, right: Vec<Span<'static>>, width: usize) -> Line<'static> {
    let gap = width.saturating_sub(spans_width(&left) + spans_width(&right));
    left.push(Span::raw(" ".repeat(gap)));
    left.extend(right);
    Line::from(left)
}

/// `t` in the `key` colour and bold, `heme` `text_dim`, then the theme name in
/// the `value` colour when there is room for it.
fn theme_chip(colors: &ThemeColors, name: Option<&str>) -> Vec<Span<'static>> {
    let mut spans = vec![
        Span::styled(
            "t",
            Style::default().fg(colors.key).add_modifier(Modifier::BOLD),
        ),
        Span::styled("heme", Style::default().fg(colors.text_dim)),
    ];
    if let Some(name) = name {
        spans.push(Span::raw(" "));
        spans.push(Span::styled(
            name.to_string(),
            Style::default().fg(colors.value),
        ));
    }
    spans
}

fn quit_spans(colors: &ThemeColors) -> Vec<Span<'static>> {
    vec![
        key_span("q", colors),
        Span::styled(" quit", Style::default().fg(colors.text_dim)),
    ]
}

fn spans_width(spans: &[Span<'static>]) -> usize {
    spans
        .iter()
        .map(|span| UnicodeWidthStr::width(span.content.as_ref()))
        .sum()
}

/// Key hints for one screen. Empty for a screen with none.
pub fn screen_hints(screen_id: &str) -> Vec<Hint> {
    let arrows = |label| Hint::new("\u{2191}\u{2193}", label, PRIO_NAV);
    let enter = |label| Hint::new("Enter", label, PRIO_PRIMARY);
    let space = |label| Hint::new("Space", label, PRIO_VERB);
    let esc = |label| Hint::new("Esc", label, PRIO_BACK);
    let ky = |label| Hint::new("y", label, PRIO_PRIMARY);
    let kn = |label| Hint::new("n", label, PRIO_BACK);

    match screen_id {
        "MainMenu" => vec![
            arrows("navigate"),
            Hint::new("1\u{2013}8", "jump", PRIO_JUMP),
            enter("select"),
        ],
        "Packages" => vec![arrows("navigate"), space("mark"), enter("run"), esc("back")],
        "Status" | "Execute" => vec![enter("continue")],
        "Help" => vec![esc("back")],
        "Settings" => vec![arrows("navigate"), enter("group menu"), esc("back")],
        "Confirm" => vec![ky("confirm"), kn("cancel")],
        "AdoptTargetSelect" | "UnstowTargetSelect" => {
            vec![arrows("navigate"), enter("select"), esc("back")]
        }
        "AdoptFileSelect" => vec![
            arrows("navigate"),
            space("toggle"),
            Hint::new("\u{2192}", "into", PRIO_VERB),
            enter("confirm"),
            Hint::new("\u{2190}", "up", PRIO_VERB),
            esc("back"),
        ],
        "AdoptConfirm" => vec![ky("proceed"), kn("back")],
        "AdoptNamePrompt" => vec![enter("confirm"), esc("back")],
        "AdoptPreview" => vec![ky("execute"), kn("cancel")],
        "UnstowBrowse" => vec![
            arrows("navigate"),
            Hint::new("\u{2192}", "into", PRIO_VERB),
            enter("unlink"),
            Hint::new("\u{2190}", "up", PRIO_VERB),
            esc("back"),
        ],
        "UnstowConfirm" => vec![ky("remove"), kn("cancel")],
        _ => Vec::new(),
    }
}

/// Hints for the profiles list. `run` is the live action that Enter runs on the selected
/// profile, or `None` when Enter is off. `e`, `r`, `x` and `c` need a selected profile.
pub fn profiles_hints(run: Option<&'static str>, selected: bool) -> Vec<Hint> {
    let mut hints = Vec::new();
    if let Some(action) = run {
        hints.push(Hint::new("Enter", action, PRIO_PRIMARY));
    }
    hints.push(Hint::new("+", "new", PRIO_VERB));
    if selected {
        hints.push(Hint::new("e", "edit", PRIO_VERB));
        hints.push(Hint::new("r", "rename", PRIO_VERB));
        hints.push(Hint::new("x", "delete", PRIO_VERB));
        hints.push(Hint::new("c", "check", PRIO_VERB));
    }
    hints.push(Hint::new("Esc", "back", PRIO_BACK));
    hints
}

/// Hints for the new and rename dialogs. `verb` is what Enter does: `create` or `rename`.
pub fn name_dialog_hints(verb: &'static str) -> Vec<Hint> {
    vec![
        Hint::new("Enter", verb, PRIO_PRIMARY),
        Hint::new("Esc", "cancel", PRIO_BACK),
    ]
}

/// Hints for the delete dialog.
pub fn delete_dialog_hints() -> Vec<Hint> {
    vec![
        Hint::new("y", "delete", PRIO_PRIMARY),
        Hint::new("n", "keep", PRIO_BACK),
    ]
}

/// Hints for the profile editor. `space_toggles` is true when Space flips something under
/// the cursor (a package, or an include), false on the description row, where Enter edits.
pub fn profile_edit_hints(space_toggles: bool) -> Vec<Hint> {
    let middle = if space_toggles {
        Hint::new("Space", "toggle", PRIO_VERB)
    } else {
        Hint::new("Enter", "edit", PRIO_PRIMARY)
    };
    vec![
        Hint::new("Tab", "pane", PRIO_NAV),
        middle,
        Hint::new("w", "save", PRIO_VERB),
        Hint::new("Esc", "back", PRIO_BACK),
    ]
}

/// Hints while the profile description is being typed.
pub fn field_edit_hints() -> Vec<Hint> {
    vec![
        Hint::new("Enter", "done", PRIO_PRIMARY),
        Hint::new("Esc", "done", PRIO_BACK),
    ]
}

/// Hints for the leave dialog that opens when a profile has unsaved edits.
pub fn leave_dialog_hints() -> Vec<Hint> {
    vec![
        Hint::new("y", "save", PRIO_PRIMARY),
        Hint::new("n", "discard", PRIO_BACK),
        Hint::new("Esc", "stay", PRIO_BACK),
    ]
}

/// Hints for the packages browser. `run` is the Enter label (`stow 3`), or `None` when Enter
/// does nothing. `on_row` says there is a row to mark and fold; `on_package` that the cursor
/// is on a package, which has profiles and ignore files to edit.
pub fn packages_hints(run: Option<String>, on_row: bool, on_package: bool) -> Vec<Hint> {
    let mut hints = Vec::new();
    if let Some(label) = run {
        hints.push(Hint::new("Enter", label, PRIO_PRIMARY));
    }
    if on_row {
        hints.push(Hint::new("Space", "mark", PRIO_VERB));
    }
    if on_package {
        hints.push(Hint::new("i", "ignore", PRIO_VERB));
        hints.push(Hint::new("p", "profiles", PRIO_VERB));
    }
    if on_row {
        hints.push(Hint::new("\u{2190}\u{2192}", "fold", PRIO_VERB));
    }
    hints.push(Hint::new("Esc", "back", PRIO_BACK));
    hints
}

/// Hints for the ignore editor's list. `on_line` says the cursor is on a line (so `e` and `x`
/// act on it); `Tab` switches between the package's two ignore files.
pub fn ignore_hints(on_line: bool) -> Vec<Hint> {
    let mut hints = vec![
        Hint::new("Tab", "file", PRIO_NAV),
        Hint::new("+", "add", PRIO_VERB),
    ];
    if on_line {
        hints.push(Hint::new("e", "edit", PRIO_VERB));
        hints.push(Hint::new("x", "delete", PRIO_VERB));
    }
    hints.push(Hint::new("w", "save", PRIO_VERB));
    hints.push(Hint::new("Esc", "back", PRIO_BACK));
    hints
}

/// Hints while an ignore line is being typed: Enter keeps it, Esc drops the change.
pub fn ignore_edit_hints() -> Vec<Hint> {
    vec![
        Hint::new("Enter", "done", PRIO_PRIMARY),
        Hint::new("Esc", "cancel", PRIO_BACK),
    ]
}

/// Hints for the profile membership dialog.
pub fn membership_dialog_hints() -> Vec<Hint> {
    vec![
        Hint::new("Space", "toggle", PRIO_VERB),
        Hint::new("Enter", "save", PRIO_PRIMARY),
        Hint::new("Esc", "cancel", PRIO_BACK),
    ]
}

#[cfg(test)]
mod tests {
    use super::{
        delete_dialog_hints, fit, footer_line, hint_spans, hints_width as hint_row_width,
        name_dialog_hints, profiles_hints, screen_hints, Hint,
    };
    use crate::theme::ThemeColors;
    use andre_core::Theme;
    use ratatui::{
        style::Modifier,
        text::{Line, Span},
    };
    use unicode_width::UnicodeWidthStr;

    /// Inner width at the 80-column minimum: border and margin take four.
    const INNER_WIDTH: usize = 76;

    const SCREENS: [&str; 15] = [
        "MainMenu",
        "Packages",
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

    /// The hints as they read on the row: `key label`, two columns apart.
    fn hint_text(hints: &[Hint]) -> String {
        hints
            .iter()
            .map(|h| format!("{} {}", h.key, h.label))
            .collect::<Vec<_>>()
            .join("  ")
    }

    /// Columns the hints take on `screen`.
    fn hints_width(screen: &str) -> usize {
        hint_text(&screen_hints(screen)).width()
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
        let hints = hint_text(&screen_hints("MainMenu"));
        assert_eq!(
            hints,
            "\u{2191}\u{2193} navigate  1\u{2013}8 jump  Enter select"
        );
    }

    #[test]
    fn test_key_names_use_key_colour() {
        let spans = hint_spans(&screen_hints("Settings"), &colors());
        let enter = spans.iter().find(|s| s.content == "Enter").unwrap();
        assert_eq!(enter.style.fg, Some(colors().key));
    }

    #[test]
    fn test_each_hint_is_its_key_then_its_label_in_text_dim() {
        let hints = [Hint::new("y", "confirm", 4), Hint::new("n", "cancel", 2)];
        let spans = hint_spans(&hints, &colors());
        assert_eq!(text(&spans), "y confirm  n cancel");
        assert_eq!(
            span(&Line::from(spans.clone()), " confirm").style.fg,
            Some(colors().text_dim)
        );
        assert_eq!(span(&Line::from(spans), "n").style.fg, Some(colors().key));
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
        assert_eq!(hint_text(&screen_hints("Nowhere")), "");
    }

    #[test]
    fn test_main_menu_shows_theme_chip_for_every_theme_at_80_columns() {
        for theme in Theme::ALL {
            let line = footer_line(
                &ThemeColors::from_theme(theme),
                &screen_hints("MainMenu"),
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
        let line = footer_line(&colors(), &screen_hints("MainMenu"), "nord", INNER_WIDTH);
        let t = span(&line, "t");
        assert_eq!(t.style.fg, Some(colors().key));
        assert!(t.style.add_modifier.contains(Modifier::BOLD));
        assert_eq!(span(&line, "heme").style.fg, Some(colors().text_dim));
        assert_eq!(span(&line, "nord").style.fg, Some(colors().value));
    }

    #[test]
    fn test_quit_key_is_key_and_label_is_text_dim() {
        let line = footer_line(&colors(), &screen_hints("MainMenu"), "nord", INNER_WIDTH);
        assert_eq!(span(&line, "q").style.fg, Some(colors().key));
        assert_eq!(span(&line, " quit").style.fg, Some(colors().text_dim));
    }

    #[test]
    fn test_theme_chip_gap_is_exactly_two_columns_when_tight() {
        let screen = "MainMenu";
        let width = full_chip_width(screen, Theme::CatppuccinMacchiato);
        let line = footer_line(
            &ThemeColors::from_theme(Theme::CatppuccinMacchiato),
            &screen_hints(screen),
            "catppuccin-macchiato",
            width,
        );
        assert_eq!(
            line_text(&line),
            "\u{2191}\u{2193} navigate  1\u{2013}8 jump  Enter select  theme catppuccin-macchiato  q quit"
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

        let line = footer_line(&colors, &screen_hints(screen), theme.name(), INNER_WIDTH);
        let t = line_text(&line);
        assert!(t.contains("theme"), "word stub expected: {t:?}");
        assert!(!t.contains("catppuccin"), "name must be dropped: {t:?}");
        assert_eq!(span(&line, "t").style.fg, Some(colors.key));
        assert_eq!(span(&line, "heme").style.fg, Some(colors.text_dim));

        // At exactly the width the full chip needs, the name is back; one
        // column less drops it.
        let wide = footer_line(
            &colors,
            &screen_hints(screen),
            theme.name(),
            full_chip_width(screen, theme),
        );
        assert!(line_text(&wide).contains("theme catppuccin-macchiato"));
        let short = footer_line(
            &colors,
            &screen_hints(screen),
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
                &screen_hints(longest),
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
                    &screen_hints(screen),
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
    fn test_footer_drops_hints_so_quit_is_still_shown_when_very_narrow() {
        for width in [0, 1, 5, 6, 8, 20, 30] {
            let line = footer_line(
                &colors(),
                &screen_hints("AdoptFileSelect"),
                "default",
                width,
            );
            assert!(
                line_width(&line) <= width.max(6),
                "width {width}: {:?}",
                line_text(&line)
            );
        }
        let line = footer_line(&colors(), &screen_hints("AdoptFileSelect"), "default", 30);
        let t = line_text(&line);
        assert!(t.ends_with("q quit"), "{t:?}");
        assert_eq!(line_width(&line), 30);
        assert!(!t.contains("theme"), "no room for the chip: {t:?}");
    }

    #[test]
    fn test_narrow_footer_drops_the_lowest_priority_hint_whole() {
        // At 60 columns the row has 52 columns for hints after `q quit`. `navigate` is the
        // lowest priority and is dropped whole; the rest keep their order and wording.
        let line = footer_line(&colors(), &screen_hints("AdoptFileSelect"), "default", 60);
        let t = line_text(&line);
        assert!(
            t.starts_with("Space toggle  \u{2192} into  Enter confirm"),
            "{t:?}"
        );
        assert!(!t.contains("navigate"), "{t:?}");
        assert!(!t.contains("navi"), "no partial word: {t:?}");
        assert_eq!(line_width(&line), 60);
    }

    #[test]
    fn test_fit_drops_lowest_priority_first_and_keeps_order() {
        let hints = screen_hints("AdoptFileSelect");
        // Room for the hints without `navigate` (11 columns and a gap) but not with it.
        let room = hint_row_width(&hints) - 13;
        let kept = fit(hints.clone(), room);
        assert_eq!(
            kept.iter().map(|h| h.label.as_ref()).collect::<Vec<_>>(),
            ["toggle", "into", "confirm", "up", "back"]
        );
    }

    #[test]
    fn test_fit_never_keeps_a_hint_under_one_it_outranks_when_dropping() {
        // Every room, every screen: a dropped hint never outranks a kept one, the kept
        // hints fit the room, and they keep their order.
        for screen in SCREENS {
            let hints = screen_hints(screen);
            for room in 0..=INNER_WIDTH {
                let kept = fit(hints.clone(), room);
                assert!(hint_row_width(&kept) <= room, "{screen} room {room}");
                let mut rest = hints.iter();
                for k in &kept {
                    assert!(rest.any(|h| h == k), "{screen} room {room}: order changed");
                }
                for d in &hints {
                    if kept.contains(d) {
                        continue;
                    }
                    for k in &kept {
                        assert!(
                            d.prio <= k.prio,
                            "{screen} room {room}: dropped {d:?} over {k:?}"
                        );
                    }
                }
            }
        }
    }

    #[test]
    fn test_profiles_hints_name_the_live_action_and_drop_what_does_not_apply() {
        assert_eq!(
            hint_text(&profiles_hints(Some("stow"), true)),
            "Enter stow  + new  e edit  r rename  x delete  c check  Esc back"
        );
        assert_eq!(
            hint_text(&profiles_hints(Some("unstow"), true)),
            "Enter unstow  + new  e edit  r rename  x delete  c check  Esc back"
        );
        // A profile that does not resolve: no Enter, but the profile can still be renamed.
        assert_eq!(
            hint_text(&profiles_hints(None, true)),
            "+ new  e edit  r rename  x delete  c check  Esc back"
        );
        // No profiles: nothing to run, rename or delete.
        assert_eq!(hint_text(&profiles_hints(None, false)), "+ new  Esc back");
    }

    #[test]
    fn test_dialog_hints_say_what_enter_and_y_do() {
        assert_eq!(
            hint_text(&name_dialog_hints("create")),
            "Enter create  Esc cancel"
        );
        assert_eq!(
            hint_text(&name_dialog_hints("rename")),
            "Enter rename  Esc cancel"
        );
        assert_eq!(hint_text(&delete_dialog_hints()), "y delete  n keep");
    }

    #[test]
    fn test_profiles_hints_fit_at_80_columns_with_quit() {
        let profiles = [
            profiles_hints(Some("restow"), true),
            profiles_hints(None, false),
            name_dialog_hints("create"),
            delete_dialog_hints(),
        ];
        for hints in profiles {
            let line = footer_line(&colors(), &hints, "catppuccin-macchiato", INNER_WIDTH);
            assert!(
                line_text(&line).ends_with("q quit"),
                "{:?}",
                line_text(&line)
            );
            assert_eq!(line_width(&line), INNER_WIDTH);
        }
    }
}
