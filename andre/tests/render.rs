use ratatui::backend::TestBackend;
use ratatui::buffer::Buffer;
use ratatui::style::{Color, Modifier};
use ratatui::Terminal;

use andre::app::App;
use andre::theme::ThemeColors;
use andre::ui::icons::NERD;
use andre_core::{IconSet, StowAction, Theme};

mod helpers;
use helpers::{make_empty_app, make_test_app};

fn render_app(app: &mut App, width: u16, height: u16) -> Buffer {
    app.set_terminal_size(width, height);
    let backend = TestBackend::new(width, height);
    let mut terminal = Terminal::new(backend).unwrap();
    terminal.draw(|f| andre::ui::render(app, f)).unwrap();
    terminal.backend().buffer().clone()
}

fn line_contains(buf: &Buffer, fragment: &str) -> bool {
    for y in 0..buf.area().height {
        let mut line = String::new();
        for x in 0..buf.area().width {
            line.push_str(buf[(x, y)].symbol());
        }
        if line.contains(fragment) {
            return true;
        }
    }
    false
}

/// The top bar's row: the frame border, the one-row margin, the 7-row banner
/// and a separator come first.
const TOP_BAR_ROW: u16 = 9;

fn row_text(buf: &Buffer, y: u16) -> String {
    (0..buf.area().width)
        .map(|x| buf[(x, y)].symbol())
        .collect()
}

fn top_bar_contains(buf: &Buffer, fragment: &str) -> bool {
    row_text(buf, TOP_BAR_ROW).contains(fragment)
}

/// The bottom bar's row: the last row inside the frame border.
fn footer_row(buf: &Buffer) -> String {
    row_text(buf, buf.area().height - 2)
}

/// The first row whose text contains `fragment`.
fn row_containing(buf: &Buffer, fragment: &str) -> Option<String> {
    (0..buf.area().height)
        .map(|y| row_text(buf, y))
        .find(|row| row.contains(fragment))
}

/// Index of the first cell whose text is `fragment` on row `y`.
fn column_of(buf: &Buffer, y: u16, fragment: &str) -> Option<u16> {
    let len = fragment.chars().count() as u16;
    (0..buf.area().width.saturating_sub(len - 1)).find(|&x| {
        (0..len)
            .map(|i| buf[(x + i, y)].symbol())
            .collect::<String>()
            == fragment
    })
}

/// Foreground of the first cell drawn with `glyph`.
fn first_glyph_color(buf: &Buffer, glyph: &str) -> Option<Color> {
    for y in 0..buf.area().height {
        for x in 0..buf.area().width {
            if buf[(x, y)].symbol() == glyph {
                return Some(buf[(x, y)].fg);
            }
        }
    }
    None
}

fn push_screen(app: &mut App, name: &str) {
    match name {
        "Help" => app
            .component_stack
            .push(Box::new(andre::components::help::HelpComponent::default())),
        "Settings" => app.component_stack.push(Box::new(
            andre::components::settings::SettingsComponent::default(),
        )),
        "GroupSelect" => app.component_stack.push(Box::new(
            andre::components::stow::GroupSelectComponent::default(),
        )),
        "PackageSelect" => app.component_stack.push(Box::new(
            andre::components::stow::PackageSelectComponent::new(vec!["home".to_string()]),
        )),
        "Status" => app.component_stack.push(Box::new(
            andre::components::status::StatusComponent::default(),
        )),
        "AdoptTargetSelect" => app.component_stack.push(Box::new(
            andre::components::target_select::TargetSelectComponent::adopt(),
        )),
        "UnstowTargetSelect" => app.component_stack.push(Box::new(
            andre::components::target_select::TargetSelectComponent::unstow(),
        )),
        "Confirm" => {
            app.component_stack
                .push(Box::new(andre::components::confirm::ConfirmComponent::new(
                    std::collections::HashMap::new(),
                    &app.ctx,
                )))
        }
        "Execute" => {
            let (_tx, rx) = tokio::sync::mpsc::channel(1);
            app.component_stack
                .push(Box::new(andre::components::execute::ExecuteComponent::new(
                    rx,
                    0,
                    Vec::new(),
                )));
        }
        _ => {}
    }
}

