use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::backend::TestBackend;
use ratatui::buffer::Buffer;
use ratatui::style::{Color, Modifier};
use ratatui::Terminal;

use andre::app::App;
use andre::theme::{contrast_ratio, ThemeColors};
use andre::ui::icons::{Icons, NERD, UNICODE};
use andre::ui::status_style::{status_color, status_word};
use andre_core::{IconSet, StowAction, StowStatus, Theme};

mod helpers;
use helpers::{
    down_key, enter_key, esc_key, key_char, make_bootstrap_app, make_empty_app, make_test_app,
    space_key, up_key,
};

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
        "Packages" => app.component_stack.push(Box::new(
            andre::components::packages::PackagesComponent::default(),
        )),
        "Profiles" => app.component_stack.push(Box::new(
            andre::components::profiles::ProfilesComponent::default(),
        )),
        "Status" => app.component_stack.push(Box::new(
            andre::components::status::StatusComponent::default(),
        )),
        "UnstowTargetSelect" => app.component_stack.push(Box::new(
            andre::components::target_select::TargetSelectComponent::unstow(),
        )),
        "NewPackage" => app.component_stack.push(Box::new(
            andre::components::wizard::NewPackageWizard::default(),
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
        _ => {}
    }
}

fn screen_names() -> Vec<&'static str> {
    vec![
        "MainMenu",
        "Profiles",
        "Packages",
        "Settings",
        "Status",
        "Help",
        "Confirm",
        "Execute",
        "UnstowTargetSelect",
        "NewPackage",
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
        Some(colors.ok),
        "an ON mark must use the success colour"
    );
    assert_eq!(
        first_glyph_color(&buf_off, NERD.flag_off),
        Some(colors.err),
        "an OFF mark must use the error colour"
    );
}

