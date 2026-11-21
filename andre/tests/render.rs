use ratatui::backend::TestBackend;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier};
use ratatui::Terminal;

use andre::app::App;
use andre::theme::{contrast_ratio, ThemeColors};
use andre::ui::icons::{Icons, NERD, UNICODE};
use andre::ui::packages::{align_columns, render_package_select, FlatItem};
use andre::ui::status_style::{status_color, status_word};
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
        "PackageSelect" => {
            // Skip the one-second scanning splash so the package rows are drawn.
            let mut packages =
                andre::components::stow::PackageSelectComponent::new(vec!["home".to_string()]);
            packages.entered_at = None;
            app.component_stack.push(Box::new(packages));
        }
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
        "UnstowBrowse" => {
            let path = app.ctx.home_dir.join("dotfiles/home");
            app.component_stack.push(Box::new(
                andre::components::unstow::UnstowBrowseComponent::with_path(
                    "home".to_string(),
                    path,
                ),
            ));
        }
        "AdoptFileSelect" => {
            let path = app.ctx.home_dir.join("dotfiles/home");
            app.component_stack.push(Box::new(
                andre::components::adopt::AdoptFileSelectComponent::with_path(
                    "home".to_string(),
                    path,
                ),
            ));
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
    assert!(
        top_bar_contains(&buf, "verbose ▰▰▰▱▱"),
        "verbosity meter not shown"
    );
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

/// Regression guard for the mark colours: an ON chip's mark is drawn in the
/// success colour and an OFF chip's mark in the error colour, so the state reads
/// at a glance.
#[test]
fn test_top_bar_marks_are_green_when_on_and_red_when_off() {
    let (mut app, _) = make_test_app();
    let colors = ThemeColors::from_theme(Theme::Default);
    let buf_off = render_app(&mut app, 140, 24);

    app.ctx.core.flags.dry_run = true;
    let buf_on = render_app(&mut app, 140, 24);

    assert_eq!(
        first_glyph_color(&buf_on, NERD.flag_on),
        Some(colors.success),
        "an ON mark must use the success colour"
    );
    assert_eq!(
        first_glyph_color(&buf_off, NERD.flag_off),
        Some(colors.error),
        "an OFF mark must use the error colour"
    );
}

/// Mixed marks in one row, located by column: each mark sits two columns left
/// of its label (`<mark> <label>`), and the action pill is filled with `primary`.
#[test]
fn test_top_bar_marks_sit_beside_their_labels_by_column() {
    let (mut app, _) = make_test_app();
    app.ctx.core.flags.dry_run = true;
    app.ctx.core.flags.dotfiles = false;
    let colors = ThemeColors::from_theme(Theme::Default);
    let buf = render_app(&mut app, 120, 24);
    let y = top_bar_row(&buf);

    let pill = column_of(&buf, y, " stow ").expect("action pill missing");
    assert_eq!(buf[(pill, y)].bg, colors.primary, "pill must be filled");

    let cases = [
        ("dry-run", NERD.flag_on, colors.success),
        ("no-fold", NERD.flag_off, colors.error),
        ("adopt", NERD.flag_off, colors.error),
        ("dotfiles", NERD.flag_off, colors.error),
    ];
    for (label, glyph, fg) in cases {
        let x = column_of(&buf, y, label).unwrap_or_else(|| panic!("{label} missing"));
        let mark = &buf[(x - 2, y)];
        assert_eq!(mark.symbol(), glyph, "{label}: wrong mark");
        assert_eq!(mark.fg, fg, "{label}: wrong mark colour");
        assert!(x - 2 > pill, "{label}: mark must follow the pill");
    }
}

/// The action pill, the chips and their padding form one group, centred in the
/// bar's width (the terminal less the frame border and the one-column margin).
/// The odd column, if any, goes on the right.
#[test]
fn test_top_bar_group_is_centred_at_80_and_120_columns() {
    let colors = ThemeColors::from_theme(Theme::Default);
    for width in [80u16, 120] {
        let (mut app, _) = make_test_app();
        app.ctx.core.flags.action = StowAction::Unstow;
        app.ctx.core.flags.verbosity = 1;
        app.ctx.core.flags.dry_run = true;
        app.ctx.core.flags.dotfiles = true;
        let buf = render_app(&mut app, width, 24);
        let y = top_bar_row(&buf);

        // Columns 0 and the last are the frame border; column 1 is the margin.
        let bar = 2..width - 2;
        let drawn = |x: u16| buf[(x, y)].symbol() != " " || buf[(x, y)].bg != colors.background;
        let first = bar.clone().find(|&x| drawn(x)).expect("top bar is empty");
        let last = bar.clone().rev().find(|&x| drawn(x)).unwrap();
        let left = (first - bar.start) as usize;
        let right = (bar.end - 1 - last) as usize;
        assert!(
            right == left || right == left + 1,
            "{width} columns: left pad {left}, right pad {right}: {:?}",
            row_text(&buf, y)
        );
    }
}

#[test]
fn test_top_bar_uses_unicode_icons_when_configured() {
    let (mut app, _) = make_test_app();
    app.ctx.core.icons = IconSet::Unicode;
    let buf = render_app(&mut app, 80, 24);
    assert!(top_bar_contains(&buf, "✗"), "unicode off-mark missing");
    assert!(!top_bar_contains(&buf, NERD.flag_off), "nerd glyph leaked");
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
        top_bar_contains(&buf, "verbose ▰▰▱▱▱"),
        "verbosity toggle not live"
    );
    assert!(
        top_bar_contains(&buf, "dry-run") && top_bar_contains(&buf, NERD.flag_on),
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
    assert!(
        top.contains("verbose ▰▰▰▰▰"),
        "full labels expected: {top:?}"
    );
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
            "verbose ▰▰▰▱▱",
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

/// Worst case at 80 columns: unstow, every flag on, debug. Each verbosity level
/// keeps full labels, one meter of `MAX_VERBOSITY` segments, and the divider.
#[test]
fn test_top_bar_verbose_meter_fits_80_columns_at_low_mid_and_max_level() {
    let max = andre_core::MAX_VERBOSITY;
    let colors = ThemeColors::from_theme(Theme::Default);
    for level in [0, 2, max] {
        let (mut app, _tmp) = make_test_app();
        app.ctx.core.flags.action = StowAction::Unstow;
        app.ctx.core.flags.verbosity = level;
        app.ctx.core.flags.dry_run = true;
        app.ctx.core.flags.no_folding = true;
        app.ctx.core.flags.adopt = true;
        app.ctx.core.flags.dotfiles = true;
        app.ctx.debug = true;
        let buf = render_app(&mut app, 80, 24);
        let top = row_text(&buf, top_bar_row(&buf));
        let meter = format!(
            "{}{}",
            NERD.meter_full.repeat(level as usize),
            NERD.meter_empty.repeat((max - level) as usize)
        );
        for label in [
            "unstow", "verbose", "dry-run", "no-fold", "adopt", "dotfiles", "debug",
        ] {
            assert!(
                top.contains(label),
                "level {level}: {label:?} missing: {top:?}"
            );
        }
        assert!(
            top.contains(&format!("verbose {meter}  │")),
            "level {level}: meter then divider expected: {top:?}"
        );
        // The first `│` after the label is the divider; column 0 is the frame.
        let y = top_bar_row(&buf);
        let from = column_of(&buf, y, "verbose").expect("verbose label missing");
        let x = (from..buf.area().width)
            .find(|&x| buf[(x, y)].symbol() == "│")
            .expect("divider missing");
        assert_eq!(buf[(x, y)].fg, colors.muted, "divider colour");
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
    contrast_ratio(a, b).expect("theme colours are RGB")
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

/// Every screen the render tests can draw, with the cursor row that `push_screen`
/// leaves selected. Each one is checked for the shared cursor style.
const CURSOR_SCREENS: [&str; 7] = [
    "GroupSelect",
    "PackageSelect",
    "Settings",
    "AdoptTargetSelect",
    "UnstowTargetSelect",
    "UnstowBrowse",
    "AdoptFileSelect",
];

fn buffer_text(buf: &Buffer) -> String {
    (0..buf.area().height)
        .map(|y| row_text(buf, y))
        .collect::<Vec<_>>()
        .join("\n")
}

/// The accent bar of the cursor row, if the screen draws one.
fn find_accent(buf: &Buffer) -> Option<(u16, u16)> {
    (0..buf.area().height)
        .flat_map(|y| (0..buf.area().width).map(move |x| (x, y)))
        .find(|&(x, y)| buf[(x, y)].symbol() == "\u{258c}")
}

#[test]
fn test_no_screen_draws_the_old_cursor_markers() {
    for screen in screen_names() {
        let (mut app, _tmp) = make_test_app();
        push_screen(&mut app, screen);
        let buf = render_updated(&mut app, 80, 24);
        let text = buffer_text(&buf);
        assert!(
            !text.contains(">>"),
            "{screen}: the \">>\" cursor marker is back"
        );
        assert!(
            !text.contains("-->"),
            "{screen}: the \"-->\" cursor marker is back"
        );
    }
}

/// Puts the settings cursor on the first setting, past the info rows above it.
fn select_first_setting(app: &mut App) {
    app.component_stack
        .last_mut()
        .unwrap()
        .as_any_mut()
        .downcast_mut::<andre::components::settings::SettingsComponent>()
        .unwrap()
        .cursor = 2;
}

#[test]
fn test_every_list_screen_draws_its_cursor_with_the_shared_style() {
    let colors = ThemeColors::from_theme(Theme::Default);
    for screen in CURSOR_SCREENS {
        let (mut app, _tmp) = make_test_app();
        push_screen(&mut app, screen);
        if screen == "Settings" {
            select_first_setting(&mut app);
        }
        let buf = render_updated(&mut app, 80, 24);

        let (x0, y) = find_accent(&buf)
            .unwrap_or_else(|| panic!("{screen}: no cursor accent bar on the cursor row"));
        assert_eq!(
            buf[(x0, y)].bg,
            colors.background,
            "{screen}: accent bar must sit on the normal background"
        );
        let right_border = (x0 + 1..buf.area().width)
            .find(|&x| buf[(x, y)].symbol() == "\u{2502}")
            .unwrap_or_else(|| panic!("{screen}: no panel border after the cursor row"));
        for x in x0 + 1..right_border {
            assert_eq!(
                buf[(x, y)].bg,
                colors.selection,
                "{screen}: cursor row not filled at column {x}"
            );
        }
    }
}

/// On the cursor row, text is drawn in the label or accent colour, in every theme and on
/// every list screen. A glyph may keep its status colour, but only where that colour
/// reads at 3:1 on the selection fill.
#[test]
fn test_cursor_row_text_is_label_coloured_and_glyphs_read_at_3_to_1() {
    for theme in Theme::ALL {
        let colors = ThemeColors::from_theme(theme);
        assert!(
            contrast(colors.secondary, colors.selection) >= 4.5,
            "{}: label on the selection fill is below 4.5:1",
            theme.name()
        );
        assert!(
            contrast(colors.primary, colors.selection) >= 3.0,
            "{}: accent on the selection fill is below 3:1",
            theme.name()
        );
        for screen in CURSOR_SCREENS {
            let (mut app, _tmp) = make_test_app();
            app.ctx.core.theme = theme;
            push_screen(&mut app, screen);
            if screen == "Settings" {
                select_first_setting(&mut app);
            }
            let buf = render_updated(&mut app, 80, 24);

            let (x0, y) = find_accent(&buf)
                .unwrap_or_else(|| panic!("{}: {screen}: no cursor accent", theme.name()));
            let right_border = (x0 + 1..buf.area().width)
                .find(|&x| buf[(x, y)].symbol() == "\u{2502}")
                .expect("panel border after the cursor row");
            for x in x0 + 1..right_border {
                let cell = &buf[(x, y)];
                if cell.symbol() == " " || cell.fg == colors.secondary || cell.fg == colors.primary
                {
                    continue;
                }
                assert!(
                    contrast(cell.fg, colors.selection) >= 3.0,
                    "{}: {screen}: cursor row glyph at column {x} is {:?}, below 3:1 on the fill",
                    theme.name(),
                    cell.fg
                );
            }
        }
    }
}

#[test]
fn test_panel_titles_are_lowercase_and_skip_the_breadcrumb() {
    // (screen, lowercase title, the title-case title it replaced). Help and Settings
    // have no old panel title to check: the banner draws their breadcrumb.
    let cases = [
        ("GroupSelect", "select groups", "Select Groups"),
        ("PackageSelect", "select packages", "Select Packages"),
        ("Status", "stow status", "Stow Status"),
        ("Settings", "settings", ""),
        ("Help", "help", ""),
        (
            "AdoptTargetSelect",
            "select target for adoption",
            "Select Target for Adoption",
        ),
        (
            "UnstowTargetSelect",
            "select target for unstow",
            "Select Target for Unstow",
        ),
    ];
    for (screen, title, old_title) in cases {
        let (mut app, _tmp) = make_test_app();
        push_screen(&mut app, screen);
        let buf = render_updated(&mut app, 80, 24);
        let text = buffer_text(&buf);
        assert!(
            text.contains(title),
            "{screen}: lowercase panel title {title:?} missing"
        );
        if !old_title.is_empty() {
            assert!(
                !text.contains(old_title),
                "{screen}: old panel title {old_title:?} is back"
            );
        }
        // The compact banner is the only place the breadcrumb is drawn now.
        assert_eq!(
            text.matches("Main >").count(),
            1,
            "{screen}: the breadcrumb is repeated in a panel title"
        );
    }

    // Legends are titled like every other panel and have no header row.
    for screen in ["PackageSelect", "Status", "UnstowBrowse"] {
        let (mut app, tmp) = make_test_app();
        add_status_fixtures(tmp.path());
        push_screen(&mut app, screen);
        let buf = render_updated(&mut app, 80, 24);
        let text = buffer_text(&buf);
        assert!(text.contains(" legend "), "{screen}: legend title missing");
        assert!(
            !text.contains("Legend:"),
            "{screen}: legend header row is back"
        );
    }
}

#[test]
fn test_group_checkboxes_follow_the_icon_set() {
    let cases = [
        (IconSet::Nerd, NERD.checkbox_on, NERD.checkbox_off),
        (IconSet::Unicode, UNICODE.checkbox_on, UNICODE.checkbox_off),
    ];
    for (icons, on, off) in cases {
        for (selected, glyph, other) in [(vec!["home".to_string()], on, off), (Vec::new(), off, on)]
        {
            let (mut app, _tmp) = make_test_app();
            app.ctx.core.icons = icons;
            let mut groups = andre::components::stow::GroupSelectComponent::default();
            groups.selected = selected;
            app.component_stack.push(Box::new(groups));
            let buf = render_updated(&mut app, 80, 24);
            let row = row_containing(&buf, "home").expect("group row missing");
            assert!(
                row.contains(glyph),
                "{icons:?}: checkbox {glyph:?} missing: {row:?}"
            );
            assert!(!row.contains(other), "{icons:?}: wrong checkbox in {row:?}");
            let text = buffer_text(&buf);
            assert!(
                !text.contains("[x]") && !text.contains("[ ]"),
                "text checkboxes are back"
            );
        }
    }
}

#[test]
fn test_package_status_glyphs_follow_the_icon_set() {
    for icons in [IconSet::Nerd, IconSet::Unicode] {
        let (mut app, _tmp) = make_test_app();
        app.ctx.core.icons = icons;
        push_screen(&mut app, "PackageSelect");
        let buf = render_updated(&mut app, 80, 24);
        // The fixture package is not linked yet, so it reads as unstowed.
        let glyph = Icons::for_set(icons).status(StowStatus::Unstowed);
        let row = row_containing(&buf, "test  home").expect("package row missing");
        assert!(
            row.contains(glyph),
            "{icons:?}: status glyph {glyph:?} missing: {row:?}"
        );
    }
}

/// One package per status the status screen can show, in the test home. `plain` is
/// unstowed, `linked` stowed, `clash` conflicts with a real file, `half` is partly
/// stowed and `hollow` has no files.
const STATUS_FIXTURES: [(&str, StowStatus); 5] = [
    ("plain", StowStatus::Unstowed),
    ("linked", StowStatus::Stowed),
    ("clash", StowStatus::Conflict),
    ("half", StowStatus::Partial),
    ("hollow", StowStatus::Empty),
];

/// Writes the packages in `STATUS_FIXTURES` under the test home and links them, or
/// blocks them, as their status needs.
fn add_status_fixtures(home: &std::path::Path) {
    let source = home.join("dotfiles/home");
    let write = |package: &str, file: &str| {
        let dir = source.join(package);
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join(file), "x").unwrap();
        dir.join(file)
    };
    write("plain", ".plain_rc");
    std::os::unix::fs::symlink(write("linked", ".linked_rc"), home.join(".linked_rc")).unwrap();
    write("clash", ".clash_rc");
    std::fs::write(home.join(".clash_rc"), "real file").unwrap();
    write("half", ".half_a");
    write("half", ".half_b");
    std::os::unix::fs::symlink(source.join("half/.half_a"), home.join(".half_a")).unwrap();
    std::fs::create_dir_all(source.join("hollow")).unwrap();
}

/// The first cell on row `y` drawn with `glyph`, as (column, foreground).
fn glyph_on_row(buf: &Buffer, y: u16, glyph: &str) -> Option<(u16, Color)> {
    (0..buf.area().width)
        .find(|&x| buf[(x, y)].symbol() == glyph)
        .map(|x| (x, buf[(x, y)].fg))
}

/// The package status vocabulary is the same on the package select and the status
/// screens: one glyph, one colour and one lowercase word per status.
#[test]
fn test_package_and_status_screens_share_glyph_colour_and_word() {
    for set in [IconSet::Nerd, IconSet::Unicode] {
        let icons = Icons::for_set(set);

        let (mut packages_app, tmp) = make_test_app();
        add_status_fixtures(tmp.path());
        packages_app.ctx.core.icons = set;
        let colors = ThemeColors::from_theme(packages_app.ctx.core.theme);
        push_screen(&mut packages_app, "PackageSelect");
        let packages = render_updated(&mut packages_app, 80, 24);
        // The cursor row keeps its status glyph only where the colour reads at 3:1, so
        // its colour is checked by the cursor-row test instead.
        let cursor_row = find_accent(&packages).map(|(_, y)| y);

        let (mut status_app, tmp) = make_test_app();
        add_status_fixtures(tmp.path());
        status_app.ctx.core.icons = set;
        push_screen(&mut status_app, "Status");
        let status = render_updated(&mut status_app, 80, 24);

        let mut checked = 0;
        for (name, state) in STATUS_FIXTURES {
            let glyph = icons.status(state);
            let word = status_word(state);
            let colour = status_color(&colors, state);

            let package_row = (0..packages.area().height).find(|&y| {
                let text = row_text(&packages, y);
                text.contains(name) && text.contains("home")
            });
            let status_row = (0..status.area().height)
                .find(|&y| row_text(&status, y).contains(&format!("{name}  {word}")));
            let package_y = package_row.unwrap_or_else(|| panic!("{set:?}: {name} row missing"));
            let status_y =
                status_row.unwrap_or_else(|| panic!("{set:?}: {name} status row missing"));

            let (_, status_fg) = glyph_on_row(&status, status_y, glyph)
                .unwrap_or_else(|| panic!("{set:?}: {name}: status glyph {glyph:?} missing"));
            assert_eq!(status_fg, colour, "{set:?}: {name} on the status screen");

            if Some(package_y) != cursor_row {
                let (_, package_fg) = glyph_on_row(&packages, package_y, glyph)
                    .unwrap_or_else(|| panic!("{set:?}: {name}: glyph {glyph:?} missing"));
                assert_eq!(package_fg, colour, "{set:?}: {name} on the package select");
                checked += 1;
            }
        }
        assert!(
            checked >= 4,
            "{set:?}: only {checked} package rows were checked"
        );
    }
}

/// A package whose name is 60 columns wide, so the package column has to be cut.
fn long_package() -> String {
    "a".repeat(60)
}

/// A package with a CJK name: each character is two columns wide.
const CJK_PACKAGE: &str = "漢字漢字";

fn write_package(source: &std::path::Path, package: &str, file: &str) {
    let dir = source.join(package);
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join(file), "x").unwrap();
}

/// Two groups, so the group column has two names. `home` holds the status fixtures and
/// the 60-column package. `workstation` holds `deploy` and the CJK package.
fn make_two_group_app() -> (App, tempfile::TempDir) {
    let tmp = tempfile::TempDir::new().unwrap();
    let path = tmp.path().join("andre.yml");
    std::fs::write(
        &path,
        r#"
global:
  stow:
    verbosity: 0
groups:
  home:
    source: dotfiles/home
    target: "~"
  workstation:
    source: dotfiles/workstation
    target: "~/work"
"#,
    )
    .unwrap();
    add_status_fixtures(tmp.path());
    write_package(
        &tmp.path().join("dotfiles/home"),
        &long_package(),
        ".long_rc",
    );
    let work = tmp.path().join("dotfiles/workstation");
    write_package(&work, "deploy", ".deploy_rc");
    write_package(&work, CJK_PACKAGE, ".kanji_rc");
    std::fs::create_dir_all(tmp.path().join("work")).unwrap();
    let app = App::with_home(path, tmp.path().to_path_buf()).unwrap();
    (app, tmp)
}

fn push_packages(app: &mut App, groups: &[&str]) {
    let groups = groups.iter().map(|group| group.to_string()).collect();
    let mut packages = andre::components::stow::PackageSelectComponent::new(groups);
    packages.entered_at = None;
    app.component_stack.push(Box::new(packages));
}

/// Each package row of `make_two_group_app`: the text that finds it, its group, its
/// status, and whether it has files to show as detail.
fn two_group_rows() -> Vec<(String, &'static str, StowStatus, bool)> {
    let mut rows: Vec<_> = STATUS_FIXTURES
        .iter()
        .map(|&(name, state)| (name.to_string(), "home", state, name != "hollow"))
        .collect();
    // The long name is cut to 32 columns: 31 characters and `…`.
    rows.push((
        format!("{}…", "a".repeat(31)),
        "home",
        StowStatus::Unstowed,
        true,
    ));
    rows.push((
        "deploy".to_string(),
        "workstation",
        StowStatus::Unstowed,
        true,
    ));
    rows.push(("漢".to_string(), "workstation", StowStatus::Unstowed, true));
    rows
}

/// The first row where `locator` is drawn, matched cell by cell so that a wide character
/// counts as two columns.
fn package_row(buf: &Buffer, locator: &str) -> u16 {
    (0..buf.area().height)
        .find(|&y| column_of(buf, y, locator).is_some())
        .unwrap_or_else(|| panic!("{locator} row missing"))
}

/// On the package select, the group, status word and detail columns line up on every
/// package row, including the rows with a 60-column name and a CJK name.
#[test]
fn test_package_rows_align_group_status_and_detail_columns() {
    let (mut app, _tmp) = make_two_group_app();
    push_packages(&mut app, &["home", "workstation"]);
    // At 80x24 only six package rows fit beside the legend, so this needs more height.
    let buf = render_updated(&mut app, 80, 30);

    let mut group_cols = Vec::new();
    let mut word_cols = Vec::new();
    let mut detail_cols = Vec::new();
    for (locator, group, state, has_detail) in two_group_rows() {
        let y = package_row(&buf, &locator);
        group_cols
            .push(column_of(&buf, y, group).unwrap_or_else(|| panic!("{locator}: group missing")));
        let word = status_word(state);
        word_cols
            .push(column_of(&buf, y, &word).unwrap_or_else(|| panic!("{locator}: {word} missing")));
        if has_detail {
            detail_cols
                .push(column_of(&buf, y, "(").unwrap_or_else(|| panic!("{locator}: no detail")));
        }
    }
    assert!(
        group_cols.windows(2).all(|pair| pair[0] == pair[1]),
        "group names start at different columns: {group_cols:?}"
    );
    assert!(
        word_cols.windows(2).all(|pair| pair[0] == pair[1]),
        "status words start at different columns: {word_cols:?}"
    );
    assert!(
        detail_cols.windows(2).all(|pair| pair[0] == pair[1]),
        "detail columns start at different columns: {detail_cols:?}"
    );
}

/// A 60-column package name is cut with `…`, and no row loses its status word at 80x24.
#[test]
fn test_long_package_name_is_cut_and_every_status_word_stays_visible() {
    let (mut app, _tmp) = make_two_group_app();
    push_packages(&mut app, &["home"]);
    // The six `home` packages fill the 80x24 list exactly, so every row is on screen.
    let buf = render_updated(&mut app, 80, 24);

    let long_row = package_row(&buf, &format!("{}…", "a".repeat(31)));
    assert!(
        column_of(&buf, long_row, &long_package()).is_none(),
        "the full 60-column name is drawn"
    );
    for (locator, group, state, _) in two_group_rows() {
        if group != "home" {
            continue;
        }
        let y = package_row(&buf, &locator);
        let word = status_word(state);
        assert!(
            column_of(&buf, y, &word).is_some(),
            "{locator}: status word {word} cut off"
        );
    }
}

/// A CJK package name is padded by its display width, so its group column matches the
/// ASCII row in the same group.
#[test]
fn test_cjk_package_name_keeps_the_group_column_in_line() {
    let (mut app, _tmp) = make_two_group_app();
    push_packages(&mut app, &["home", "workstation"]);
    let buf = render_updated(&mut app, 80, 30);

    let cjk = package_row(&buf, "漢");
    let ascii = package_row(&buf, "deploy");
    assert_eq!(
        column_of(&buf, cjk, "workstation"),
        column_of(&buf, ascii, "workstation"),
        "the CJK row's group column differs from the ASCII row's"
    );
    assert!(column_of(&buf, cjk, &status_word(StowStatus::Unstowed)).is_some());
}

/// The column of the top-right corner of the panel whose top border shows `title`:
/// the panel's right border.
fn panel_right_column(buf: &Buffer, title: &str) -> u16 {
    let y = (0..buf.area().height)
        .find(|&y| row_text(buf, y).contains(title))
        .unwrap_or_else(|| panic!("{title} panel missing"));
    (0..buf.area().width)
        .rev()
        .find(|&x| buf[(x, y)].symbol() == "\u{256e}")
        .unwrap_or_else(|| panic!("{title} panel has no top-right corner"))
}

/// The rows strictly between the top and bottom borders of the panel titled `title`.
fn panel_rows(buf: &Buffer, title: &str) -> std::ops::Range<u16> {
    let column = panel_right_column(buf, title);
    let top = (0..buf.area().height)
        .find(|&y| row_text(buf, y).contains(title))
        .unwrap_or_else(|| panic!("{title} panel missing"));
    let bottom = (top + 1..buf.area().height)
        .find(|&y| buf[(column, y)].symbol() == "\u{256f}")
        .unwrap_or_else(|| panic!("{title} panel has no bottom-right corner"));
    top + 1..bottom
}

/// The rows in `rows` whose cell in `column` is drawn in `colour`.
fn rows_with_fg(buf: &Buffer, column: u16, rows: std::ops::Range<u16>, colour: Color) -> Vec<u16> {
    rows.filter(|&y| buf[(column, y)].fg == colour).collect()
}

/// The top border title of the package list panel.
const PACKAGES_TITLE: &str = " select packages ";

#[test]
fn test_package_list_draws_its_scrollbar_thumb_when_rows_overflow() {
    let (mut app, _tmp) = make_two_group_app();
    push_packages(&mut app, &["home", "workstation"]);
    let colors = ThemeColors::from_theme(app.ctx.core.theme);
    assert_ne!(
        colors.indicator, colors.border,
        "thumb and track must differ"
    );
    let buf = render_updated(&mut app, 80, 24);

    let column = panel_right_column(&buf, PACKAGES_TITLE);
    let rows = panel_rows(&buf, PACKAGES_TITLE);
    let thumb = rows_with_fg(&buf, column, rows.clone(), colors.indicator);
    assert!(!thumb.is_empty(), "no scrollbar thumb on the right border");
    assert!(thumb.len() < rows.len(), "the thumb fills the whole track");
    for y in rows.filter(|y| !thumb.contains(y)) {
        assert_eq!(
            buf[(column, y)].fg,
            colors.border,
            "track cell at row {y} is not in the border colour"
        );
    }
}

#[test]
fn test_package_list_draws_no_scrollbar_when_every_row_fits() {
    let (mut app, _tmp) = make_two_group_app();
    push_packages(&mut app, &["home"]);
    let colors = ThemeColors::from_theme(app.ctx.core.theme);
    let buf = render_updated(&mut app, 80, 24);

    let column = panel_right_column(&buf, PACKAGES_TITLE);
    let rows = panel_rows(&buf, PACKAGES_TITLE);
    assert!(rows_with_fg(&buf, column, rows.clone(), colors.indicator).is_empty());
    assert!(
        rows.clone()
            .all(|y| buf[(column, y)].symbol() == "\u{2502}"),
        "the right border is not drawn on every row"
    );
}

#[test]
fn test_package_list_right_border_lines_up_with_the_legend() {
    let (mut app, _tmp) = make_two_group_app();
    push_packages(&mut app, &["home", "workstation"]);
    let buf = render_updated(&mut app, 80, 24);
    assert_eq!(
        panel_right_column(&buf, PACKAGES_TITLE),
        panel_right_column(&buf, " legend "),
        "the package list's right border is not in the legend's column"
    );
}

/// A config with `count` groups and no packages: enough groups to overflow the list.
fn make_many_group_app(count: usize) -> (App, tempfile::TempDir) {
    let tmp = tempfile::TempDir::new().unwrap();
    let path = tmp.path().join("andre.yml");
    let mut yaml = String::from("global:\n  stow:\n    verbosity: 0\ngroups:\n");
    for i in 0..count {
        std::fs::create_dir_all(tmp.path().join(format!("dotfiles/group{i:02}"))).unwrap();
        yaml.push_str(&format!(
            "  group{i:02}:\n    source: dotfiles/group{i:02}\n    target: \"~\"\n"
        ));
    }
    std::fs::write(&path, yaml).unwrap();
    let app = App::with_home(path, tmp.path().to_path_buf()).unwrap();
    (app, tmp)
}

#[test]
fn test_group_list_draws_its_scrollbar_thumb_when_groups_overflow() {
    let (mut app, _tmp) = make_many_group_app(30);
    push_screen(&mut app, "GroupSelect");
    let colors = ThemeColors::from_theme(app.ctx.core.theme);
    let buf = render_updated(&mut app, 80, 24);

    let title = " select groups ";
    let column = panel_right_column(&buf, title);
    let rows = panel_rows(&buf, title);
    assert!(
        !rows_with_fg(&buf, column, rows, colors.indicator).is_empty(),
        "no scrollbar thumb on the group list's right border"
    );
}

#[test]
fn test_execute_list_draws_its_scrollbar_thumb_on_the_panel_border() {
    use andre::components::execute::ExecuteComponent;
    use andre::execute::{ExecutionResult, ExecutionState};

    let (mut app, _tmp) = make_test_app();
    let colors = ThemeColors::from_theme(app.ctx.core.theme);
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

    let buf = render_app(&mut app, 80, 24);
    let title = " execution ";
    let column = panel_right_column(&buf, title);
    let rows = panel_rows(&buf, title);
    assert!(
        !rows_with_fg(&buf, column, rows, colors.indicator).is_empty(),
        "no scrollbar thumb on the execute list's panel border"
    );
}

/// The top border title of the stow status panel.
const STATUS_TITLE: &str = " stow status ";

#[test]
fn test_status_list_draws_its_scrollbar_thumb_when_rows_overflow() {
    let (mut app, _tmp) = make_two_group_app();
    push_screen(&mut app, "Status");
    let colors = ThemeColors::from_theme(app.ctx.core.theme);
    let buf = render_updated(&mut app, 80, 24);

    let column = panel_right_column(&buf, STATUS_TITLE);
    let rows = panel_rows(&buf, STATUS_TITLE);
    assert_eq!(rows.len(), 6, "the status list should show 6 rows at 80x24");
    assert!(
        !rows_with_fg(&buf, column, rows, colors.indicator).is_empty(),
        "no scrollbar thumb on the status list's right border"
    );
}

#[test]
fn test_status_list_draws_no_scrollbar_when_every_row_fits() {
    let (mut app, _tmp) = make_empty_app();
    push_screen(&mut app, "Status");
    let colors = ThemeColors::from_theme(app.ctx.core.theme);
    let buf = render_updated(&mut app, 80, 24);

    let column = panel_right_column(&buf, STATUS_TITLE);
    let rows = panel_rows(&buf, STATUS_TITLE);
    assert!(rows_with_fg(&buf, column, rows.clone(), colors.indicator).is_empty());
    assert!(
        rows.clone()
            .all(|y| buf[(column, y)].symbol() == "\u{2502}"),
        "the right border is not drawn on every row"
    );
}

/// One group. `wide` has three long file names, so its detail is longer than a row;
/// `plain` has one short name.
fn make_long_detail_app() -> (App, tempfile::TempDir) {
    let tmp = tempfile::TempDir::new().unwrap();
    let path = tmp.path().join("andre.yml");
    std::fs::write(
        &path,
        r#"
global:
  stow:
    verbosity: 0
groups:
  home:
    source: dotfiles/home
    target: "~"
"#,
    )
    .unwrap();
    let source = tmp.path().join("dotfiles/home");
    for name in ["alpha", "beta", "gamma"] {
        write_package(
            &source,
            "wide",
            &format!(".wide_{name}_configuration_file_rc"),
        );
    }
    write_package(&source, "plain", ".plain_rc");
    let app = App::with_home(path, tmp.path().to_path_buf()).unwrap();
    (app, tmp)
}

#[test]
fn test_package_detail_is_cut_with_an_ellipsis_inside_the_border() {
    let (mut app, _tmp) = make_long_detail_app();
    push_packages(&mut app, &["home"]);
    let buf = render_updated(&mut app, 80, 24);

    let y = package_row(&buf, "wide");
    let border = panel_right_column(&buf, PACKAGES_TITLE);
    assert_eq!(
        buf[(border - 1, y)].symbol(),
        "\u{2026}",
        "the detail is not cut with an ellipsis at the panel edge"
    );
    assert_eq!(
        buf[(border, y)].symbol(),
        "\u{2502}",
        "the row runs past the border"
    );
}

#[test]
fn test_short_package_detail_keeps_its_closing_paren() {
    let (mut app, _tmp) = make_long_detail_app();
    push_packages(&mut app, &["home"]);
    let buf = render_updated(&mut app, 80, 24);

    let y = package_row(&buf, "plain");
    assert!(
        row_text(&buf, y).contains("(.plain_rc)"),
        "a short detail lost its closing paren"
    );
}

#[test]
fn test_package_detail_needs_two_columns_of_room() {
    let colors = ThemeColors::from_theme(Theme::Default);
    let mut items = vec![FlatItem {
        group_name: "home".into(),
        package_name: "wide".into(),
        selected: false,
        status: StowStatus::Unstowed,
        preview: vec![".wide_alpha_configuration_file_rc".into()],
        package_display: String::new(),
        group_display: String::new(),
        package_pad: 0,
        group_pad: 0,
    }];
    align_columns(&mut items);

    // A 30-column panel leaves one column for the detail: too little, so it is dropped.
    // A 31-column panel leaves two: the detail opens with `(` and is cut with `…`.
    for (width, detail) in [(30, None), (31, Some("(\u{2026}"))] {
        let backend = TestBackend::new(width, 12);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal
            .draw(|f| {
                render_package_select(
                    f,
                    Rect::new(0, 0, width, 12),
                    &colors,
                    Icons::for_set(IconSet::Unicode),
                    PACKAGES_TITLE,
                    &items,
                    0,
                    None,
                    0,
                );
            })
            .unwrap();
        let text = buffer_text(terminal.backend().buffer());
        match detail {
            Some(fragment) => assert!(
                text.contains(fragment),
                "width {width}: the detail is missing"
            ),
            None => assert!(
                !text.contains('('),
                "width {width}: the detail is drawn with no room for it"
            ),
        }
    }
}