fn screen_names() -> Vec<&'static str> {
    vec![
        "MainMenu",
        "GroupSelect",
        "Settings",
        "Status",
        "Help",
        "Confirm",
        "Execute",
        "AdoptTargetSelect",
        "UnstowTargetSelect",
    ]
}

#[test]
fn test_all_screens_render_without_panic() {
    for name in screen_names() {
        let (mut app, _tmp) = make_test_app();
        if name != "MainMenu" {
            push_screen(&mut app, name);
        }
        render_app(&mut app, 80, 24);
    }
}

#[test]
fn test_status_bar_shows_verbosity() {
    let (mut app, _) = make_test_app();
    app.ctx.core.flags.verbosity = 3;
    let buf = render_app(&mut app, 80, 24);
    assert!(top_bar_contains(&buf, "verbose 3"), "verbosity not shown");
}

#[test]
fn test_status_bar_shows_lowercase_action_pill() {
    let (mut app, _) = make_test_app();
    let buf = render_app(&mut app, 80, 24);
    assert!(top_bar_contains(&buf, " stow "), "action pill not shown");
    assert!(
        !top_bar_contains(&buf, "STOW"),
        "action must not be uppercase"
    );
}

#[test]
fn test_status_bar_action_pill_follows_restow() {
    let (mut app, _) = make_test_app();
    app.ctx.core.flags.action = StowAction::Restow;
    let buf = render_app(&mut app, 80, 24);
    assert!(top_bar_contains(&buf, " restow "), "restow not shown");
}

#[test]
fn test_status_bar_shows_debug_chip_only_in_debug_mode() {
    let (mut app, _) = make_test_app();
    let buf = render_app(&mut app, 80, 24);
    assert!(
        !top_bar_contains(&buf, "debug"),
        "debug chip without debug mode"
    );

    app.ctx.debug = true;
    let buf = render_app(&mut app, 80, 24);
    assert!(
        top_bar_contains(&buf, "debug"),
        "debug chip missing in debug mode"
    );
}

/// Regression guard for the toggle colours: an ON toggle's icon is drawn in the
/// success colour and an OFF toggle's icon in muted, so the state reads at a glance.
#[test]
fn test_top_bar_toggle_icons_distinguish_on_from_off() {
    let (mut app, _) = make_test_app();
    let colors = ThemeColors::from_theme(Theme::Default);
    let buf_off = render_app(&mut app, 140, 24);

    app.ctx.core.flags.dry_run = true;
    let buf_on = render_app(&mut app, 140, 24);

    assert_eq!(
        first_glyph_color(&buf_on, NERD.toggle_on),
        Some(colors.success),
        "an ON toggle icon must use the success colour"
    );
    assert_eq!(
        first_glyph_color(&buf_off, NERD.toggle_off),
        Some(colors.muted),
        "an OFF toggle icon must use the muted colour"
    );
}

#[test]
fn test_top_bar_uses_unicode_icons_when_configured() {
    let (mut app, _) = make_test_app();
    app.ctx.core.icons = IconSet::Unicode;
    let buf = render_app(&mut app, 80, 24);
    assert!(top_bar_contains(&buf, "○"), "unicode off-icon missing");
    assert!(
        !top_bar_contains(&buf, NERD.toggle_off),
        "nerd glyph leaked"
    );
}

#[test]
fn test_menu_has_items() {
    let (mut app, _) = make_test_app();
    let buf = render_app(&mut app, 80, 24);
    assert!(line_contains(&buf, "Config"), "Config not in menu");
    assert!(line_contains(&buf, "Help"), "Help not in menu");
}

#[test]
fn test_border_present() {
    let (mut app, _) = make_test_app();
    let buf = render_app(&mut app, 80, 24);
    assert!(line_contains(&buf, "\u{256D}"), "border not found");
}

#[test]
fn test_large_terminal_no_panic() {
    let (mut app, _) = make_test_app();
    render_app(&mut app, 200, 60);
}

#[test]
fn test_tiny_terminal_no_panic() {
    let (mut app, _) = make_test_app();
    render_app(&mut app, 20, 8);
}

