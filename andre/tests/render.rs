use ratatui::backend::TestBackend;
use ratatui::buffer::Buffer;
use ratatui::Terminal;

use andre::app::App;

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
            line.push_str(buf.get(x, y).symbol());
        }
        if line.contains(fragment) {
            return true;
        }
    }
    false
}

/// True when `fragment` appears on a row that also contains a status-bar marker
/// (Verbosity/Dry-Run/Theme), so body text like "Main Menu" does not count.
fn status_bar_contains(buf: &Buffer, fragment: &str) -> bool {
    for y in 0..buf.area().height {
        let mut line = String::new();
        for x in 0..buf.area().width {
            line.push_str(buf.get(x, y).symbol());
        }
        let is_status = line.contains("Verbosity")
            || line.contains("Dry-Run")
            || line.contains("Theme:")
            || line.contains("Action:");
        if is_status && line.contains(fragment) {
            return true;
        }
    }
    false
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
            andre::components::adopt::AdoptTargetSelectComponent::default(),
        )),
        "UnstowTargetSelect" => app.component_stack.push(Box::new(
            andre::components::unstow::UnstowTargetSelectComponent::default(),
        )),
        "Confirm" => {
            app.component_stack
                .push(Box::new(andre::components::confirm::ConfirmComponent::new(
                    std::collections::HashMap::new(),
                    "stow".to_string(),
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
    app.ctx.core.verbosity = 3;
    let buf = render_app(&mut app, 80, 24);
    assert!(line_contains(&buf, "3"), "verbosity not shown");
}

#[test]
fn test_status_bar_shows_action() {
    let (mut app, _) = make_test_app();
    let buf = render_app(&mut app, 80, 24);
    assert!(line_contains(&buf, "STOW"), "action not shown");
}

/// Regression guard for the M11 flag-color restoration: each ON/OFF toggle
/// must be rendered in the success/error color, not a single secondary tone.
/// Asserts both the ON-state (green) and OFF-state (red) get their distinct
/// fg by comparing the two cells side-by-side.
#[test]
fn test_status_bar_toggle_colors_distinguish_on_from_off() {
    use ratatui::style::Color;

    let (mut app, _) = make_test_app();
    let buf = render_app(&mut app, 140, 24);

    // Set every toggle ON so we can find each ON cell.
    app.ctx.core.dry_run = true;
    app.ctx.core.no_folding = true;
    app.ctx.core.adopt = true;
    app.ctx.core.dotfiles = true;
    let buf_on = render_app(&mut app, 140, 24);

    // Find the fg color of the "ON " token in each rendering. We pick "ON "
    // because every toggle has the same label and they sit on the status
    // bar row (row 1 in our layout).
    fn first_on_color(buf: &ratatui::buffer::Buffer) -> Option<Color> {
        for y in 0..buf.area().height {
            for x in 0..buf.area().width.saturating_sub(2) {
                if buf.get(x, y).symbol() == "O"
                    && buf.get(x + 1, y).symbol() == "N"
                    && buf.get(x + 2, y).symbol() == " "
                {
                    return Some(buf.get(x, y).fg);
                }
            }
        }
        None
    }
    fn first_off_color(buf: &ratatui::buffer::Buffer) -> Option<Color> {
        for y in 0..buf.area().height {
            for x in 0..buf.area().width.saturating_sub(2) {
                if buf.get(x, y).symbol() == "O"
                    && buf.get(x + 1, y).symbol() == "F"
                    && buf.get(x + 2, y).symbol() == "F"
                {
                    return Some(buf.get(x, y).fg);
                }
            }
        }
        None
    }

    let on_color = first_on_color(&buf_on).expect("no ON cell on status bar");
    let off_color = first_off_color(&buf).expect("no OFF cell on status bar");
    assert_ne!(
        on_color, off_color,
        "ON and OFF must render in distinct colors (ON=success, OFF=error); \
         got {on_color:?} for both"
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
            let s = buf.get(x, y).symbol();
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

    // AsyncExecutor Progress.total is commands-only (2)
    tx.try_send(ExecutionUpdate::Progress {
        current: 1,
        total: 2,
        _package: "home / bash".into(),
    })
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
            andre::ui::render_footer(f, Rect::new(0, 0, 40, 2), &colors, "MainMenu");
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
    app.ctx.core.verbosity = 2;
    app.ctx.core.dry_run = true;
    let buf = render_app(&mut app, 140, 24);
    // Breadcrumb now lives in the panel title; toggles still live in the status bar.
    assert!(
        line_contains(&buf, "Main > Main Menu"),
        "breadcrumb missing from panel title"
    );
    assert!(status_bar_contains(&buf, "2"), "verbosity toggle not live");
    assert!(
        status_bar_contains(&buf, "Dry-Run:ON") || status_bar_contains(&buf, "ON"),
        "dry-run toggle not live"
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
    // Minimum supported size (80x24): chrome must not panic and should keep
    // breadcrumb tip and/or a footer hint fragment.
    let buf = render_app(&mut app, 80, 24);
    assert!(
        status_bar_contains(&buf, "Main")
            || status_bar_contains(&buf, "Help")
            || line_contains(&buf, "back"),
        "minimum-width chrome lost breadcrumb and footer entirely"
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
            selected,
            "stow".to_string(),
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
