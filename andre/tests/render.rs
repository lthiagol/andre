use ratatui::backend::TestBackend;
use ratatui::buffer::Buffer;
use ratatui::style::{Color, Modifier};
use ratatui::Terminal;

use andre::app::App;
use andre::theme::ThemeColors;
use andre::ui::icons::NERD;
use andre_core::{IconSet, StowAction, StowStatus, Theme};

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

fn row_text(buf: &Buffer, y: u16) -> String {
    (0..buf.area().width)
        .map(|x| buf[(x, y)].symbol())
        .collect()
}

/// The top bar's row: the first full-width separator (the one under the banner)
/// plus one. Found from the drawn rows, because the banner is seven rows on the
/// main menu and one row everywhere else.
fn top_bar_row(buf: &Buffer) -> u16 {
    let width = buf.area().width;
    (1..buf.area().height)
        .find(|&y| (1..width - 1).all(|x| buf[(x, y)].symbol() == "\u{2500}"))
        .expect("banner separator missing")
        + 1
}

fn top_bar_contains(buf: &Buffer, fragment: &str) -> bool {
    row_text(buf, top_bar_row(buf)).contains(fragment)
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
    assert!(line_contains(&buf, "settings"), "settings not in menu");
    assert!(line_contains(&buf, "help"), "help not in menu");
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
fn test_main_menu_list_is_titled_actions() {
    let (mut app, _) = make_test_app();
    let buf = render_app(&mut app, 120, 24);
    assert!(
        line_contains(&buf, " actions "),
        "main menu list panel title missing"
    );
    assert!(
        !line_contains(&buf, "Main > Main Menu"),
        "the old breadcrumb panel title is back"
    );
}

#[test]
fn test_breadcrumb_updates_on_stack_push() {
    let (mut app, _tmp) = make_test_app();
    push_screen(&mut app, "Settings");
    let buf = render_app(&mut app, 120, 24);
    assert!(
        line_contains(&buf, "Main > Settings"),
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
    // The main menu's list panel is titled `actions`; toggles still live in the top bar.
    assert!(
        line_contains(&buf, " actions "),
        "list panel title missing on the main menu"
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
    let top = row_text(&buf, top_bar_row(&buf));
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
        let top = row_text(&buf, top_bar_row(&buf));
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
            line_contains(&buf, "Main > Settings"),
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

/// Like `render_app`, after one update pass. The main menu builds its detail
/// facts in `update`, so this is how a frame gets them.
fn render_updated(app: &mut App, width: u16, height: u16) -> Buffer {
    app.update_components();
    render_app(app, width, height)
}

fn menu_mut(app: &mut App) -> &mut andre::components::MainMenuComponent {
    app.component_stack
        .last_mut()
        .unwrap()
        .as_any_mut()
        .downcast_mut()
        .unwrap()
}

/// Position of the selected row's accent bar.
fn accent_cell(buf: &Buffer) -> (u16, u16) {
    (0..buf.area().height)
        .flat_map(|y| (0..buf.area().width).map(move |x| (x, y)))
        .find(|&(x, y)| buf[(x, y)].symbol() == "\u{258c}")
        .expect("no selected-row accent")
}

/// WCAG contrast ratio between two RGB colours.
fn contrast(a: Color, b: Color) -> f64 {
    fn luminance(c: Color) -> f64 {
        let Color::Rgb(r, g, b) = c else {
            panic!("theme colours are RGB: {c:?}")
        };
        let linear = |v: u8| {
            let v = v as f64 / 255.0;
            if v <= 0.03928 {
                v / 12.92
            } else {
                ((v + 0.055) / 1.055).powf(2.4)
            }
        };
        0.2126 * linear(r) + 0.7152 * linear(g) + 0.0722 * linear(b)
    }
    let (la, lb) = (luminance(a), luminance(b));
    let (hi, lo) = if la > lb { (la, lb) } else { (lb, la) };
    (hi + 0.05) / (lo + 0.05)
}

#[test]
fn test_menu_selected_row_is_filled_with_selection_colour() {
    let (mut app, _) = make_test_app();
    menu_mut(&mut app).index = 2;
    let buf = render_app(&mut app, 80, 24);
    let colors = ThemeColors::from_theme(Theme::Default);

    let (x0, y) = accent_cell(&buf);
    assert_eq!(
        buf[(x0, y)].fg,
        colors.primary,
        "accent bar must use the primary colour"
    );
    assert_eq!(
        buf[(x0, y)].bg,
        colors.background,
        "accent bar sits on the normal background, not the selection fill"
    );
    let right_border = (x0..buf.area().width)
        .find(|&x| buf[(x, y)].symbol() == "\u{2502}")
        .expect("list panel right border");
    for x in x0 + 1..right_border {
        assert_eq!(
            buf[(x, y)].bg,
            colors.selection,
            "selected row not filled at column {x}"
        );
    }
    assert_ne!(
        buf[(x0 + 1, y + 1)].bg,
        colors.selection,
        "only the selected row is filled"
    );
}

/// The background the accent bar sits on. `Reset` (Default theme) means the terminal's
/// own background, which this test cannot see. It is taken as black, the dark terminal
/// the Default theme is built for, so the accent check does not cover light terminals.
fn painted_background(colors: &ThemeColors) -> Color {
    match colors.background {
        Color::Reset => Color::Rgb(0, 0, 0),
        other => other,
    }
}

/// Every theme keeps the selected row readable. The label is normal text, so it must
/// reach WCAG AA 4.5:1 on the selection fill. The icon and the accent bar are
/// non-text, so they need 3:1 (WCAG 1.4.11).
#[test]
fn test_selected_row_text_is_readable_in_every_theme() {
    for theme in Theme::ALL {
        let colors = ThemeColors::from_theme(theme);
        let (mut app, _) = make_test_app();
        app.ctx.core.theme = theme;
        for index in 0..7 {
            menu_mut(&mut app).index = index;
            let buf = render_app(&mut app, 80, 24);
            let (x0, y) = accent_cell(&buf);
            let accent = &buf[(x0, y)];
            let icon = &buf[(x0 + 1, y)];
            // The label starts after the accent, the icon and two spaces.
            let label = &buf[(x0 + 4, y)];
            assert_eq!(
                label.bg,
                colors.selection,
                "{}: selected row not filled with the selection colour",
                theme.name()
            );
            assert_eq!(
                accent.bg,
                colors.background,
                "{}: accent bar is filled",
                theme.name()
            );

            let label_ratio = contrast(label.fg, label.bg);
            assert!(
                label_ratio >= 4.5,
                "{}: selected label contrast {label_ratio:.2} < 4.5 for item {index}",
                theme.name()
            );
            let icon_ratio = contrast(icon.fg, icon.bg);
            assert!(
                icon_ratio >= 3.0,
                "{}: selected icon contrast {icon_ratio:.2} < 3.0 for item {index}",
                theme.name()
            );
            let accent_ratio = contrast(accent.fg, painted_background(&colors));
            assert!(
                accent_ratio >= 3.0,
                "{}: accent contrast {accent_ratio:.2} < 3.0 for item {index}",
                theme.name()
            );
        }
    }
}

#[test]
fn test_main_menu_keeps_the_big_banner_and_other_screens_get_one_row() {
    let (mut app, _) = make_test_app();
    let buf = render_app(&mut app, 80, 24);
    assert!(
        line_contains(&buf, "\u{2551} A. \u{2551}"),
        "big banner missing on the main menu"
    );
    assert_eq!(
        top_bar_row(&buf),
        9,
        "main menu top bar sits under the seven-row banner"
    );

    for name in screen_names().into_iter().filter(|n| *n != "MainMenu") {
        let (mut app, _) = make_test_app();
        push_screen(&mut app, name);
        let buf = render_app(&mut app, 80, 24);
        assert!(
            !line_contains(&buf, "\u{2551} A. \u{2551}"),
            "{name}: the big banner belongs to the main menu only"
        );
        assert!(
            row_text(&buf, 1).contains("andre"),
            "{name}: compact title missing: {:?}",
            row_text(&buf, 1)
        );
        assert_eq!(
            top_bar_row(&buf),
            3,
            "{name}: top bar should follow the one-row title"
        );
    }
}

#[test]
fn test_compact_title_shows_the_breadcrumb() {
    let (mut app, _) = make_test_app();
    push_screen(&mut app, "GroupSelect");
    let buf = render_app(&mut app, 80, 24);
    assert!(
        row_text(&buf, 1).contains("andre  Main > Groups"),
        "compact title lost the breadcrumb: {:?}",
        row_text(&buf, 1)
    );
}

#[test]
fn test_content_is_capped_at_120_columns_and_centred() {
    // Left and right corners of the main menu's content, from the drawn borders.
    fn content_span(buf: &Buffer) -> (u16, u16) {
        let y = (0..buf.area().height)
            .find(|&y| row_text(buf, y).contains(" actions "))
            .expect("list panel title");
        let left = (0..buf.area().width)
            .find(|&x| buf[(x, y)].symbol() == "\u{256d}")
            .expect("list panel corner");
        let right = (0..buf.area().width)
            .rev()
            .find(|&x| buf[(x, y)].symbol() == "\u{256e}")
            .expect("detail panel corner");
        (left, right)
    }

    let (mut app, _) = make_test_app();
    let buf = render_app(&mut app, 80, 24);
    assert_eq!(content_span(&buf), (2, 77), "80 columns use the full width");

    // The bars span columns 2..=197 here; a 120-column content area sits in the middle.
    let buf = render_app(&mut app, 200, 50);
    assert_eq!(
        content_span(&buf),
        (40, 159),
        "wide content must be centred"
    );
}

#[test]
fn test_detail_facts_are_built_in_update_not_render() {
    let (mut app, _tmp) = make_test_app();
    let buf = render_updated(&mut app, 120, 24);
    assert!(
        row_containing(&buf, "stowed")
            .expect("stowed fact missing")
            .contains('\u{2013}'),
        "no cached status yet, so the count is unknown"
    );

    app.ctx
        .status_cache
        .borrow_mut()
        .insert(("home".to_string(), "test".to_string()), StowStatus::Stowed);

    // Render alone does not read the cache again.
    let buf = render_app(&mut app, 120, 24);
    assert!(
        row_containing(&buf, "stowed")
            .expect("stowed fact missing")
            .contains('\u{2013}'),
        "render must not rebuild the facts"
    );

    // The next update notices the new cache generation.
    let buf = render_updated(&mut app, 120, 24);
    assert!(
        row_containing(&buf, "stowed")
            .expect("stowed fact missing")
            .contains("1 of 1"),
        "update must refresh the facts after a status change"
    );
}

#[test]
fn test_detail_panel_describes_the_selected_item() {
    let (mut app, _tmp) = make_test_app();
    menu_mut(&mut app).index = 1;
    let buf = render_updated(&mut app, 80, 24);
    assert!(
        line_contains(&buf, "Import files"),
        "adopt description missing"
    );
    assert!(
        row_containing(&buf, "targets")
            .expect("adopt targets fact missing")
            .contains('~'),
        "targets should be shown relative to home"
    );
}

#[test]
fn test_detail_text_is_inset_from_the_panel_border() {
    let (mut app, _tmp) = make_test_app();
    let buf = render_updated(&mut app, 80, 24);
    let y = (0..buf.area().height)
        .find(|&y| row_text(&buf, y).contains("Pick groups"))
        .expect("stow description missing from the detail panel");
    let x = column_of(&buf, y, "Pick groups").expect("description column");
    assert_eq!(
        buf[(x - 2, y)].symbol(),
        "\u{2502}",
        "detail panel left border"
    );
    assert_eq!(
        buf[(x - 1, y)].symbol(),
        " ",
        "description must not touch the border"
    );
}