#[test]
fn test_empty_app_renders() {
    let (mut app, _) = make_empty_app();
    let buf = render_app(&mut app, 80, 24);
    assert!(
        !line_contains(&buf, "panic"),
        "panic text in empty app render"
    );
}

#[test]
fn test_config_screen_renders() {
    let (mut app, _tmp) = make_test_app();
    push_screen(&mut app, "Settings");
    let buf = render_app(&mut app, 80, 24);
    assert!(
        line_contains(&buf, "Config"),
        "config screen title not shown"
    );
}

#[test]
fn test_banner_has_content() {
    let (mut app, _) = make_test_app();
    let buf = render_app(&mut app, 80, 24);
    let top_content = (0..5).any(|y| {
        let mut line = String::new();
        for x in 0..buf.area().width {
            let s = buf[(x, y)].symbol();
            if s != " " {
                line.push_str(s);
            }
        }
        !line.is_empty()
    });
    assert!(top_content, "banner area empty");
}

#[test]
fn test_help_screen_has_key_info() {
    let (mut app, _tmp) = make_test_app();
    push_screen(&mut app, "Help");
    let buf = render_app(&mut app, 80, 24);
    assert!(!line_contains(&buf, "panic"), "help screen render failed");
}

#[test]
fn test_config_screen_shows_verbosity() {
    let (mut app, _tmp) = make_test_app();
    push_screen(&mut app, "Settings");
    let buf = render_app(&mut app, 80, 24);
    assert!(
        line_contains(&buf, "verbosity"),
        "verbosity row not visible"
    );
}

#[test]
fn test_execute_screen_shows_running() {
    let (mut app, _tmp) = make_test_app();
    push_screen(&mut app, "Execute");
    let buf = render_app(&mut app, 80, 24);
    assert!(line_contains(&buf, "Executing"), "running state not shown");
    assert!(
        !line_contains(&buf, "Press Ctrl+C to cancel"),
        "dead Ctrl+C cancel hint must not appear"
    );
}

#[test]
fn test_execute_gauge_ratio_matches_completed_total() {
    use andre::components::execute::ExecuteComponent;
    use andre::execute::ExecutionResult;

    let (mut app, _tmp) = make_test_app();
    let (_tx, rx) = tokio::sync::mpsc::channel(1);
    let mut exec = ExecuteComponent::new(
        rx,
        5,
        vec![
            ExecutionResult {
                group: "home".into(),
                package: "bash".into(),
                success: true,
                error_message: None,
            },
            ExecutionResult {
                group: "home".into(),
                package: "vim".into(),
                success: false,
                error_message: Some("conflict".into()),
            },
        ],
    );
    exec.current = 3;
    app.component_stack.push(Box::new(exec));

    let buf = render_app(&mut app, 80, 24);
    assert!(
        line_contains(&buf, "2/5"),
        "gauge label must show completed/total"
    );
}

#[test]
fn test_execute_rows_show_icon_name_under_gauge() {
    use andre::components::execute::ExecuteComponent;
    use andre::execute::ExecutionResult;

    let (mut app, _tmp) = make_test_app();
    let (_tx, rx) = tokio::sync::mpsc::channel(1);
    app.component_stack.push(Box::new(ExecuteComponent::new(
        rx,
        2,
        vec![
            ExecutionResult {
                group: "home".into(),
                package: "bash".into(),
                success: true,
                error_message: None,
            },
            ExecutionResult {
                group: "work".into(),
                package: "git".into(),
                success: false,
                error_message: Some("fail".into()),
            },
        ],
    )));

    let buf = render_app(&mut app, 80, 24);
    assert!(line_contains(&buf, "2/2"), "gauge label missing");
    assert!(
        line_contains(&buf, "\u{2713}")
            && line_contains(&buf, "home")
            && line_contains(&buf, "bash"),
        "success row icon/name missing under gauge"
    );
    assert!(
        line_contains(&buf, "\u{2717}")
            && line_contains(&buf, "work")
            && line_contains(&buf, "git"),
        "fail row icon/name missing under gauge"
    );
}