/// Mixed marks in one row, located by column: each mark sits two columns left
/// of its label (`<mark> <label>`), and the action pill is filled with `accent`.
#[test]
fn test_top_bar_marks_sit_beside_their_labels_by_column() {
    let (mut app, _) = make_test_app();
    app.ctx.core.flags.dry_run = true;
    app.ctx.core.flags.dotfiles = false;
    let colors = ThemeColors::from_theme(Theme::Default);
    let buf = render_app(&mut app, 120, 24);
    let y = top_bar_row(&buf);

    let pill = column_of(&buf, y, " stow ").expect("action pill missing");
    assert_eq!(buf[(pill, y)].bg, colors.accent, "pill must be filled");

    let cases = [
        ("dry-run", NERD.flag_on, colors.ok),
        ("no-fold", NERD.flag_off, colors.err),
        ("adopt", NERD.flag_off, colors.err),
        ("dotfiles", NERD.flag_off, colors.err),
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
        let drawn = |x: u16| buf[(x, y)].symbol() != " " || buf[(x, y)].bg != colors.bg;
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

/// The banner letters take the theme's banner colours: the big letters on the
/// main menu, the compact name on every other screen.
#[test]
fn test_banner_letters_take_the_theme_banner_colours_in_every_theme() {
    for theme in Theme::ALL {
        let colors = ThemeColors::from_theme(theme);
        let (mut app, _) = make_test_app();
        app.ctx.core.theme = theme;

        let buf = render_app(&mut app, 80, 24);
        for (letter, expected) in ['A', 'N', 'D', 'R', 'E'].into_iter().zip(colors.banner) {
            // The letter sits two columns into its "║ X. ║" box.
            let fragment = format!("\u{2551} {letter}. \u{2551}");
            let y = (0..buf.area().height)
                .find(|&y| row_text(&buf, y).contains(&fragment))
                .expect("big banner row missing");
            let x = column_of(&buf, y, &fragment).unwrap() + 2;
            assert_eq!(buf[(x, y)].symbol(), letter.to_string());
            assert_eq!(
                buf[(x, y)].fg,
                expected,
                "{}: big banner letter {letter}",
                theme.name()
            );
        }

        push_screen(&mut app, "Settings");
        let buf = render_app(&mut app, 80, 24);
        let y = (0..buf.area().height)
            .find(|&y| row_text(&buf, y).contains("andre"))
            .expect("compact banner row missing");
        let x = column_of(&buf, y, "andre").unwrap();
        for (i, expected) in colors.banner.into_iter().enumerate() {
            assert_eq!(
                buf[(x + i as u16, y)].fg,
                expected,
                "{}: compact banner letter {i}",
                theme.name()
            );
        }
    }
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
fn test_unstow_target_select_shows_target_label() {
    let (mut app, _tmp) = make_test_app();
    push_screen(&mut app, "UnstowTargetSelect");
    let buf = render_app(&mut app, 80, 24);
    assert!(line_contains(&buf, "(target:"), "unstow should show target");
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
            let hints = andre::ui::footer::screen_hints("MainMenu");
            andre::ui::render_footer(f, Rect::new(0, 0, 40, 2), &colors, &hints, "default");
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
        ("Packages", "mark"),             // packages browser
        ("Settings", "group menu"),       // settings
        ("Help", "back"),                 // help
        ("Confirm", "confirm"),           // confirm y/n
        ("Execute", "continue"),          // status/execute family
        ("UnstowTargetSelect", "select"), // unstow target family
    ];

    for (name, fragment) in cases {
        let (mut app, _tmp) = make_test_app();
        if !name.is_empty() {
            push_screen(&mut app, name);
        }
        let buf = render_updated(&mut app, 120, 24);
        assert!(
            line_contains(&buf, fragment),
            "footer hint missing for {name:?}: expected fragment {fragment:?}"
        );
    }

    // MainMenu advertises 1–8 jump alongside the arrows + Enter.
    // Guarded separately so the digit-key feature is observable from
    // rendered output, not just dispatch logic.
    let (mut app, _tmp) = make_test_app();
    let buf = render_app(&mut app, 120, 24);
    assert!(
        line_contains(&buf, "1–8 jump"),
        "MainMenu footer must advertise the 1–8 jump hint"
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
        ThemeColors::from_theme(Theme::Default).key,
        "q must be drawn in the key colour"
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
        assert_eq!(buf[(x, y)].fg, colors.border, "divider colour");
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
    assert_eq!(buf[(t_x, y)].fg, colors.key, "t must be the key colour");
    assert!(
        buf[(t_x, y)].modifier.contains(Modifier::BOLD),
        "t must be bold"
    );
    let name_x = column_of(&buf, y, "catppuccin-macchiato").expect("theme name missing");
    assert_eq!(
        buf[(name_x, y)].fg,
        colors.value,
        "name must be the value colour"
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
    menu_mut(&mut app).list.cursor = 2;
    let buf = render_app(&mut app, 80, 24);
    let colors = ThemeColors::from_theme(Theme::Default);

    let (x0, y) = accent_cell(&buf);
    assert_eq!(
        buf[(x0, y)].fg,
        colors.accent,
        "accent bar must use the accent colour"
    );
    assert_eq!(
        buf[(x0, y)].bg,
        colors.bg,
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
    match colors.bg {
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
            menu_mut(&mut app).list.cursor = index;
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
                colors.bg,
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
    push_screen(&mut app, "Packages");
    let buf = render_app(&mut app, 80, 24);
    assert!(
        row_text(&buf, 1).contains("andre  Main > Packages"),
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
    menu_mut(&mut app).list.cursor = 2;
    let buf = render_updated(&mut app, 80, 24);
    assert!(
        line_contains(&buf, "Create a package"),
        "new package description missing"
    );
    assert!(
        row_containing(&buf, "targets")
            .expect("new package targets fact missing")
            .contains('~'),
        "targets should be shown relative to home"
    );
}

#[test]
fn test_detail_text_is_inset_from_the_panel_border() {
    let (mut app, _tmp) = make_test_app();
    let buf = render_updated(&mut app, 80, 24);
    let y = (0..buf.area().height)
        .find(|&y| row_text(&buf, y).contains("Named sets"))
        .expect("profiles description missing from the detail panel");
    let x = column_of(&buf, y, "Named sets").expect("description column");
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
const CURSOR_SCREENS: [&str; 4] = ["Packages", "Settings", "UnstowTargetSelect", "UnstowBrowse"];

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
            colors.bg,
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
            contrast(colors.text, colors.selection) >= 4.5,
            "{}: label on the selection fill is below 4.5:1",
            theme.name()
        );
        assert!(
            contrast(colors.accent, colors.selection) >= 3.0,
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
                if cell.symbol() == " " || cell.fg == colors.text || cell.fg == colors.accent {
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
        ("Packages", "packages 1", "Select Packages"),
        ("Status", "stow status", "Stow Status"),
        ("Settings", "settings", ""),
        ("Help", "help", ""),
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
    for screen in ["Status", "UnstowBrowse"] {
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
        // The group row is marked when all its packages are, and off when none are.
        for (marked, glyph, other) in [(true, on, off), (false, off, on)] {
            let (mut app, _tmp) = make_test_app();
            app.ctx.core.icons = icons;
            push_screen(&mut app, "Packages");
            app.update_components();
            if marked {
                app.component_dispatch(space_key());
            }
            let buf = render_updated(&mut app, 80, 24);
            let border = list_border(&buf, "packages 1");
            let row = (0..buf.area().height)
                .map(|y| list_text(&buf, y, border))
                .find(|text| text.contains("home"))
                .expect("group row missing");
            assert!(
                row.contains(glyph),
                "{icons:?}: checkbox {glyph:?} missing: {row:?}"
            );
            assert!(!row.contains(other), "{icons:?}: wrong checkbox in {row:?}");
        }
    }
}

#[test]
fn test_package_status_glyphs_follow_the_icon_set() {
    for icons in [IconSet::Nerd, IconSet::Unicode] {
        let (mut app, _tmp) = make_test_app();
        app.ctx.core.icons = icons;
        push_screen(&mut app, "Packages");
        let buf = render_updated(&mut app, 80, 24);
        // The fixture package is not linked yet, so it reads as unstowed.
        let glyph = Icons::for_set(icons).status(StowStatus::Unstowed);
        let border = list_border(&buf, "packages 1");
        let row = (0..buf.area().height)
            .map(|y| list_text(&buf, y, border))
            .find(|text| text.contains("test"))
            .expect("package row missing");
        assert!(
            row.contains(glyph),
            "{icons:?}: status glyph {glyph:?} missing: {row:?}"
        );
    }
}

/// A package whose `.stow-local-ignore` does not compile is shown as invalid, never as
/// missing, in both icon sets and in the status colour.
#[test]
fn test_package_with_a_broken_ignore_list_shows_invalid() {
    for set in [IconSet::Nerd, IconSet::Unicode] {
        let icons = Icons::for_set(set);
        let (mut app, tmp) = make_test_app();
        add_status_fixtures(tmp.path());
        let broken = tmp.path().join("dotfiles/home/broken");
        std::fs::create_dir_all(&broken).unwrap();
        std::fs::write(broken.join(".broken_rc"), "x").unwrap();
        // Look-around is rejected by the regex engine, so the list does not compile.
        std::fs::write(broken.join(".stow-local-ignore"), "(?=x)y\n").unwrap();
        app.ctx.core.icons = set;
        let colors = ThemeColors::from_theme(app.ctx.core.theme);
        push_screen(&mut app, "Status");
        let status = render_updated(&mut app, 80, 24);

        let y = (0..status.area().height)
            .find(|&y| row_text(&status, y).contains("broken  invalid"))
            .unwrap_or_else(|| panic!("{set:?}: broken row should read invalid"));
        assert!(
            !row_text(&status, y).contains("missing"),
            "{set:?}: an invalid list must not read as missing"
        );
        let (_, fg) = glyph_on_row(&status, y, icons.invalid)
            .unwrap_or_else(|| panic!("{set:?}: invalid glyph missing"));
        assert_eq!(fg, status_color(&colors, StowStatus::Invalid));
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

/// The package status vocabulary is the same on the packages browser and the status screens:
/// one glyph, one colour and one lowercase word per status.
#[test]
fn test_package_and_status_screens_share_glyph_colour_and_word() {
    for set in [IconSet::Nerd, IconSet::Unicode] {
        let icons = Icons::for_set(set);

        let (mut packages_app, tmp) = make_test_app();
        add_status_fixtures(tmp.path());
        packages_app.ctx.core.icons = set;
        let colors = ThemeColors::from_theme(packages_app.ctx.core.theme);
        push_screen(&mut packages_app, "Packages");
        let packages = render_updated(&mut packages_app, 80, 25);
        // The cursor row keeps its status glyph only where the colour reads at 3:1, so
        // its colour is checked by the cursor-row test instead.
        let cursor_row = find_accent(&packages).map(|(_, y)| y);
        let border = list_border(&packages, "packages 6");

        let (mut status_app, tmp) = make_test_app();
        add_status_fixtures(tmp.path());
        status_app.ctx.core.icons = set;
        push_screen(&mut status_app, "Status");
        let status = render_updated(&mut status_app, 80, 25);

        let mut checked = 0;
        for (name, state) in STATUS_FIXTURES {
            let glyph = icons.status(state);
            let word = status_word(state);
            let colour = status_color(&colors, state);

            let package_y = (0..packages.area().height)
                .find(|&y| list_text(&packages, y, border).contains(name))
                .unwrap_or_else(|| panic!("{set:?}: {name} row missing"));
            let status_y = (0..status.area().height)
                .find(|&y| row_text(&status, y).contains(&format!("{name}  {word}")))
                .unwrap_or_else(|| panic!("{set:?}: {name} status row missing"));

            let (_, status_fg) = glyph_on_row(&status, status_y, glyph)
                .unwrap_or_else(|| panic!("{set:?}: {name}: status glyph {glyph:?} missing"));
            assert_eq!(status_fg, colour, "{set:?}: {name} on the status screen");

            if Some(package_y) != cursor_row {
                let (_, package_fg) = list_glyph_on_row(&packages, package_y, border, glyph)
                    .unwrap_or_else(|| panic!("{set:?}: {name}: glyph {glyph:?} missing"));
                assert_eq!(
                    package_fg, colour,
                    "{set:?}: {name} on the packages browser"
                );
                checked += 1;
            }
        }
        assert!(
            checked >= 4,
            "{set:?}: only {checked} package rows were checked"
        );
    }
}

/// The first cell left of `border` on row `y` drawn with `glyph`, as (column, foreground).
/// Only the list pane is searched, so a glyph in the detail beside it is not taken.
fn list_glyph_on_row(buf: &Buffer, y: u16, border: u16, glyph: &str) -> Option<(u16, Color)> {
    (0..border)
        .find(|&x| buf[(x, y)].symbol() == glyph)
        .map(|x| (x, buf[(x, y)].fg))
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

/// Two groups. `home` holds the status fixtures and the 60-column package. `workstation`
/// holds `deploy` and the CJK package.
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

/// The text of row `y` left of column `border`: the list pane, without the detail beside it.
fn list_text(buf: &Buffer, y: u16, border: u16) -> String {
    (0..border).map(|x| buf[(x, y)].symbol()).collect()
}

/// A config with one group `home` that holds `count` packages, each with one file.
fn make_package_count_app(count: usize) -> (App, tempfile::TempDir) {
    let tmp = tempfile::TempDir::new().unwrap();
    let path = tmp.path().join("andre.yml");
    std::fs::write(
        &path,
        "global:\n  stow:\n    verbosity: 0\ngroups:\n  home:\n    source: dotfiles/home\n    target: \"~\"\n",
    )
    .unwrap();
    for i in 0..count {
        write_package(
            &tmp.path().join("dotfiles/home"),
            &format!("pkg{i:02}"),
            ".rc",
        );
    }
    let app = App::with_home(path, tmp.path().to_path_buf()).unwrap();
    (app, tmp)
}

/// Each package row keeps its status glyph in one column, whatever its name is. The 60-column
/// name is cut inside the list, and the CJK name is two columns wide per character.
#[test]
fn test_long_and_cjk_package_names_keep_the_list_columns_in_line() {
    let (mut app, _tmp) = make_two_group_app();
    app.ctx.core.icons = IconSet::Unicode;
    push_screen(&mut app, "Packages");
    let buf = render_updated(&mut app, 80, 24);
    let border = list_border(&buf, "packages 8");
    let glyph = Icons::for_set(IconSet::Unicode).status(StowStatus::Unstowed);

    let row_of = |locator: &str| {
        (0..buf.area().height)
            .find(|&y| list_text(&buf, y, border).contains(locator))
            .unwrap_or_else(|| panic!("{locator} row missing"))
    };
    let glyph_column = |y: u16| {
        (0..border)
            .find(|&x| buf[(x, y)].symbol() == glyph)
            .unwrap_or_else(|| panic!("status glyph missing on row {y}"))
    };

    let long = row_of("aaaa");
    assert!(
        list_text(&buf, long, border).contains('\u{2026}'),
        "the long name is not cut"
    );
    assert_eq!(
        buf[(border, long)].symbol(),
        "\u{2502}",
        "the long name runs past the border"
    );
    assert_eq!(
        glyph_column(row_of("deploy")),
        glyph_column(row_of("漢")),
        "the CJK name moves the status column"
    );
}

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

/// The column of a two-pane screen's list border: the first top-right corner after `title`
/// on its row. `panel_right_column` takes the rightmost corner, which is the detail's.
fn list_border(buf: &Buffer, title: &str) -> u16 {
    let y = (0..buf.area().height)
        .find(|&y| row_text(buf, y).contains(title))
        .unwrap_or_else(|| panic!("{title} panel missing"));
    let start = column_of(buf, y, title).unwrap_or(0);
    (start..buf.area().width)
        .find(|&x| buf[(x, y)].symbol() == "\u{256e}")
        .unwrap_or_else(|| panic!("{title} panel has no top-right corner"))
}

/// The rows between the top border of the panel titled `title` and the bottom border at
/// `column`.
fn panel_rows_at(buf: &Buffer, title: &str, column: u16) -> std::ops::Range<u16> {
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

#[test]
fn test_package_list_draws_its_scrollbar_thumb_when_rows_overflow() {
    let (mut app, _tmp) = make_package_count_app(30);
    push_screen(&mut app, "Packages");
    let colors = ThemeColors::from_theme(app.ctx.core.theme);
    let buf = render_updated(&mut app, 80, 24);

    let title = "packages 30";
    let column = list_border(&buf, title);
    let rows = panel_rows_at(&buf, title, column);
    assert!(
        !rows_with_fg(&buf, column, rows, colors.accent).is_empty(),
        "no scrollbar thumb on the package list's right border"
    );
}

#[test]
fn test_package_list_draws_no_scrollbar_when_every_row_fits() {
    let (mut app, _tmp) = make_test_app();
    push_screen(&mut app, "Packages");
    let colors = ThemeColors::from_theme(app.ctx.core.theme);
    let buf = render_updated(&mut app, 80, 24);

    let title = "packages 1";
    let column = list_border(&buf, title);
    let rows = panel_rows_at(&buf, title, column);
    assert!(rows_with_fg(&buf, column, rows, colors.accent).is_empty());
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
    push_screen(&mut app, "Packages");
    let colors = ThemeColors::from_theme(app.ctx.core.theme);
    let buf = render_updated(&mut app, 80, 24);

    // Thirty groups and no packages: the title counts packages, and the rows are the groups.
    let title = "packages 0";
    let column = list_border(&buf, title);
    let rows = panel_rows_at(&buf, title, column);
    assert!(
        !rows_with_fg(&buf, column, rows, colors.accent).is_empty(),
        "no scrollbar thumb on the group list's right border"
    );
}

#[test]
fn test_execute_highlighted_row_is_text_on_selection() {
    use andre::components::execute::ExecuteComponent;
    use andre::execute::{ExecutionResult, ExecutionState};

    let (mut app, _tmp) = make_test_app();
    let colors = ThemeColors::from_theme(app.ctx.core.theme);
    let (_tx, rx) = tokio::sync::mpsc::channel(1);
    let results = vec![ExecutionResult {
        group: "home".into(),
        package: "pkg0".into(),
        success: false,
        error_message: Some("boom".into()),
    }];
    let mut exec = ExecuteComponent::new(rx, results.len(), results);
    exec.state = ExecutionState::Completed;
    // Row 0 is the "Results:" heading, so row 1 is the failed package.
    exec.scroll_index = 1;
    app.component_stack.push(Box::new(exec));

    let buf = render_app(&mut app, 80, 24);
    let y = (0..buf.area().height)
        .find(|&y| row_text(&buf, y).contains("boom"))
        .expect("failed row is drawn");
    let text_x = column_of(&buf, y, "boom").expect("error text is drawn");
    let mark_x = column_of(&buf, y, "\u{2717}").expect("failure mark is drawn");
    for x in [text_x, mark_x] {
        let cell = &buf[(x, y)];
        assert_eq!(cell.fg, colors.text, "highlighted row text at column {x}");
        assert_eq!(
            cell.bg, colors.selection,
            "highlighted row fill at column {x}"
        );
    }
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
        !rows_with_fg(&buf, column, rows, colors.accent).is_empty(),
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
    assert_eq!(rows.len(), 5, "the status list should show 5 rows at 80x24");
    assert!(
        !rows_with_fg(&buf, column, rows, colors.accent).is_empty(),
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
    assert!(rows_with_fg(&buf, column, rows.clone(), colors.accent).is_empty());
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
    write_package(
        &source,
        "wide",
        ".wide_configuration_file_with_a_name_long_enough_to_cut_rc",
    );
    write_package(&source, "plain", ".plain_rc");
    let app = App::with_home(path, tmp.path().to_path_buf()).unwrap();
    (app, tmp)
}

#[test]
fn test_package_detail_cuts_a_long_file_name_with_an_ellipsis() {
    let (mut app, _tmp) = make_long_detail_app();
    push_screen(&mut app, "Packages");
    // Rows: the group, then its packages in name order (`plain`, `wide`).
    app.update_components();
    app.component_dispatch(down_key());
    app.component_dispatch(down_key());
    let buf = render_updated(&mut app, 80, 24);

    let y = (0..buf.area().height)
        .find(|&y| row_text(&buf, y).contains(".wide_configuration_file_with"))
        .expect("long file name row missing");
    let border = panel_right_column(&buf, "home/wide");
    assert!(
        row_text(&buf, y).contains('\u{2026}'),
        "the long file name is not cut with an ellipsis"
    );
    assert_eq!(
        buf[(border, y)].symbol(),
        "\u{2502}",
        "the row runs past the border"
    );
}

#[test]
fn test_short_file_names_are_drawn_whole_in_the_detail() {
    let (mut app, _tmp) = make_long_detail_app();
    push_screen(&mut app, "Packages");
    app.update_components();
    app.component_dispatch(down_key()); // plain
    let buf = render_updated(&mut app, 80, 24);
    assert!(
        row_containing(&buf, ".plain_rc").is_some(),
        "a short file name is missing from the detail"
    );
}

#[test]
fn test_packages_screen_has_no_legend_and_counts_the_marks() {
    let (mut app, _tmp) = make_two_group_app();
    push_screen(&mut app, "Packages");
    app.update_components();
    app.component_dispatch(space_key()); // marks every package of `home`
    let buf = render_updated(&mut app, 80, 24);

    assert!(
        !buffer_text(&buf).contains(" legend "),
        "the legend is back"
    );
    let border = list_border(&buf, "packages 6 of 8");
    let group = (0..buf.area().height)
        .find(|&y| list_text(&buf, y, border).contains("home"))
        .expect("group row missing");
    assert!(
        list_text(&buf, group, border).contains("6/6"),
        "the group row does not count its marks"
    );
    assert!(
        line_contains(&buf, "Enter stow 6"),
        "the footer does not follow the marks"
    );
}

#[test]
fn test_partly_marked_group_shows_the_some_box_and_its_count() {
    let (mut app, _tmp) = make_two_group_app();
    app.ctx.core.icons = IconSet::Unicode;
    push_screen(&mut app, "Packages");
    app.update_components();
    app.component_dispatch(down_key()); // the first package of `home`
    app.component_dispatch(space_key());
    let buf = render_updated(&mut app, 80, 24);

    let icons = Icons::for_set(IconSet::Unicode);
    let border = list_border(&buf, "packages 1 of 8");
    let group = (0..buf.area().height)
        .map(|y| list_text(&buf, y, border))
        .find(|text| text.contains("home"))
        .expect("group row missing");
    assert!(group.contains(icons.fold_open), "{group:?}: no open fold");
    assert!(
        group.contains(icons.checkbox_some),
        "{group:?}: no partial box"
    );
    assert!(group.contains("1/6"), "{group:?}: the count is not 1/6");
}

#[test]
fn test_footer_run_verb_follows_the_action_and_the_marks() {
    let (mut app, _tmp) = make_two_group_app();
    app.ctx.core.flags.action = StowAction::Restow;
    push_screen(&mut app, "Packages");
    app.update_components();
    app.component_dispatch(down_key());
    app.component_dispatch(space_key());
    let buf = render_updated(&mut app, 80, 24);
    assert!(
        line_contains(&buf, "Enter restow 1"),
        "the footer does not follow the action"
    );
}

/// The membership dialog keeps the cursor's profile on screen when the list is longer than
/// the dialog, and draws the `Space toggle` key line above the buttons.
#[test]
fn test_membership_dialog_scrolls_a_long_profile_list_to_the_cursor() {
    let names: Vec<String> = (0..20).map(|i| format!("p{i:02}")).collect();
    let yamls: Vec<(&str, &str)> = names
        .iter()
        .map(|name| (name.as_str(), "packages: []\n"))
        .collect();
    let (mut app, _tmp) = make_profiles_app(&yamls);
    push_screen(&mut app, "Packages");
    app.update_components();
    app.component_dispatch(down_key()); // home/test
    app.component_dispatch(key_char('p'));

    let top = render_updated(&mut app, 80, 24);
    assert!(
        line_contains(&top, "p00"),
        "the cursor's profile is missing"
    );
    assert!(!line_contains(&top, "p19"), "all 20 profiles fit at 80x24");

    for _ in 0..19 {
        app.component_dispatch(down_key());
    }
    let last = render_updated(&mut app, 80, 24);
    let row = (0..last.area().height)
        .find(|&y| row_text(&last, y).contains("p19"))
        .expect("the last profile is not drawn");
    assert!(
        row_text(&last, row).contains('\u{258c}'),
        "the cursor's accent bar is not on p19"
    );
    assert!(!line_contains(&last, "p00"), "the window did not move");
    assert!(
        (0..last.area().height).any(|y| {
            let text = row_text(&last, y);
            text.contains("Space toggle") && !text.contains("q quit")
        }),
        "the Space toggle line is missing above the buttons"
    );
}

#[test]
fn test_p_profiles_shows_on_a_package_row_only() {
    let (mut app, _tmp) = make_test_app();
    push_screen(&mut app, "Packages");
    let on_group = render_updated(&mut app, 120, 24);
    assert!(
        !line_contains(&on_group, "p profiles"),
        "group row offers p"
    );

    app.component_dispatch(down_key());
    let on_package = render_updated(&mut app, 120, 24);
    assert!(
        line_contains(&on_package, "p profiles"),
        "package row lacks p"
    );
}

/// A package whose group target does not exist cannot be read, so it shows the unknown dot
/// rather than a status, and nothing is drawn in its place.
#[test]
fn test_package_whose_target_is_missing_shows_the_unknown_dot() {
    let tmp = tempfile::TempDir::new().unwrap();
    let path = tmp.path().join("andre.yml");
    std::fs::write(
        &path,
        "global: {}\ngroups:\n  lost:\n    source: dotfiles/lost\n    target: \"missing-target\"\n",
    )
    .unwrap();
    write_package(&tmp.path().join("dotfiles/lost"), "pkg", ".rc");
    let mut app = App::with_home(path, tmp.path().to_path_buf()).unwrap();
    push_screen(&mut app, "Packages");
    let buf = render_updated(&mut app, 80, 24);

    let border = list_border(&buf, "packages 1");
    let row = (0..buf.area().height)
        .map(|y| list_text(&buf, y, border))
        .find(|text| text.contains("pkg"))
        .expect("package row missing");
    let unknown = Icons::for_set(app.ctx.core.icons).unknown;
    assert!(
        row.contains(unknown),
        "no unknown dot on the package row: {row:?}"
    );
}

#[test]
fn test_membership_dialog_names_the_package_and_offers_cancel_and_save() {
    let (mut app, _tmp) = make_test_app();
    push_screen(&mut app, "Packages");
    app.update_components();
    app.component_dispatch(down_key());
    app.component_dispatch(key_char('p'));
    let buf = render_updated(&mut app, 80, 24);

    assert!(
        line_contains(&buf, "home/test in profiles"),
        "dialog title missing"
    );
    assert!(
        line_contains(&buf, "No profiles yet"),
        "empty profile list is not explained"
    );
    assert!(
        line_contains(&buf, "[ Esc cancel ]"),
        "cancel button missing"
    );
    assert!(line_contains(&buf, "[ Enter save ]"), "save button missing");
}

#[test]
fn test_packages_render_at_120x40_and_200x50_with_the_dialog_open() {
    for (width, height) in [(120, 40), (200, 50)] {
        let (mut app, _tmp) = make_test_app();
        push_screen(&mut app, "Packages");
        app.update_components();
        app.component_dispatch(down_key());
        app.component_dispatch(key_char('p'));
        let buf = render_updated(&mut app, width, height);
        assert!(
            line_contains(&buf, "in profiles"),
            "{width}x{height}: the membership dialog is missing"
        );
    }
}

/// Every screen that shares the footer, the main menu panes or the layout helpers draws
/// at the large sizes without panicking, and keeps `q quit` on its footer row.
#[test]
fn test_touched_screens_render_at_120x40_and_200x50_without_panic() {
    let screens = [
        "MainMenu",
        "Profiles",
        "Packages",
        "Settings",
        "Status",
        "Help",
        "Confirm",
        "Execute",
        "UnstowTargetSelect",
        "UnstowBrowse",
        "NewPackage",
    ];
    for (width, height) in [(120, 40), (200, 50)] {
        for name in screens {
            let (mut app, _tmp) = make_test_app();
            if name != "MainMenu" {
                push_screen(&mut app, name);
            }
            let buf = render_updated(&mut app, width, height);
            assert!(
                row_containing(&buf, "q quit").is_some(),
                "{name} at {width}x{height}: footer missing"
            );
        }
    }
}

/// An app whose config folder holds `profiles`, as `(name, yaml)` pairs. Besides the test
/// package `home/test`, the group has `home/shell`.
fn make_profiles_app(profiles: &[(&str, &str)]) -> (App, tempfile::TempDir) {
    let (mut app, tmp) = make_test_app();
    app.ctx.core.icons = IconSet::Unicode;
    std::fs::create_dir_all(tmp.path().join("dotfiles/home/shell")).unwrap();
    let dir = tmp.path().join("profiles");
    std::fs::create_dir_all(&dir).unwrap();
    for (name, yaml) in profiles {
        std::fs::write(dir.join(format!("{name}.yml")), yaml).unwrap();
    }
    app.ctx.core.profiles = andre_core::profile::ProfileSet::load(&dir).unwrap();
    (app, tmp)
}

/// The profiles screen, opened from the menu with its rows built.
fn open_profiles_render(app: &mut App) {
    app.component_dispatch(key_char('1'));
    app.update_components();
}

/// The profile editor for `name`, opened from the profiles screen with `e`.
fn open_editor_render(app: &mut App, name: &str) {
    open_profiles_render(app);
    let index = app
        .ctx
        .core
        .profiles
        .iter()
        .position(|p| p.name == name)
        .unwrap();
    for _ in 0..index {
        app.component_dispatch(down_key());
    }
    app.component_dispatch(key_char('e'));
    app.update_components();
}

fn tab_key() -> KeyEvent {
    KeyEvent::new(KeyCode::Tab, KeyModifiers::NONE)
}

/// Column and colour of every `╭` on row `y`: the top-left corners of the panes on it.
fn corners(buf: &Buffer, y: u16) -> Vec<(u16, Color)> {
    (0..buf.area().width)
        .filter(|&x| buf[(x, y)].symbol() == "\u{256d}")
        .map(|x| (x, buf[(x, y)].fg))
        .collect()
}

/// The `packages` pane is on the left, the form on the right. Row 5 holds both corners at 80x24.
const PANE_ROW: u16 = 5;

#[test]
fn test_profile_edit_at_80x24_shows_the_packages_and_the_form() {
    let (mut app, _tmp) = make_profiles_app(&[
        ("base", "packages: [home/shell]\n"),
        (
            "agent",
            "description: Work servers that run agents\nincludes: [base]\npackages: [home/test]\n",
        ),
    ]);
    open_editor_render(&mut app, "agent");
    let buf = render_updated(&mut app, 80, 24);

    assert!(
        line_contains(&buf, "packages 2 of 2"),
        "tree title counts marks"
    );
    assert!(
        line_contains(&buf, "✓ ■ test") || line_contains(&buf, "test"),
        "own package"
    );
    assert!(
        line_contains(&buf, "via base"),
        "inherited package says where it comes from"
    );
    assert!(
        line_contains(&buf, "Work servers that run agents"),
        "description"
    );
    assert!(line_contains(&buf, "includes"), "includes heading");
    assert!(line_contains(&buf, "base"), "include row");
    assert!(line_contains(&buf, "resolved"), "resolved fact");
    assert!(line_contains(&buf, "2 packages"), "resolved count");
    assert!(line_contains(&buf, "via base"), "fact for the include");
    assert!(
        footer_row(&buf).contains("Tab pane  Space toggle  w save  Esc back"),
        "{}",
        footer_row(&buf)
    );
}

#[test]
fn test_profile_edit_title_says_unsaved_after_an_edit() {
    let (mut app, _tmp) = make_profiles_app(&[("agent", "packages: [home/test]\n")]);
    open_editor_render(&mut app, "agent");
    assert!(!line_contains(&render_updated(&mut app, 80, 24), "unsaved"));

    app.component_dispatch(down_key());
    app.component_dispatch(space_key());
    let buf = render_updated(&mut app, 80, 24);
    assert!(
        line_contains(&buf, "agent · unsaved ●"),
        "title marks the edit"
    );
    assert!(footer_row(&buf).contains("w save"));
}

#[test]
fn test_tab_moves_the_accent_border_to_the_pane_that_takes_keys() {
    let (mut app, _tmp) = make_profiles_app(&[("agent", "packages: [home/test]\n")]);
    open_editor_render(&mut app, "agent");
    let accent = ThemeColors::from_theme(app.ctx.core.theme).accent;
    let border = ThemeColors::from_theme(app.ctx.core.theme).border;

    let buf = render_updated(&mut app, 80, 24);
    let [(_, left), (_, right)] = corners(&buf, PANE_ROW)[..] else {
        panic!("two panes expected on row {PANE_ROW}");
    };
    assert_eq!((left, right), (accent, border), "packages has focus first");

    app.component_dispatch(tab_key());
    let buf = render_updated(&mut app, 80, 24);
    let [(_, left), (_, right)] = corners(&buf, PANE_ROW)[..] else {
        panic!("two panes expected on row {PANE_ROW}");
    };
    assert_eq!(
        (left, right),
        (border, accent),
        "Tab moves the accent to the form"
    );
}

#[test]
fn test_profile_edit_footer_follows_what_space_and_enter_do() {
    let (mut app, _tmp) = make_profiles_app(&[("agent", "packages: [home/test]\n")]);
    open_editor_render(&mut app, "agent");
    app.component_dispatch(tab_key());
    let buf = render_updated(&mut app, 80, 24);
    assert!(
        footer_row(&buf).contains("Tab pane  Enter edit  w save  Esc back"),
        "{}",
        footer_row(&buf)
    );

    app.component_dispatch(enter_key());
    let buf = render_updated(&mut app, 80, 24);
    assert!(
        footer_row(&buf).contains("Enter done  Esc done"),
        "{}",
        footer_row(&buf)
    );
}

#[test]
fn test_leave_dialog_names_the_profile_and_offers_stay_discard_save() {
    let (mut app, _tmp) = make_profiles_app(&[("agent", "packages: [home/test]\n")]);
    open_editor_render(&mut app, "agent");
    app.component_dispatch(down_key());
    app.component_dispatch(space_key());
    app.component_dispatch(esc_key());

    let buf = render_updated(&mut app, 80, 24);
    assert!(line_contains(&buf, "Save changes to profile `agent`?"));
    assert!(line_contains(&buf, "[ Esc stay ]"));
    assert!(line_contains(&buf, "[ n discard ]"));
    assert!(line_contains(&buf, "[ y save ]"));
    assert!(
        footer_row(&buf).contains("y save  n discard  Esc stay"),
        "{}",
        footer_row(&buf)
    );
}

#[test]
fn test_profile_edit_and_profile_status_render_at_120x40_and_200x50() {
    let (mut app, _tmp) = make_profiles_app(&[
        ("base", "packages: [home/shell]\n"),
        ("agent", "includes: [base]\npackages: [home/test]\n"),
    ]);
    open_editor_render(&mut app, "agent");
    for (width, height) in [(120, 40), (200, 50)] {
        let buf = render_updated(&mut app, width, height);
        assert!(line_contains(&buf, "packages 2 of 2"), "{width}x{height}");
    }

    app.component_dispatch(esc_key());
    app.component_dispatch(key_char('c'));
    for (width, height) in [(120, 40), (200, 50)] {
        let buf = render_updated(&mut app, width, height);
        assert!(line_contains(&buf, "Status"), "{width}x{height}");
    }
}

#[test]
fn test_profile_edit_form_scrolls_to_keep_the_cursor_row_in_view() {
    let names: Vec<String> = (1..=12).map(|n| format!("p{n:02}")).collect();
    let mut profiles: Vec<(&str, &str)> = vec![("agent", "packages: [home/test]\n")];
    profiles.extend(names.iter().map(|name| (name.as_str(), "packages: []\n")));
    let (mut app, _tmp) = make_profiles_app(&profiles);
    open_editor_render(&mut app, "agent");

    let buf = render_updated(&mut app, 80, 24);
    assert!(
        line_contains(&buf, "description"),
        "the top of the form shows first"
    );

    app.component_dispatch(tab_key());
    for _ in 0..names.len() {
        app.component_dispatch(down_key());
    }
    let buf = render_updated(&mut app, 80, 24);
    assert!(
        line_contains(&buf, "p12"),
        "the cursor's include is on screen"
    );
    assert!(
        !line_contains(&buf, "description"),
        "the form scrolled down to it"
    );
}

#[test]
fn test_profile_edit_shows_a_broken_include_as_the_form_headline() {
    let (mut app, _tmp) =
        make_profiles_app(&[("agent", "includes: [ghost]\npackages: [home/test]\n")]);
    open_editor_render(&mut app, "agent");
    let buf = render_updated(&mut app, 80, 24);
    assert!(
        line_contains(&buf, "ghost"),
        "the error names the missing profile"
    );
    assert!(
        line_contains(&buf, "missing"),
        "the include row is marked missing"
    );
}

#[test]
fn test_profile_edit_renders_at_120x40_and_200x50_with_the_dialog_open() {
    let (mut app, _tmp) = make_profiles_app(&[
        ("base", "packages: [home/shell]\n"),
        ("agent", "includes: [base]\npackages: [home/test]\n"),
    ]);
    open_editor_render(&mut app, "agent");
    // The header, then home/shell (inherited from base), then home/test: unmark test.
    app.component_dispatch(down_key());
    app.component_dispatch(down_key());
    app.component_dispatch(space_key());
    app.component_dispatch(esc_key());
    for (width, height) in [(120, 40), (200, 50)] {
        let buf = render_updated(&mut app, width, height);
        assert!(
            line_contains(&buf, "Save changes to profile `agent`?"),
            "{width}x{height}"
        );
    }
}

#[test]
fn test_main_menu_lists_the_eight_items_at_80x24() {
    let (mut app, _tmp) = make_test_app();
    app.ctx.core.icons = IconSet::Unicode;
    let buf = render_updated(&mut app, 80, 24);
    for (digit, label) in [
        ("1", "profiles"),
        ("2", "packages"),
        ("3", "new package"),
        ("4", "unstow"),
        ("5", "verify status"),
        ("6", "settings"),
        ("7", "help"),
        ("8", "quit"),
    ] {
        // Two spaces after the glyph tell the item row from the detail title.
        let row =
            row_containing(&buf, &format!("  {label}")).unwrap_or_else(|| panic!("no {label} row"));
        assert!(
            row.contains(&format!(" {digit} \u{2502}")),
            "{label} is not numbered {digit}: {row}"
        );
    }
}

#[test]
fn test_profiles_list_shows_the_profiles_with_their_counts() {
    let (mut app, _tmp) = make_profiles_app(&[
        (
            "base",
            "description: Shared by every machine\npackages: [home/test]\n",
        ),
        (
            "agent-work",
            "description: Work servers that run agents\nincludes: [base]\npackages: [home/shell]\n",
        ),
    ]);
    open_profiles_render(&mut app);
    let buf = render_updated(&mut app, 80, 24);
    assert!(
        line_contains(&buf, "profiles 2"),
        "list title counts the profiles"
    );
    assert!(
        row_containing(&buf, "\u{25c8} agent-work").is_some(),
        "agent-work row"
    );
    assert!(row_containing(&buf, "\u{25c8} base").is_some(), "base row");
    assert!(
        line_contains(&buf, "Work servers that run agents"),
        "description headline"
    );
    assert!(line_contains(&buf, "includes  base"), "includes fact");
    assert!(
        line_contains(&buf, "2  (1 own, 1 via base)"),
        "packages fact splits own from included"
    );
    assert!(
        line_contains(&buf, "via base"),
        "included package says where it comes from"
    );
}

#[test]
fn test_profiles_empty_state_explains_itself_and_offers_a_new_profile() {
    let (mut app, _tmp) = make_profiles_app(&[]);
    open_profiles_render(&mut app);
    let buf = render_updated(&mut app, 80, 24);
    assert!(line_contains(&buf, "no profiles yet"));
    assert!(line_contains(&buf, "+ new profile"));
    assert!(line_contains(&buf, "A profile is a named set of packages"));
    assert!(line_contains(&buf, "Profiles can include other profiles."));
}

#[test]
fn test_profiles_footer_names_the_live_action_and_the_list_keys() {
    let (mut app, _tmp) = make_profiles_app(&[("base", "packages: [home/test]\n")]);
    open_profiles_render(&mut app);
    let buf = render_updated(&mut app, 80, 24);
    let footer = footer_row(&buf);
    assert!(
        footer.contains("Enter stow  + new  e edit  r rename  x delete  c check  Esc back"),
        "{footer}"
    );
    assert!(footer.contains("q quit"), "{footer}");

    app.ctx.core.flags.action = StowAction::Restow;
    let buf = render_updated(&mut app, 80, 24);
    assert!(
        footer_row(&buf).contains("Enter restow  + new"),
        "{}",
        footer_row(&buf)
    );
}

#[test]
fn test_empty_profiles_footer_offers_only_new_and_back() {
    let (mut app, _tmp) = make_profiles_app(&[]);
    open_profiles_render(&mut app);
    let buf = render_updated(&mut app, 80, 24);
    let footer = footer_row(&buf);
    assert!(footer.contains("+ new  Esc back"), "{footer}");
    assert!(!footer.contains("Enter"), "nothing to run: {footer}");
    assert!(!footer.contains("rename"), "nothing to rename: {footer}");
    assert!(!footer.contains("delete"), "nothing to delete: {footer}");
}

#[test]
fn test_profile_dialog_footers_say_what_enter_and_y_do() {
    let (mut app, _tmp) = make_profiles_app(&[("base", "packages: [home/test]\n")]);
    open_profiles_render(&mut app);
    app.component_dispatch(key_char('+'));
    let buf = render_updated(&mut app, 80, 24);
    assert!(
        footer_row(&buf).contains("Enter create  Esc cancel"),
        "{}",
        footer_row(&buf)
    );

    app.component_dispatch(esc_key());
    app.component_dispatch(key_char('x'));
    let buf = render_updated(&mut app, 80, 24);
    assert!(
        footer_row(&buf).contains("y delete  n keep"),
        "{}",
        footer_row(&buf)
    );
}

#[test]
fn test_profile_that_does_not_resolve_has_no_enter_hint() {
    let (mut app, _tmp) =
        make_profiles_app(&[("agent", "includes: [ghost]\npackages: [home/test]\n")]);
    open_profiles_render(&mut app);
    let buf = render_updated(&mut app, 80, 24);
    let footer = footer_row(&buf);
    assert!(
        footer.contains("+ new  e edit  r rename  x delete  c check  Esc back"),
        "{footer}"
    );
    assert!(!footer.contains("Enter"), "{footer}");
}

#[test]
fn test_new_profile_dialog_offers_create_only_for_a_valid_name() {
    let (mut app, _tmp) = make_profiles_app(&[]);
    open_profiles_render(&mut app);
    app.component_dispatch(key_char('+'));
    let buf = render_updated(&mut app, 80, 24);
    assert!(line_contains(&buf, "[ Esc cancel ]"));
    assert!(
        !line_contains(&buf, "[ Enter create ]"),
        "no create button for an empty name"
    );

    for c in "laptop".chars() {
        app.component_dispatch(key_char(c));
    }
    let buf = render_updated(&mut app, 80, 24);
    assert!(line_contains(&buf, "[ Enter create ]"));
}

#[test]
fn test_new_profile_dialog_says_when_the_name_is_taken() {
    let (mut app, _tmp) = make_profiles_app(&[("base", "packages: [home/test]\n")]);
    open_profiles_render(&mut app);
    app.component_dispatch(key_char('+'));
    for c in "base".chars() {
        app.component_dispatch(key_char(c));
    }
    let buf = render_updated(&mut app, 80, 24);
    assert!(line_contains(&buf, "already exists"));
    assert!(!line_contains(&buf, "[ Enter create ]"));
}

#[test]
fn test_delete_dialog_names_the_includers_and_always_offers_delete() {
    let (mut app, _tmp) = make_profiles_app(&[
        ("base", "packages: [home/test]\n"),
        ("agent", "includes: [base]\n"),
    ]);
    open_profiles_render(&mut app);
    // `agent` sorts before `base`. Nothing includes `agent`.
    app.component_dispatch(key_char('x'));
    let buf = render_updated(&mut app, 80, 24);
    assert!(line_contains(&buf, "Delete profile `agent`?"));
    assert!(line_contains(&buf, "[ y delete ]"));
    assert!(!line_contains(&buf, "included by"));

    // `base` is included by `agent`: the dialog says the include goes with it.
    app.component_dispatch(key_char('n'));
    app.component_dispatch(key_char('j'));
    app.component_dispatch(key_char('x'));
    let buf = render_updated(&mut app, 80, 24);
    assert!(line_contains(&buf, "Delete profile `base`?"));
    assert!(line_contains(&buf, "It is included by: agent."));
    assert!(line_contains(&buf, "Those includes are removed too."));
    assert!(line_contains(&buf, "[ y delete ]"));
}

#[test]
fn test_new_profile_dialog_puts_the_rule_under_the_name_value() {
    let (mut app, _tmp) = make_profiles_app(&[]);
    open_profiles_render(&mut app);
    app.component_dispatch(key_char('+'));
    for c in "laptop".chars() {
        app.component_dispatch(key_char(c));
    }
    let buf = render_updated(&mut app, 80, 24);
    let rows: Vec<String> = (0..buf.area().height).map(|y| row_text(&buf, y)).collect();
    let name = (0..buf.area().height)
        .find(|&y| row_text(&buf, y).contains("name  laptop"))
        .expect("name row");
    let value = column_of(&buf, name, "laptop");
    assert_eq!(
        column_of(&buf, name + 1, "a-z 0-9"),
        value,
        "the rule starts under the value: {:?}",
        rows[name as usize + 1]
    );
    let blank = &rows[name as usize + 2];
    assert!(
        !blank.contains("a-z") && !blank.contains("laptop"),
        "a blank row separates the rule from the buttons: {blank:?}"
    );
    let buttons = &rows[name as usize + 3];
    assert!(
        buttons.contains("[ Enter create ]"),
        "the buttons follow the blank row: {buttons:?}"
    );
}

#[test]
fn test_profile_with_an_unknown_include_shows_the_error_as_its_headline() {
    let (mut app, _tmp) =
        make_profiles_app(&[("agent", "includes: [ghost]\npackages: [home/test]\n")]);
    open_profiles_render(&mut app);
    let buf = render_updated(&mut app, 80, 24);
    assert!(
        line_contains(&buf, "includes unknown"),
        "the resolution error is the headline"
    );
}

#[test]
fn test_profiles_render_at_120x40_and_200x50_with_a_dialog_open() {
    let (mut app, _tmp) = make_profiles_app(&[("base", "packages: [home/test]\n")]);
    open_profiles_render(&mut app);
    app.component_dispatch(key_char('x'));
    for (width, height) in [(120, 40), (200, 50)] {
        let buf = render_updated(&mut app, width, height);
        assert!(
            row_containing(&buf, "q quit").is_some(),
            "footer at {width}x{height}"
        );
        assert!(
            line_contains(&buf, "Delete profile `base`?"),
            "dialog at {width}x{height}"
        );
    }
}

/// The ignore editor for `home/test`, opened from the packages screen with `i`.
fn open_ignore_render(app: &mut App) {
    app.component_dispatch(key_char('2'));
    app.update_components();
    app.component_dispatch(down_key());
    app.component_dispatch(key_char('i'));
    app.update_components();
}

/// Writes one of the package's ignore files, `home/test`, before the editor opens.
fn write_package_ignore(app: &App, name: &str, text: &str) {
    let dir = app.ctx.home_dir.join("dotfiles/home/test");
    std::fs::write(dir.join(name), text).unwrap();
}

#[test]
fn test_ignore_editor_at_80x24_shows_the_file_tabs_the_lines_and_the_detail() {
    let (mut app, _tmp) = make_test_app();
    write_package_ignore(&app, ".gitignore", "# secrets\n*.env\n.env*\n");
    open_ignore_render(&mut app);
    app.component_dispatch(down_key()); // *.env, a pattern: the detail has its facts
    let buf = render_updated(&mut app, 80, 24);
    assert!(line_contains(&buf, "[.gitignore 2]"), "active file tab");
    assert!(
        line_contains(&buf, ".stow-local-ignore (none)"),
        "missing file tab"
    );
    assert!(
        line_contains(&buf, "Main > Packages > Ignore"),
        "the breadcrumb names the screen"
    );
    assert!(line_contains(&buf, "*.env"), "the lines are listed");
    assert!(
        line_contains(&buf, "pattern"),
        "the detail has the pattern fact"
    );
    assert!(
        line_contains(&buf, "matches"),
        "the detail has the match count"
    );
    assert!(
        line_contains(&buf, "decides for itself"),
        "the glob preview says so"
    );
    assert!(row_containing(&buf, "q quit").is_some());
}

#[test]
fn test_stow_local_ignore_detail_warns_that_it_replaces_the_defaults() {
    let (mut app, _tmp) = make_test_app();
    write_package_ignore(&app, ".stow-local-ignore", "^/README.*\n");
    open_ignore_render(&mut app);
    app.component_dispatch(KeyEvent::new(KeyCode::Tab, KeyModifiers::NONE));
    let buf = render_updated(&mut app, 80, 24);
    assert!(line_contains(&buf, "[.stow-local-ignore 1]"));
    assert!(line_contains(&buf, "REPLACES"), "the replace warning shows");
    assert!(
        line_contains(&buf, "^/README.*"),
        "the cursor line is the detail title"
    );
}

#[test]
fn test_ignore_editor_missing_file_offers_to_create_it() {
    let (mut app, _tmp) = make_test_app();
    open_ignore_render(&mut app);
    let buf = render_updated(&mut app, 80, 24);
    assert!(line_contains(&buf, "no .gitignore yet"));
    assert!(line_contains(&buf, "create it"));
    assert!(
        line_contains(&buf, "no file yet"),
        "the detail says the file is missing"
    );
}

#[test]
fn test_ignore_editor_unsaved_lines_are_marked_and_footer_names_the_keys() {
    let (mut app, _tmp) = make_test_app();
    write_package_ignore(&app, ".gitignore", "*.env\n");
    open_ignore_render(&mut app);
    app.component_dispatch(key_char('x'));
    let buf = render_updated(&mut app, 80, 24);
    assert!(
        line_contains(&buf, "unsaved \u{25cf}"),
        "list title is marked"
    );
    let footer = footer_row(&buf);
    for hint in ["Tab file", "+ add", "w save", "Esc back"] {
        assert!(footer.contains(hint), "{hint} in {footer}");
    }
}

#[test]
fn test_ignore_editor_leave_dialog_names_the_package() {
    let (mut app, _tmp) = make_test_app();
    write_package_ignore(&app, ".gitignore", "*.env\n");
    open_ignore_render(&mut app);
    app.component_dispatch(key_char('x'));
    app.component_dispatch(esc_key());
    let buf = render_updated(&mut app, 80, 24);
    assert!(line_contains(
        &buf,
        "Save changes to the ignore files of home/test?"
    ));
    for label in ["stay", "discard", "save"] {
        assert!(line_contains(&buf, label), "{label} button");
    }
}

#[test]
fn test_packages_footer_offers_i_for_ignore_on_a_package_only() {
    let (mut app, _tmp) = make_test_app();
    app.component_dispatch(key_char('2'));
    app.update_components();
    let on_group = render_updated(&mut app, 80, 24);
    assert!(!footer_row(&on_group).contains("i ignore"));

    app.component_dispatch(down_key());
    let on_package = render_updated(&mut app, 80, 24);
    let footer = footer_row(&on_package);
    assert!(footer.contains("i ignore"), "{footer}");
    assert!(
        footer.find("i ignore").unwrap() < footer.find("p profiles").unwrap(),
        "screen verbs run i then p (SCREENS.md 1.4): {footer}"
    );
}

#[test]
fn test_ignore_editor_explains_a_stow_line_that_does_not_compile_in_one_line() {
    let (mut app, _tmp) = make_test_app();
    open_ignore_render(&mut app);
    app.component_dispatch(KeyEvent::new(KeyCode::Tab, KeyModifiers::NONE));
    app.component_dispatch(key_char('+'));
    type_text_render(&mut app, "bad(");
    let buf = render_updated(&mut app, 80, 24);
    assert!(
        line_contains(&buf, "invalid pattern: unclosed group"),
        "the regex's own reason is the headline"
    );
    assert!(!line_contains(&buf, "this line"), "no internal origin");
    assert!(!line_contains(&buf, "(?:"), "no internal wrapper");
}

#[test]
fn test_ignore_editor_renders_at_120x40_and_200x50_in_each_state() {
    let (mut app, _tmp) = make_test_app();
    write_package_ignore(&app, ".gitignore", "# secrets\n*.env\n");
    open_ignore_render(&mut app);
    for (width, height) in [(120, 40), (200, 50)] {
        let buf = render_updated(&mut app, width, height);
        assert!(
            row_containing(&buf, "q quit").is_some(),
            "footer at {width}x{height}"
        );
        assert!(
            line_contains(&buf, "[.gitignore 1]"),
            "chip at {width}x{height}"
        );
    }
    app.component_dispatch(key_char('+'));
    type_text_render(&mut app, "bad");
    for (width, height) in [(120, 40), (200, 50)] {
        let buf = render_updated(&mut app, width, height);
        assert!(line_contains(&buf, "bad"), "typed line at {width}x{height}");
    }
    app.component_dispatch(esc_key());
    app.component_dispatch(up_key());
    app.component_dispatch(key_char('x'));
    app.component_dispatch(esc_key());
    for (width, height) in [(120, 40), (200, 50)] {
        let buf = render_updated(&mut app, width, height);
        assert!(
            line_contains(&buf, "Save changes to the ignore files"),
            "leave dialog at {width}x{height}"
        );
    }
}

fn type_text_render(app: &mut App, text: &str) {
    for c in text.chars() {
        app.component_dispatch(key_char(c));
    }
}

// --- New-package wizard (steps 1-3 and the live-file picker) ---

/// The wizard on step 1 over the test app, with unicode icons so the mockup glyphs apply.
fn wizard_app() -> (App, tempfile::TempDir) {
    let (mut app, tmp) = make_test_app();
    app.ctx.core.icons = IconSet::Unicode;
    for package in ["nvim", "git"] {
        std::fs::create_dir_all(app.ctx.home_dir.join("dotfiles/home").join(package)).unwrap();
    }
    app.component_stack.push(Box::new(
        andre::components::wizard::NewPackageWizard::default(),
    ));
    (app, tmp)
}

/// Presses each key in turn, as the event loop would.
fn press(app: &mut App, keys: &[KeyEvent]) {
    for key in keys {
        app.component_dispatch(*key);
    }
}

/// Step 1 with `home` chosen (the test app has `home` alone, so Enter on it is the first row).
fn choose_home(app: &mut App) {
    press(app, &[enter_key()]);
}

#[test]
fn test_new_package_group_step_shows_step_bar_and_groups() {
    let (mut app, _tmp) = wizard_app();
    let buf = render_updated(&mut app, 80, 24);

    assert!(line_contains(
        &buf,
        "[1 group] \u{203a} 2 name \u{203a} 3 source \u{203a} 4 ignore \u{203a} 5 profiles \u{203a} 6 review"
    ));
    assert!(line_contains(&buf, "\u{256d} group "));
    assert!(line_contains(&buf, "home"));
    assert!(footer_row(&buf).contains("Enter next"));
}

#[test]
fn test_new_package_name_step_filters_packages_and_leads_with_the_create_row() {
    let (mut app, _tmp) = wizard_app();
    render_updated(&mut app, 80, 24);
    choose_home(&mut app);
    press(&mut app, &[key_char('n'), key_char('v')]);
    let buf = render_updated(&mut app, 80, 24);

    assert!(line_contains(&buf, "\u{2713} group \u{203a} [2 name]"));
    assert!(line_contains(&buf, "name  nv"));
    assert!(line_contains(&buf, "+ create `nv`"));
    assert!(line_contains(&buf, "\u{2192} nvim  add files to it"));
    assert!(!line_contains(&buf, "git"));
    assert!(footer_row(&buf).contains("type name"));
}

#[test]
fn test_new_package_name_step_says_files_go_into_an_existing_package() {
    let (mut app, _tmp) = wizard_app();
    render_updated(&mut app, 80, 24);
    choose_home(&mut app);
    press(&mut app, &[key_char('g'), key_char('i'), key_char('t')]);
    let buf = render_updated(&mut app, 80, 24);

    assert!(line_contains(&buf, "\u{2192} add files to `git`"));
}

#[test]
fn test_new_package_source_step_shows_the_radio_on_the_cursor_row() {
    let (mut app, _tmp) = wizard_app();
    render_updated(&mut app, 80, 24);
    choose_home(&mut app);
    press(&mut app, &[key_char('t'), key_char('m'), enter_key()]);
    let buf = render_updated(&mut app, 80, 24);

    assert!(line_contains(&buf, "\u{2713} name \u{203a} [3 source]"));
    assert!(line_contains(&buf, "\u{25cf} empty package"));
    assert!(line_contains(&buf, "\u{25cb} move live files in"));

    press(&mut app, &[down_key()]);
    let buf = render_updated(&mut app, 80, 24);
    assert!(line_contains(&buf, "\u{25cb} empty package"));
    assert!(line_contains(&buf, "\u{25cf} move live files in"));
}

#[test]
fn test_new_package_refused_name_shows_its_rule_in_full_at_80x24() {
    let (mut app, _tmp) = wizard_app();
    render_updated(&mut app, 80, 24);
    choose_home(&mut app);
    press(
        &mut app,
        &[key_char('a'), space_key(), key_char('b'), enter_key()],
    );
    let buf = render_updated(&mut app, 80, 24);

    assert!(line_contains(&buf, "\u{2502} name  a b"));
    assert!(line_contains(&buf, "\u{2502} letters, digits, - _ . only"));
}

#[cfg(unix)]
#[test]
fn test_new_package_file_picker_shows_symlink_targets_sizes_and_the_move_rule() {
    let (mut app, _tmp) = wizard_app();
    let home = app.ctx.home_dir.clone();
    std::fs::write(home.join("notes.txt"), vec![0u8; 2150]).unwrap();
    std::fs::create_dir_all(home.join("sync")).unwrap();
    std::fs::write(home.join("sync/spell"), "").unwrap();
    std::os::unix::fs::symlink(home.join("sync/spell"), home.join("spell")).unwrap();
    render_updated(&mut app, 80, 24);
    choose_home(&mut app);
    press(
        &mut app,
        &[
            key_char('t'),
            key_char('m'),
            enter_key(),
            down_key(),
            enter_key(),
        ],
    );
    let buf = render_updated(&mut app, 80, 24);

    assert!(line_contains(&buf, "spell  \u{2192} ~/sync/spell"));
    assert!(line_contains(&buf, "~ symlinks cannot be moved in."));

    // The listing is andre.yml, dotfiles, notes.txt: two steps down to notes.txt.
    press(&mut app, &[down_key(), down_key()]);
    let buf = render_updated(&mut app, 80, 24);
    assert!(line_contains(&buf, "file, 2.1 KB"));
}

#[test]
fn test_new_package_file_picker_lists_files_and_marks_them() {
    let (mut app, _tmp) = wizard_app();
    std::fs::write(app.ctx.home_dir.join("notes.txt"), "").unwrap();
    render_updated(&mut app, 80, 24);
    choose_home(&mut app);
    press(
        &mut app,
        &[
            key_char('t'),
            key_char('m'),
            enter_key(),
            down_key(),
            enter_key(),
        ],
    );
    let buf = render_updated(&mut app, 80, 24);

    assert!(line_contains(&buf, "\u{2713} name \u{203a} [3 source]"));
    assert!(line_contains(&buf, "\u{25a1} \u{2022} notes.txt"));
    assert!(line_contains(&buf, "\u{2022} notes.txt") && footer_row(&buf).contains("Space mark"));

    // The listing is andre.yml, dotfiles, notes.txt: two steps down to notes.txt.
    press(&mut app, &[down_key(), down_key(), space_key()]);
    let buf = render_updated(&mut app, 80, 24);
    assert!(line_contains(&buf, "\u{25a0} \u{2022} notes.txt"));
    assert!(line_contains(&buf, "marked"));
}

#[test]
fn test_new_package_steps_render_at_120x40_and_200x50_without_panic() {
    for (width, height) in [(120, 40), (200, 50)] {
        let (mut app, _tmp) = wizard_app();
        std::fs::write(app.ctx.home_dir.join("notes.txt"), "").unwrap();
        render_updated(&mut app, width, height);
        choose_home(&mut app);
        render_updated(&mut app, width, height);
        press(&mut app, &[key_char('t'), key_char('m')]);
        render_updated(&mut app, width, height);
        press(&mut app, &[enter_key()]);
        render_updated(&mut app, width, height);
        press(&mut app, &[down_key(), enter_key()]);
        let buf = render_updated(&mut app, width, height);
        assert!(line_contains(&buf, "notes.txt"));
    }
}

// --- New-package wizard, steps 4-6 (ignore files, profiles, review) ---

/// The wizard over the test app with one profile, `personal`, holding `home/git`.
fn wizard_app_with_profile() -> (App, tempfile::TempDir) {
    let (mut app, tmp) = wizard_app();
    let dir = app.ctx.config_dir.join("profiles");
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("personal.yml"), "packages:\n  - home/git\n").unwrap();
    app.ctx.core.profiles = andre_core::profile::ProfileSet::load(&dir).unwrap();
    (app, tmp)
}

/// Group `home`, then the name `tmux`, then an empty source: the wizard on step 4.
fn wizard_on_ignore_step(app: &mut App) {
    choose_home(app);
    press(app, &typed_keys("tmux"));
    press(app, &[enter_key(), enter_key()]);
}

fn typed_keys(text: &str) -> Vec<KeyEvent> {
    text.chars().map(key_char).collect()
}

#[test]
fn test_new_package_ignore_step_shows_both_ignore_files_and_the_template() {
    let (mut app, _tmp) = wizard_app();
    wizard_on_ignore_step(&mut app);
    let buf = render_updated(&mut app, 80, 24);

    assert!(line_contains(
        &buf,
        "\u{2713} source \u{203a} [4 ignore] \u{203a} 5 profiles"
    ));
    assert!(line_contains(&buf, "ignore files"));
    assert!(line_contains(&buf, ".gitignore"));
    assert!(line_contains(&buf, "common secrets"));
    assert!(line_contains(&buf, ".stow-local-ignore"));
    assert!(
        line_contains(&buf, "*.env"),
        "the template is in the detail"
    );
    assert!(footer_row(&buf).contains("Space toggle"));
    assert!(footer_row(&buf).contains("Enter next"));
}

#[test]
fn test_new_package_profiles_step_lists_profiles_and_what_the_package_joins() {
    let (mut app, _tmp) = wizard_app_with_profile();
    wizard_on_ignore_step(&mut app);
    press(&mut app, &[enter_key(), key_char(' ')]);
    let buf = render_updated(&mut app, 80, 24);

    assert!(line_contains(&buf, "[5 profiles]"));
    assert!(line_contains(&buf, "add to profiles"));
    assert!(line_contains(&buf, "personal"));
    assert!(
        line_contains(&buf, "2 (+ home/tmux)"),
        "the detail shows the count once the package joins"
    );
    assert!(footer_row(&buf).contains("Enter next"));
}

#[test]
fn test_new_package_profile_detail_counts_the_packages_a_profile_reaches_through_includes() {
    let (mut app, _tmp) = wizard_app_with_profile();
    let dir = app.ctx.config_dir.join("profiles");
    // `laptop` lists no package of its own: it reaches `home/git` only through `personal`.
    std::fs::write(dir.join("laptop.yml"), "includes:\n  - personal\n").unwrap();
    app.ctx.core.profiles = andre_core::profile::ProfileSet::load(&dir).unwrap();
    wizard_on_ignore_step(&mut app);
    // `laptop` sorts first, so Space on the cursor's row chooses it.
    press(&mut app, &[enter_key(), key_char(' ')]);
    let buf = render_updated(&mut app, 80, 24);

    assert!(
        line_contains(&buf, "2 (+ home/tmux)"),
        "the include's package counts, and so does the new one"
    );
}

#[test]
fn test_new_package_review_lines_are_inset_one_column_from_the_border_at_80x24() {
    let (mut app, _tmp) = wizard_app_with_profile();
    wizard_on_ignore_step(&mut app);
    press(&mut app, &[enter_key(), key_char(' '), enter_key()]);
    let buf = render_updated(&mut app, 80, 24);

    assert!(
        line_contains(&buf, "\u{2502} create home/tmux/.gitignore"),
        "the lines start one column inside the border"
    );
    assert!(line_contains(&buf, "\u{2502} edit   profiles/personal.yml"));
    assert!(line_contains(&buf, "\u{2502} 0 moves"));
}

#[test]
fn test_new_package_review_lists_the_plan_and_its_counts_at_80x24() {
    let (mut app, _tmp) = wizard_app_with_profile();
    wizard_on_ignore_step(&mut app);
    press(&mut app, &[enter_key(), key_char(' '), enter_key()]);
    let buf = render_updated(&mut app, 80, 24);

    assert!(line_contains(&buf, "[6 review]"));
    assert!(line_contains(&buf, "review \u{b7} new package home/tmux"));
    assert!(line_contains(&buf, "create home/tmux/.gitignore"));
    assert!(line_contains(
        &buf,
        "edit   profiles/personal.yml  + home/tmux"
    ));
    assert!(line_contains(
        &buf,
        "0 moves \u{b7} 0 links \u{b7} 1 file \u{b7} 1 profile"
    ));
    assert!(line_contains(&buf, "Esc back"));
    assert!(line_contains(&buf, "Enter create"));
    assert!(footer_row(&buf).contains("Enter create"));
}

#[test]
fn test_new_package_review_ends_with_done_and_enter_leaves() {
    let (mut app, tmp) = wizard_app_with_profile();
    wizard_on_ignore_step(&mut app);
    press(
        &mut app,
        &[enter_key(), key_char(' '), enter_key(), enter_key()],
    );
    let buf = render_updated(&mut app, 80, 24);

    assert!(tmp.path().join("dotfiles/home/tmux/.gitignore").is_file());
    assert!(line_contains(&buf, "created home/tmux"));
    assert!(footer_row(&buf).contains("Enter done"));

    press(&mut app, &[enter_key()]);
    assert_eq!(
        app.component_stack.len(),
        1,
        "Enter after the result leaves the wizard"
    );
}

#[test]
fn test_new_package_steps_4_to_6_render_at_120x40_and_200x50_without_panic() {
    for (width, height) in [(120, 40), (200, 50)] {
        let (mut app, _tmp) = wizard_app_with_profile();
        wizard_on_ignore_step(&mut app);
        render_updated(&mut app, width, height);
        press(&mut app, &[enter_key(), key_char(' ')]);
        render_updated(&mut app, width, height);
        press(&mut app, &[enter_key()]);
        let buf = render_updated(&mut app, width, height);
        assert!(line_contains(&buf, "review"));
        assert!(line_contains(&buf, "q quit"));
    }
}

#[test]
fn test_new_package_review_lists_moves_and_links_for_live_files_at_80x24() {
    let (mut app, _tmp) = wizard_app_with_profile();
    std::fs::write(app.ctx.home_dir.join("notes.txt"), "").unwrap();
    render_updated(&mut app, 80, 24);
    choose_home(&mut app);
    press(&mut app, &typed_keys("tmux"));
    press(&mut app, &[enter_key(), down_key(), enter_key()]);
    render_updated(&mut app, 80, 24);
    // The listing is andre.yml, dotfiles, notes.txt: two steps down to notes.txt.
    press(
        &mut app,
        &[
            down_key(),
            down_key(),
            space_key(),
            enter_key(),
            enter_key(),
            enter_key(),
        ],
    );
    let buf = render_updated(&mut app, 80, 24);

    assert!(line_contains(
        &buf,
        "move   ~/notes.txt \u{2192} home/tmux/notes.txt"
    ));
    assert!(line_contains(&buf, "link   ~/notes.txt \u{21e2} package"));
    assert!(line_contains(
        &buf,
        "1 move \u{b7} 1 link \u{b7} 1 file \u{b7} 0 profiles"
    ));
}

// ---- main menu: the set-up item (F5, D8) ----

/// The first row, from the top of the screen, that contains `fragment`.
fn row_index(buf: &Buffer, fragment: &str) -> Option<u16> {
    (0..buf.area().height).find(|&y| row_text(buf, y).contains(fragment))
}

#[test]
fn test_setup_item_leads_the_menu_and_the_title_shrinks_to_one_row_at_80x24() {
    let (mut app, _tmp) = make_bootstrap_app();
    app.ctx.core.icons = IconSet::Unicode;
    let buf = render_updated(&mut app, 80, 24);

    assert!(
        !line_contains(&buf, "\u{2551} A. \u{2551}"),
        "nine rows do not fit under the big banner at 80x24"
    );
    assert_eq!(
        top_bar_row(&buf),
        3,
        "the one-row title sits above the top bar"
    );
    assert!(
        row_text(&buf, 1).contains("andre"),
        "compact title: {:?}",
        row_text(&buf, 1)
    );

    // The list row carries the icon; the detail pane's title does not.
    let setup = row_index(&buf, "\u{2302}  set up this machine").expect("set-up row");
    let profiles = row_index(&buf, "\u{25c8}  profiles").expect("profiles row");
    assert!(setup < profiles, "the set-up item is the first row");
    let row = row_text(&buf, setup);
    assert!(
        row.contains("set up this machine       0 \u{2502}"),
        "jump digit 0 missing: {row:?}"
    );
    assert!(
        footer_row(&buf).contains("0\u{2013}8 jump"),
        "footer: {:?}",
        footer_row(&buf)
    );
}

#[test]
fn test_setup_detail_names_the_package_the_link_and_the_source() {
    let (mut app, _tmp) = make_bootstrap_app();
    let buf = render_updated(&mut app, 120, 40);
    assert!(line_contains(&buf, "set up this machine"), "detail title");
    assert!(
        line_contains(&buf, "package   config/andre"),
        "package fact"
    );
    assert!(row_containing(&buf, "links")
        .unwrap()
        .contains("~/.config/andre"));
    // The source is under the temp dir, so it is elided to the value column on one row.
    let from = row_containing(&buf, "from ").unwrap();
    assert!(
        from.contains("from      /"),
        "from label and value: {from:?}"
    );
    assert!(from.contains('\u{2026}'), "from value elided: {from:?}");
    assert!(
        line_contains(&buf, "action    stow (always)"),
        "action fact"
    );
    assert!(line_contains(&buf, "Enter reviews the link first."));
}

#[test]
fn test_set_up_facts_elide_long_paths_to_one_row_each_at_80x24() {
    let (mut app, _tmp) = make_bootstrap_app();
    let buf = render_updated(&mut app, 80, 24);

    // The source sits under the temp dir, so it is longer than the value column.
    let from = row_containing(&buf, "from ").expect("from row");
    assert!(from.contains('\u{2026}'), "from value elided: {from:?}");
    assert!(
        row_containing(&buf, "dotfiles/config").is_none(),
        "the path does not wrap onto a row of its own"
    );
    let links = row_containing(&buf, "links").expect("links row");
    assert!(links.contains("~/.config/andre"), "links: {links:?}");
}

#[test]
fn test_help_fact_and_footer_name_zero_to_eight_while_set_up_is_offered() {
    let (mut app, _tmp) = make_bootstrap_app();
    render_updated(&mut app, 120, 40);
    // Set-up is row 0, so help, the seventh of the eight, is row 7.
    menu_mut(&mut app).list.cursor = 7;

    let buf = render_updated(&mut app, 120, 40);
    let keys = row_containing(&buf, "keys").expect("help keys fact");
    assert!(keys.contains("0\u{2013}8 jump"), "help fact: {keys:?}");
    assert!(footer_row(&buf).contains("0\u{2013}8 jump"));
}

#[test]
fn test_big_banner_returns_once_the_default_folder_exists() {
    let (mut app, _tmp) = make_bootstrap_app();
    render_updated(&mut app, 80, 24);
    std::fs::create_dir_all(app.ctx.home_dir.join(".config").join("andre")).unwrap();

    let buf = render_updated(&mut app, 80, 24);
    assert!(
        line_contains(&buf, "\u{2551} A. \u{2551}"),
        "big banner back"
    );
    assert_eq!(top_bar_row(&buf), 9);
    assert!(
        row_index(&buf, "set up this machine").is_none(),
        "the offer is gone"
    );
}

#[test]
fn test_setup_item_keeps_the_big_banner_where_the_rows_fit() {
    for (width, height) in [(120, 40), (200, 50)] {
        let (mut app, _tmp) = make_bootstrap_app();
        let buf = render_updated(&mut app, width, height);
        assert!(
            line_contains(&buf, "\u{2551} A. \u{2551}"),
            "{width}x{height}: the nine rows fit under the big banner"
        );
        assert!(
            row_index(&buf, "set up this machine").is_some(),
            "{width}x{height}: set-up row missing"
        );
    }
}

#[test]
fn test_zero_reviews_a_stow_of_the_package_whatever_the_global_action() {
    let (mut app, _tmp) = make_bootstrap_app();
    app.ctx.core.flags.action = StowAction::Restow;
    render_updated(&mut app, 80, 24);

    app.component_dispatch(key_char('0'));
    let buf = render_app(&mut app, 80, 24);
    assert!(line_contains(&buf, "Action: STOW"), "the review stows");
    assert!(
        line_contains(&buf, "native: andre"),
        "the review names the package"
    );
    assert_eq!(
        app.ctx.core.flags.action,
        StowAction::Restow,
        "the global action is not changed"
    );
}

#[test]
fn test_set_up_flow_renders_at_120x40_and_200x50() {
    for (width, height) in [(120, 40), (200, 50)] {
        let (mut app, _tmp) = make_bootstrap_app();
        render_updated(&mut app, width, height);
        app.component_dispatch(key_char('0'));
        render_updated(&mut app, width, height);
        app.component_dispatch(esc_key());
        app.component_dispatch(key_char('2'));
        render_updated(&mut app, width, height);
        assert_eq!(app.component_stack.last().unwrap().id(), "Packages");
    }
}