#[test]
fn test_execute_progress_preserves_total_with_skips() {
    use andre::components::execute::ExecuteComponent;
    use andre::execute::{ExecutionResult, ExecutionUpdate};

    let (mut app, _tmp) = make_test_app();
    let (tx, rx) = tokio::sync::mpsc::channel(4);
    let skipped = vec![ExecutionResult {
        group: "home".into(),
        package: "missing".into(),
        success: false,
        error_message: Some("Skipped".into()),
    }];
    // Full total = 1 skip + 2 commands
    app.component_stack
        .push(Box::new(ExecuteComponent::new(rx, 3, skipped)));

    tx.try_send(ExecutionUpdate::Progress { current: 1 })
        .unwrap();
    app.update_components();

    let buf = render_app(&mut app, 80, 24);
    assert!(
        line_contains(&buf, "1/3"),
        "gauge must keep full total including skips after Progress (got no 1/3)"
    );
    assert!(
        !line_contains(&buf, "1/2"),
        "gauge must not shrink denominator to commands-only total"
    );
}

#[test]
fn test_execute_spinner_clamps_in_flight_to_total() {
    use andre::components::execute::ExecuteComponent;
    use andre::execute::ExecutionResult;

    let (mut app, _tmp) = make_test_app();
    let (_tx, rx) = tokio::sync::mpsc::channel(1);
    app.component_stack.push(Box::new(ExecuteComponent::new(
        rx,
        2,
        vec![
            ExecutionResult {
                group: "home".into(),
                package: "a".into(),
                success: true,
                error_message: None,
            },
            ExecutionResult {
                group: "home".into(),
                package: "b".into(),
                success: true,
                error_message: None,
            },
        ],
    )));

    let buf = render_app(&mut app, 80, 24);
    assert!(
        line_contains(&buf, "2/2"),
        "when all results are in, spinner/gauge must not show 3/2"
    );
    assert!(
        !line_contains(&buf, "3/2"),
        "in-flight counter must clamp to total"
    );
}

#[test]
fn test_execute_completion_summary_on_finished() {
    use andre::components::execute::ExecuteComponent;
    use andre::execute::{ExecutionResult, ExecutionState};

    let (mut app, _tmp) = make_test_app();
    let (_tx, rx) = tokio::sync::mpsc::channel(1);
    let mut exec = ExecuteComponent::new(
        rx,
        2,
        vec![
            ExecutionResult {
                group: "home".into(),
                package: "bash".into(),
                success: true,
                error_message: None,
            },
            ExecutionResult {
                group: "home".into(),
                package: "vim".into(),
                success: false,
                error_message: Some("conflict".into()),
            },
        ],
    );
    exec.state = ExecutionState::Completed;
    app.component_stack.push(Box::new(exec));

    let buf = render_app(&mut app, 80, 24);
    assert!(
        line_contains(&buf, "1 succeeded, 1 failed"),
        "completion summary missing on Finished"
    );
    assert!(
        line_contains(&buf, "Press Enter to continue"),
        "continue prompt missing on Finished"
    );
    assert!(
        line_contains(&buf, "2/2"),
        "gauge should show full progress"
    );
}

#[test]
fn test_execute_completion_summary_distinguishes_skips() {
    use andre::components::execute::ExecuteComponent;
    use andre::execute::{ExecutionResult, ExecutionState};

    let (mut app, _tmp) = make_test_app();
    let (_tx, rx) = tokio::sync::mpsc::channel(1);
    let mut exec = ExecuteComponent::new(
        rx,
        2,
        vec![
            ExecutionResult {
                group: "home".into(),
                package: "bash".into(),
                success: true,
                error_message: None,
            },
            ExecutionResult {
                group: "home".into(),
                package: "gone".into(),
                success: false,
                error_message: Some("Skipped: source directory not found".into()),
            },
        ],
    );
    exec.state = ExecutionState::Completed;
    app.component_stack.push(Box::new(exec));

    let buf = render_app(&mut app, 80, 24);
    assert!(
        line_contains(&buf, "1 succeeded, 0 failed, 1 skipped"),
        "skips must not count as failed in completion summary"
    );
}

#[test]
fn test_execute_completed_list_scrolls() {
    use andre::components::execute::ExecuteComponent;
    use andre::execute::{ExecutionResult, ExecutionState};
    use helpers::down_key;

    let (mut app, _tmp) = make_test_app();
    let (_tx, rx) = tokio::sync::mpsc::channel(1);
    let results: Vec<_> = (0..20)
        .map(|i| ExecutionResult {
            group: "home".into(),
            package: format!("pkg{i}"),
            success: true,
            error_message: None,
        })
        .collect();
    let mut exec = ExecuteComponent::new(rx, results.len(), results);
    exec.state = ExecutionState::Completed;
    app.component_stack.push(Box::new(exec));

    // Drive scroll down; Completed must honor scroll_index in render.
    for _ in 0..5 {
        app.component_dispatch(down_key());
    }
    let top = app.component_stack.last().unwrap();
    let exec = top.as_any().downcast_ref::<ExecuteComponent>().unwrap();
    assert_eq!(
        exec.scroll_index, 5,
        "Completed state should advance scroll_index on down"
    );
    let buf = render_app(&mut app, 80, 24);
    assert!(
        line_contains(&buf, "Results:") || line_contains(&buf, "pkg"),
        "completed list must still render while scrolled"
    );
}

#[test]
fn test_adopt_target_select_shows_adopt_from_label() {
    let (mut app, _tmp) = make_test_app();
    let (mut app2, _tmp2) = make_test_app();
    // Normal unstow target select shows "target"
    push_screen(&mut app, "UnstowTargetSelect");
    let buf = render_app(&mut app, 80, 24);
    assert!(line_contains(&buf, "(target:"), "unstow should show target");

    // Adopt target select shows "Adopt from"
    push_screen(&mut app2, "AdoptTargetSelect");
    let buf = render_app(&mut app2, 80, 24);
    assert!(
        line_contains(&buf, "(Adopt from:"),
        "adopt should show Adopt from label"
    );
}

#[test]
fn test_footer_narrow_area_shows_screen_hints() {
    // Footer narrow-path is area-width based. Full-frame render never reaches it
    // under the 80-col gate, so exercise render_footer directly on a tight rect.
    use andre::theme::ThemeColors;
    use andre_core::state::Theme;
    use ratatui::layout::Rect;

    let backend = TestBackend::new(40, 2);
    let mut terminal = Terminal::new(backend).unwrap();
    let colors = ThemeColors::from_theme(Theme::Default);
    terminal
        .draw(|f| {
            andre::ui::render_footer(f, Rect::new(0, 0, 40, 2), &colors, "MainMenu", "default");
        })
        .unwrap();
    let buf = terminal.backend().buffer().clone();
    assert!(
        line_contains(&buf, "navigate") || line_contains(&buf, "select"),
        "footer hints missing on narrow footer area"
    );
}

#[test]
fn test_help_screen_scrollbar_render() {
    let (mut app, _tmp) = make_test_app();
    push_screen(&mut app, "Help");
    // Render at a small height so scrollbar is needed
    let buf = render_app(&mut app, 80, 10);
    assert!(
        !line_contains(&buf, "panic"),
        "help screen render failed at small height"
    );
}

/// Footer context line assertions for major screen families (M02 AC-01).
#[test]
fn test_footer_context_hints_per_screen_family() {
    // (screen push name or "" for MainMenu, distinctive footer fragment)
    let cases: &[(&str, &str)] = &[
        ("", "navigate"),                 // MainMenu
        ("GroupSelect", "toggle"),        // multi-select family
        ("Settings", "group menu"),       // settings
        ("Help", "back"),                 // help
        ("Confirm", "confirm"),           // confirm y/n
        ("Execute", "continue"),          // status/execute family
        ("AdoptTargetSelect", "select"),  // adopt target family
        ("UnstowTargetSelect", "select"), // unstow target family
        ("PackageSelect", "toggle"),      // package multi-select family
    ];

    for (name, fragment) in cases {
        let (mut app, _tmp) = make_test_app();
        if !name.is_empty() {
            push_screen(&mut app, name);
        }
        let buf = render_app(&mut app, 120, 24);
        assert!(
            line_contains(&buf, fragment),
            "footer hint missing for {name:?}: expected fragment {fragment:?}"
        );
    }

    // MainMenu advertises 1–7 jump alongside the arrows + Enter.
    // Guarded separately so the digit-key feature is observable from
    // rendered output, not just dispatch logic.
    let (mut app, _tmp) = make_test_app();
    let buf = render_app(&mut app, 120, 24);
    assert!(
        line_contains(&buf, "jump"),
        "MainMenu footer must advertise the 1–7 jump hint"
    );
}

#[test]
fn test_breadcrumb_shows_root() {
    let (mut app, _) = make_test_app();
    let buf = render_app(&mut app, 120, 24);
    assert!(
        line_contains(&buf, "Main > Main Menu"),
        "root breadcrumb segment missing from panel title"
    );
}

#[test]
fn test_breadcrumb_updates_on_stack_push() {
    let (mut app, _tmp) = make_test_app();
    push_screen(&mut app, "Settings");
    let buf = render_app(&mut app, 120, 24);
    assert!(
        line_contains(&buf, "Main > Config"),
        "breadcrumb did not reflect MainMenu > Settings stack"
    );
}

#[test]
fn test_breadcrumb_updates_on_deeper_stack() {
    let (mut app, _tmp) = make_test_app();
    push_screen(&mut app, "Help");
    let buf = render_app(&mut app, 120, 24);
    assert!(
        line_contains(&buf, "Main > Help"),
        "breadcrumb missing Help segment from panel title"
    );
}

#[test]
fn test_status_toggles_still_live_with_breadcrumb() {
    let (mut app, _) = make_test_app();
    app.ctx.core.flags.verbosity = 2;
    app.ctx.core.flags.dry_run = true;
    let buf = render_app(&mut app, 140, 24);
    // Breadcrumb now lives in the panel title; toggles still live in the top bar.
    assert!(
        line_contains(&buf, "Main > Main Menu"),
        "breadcrumb missing from panel title"
    );
    assert!(
        top_bar_contains(&buf, "verbose 2"),
        "verbosity toggle not live"
    );
    assert!(
        top_bar_contains(&buf, "dry-run") && top_bar_contains(&buf, NERD.toggle_on),
        "dry-run toggle not live"
    );
}

#[test]
fn test_footer_shows_quit_at_right_and_no_global_toggles() {
    let (mut app, _) = make_test_app();
    let buf = render_app(&mut app, 80, 24);
    let hint_row = (0..buf.area().height)
        .map(|y| row_text(&buf, y))
        .find(|line| line.contains("navigate"))
        .expect("footer hint row missing");
    let content = hint_row.trim_end_matches(['│', ' ']);
    assert!(
        content.ends_with("q quit"),
        "q quit not at right: {hint_row:?}"
    );
    assert!(
        !hint_row.contains("dry-run") && !hint_row.contains("verbose"),
        "global toggles must live in the top bar only: {hint_row:?}"
    );
    let y = buf.area().height - 2;
    let q_x = column_of(&buf, y, "q quit").expect("q quit cell missing");
    assert_eq!(
        buf[(q_x, y)].fg,
        ThemeColors::from_theme(Theme::Default).indicator,
        "q must be drawn in the indicator colour"
    );
}

#[test]
fn test_top_bar_and_footer_fit_80_columns_on_one_row_each() {
    let (mut app, _) = make_test_app();
    app.ctx.core.flags.verbosity = 5;
    app.ctx.core.flags.dry_run = true;
    app.ctx.core.flags.dotfiles = true;
    app.ctx.core.flags.action = StowAction::Unstow;
    app.ctx.debug = true;
    let buf = render_app(&mut app, 80, 24);
    let top = row_text(&buf, TOP_BAR_ROW);
    assert!(top.contains("debug"), "debug chip must still fit: {top:?}");
    assert!(top.contains("unstow"), "action pill missing: {top:?}");
    assert!(top.contains("verbose 5"), "full labels expected: {top:?}");
    assert!(
        !top.contains("theme"),
        "top bar must not carry the theme: {top:?}"
    );
    let footer: Vec<String> = (0..buf.area().height)
        .map(|y| row_text(&buf, y))
        .filter(|r| r.contains("q quit"))
        .collect();
    assert_eq!(footer.len(), 1, "bottom bar must take exactly one row");
    assert!(
        footer[0].contains("theme default"),
        "theme chip belongs in the bottom bar: {:?}",
        footer[0]
    );
}

#[test]
fn test_top_bar_has_full_labels_and_no_theme_for_every_theme_at_80_columns() {
    for theme in Theme::ALL {
        let (mut app, _tmp) = make_test_app();
        app.ctx.core.theme = theme;
        app.ctx.core.flags.action = StowAction::Restow;
        app.ctx.core.flags.verbosity = 3;
        app.ctx.core.flags.dry_run = true;
        app.ctx.core.flags.no_folding = true;
        app.ctx.core.flags.adopt = true;
        app.ctx.core.flags.dotfiles = true;
        app.ctx.debug = true;
        let buf = render_app(&mut app, 80, 24);
        let top = row_text(&buf, TOP_BAR_ROW);
        for label in [
            "restow",
            "verbose 3",
            "dry-run",
            "no-fold",
            "adopt",
            "dotfiles",
            "debug",
        ] {
            assert!(
                top.contains(label),
                "{}: {label:?} missing: {top:?}",
                theme.name()
            );
        }
        assert!(!top.contains("theme"), "{}: {top:?}", theme.name());
        assert!(!top.contains(theme.name()), "{}: {top:?}", theme.name());
    }
}

#[test]
fn test_main_menu_footer_shows_theme_chip_for_every_theme_at_80_columns() {
    for theme in Theme::ALL {
        let (mut app, _tmp) = make_test_app();
        app.ctx.core.theme = theme;
        let buf = render_app(&mut app, 80, 24);
        let footer = footer_row(&buf);
        assert!(
            footer.contains(&format!("theme {}", theme.name())),
            "{}: theme chip missing from bottom bar: {footer:?}",
            theme.name()
        );
        assert!(
            footer.trim_end_matches(['│', ' ']).ends_with("q quit"),
            "{footer:?}"
        );
    }
    let (mut app, _tmp) = make_test_app();
    app.ctx.core.theme = Theme::CatppuccinMacchiato;
    let buf = render_app(&mut app, 80, 24);
    let y = buf.area().height - 2;
    let colors = ThemeColors::from_theme(Theme::CatppuccinMacchiato);
    let t_x = column_of(&buf, y, "theme").expect("theme chip missing");
    assert_eq!(
        buf[(t_x, y)].fg,
        colors.indicator,
        "t must be the indicator colour"
    );
    assert!(
        buf[(t_x, y)].modifier.contains(Modifier::BOLD),
        "t must be bold"
    );
    let name_x = column_of(&buf, y, "catppuccin-macchiato").expect("theme name missing");
    assert_eq!(
        buf[(name_x, y)].fg,
        colors.highlight,
        "name must be the highlight colour"
    );
}

#[test]
fn test_chrome_renders_across_themes() {
    use andre_core::state::Theme;

    let themes = [
        Theme::Default,
        Theme::Dracula,
        Theme::CatppuccinMocha,
        Theme::CatppuccinLatte,
        Theme::CatppuccinFrappe,
        Theme::CatppuccinMacchiato,
        Theme::Nord,
        Theme::Gruvbox,
    ];

    for theme in themes {
        let (mut app, _tmp) = make_test_app();
        app.ctx.core.theme = theme;
        push_screen(&mut app, "Settings");
        let buf = render_app(&mut app, 120, 24);
        assert!(
            line_contains(&buf, "Main > Config"),
            "breadcrumb missing under theme {:?}",
            theme.name()
        );
        assert!(
            line_contains(&buf, "group menu"),
            "footer missing under theme {:?}",
            theme.name()
        );
        assert!(
            line_contains(&buf, theme.name()) || line_contains(&buf, "Theme:"),
            "theme label missing under {:?}",
            theme.name()
        );
    }
}

#[test]
fn test_too_small_overlay_on_undersized_terminal() {
    let (mut app, _tmp) = make_test_app();
    push_screen(&mut app, "Help");
    let buf = render_app(&mut app, 40, 24);
    assert!(
        line_contains(&buf, "Terminal too small"),
        "undersized terminal must show too-small overlay"
    );
}

#[test]
fn test_chrome_degrades_on_minimum_supported_width() {
    let (mut app, _tmp) = make_test_app();
    push_screen(&mut app, "Help");
    // Minimum supported size (80x24): the top bar keeps its chips and the bottom
    // bar keeps the screen hint and `q quit`.
    let buf = render_app(&mut app, 80, 24);
    assert!(
        footer_row(&buf).contains("theme default"),
        "bottom bar lost the theme chip: {:?}",
        footer_row(&buf)
    );
    assert!(
        footer_row(&buf).contains("back"),
        "footer lost the screen hint"
    );
    assert!(footer_row(&buf).contains("q quit"), "footer lost q quit");
}

#[test]
fn test_settings_icons_row_shows_pending_set_after_enter() {
    use andre::components::settings::rows::ROW_ICONS;
    use andre::components::settings::SettingsComponent;
    use helpers::enter_key;

    let (mut app, _tmp) = make_test_app();
    push_screen(&mut app, "Settings");
    app.component_stack
        .last_mut()
        .unwrap()
        .as_any_mut()
        .downcast_mut::<SettingsComponent>()
        .unwrap()
        .cursor = ROW_ICONS;

    let buf = render_app(&mut app, 80, 40);
    let row = row_containing(&buf, "icons:").expect("icons row missing from settings");
    assert!(row.contains("nerd"), "default icon set not shown: {row:?}");

    app.component_dispatch(enter_key());
    let buf = render_app(&mut app, 80, 40);
    let row = row_containing(&buf, "icons:").expect("icons row missing from settings");
    assert!(
        row.contains("unicode"),
        "Enter must show the pending set: {row:?}"
    );
    assert_eq!(
        app.ctx.core.icons,
        IconSet::Nerd,
        "icons apply only once saved"
    );
}

#[test]
fn test_toast_renders_message() {
    let (mut app, _tmp) = make_test_app();
    app.ctx.show_toast("Config saved");
    let buf = render_app(&mut app, 100, 24);
    assert!(
        line_contains(&buf, "Config saved"),
        "toast message missing from buffer"
    );
}

#[test]
fn test_toast_renders_across_themes() {
    use andre_core::state::Theme;

    let themes = [Theme::Default, Theme::Dracula, Theme::Nord, Theme::Gruvbox];
    for theme in themes {
        let (mut app, _tmp) = make_test_app();
        app.ctx.core.theme = theme;
        app.ctx.show_toast("ok");
        let buf = render_app(&mut app, 100, 24);
        assert!(
            line_contains(&buf, "ok"),
            "toast missing under theme {:?}",
            theme.name()
        );
    }
}

#[test]
fn test_error_toast_renders_message() {
    let (mut app, _tmp) = make_test_app();
    app.ctx.show_error_toast("Save failed: disk full");
    let buf = render_app(&mut app, 100, 24);
    assert!(
        line_contains(&buf, "Save failed"),
        "error toast message missing from buffer"
    );
}

#[test]
fn test_confirm_preview_is_engine_aware_native_default() {
    // M07: native is the default engine. The confirm screen must NOT render a
    // `stow ...` command preview (that would mislead users into thinking GNU
    // stow runs). It should show a native preview instead.
    let (mut app, _tmp) = make_test_app();
    let mut selected = std::collections::HashMap::new();
    selected.insert("home".to_string(), vec!["test".to_string()]);
    app.component_stack
        .push(Box::new(andre::components::confirm::ConfirmComponent::new(
            selected, &app.ctx,
        )));
    let buf = render_app(&mut app, 120, 24);
    assert!(
        line_contains(&buf, "native:"),
        "native default should render a native preview, not a stow command"
    );
    assert!(
        !line_contains(&buf, "stow -d"),
        "confirm preview must not show a stow command under the native default"
    );
}
