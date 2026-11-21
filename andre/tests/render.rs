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
    browse_cursor_to, down_key, enter_key, esc_key, key_char, make_bootstrap_app, make_empty_app,
    make_flat_bootstrap_app, make_flat_packages_app, make_flat_test_app, make_test_app,
    make_unstow_app, push_browse, space_key, up_key,
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
        "NewPackage" => app.component_stack.push(Box::new(
            andre::components::wizard::NewPackageWizard::default(),
        )),
        "Confirm" => {
            app.component_stack
                .push(Box::new(andre::components::confirm::ConfirmComponent::new(
                    Vec::new(),
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
            let target = app.ctx.home_dir.clone();
            app.component_stack.push(Box::new(
                andre::components::unstow::UnstowBrowseComponent::at(target),
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
        "UnstowBrowse",
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
    let (mut app, _tmp) = make_flat_test_app();
    push_screen(&mut app, "Settings");
    let buf = render_updated(&mut app, 80, 24);
    assert!(
        line_contains(&buf, "settings"),
        "settings screen title not shown"
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
    let (mut app, _tmp) = make_flat_test_app();
    push_screen(&mut app, "Settings");
    let buf = render_app(&mut app, 80, 24);
    assert!(line_contains(&buf, "verbose"), "verbose row not visible");
}

#[test]
fn test_execute_screen_shows_running() {
    let (mut app, _tmp) = make_test_app();
    push_screen(&mut app, "Execute");
    let buf = render_app(&mut app, 80, 24);
    assert!(line_contains(&buf, "running"), "running state not shown");
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
                name: "home/bash".into(),
                success: true,
                error_message: None,
            },
            ExecutionResult {
                name: "home/vim".into(),
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
                name: "home/bash".into(),
                success: true,
                error_message: None,
            },
            ExecutionResult {
                name: "work/git".into(),
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
        name: "home/missing".into(),
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
                name: "home/a".into(),
                success: true,
                error_message: None,
            },
            ExecutionResult {
                name: "home/b".into(),
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
                name: "home/bash".into(),
                success: true,
                error_message: None,
            },
            ExecutionResult {
                name: "home/vim".into(),
                success: false,
                error_message: Some("conflict".into()),
            },
        ],
    );
    exec.state = ExecutionState::Completed;
    app.component_stack.push(Box::new(exec));

    let buf = render_app(&mut app, 80, 24);
    assert!(
        line_contains(&buf, "\u{2713} 1 succeeded  \u{2717} 1 failed"),
        "completion summary missing on Finished"
    );
    assert!(
        line_contains(&buf, "Enter done"),
        "done button missing on Finished"
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
                name: "home/bash".into(),
                success: true,
                error_message: None,
            },
            ExecutionResult {
                name: "home/gone".into(),
                success: false,
                error_message: Some("Skipped: source directory not found".into()),
            },
        ],
    );
    exec.state = ExecutionState::Completed;
    app.component_stack.push(Box::new(exec));

    let buf = render_app(&mut app, 80, 24);
    assert!(
        line_contains(
            &buf,
            "\u{2713} 1 succeeded  \u{2717} 0 failed  \u{2205} 1 skipped"
        ),
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
            name: format!("home/pkg{i}"),
            success: true,
            error_message: None,
        })
        .collect();
    let mut exec = ExecuteComponent::new(rx, results.len(), results);
    exec.state = ExecutionState::Completed;
    app.component_stack.push(Box::new(exec));

    // Drive the cursor down; the list must honor it in render.
    for _ in 0..5 {
        app.component_dispatch(down_key());
    }
    let top = app.component_stack.last().unwrap();
    let exec = top.as_any().downcast_ref::<ExecuteComponent>().unwrap();
    assert_eq!(
        exec.list.cursor, 5,
        "Completed state should advance the cursor on down"
    );
    let buf = render_app(&mut app, 80, 24);
    assert!(
        line_contains(&buf, "pkg"),
        "completed list must still render while scrolled"
    );
}

/// Unstow opens the browse screen at the target: there is no group list.
#[test]
fn test_unstow_opens_the_browse_screen_at_the_target() {
    let (mut app, _tmp) = make_flat_test_app();
    app.component_dispatch(key_char('4'));
    let buf = render_updated(&mut app, 80, 24);
    assert!(
        row_text(&buf, 1).contains("andre  main › unstow"),
        "title row: {:?}",
        row_text(&buf, 1)
    );
    assert!(
        line_contains(&buf, "folder"),
        "the cursor folder's detail is missing"
    );
    assert!(!line_contains(&buf, "groups"), "the group list is back");
}

#[test]
fn test_unstow_browse_shows_the_link_detail_and_no_path_line() {
    let (mut app, tmp) = make_unstow_app();
    push_browse(&mut app, tmp.path());
    browse_cursor_to(&mut app, ".bashrc");
    let buf = render_updated(&mut app, 80, 24);
    assert!(line_contains(&buf, "points to"), "link target missing");
    assert!(
        line_contains(&buf, "package   test"),
        "owning package missing"
    );
    assert!(!line_contains(&buf, "Path:"), "the path line is back");
    assert!(!line_contains(&buf, " legend "), "the legend is back");
    assert!(
        !line_contains(&buf, "notes.txt"),
        "regular files are not listed"
    );
}

/// The row index of the first row whose text contains `fragment`.
fn row_of(buf: &Buffer, fragment: &str) -> u16 {
    (0..buf.area().height)
        .find(|&y| row_text(buf, y).contains(fragment))
        .unwrap_or_else(|| panic!("no row contains {fragment:?}"))
}

/// The target wraps under its value column: the second row starts where the first row's
/// value starts.
#[test]
fn test_unstow_browse_wraps_points_to_under_its_value_column() {
    let (mut app, tmp) = make_unstow_app();
    // A package name long enough that the target does not fit one row.
    let package = tmp.path().join("dotfiles/terminal-emulator");
    std::fs::create_dir_all(&package).unwrap();
    std::fs::write(package.join(".bashrc"), "").unwrap();
    std::os::unix::fs::symlink(
        "dotfiles/terminal-emulator/.bashrc",
        tmp.path().join(".wrapped"),
    )
    .unwrap();
    push_browse(&mut app, tmp.path());
    browse_cursor_to(&mut app, ".wrapped");
    let buf = render_updated(&mut app, 80, 24);
    let y = row_of(&buf, "points to");
    let value = column_of(&buf, y, "~/dotfiles/").expect("first row of the target");
    assert_eq!(
        column_of(&buf, y + 1, "terminal-emulator"),
        Some(value),
        "{:?}",
        row_text(&buf, y + 1)
    );
}

/// Under the unicode set a marked link draws `■` in `accent`, an unmarked link `□` in
/// `text_dim`, and a folder no box at all.
#[test]
fn test_unstow_browse_marks_links_with_boxes_and_folders_with_none() {
    let (mut app, tmp) = make_unstow_app();
    app.ctx.core.icons = IconSet::Unicode;
    push_browse(&mut app, tmp.path());
    browse_cursor_to(&mut app, ".bashrc");
    app.component_dispatch(space_key());
    // The cursor moves off the links, so none of the three rows is the selected row.
    browse_cursor_to(&mut app, "dotfiles");
    let buf = render_updated(&mut app, 80, 24);
    let colors = ThemeColors::from_theme(app.ctx.core.theme);

    let marked = row_of(&buf, "~ .bashrc");
    let x = column_of(&buf, marked, "■").expect("marked link draws a filled box");
    assert_eq!(buf[(x, marked)].fg, colors.accent);

    let unmarked = row_of(&buf, "~ .profile");
    let x = column_of(&buf, unmarked, "□").expect("unmarked link draws an empty box");
    assert_eq!(buf[(x, unmarked)].fg, colors.text_dim);

    let folder = row_of(&buf, "/ dotfiles");
    let text = row_text(&buf, folder);
    assert!(
        !text.contains('■') && !text.contains('□'),
        "a folder has no box: {text:?}"
    );
}

/// The review's `points to` is a fact row directly under `remove`, with the target from the
/// packages folder under the same value column.
#[test]
fn test_unstow_review_points_to_is_a_fact_row_under_remove() {
    let (mut app, tmp) = make_unstow_app();
    push_browse(&mut app, tmp.path());
    browse_cursor_to(&mut app, ".bashrc");
    app.component_dispatch(enter_key());
    let buf = render_updated(&mut app, 80, 24);
    let y = row_of(&buf, "~/.bashrc");
    assert_eq!(
        column_of(&buf, y + 1, "points to"),
        Some(column_of(&buf, y, "remove").expect("the remove label")),
        "{:?}",
        row_text(&buf, y + 1)
    );
    assert_eq!(
        column_of(&buf, y + 1, "test/.bashrc"),
        column_of(&buf, y, "~/.bashrc"),
        "{:?}",
        row_text(&buf, y + 1)
    );
    assert!(
        !buffer_text(&buf).contains("dotfiles/test/.bashrc"),
        "{}",
        buffer_text(&buf)
    );
}

/// A review target too long for one value row wraps under its value column, as the browse
/// does, and does not break inside a folder name at column 0.
#[test]
fn test_unstow_review_wraps_points_to_under_its_value_column() {
    let (mut app, tmp) = make_unstow_app();
    let file = tmp
        .path()
        .join("dotfiles/terminal-emulator/.config/terminal-emulator/config.toml");
    std::fs::create_dir_all(file.parent().unwrap()).unwrap();
    std::fs::write(&file, "").unwrap();
    std::os::unix::fs::symlink(
        "dotfiles/terminal-emulator/.config/terminal-emulator/config.toml",
        tmp.path().join(".wrapped"),
    )
    .unwrap();
    push_browse(&mut app, tmp.path());
    browse_cursor_to(&mut app, ".wrapped");
    app.component_dispatch(enter_key());
    let buf = render_updated(&mut app, 80, 24);
    let y = row_of(&buf, "points to");
    let value = column_of(&buf, y, "terminal-emulator/").expect("first row of the target");
    assert_eq!(
        column_of(&buf, y + 1, "terminal-emulator/"),
        Some(value),
        "{:?}",
        row_text(&buf, y + 1)
    );
    assert_eq!(
        column_of(&buf, y + 2, "config.toml"),
        Some(value),
        "{:?}",
        row_text(&buf, y + 2)
    );
}

/// The review says which package to restow to put a removed link back.
#[test]
fn test_unstow_review_names_the_package_to_restow() {
    let (mut app, tmp) = make_unstow_app();
    push_browse(&mut app, tmp.path());
    browse_cursor_to(&mut app, ".bashrc");
    app.component_dispatch(enter_key());
    let buf = render_updated(&mut app, 80, 24);
    assert!(
        buffer_text(&buf).contains("Restow test"),
        "{}",
        buffer_text(&buf)
    );
}

#[test]
fn test_unstow_review_lists_the_marked_links_with_a_remove_button() {
    let (mut app, tmp) = make_unstow_app();
    push_browse(&mut app, tmp.path());
    browse_cursor_to(&mut app, ".bashrc");
    app.component_dispatch(space_key());
    browse_cursor_to(&mut app, ".profile");
    app.component_dispatch(space_key());
    let buf = render_updated(&mut app, 80, 24);
    assert!(line_contains(&buf, "2 marked"), "mark count missing");
    assert!(
        line_contains(&buf, "review 2"),
        "review hint should count the marks"
    );

    app.component_dispatch(enter_key());
    let buf = render_updated(&mut app, 80, 24);
    assert!(line_contains(&buf, "remove 2 links"), "list title missing");
    assert!(
        line_contains(&buf, "[ y remove 2 ]"),
        "remove button missing"
    );
}

/// The browse and review screens render at the two larger sizes without panicking.
#[test]
fn test_unstow_browse_and_review_render_at_120x40_and_200x50() {
    for (width, height) in [(120, 40), (200, 50)] {
        let (mut app, tmp) = make_unstow_app();
        push_browse(&mut app, tmp.path());
        render_updated(&mut app, width, height);
        browse_cursor_to(&mut app, ".bashrc");
        app.component_dispatch(space_key());
        render_updated(&mut app, width, height);
        app.component_dispatch(enter_key());
        let buf = render_updated(&mut app, width, height);
        assert!(
            row_containing(&buf, "q quit").is_some(),
            "{width}x{height}: footer missing"
        );
    }
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
        ("", "navigate"),           // MainMenu
        ("Packages", "mark"),       // packages browser
        ("Settings", "change"),     // settings
        ("Help", "back"),           // help
        ("Confirm", "cancel"),      // review y/n
        ("Execute", "inspect"),     // run screen, not finished
        ("UnstowBrowse", "review"), // unstow browse
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
        line_contains(&buf, "main › settings"),
        "breadcrumb did not reflect MainMenu > Settings stack"
    );
}

#[test]
fn test_breadcrumb_updates_on_deeper_stack() {
    let (mut app, _tmp) = make_test_app();
    push_screen(&mut app, "Help");
    let buf = render_app(&mut app, 120, 24);
    assert!(
        line_contains(&buf, "main › help"),
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
            line_contains(&buf, "main › settings"),
            "breadcrumb missing under theme {:?}",
            theme.name()
        );
        assert!(
            line_contains(&buf, "change"),
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
        line_contains(&buf, "terminal too small"),
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
    use helpers::enter_key;

    let (mut app, _tmp) = make_flat_test_app();
    push_screen(&mut app, "Settings");
    select_setting(&mut app, "icons");

    let buf = render_app(&mut app, 80, 40);
    let row = row_containing(&buf, "icons  ").expect("icons row missing from settings");
    assert!(row.contains("nerd"), "default icon set not shown: {row:?}");

    app.component_dispatch(enter_key());
    app.component_dispatch(down_key());
    let buf = render_app(&mut app, 80, 40);
    let row = row_containing(&buf, "icons  ").expect("icons row missing from settings");
    assert!(
        row.contains("unicode"),
        "Enter must show the pending set: {row:?}"
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
    let selected = vec!["home/test".to_string()];
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
        row_text(&buf, 1).contains("andre  main › packages"),
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
        .insert("home/test".to_string(), StowStatus::Stowed);

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
        row_containing(&buf, "links to")
            .expect("new package links to fact missing")
            .contains('~'),
        "links to should be shown relative to home"
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
const CURSOR_SCREENS: [&str; 3] = ["Packages", "Settings", "UnstowBrowse"];

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

/// Puts the settings cursor on the row labelled `label`, after the update that builds
/// the rows.
fn select_setting(app: &mut App, label: &str) {
    app.update_components();
    let settings = app
        .component_stack
        .last_mut()
        .unwrap()
        .as_any_mut()
        .downcast_mut::<andre::components::settings::SettingsComponent>()
        .unwrap();
    settings.list.cursor = settings
        .items()
        .iter()
        .position(|item| item.label == label)
        .unwrap_or_else(|| panic!("no settings row labelled {label}"));
}

/// The settings screen on a flat config (`dotfiles` holds the package `test`), its cursor on
/// `packages`.
fn flat_settings_on_packages() -> (App, tempfile::TempDir) {
    let (mut app, tmp) = make_flat_test_app();
    push_screen(&mut app, "Settings");
    select_setting(&mut app, "packages");
    (app, tmp)
}

#[test]
fn test_settings_at_80x24_lists_the_packages_folder_and_no_groups() {
    let (mut app, _tmp) = make_flat_test_app();
    push_screen(&mut app, "Settings");
    let buf = render_updated(&mut app, 80, 24);

    assert!(line_contains(&buf, "config"), "config header missing");
    assert!(line_contains(&buf, "packages "), "packages row missing");
    assert!(
        line_contains(&buf, "~/dotfiles"),
        "folder not shown as ~/dotfiles"
    );
    assert!(
        line_contains(&buf, "global ignore"),
        "global ignore row missing"
    );
    assert!(
        !buffer_text(&buf).contains("groups"),
        "a groups section is drawn"
    );
}

#[test]
fn test_settings_packages_detail_names_the_folder_and_where_links_go() {
    let (mut app, _tmp) = flat_settings_on_packages();
    let buf = render_updated(&mut app, 80, 24);

    assert!(line_contains(&buf, "Every folder here is a package."));
    assert!(line_contains(&buf, "links to"), "links to fact missing");
    assert!(line_contains(&buf, "Enter picks another folder."));
    assert!(
        line_contains(&buf, "Enter change"),
        "footer says Enter change"
    );
}

#[test]
fn test_settings_packages_folder_picker_at_80x24_has_its_title_and_buttons() {
    let (mut app, _tmp) = flat_settings_on_packages();
    app.component_dispatch(enter_key());
    let buf = render_updated(&mut app, 80, 24);

    assert!(
        line_contains(&buf, "packages folder"),
        "picker title missing"
    );
    assert!(line_contains(&buf, "Esc cancel"), "cancel button missing");
    assert!(line_contains(&buf, "Enter choose"), "choose button missing");
}

#[test]
fn test_settings_packages_screens_render_at_120x40_and_200x50() {
    for (width, height) in [(120, 40), (200, 50)] {
        let (mut app, _tmp) = flat_settings_on_packages();
        let buf = render_updated(&mut app, width, height);
        assert!(
            line_contains(&buf, "~/dotfiles"),
            "{width}x{height}: no folder"
        );

        app.component_dispatch(enter_key());
        render_updated(&mut app, width, height);
    }
}

/// Puts the settings cursor on the first setting.
fn select_first_setting(app: &mut App) {
    select_setting(app, "action");
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
        ("Status", "packages 1", "Stow Status"),
        ("Settings", "settings", ""),
        ("Help", "help", ""),
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
            text.matches("main ›").count(),
            1,
            "{screen}: the breadcrumb is repeated in a panel title"
        );
    }

    // The unstow browse screen has no legend: its detail says what a link is.
    let (mut app, _tmp) = make_flat_status_app();
    push_screen(&mut app, "UnstowBrowse");
    let text = buffer_text(&render_updated(&mut app, 80, 24));
    assert!(!text.contains(" legend "), "UnstowBrowse: legend is back");
    assert!(!text.contains("Path:"), "UnstowBrowse: path line is back");

    // The status screen has no legend: its counts sit in the chip row instead.
    let (mut app, _tmp) = make_flat_status_app();
    push_screen(&mut app, "Status");
    let text = buffer_text(&render_updated(&mut app, 80, 24));
    assert!(!text.contains(" legend "), "Status: legend is back");
}

#[test]
fn test_package_checkboxes_follow_the_icon_set() {
    let cases = [
        (IconSet::Nerd, NERD.checkbox_on, NERD.checkbox_off),
        (IconSet::Unicode, UNICODE.checkbox_on, UNICODE.checkbox_off),
    ];
    for (icons, on, off) in cases {
        // The package row is marked when it is, and off when it is not.
        for (marked, glyph, other) in [(true, on, off), (false, off, on)] {
            let (mut app, _tmp) = make_flat_test_app();
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
                .find(|text| text.contains("test"))
                .expect("package row missing");
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
        let (mut app, _tmp) = make_flat_test_app();
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
        let (mut app, tmp) = make_flat_status_app();
        let broken = tmp.path().join("dotfiles/home/broken");
        std::fs::create_dir_all(&broken).unwrap();
        std::fs::write(broken.join(".broken_rc"), "x").unwrap();
        // Look-around is rejected by the regex engine, so the list does not compile.
        std::fs::write(broken.join(".stow-local-ignore"), "(?=x)y\n").unwrap();
        app.ctx.core.icons = set;
        let colors = ThemeColors::from_theme(app.ctx.core.theme);
        push_screen(&mut app, "Status");
        let status = render_updated(&mut app, 80, 24);
        let border = list_border(&status, "packages 7");

        let y = (0..status.area().height)
            .find(|&y| list_text(&status, y, border).contains("broken"))
            .unwrap_or_else(|| panic!("{set:?}: broken row missing"));
        assert!(
            !list_text(&status, y, border).contains("missing"),
            "{set:?}: an invalid list must not read as missing"
        );
        let (_, fg) = list_glyph_on_row(&status, y, border, icons.invalid)
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
/// A flat app whose packages folder is `dotfiles/home`, so the status fixtures are its
/// packages (`add_status_fixtures`), with the package `test` among them. Links go to HOME.
fn make_flat_home_app() -> (App, tempfile::TempDir) {
    let tmp = tempfile::TempDir::new().unwrap();
    let path = tmp.path().join("andre.yml");
    std::fs::write(
        &path,
        "global:\n  stow:\n    verbosity: 0\npackages: dotfiles/home\n",
    )
    .unwrap();
    write_package(&tmp.path().join("dotfiles/home"), "test", ".bashrc");
    let app = App::with_home(path, tmp.path().to_path_buf()).unwrap();
    (app, tmp)
}

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

/// A flat app whose packages folder `dotfiles/home` holds the status fixtures and `test`.
/// HOME is the target, so the fixtures' links and blocks sit in the test home.
fn make_flat_status_app() -> (App, tempfile::TempDir) {
    let (app, tmp) = make_flat_home_app();
    add_status_fixtures(tmp.path());
    (app, tmp)
}

/// The package status vocabulary is the same on the packages browser and the status screens:
/// one glyph, one colour and one lowercase word per status.
#[test]
fn test_package_and_status_screens_share_glyph_colour_and_word() {
    for set in [IconSet::Nerd, IconSet::Unicode] {
        let icons = Icons::for_set(set);

        let (mut packages_app, tmp) = make_flat_home_app();
        add_status_fixtures(tmp.path());
        packages_app.ctx.core.icons = set;
        let colors = ThemeColors::from_theme(packages_app.ctx.core.theme);
        push_screen(&mut packages_app, "Packages");
        let packages = render_updated(&mut packages_app, 80, 25);
        // The cursor row keeps its status glyph only where the colour reads at 3:1, so
        // its colour is checked by the cursor-row test instead.
        let cursor_row = find_accent(&packages).map(|(_, y)| y);
        let border = list_border(&packages, "packages 6");

        let (mut status_app, _tmp) = make_flat_status_app();
        status_app.ctx.core.icons = set;
        push_screen(&mut status_app, "Status");
        let status = render_updated(&mut status_app, 80, 25);
        let status_border = list_border(&status, "packages 6");
        // At 120 columns the list pane is wide enough to show each status word.
        let wide = render_updated(&mut status_app, 120, 40);
        let wide_border = list_border(&wide, "packages 6");

        let mut checked = 0;
        for (name, state) in STATUS_FIXTURES {
            let glyph = icons.status(state);
            let word = status_word(state);
            let colour = status_color(&colors, state);

            let package_y = (0..packages.area().height)
                .find(|&y| list_text(&packages, y, border).contains(name))
                .unwrap_or_else(|| panic!("{set:?}: {name} row missing"));
            let status_y = (0..status.area().height)
                .find(|&y| list_text(&status, y, status_border).contains(name))
                .unwrap_or_else(|| panic!("{set:?}: {name} status row missing"));

            let (_, status_fg) = list_glyph_on_row(&status, status_y, status_border, glyph)
                .unwrap_or_else(|| panic!("{set:?}: {name}: status glyph {glyph:?} missing"));
            assert_eq!(status_fg, colour, "{set:?}: {name} on the status screen");

            let wide_y = (0..wide.area().height)
                .find(|&y| list_text(&wide, y, wide_border).contains(name))
                .unwrap_or_else(|| panic!("{set:?}: {name} wide status row missing"));
            assert!(
                list_text(&wide, wide_y, wide_border).contains(&word),
                "{set:?}: {name} should read {word:?} on the status screen"
            );

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
        "global:\n  stow:\n    verbosity: 0\npackages: dotfiles/home\n",
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
    let (mut app, tmp) = make_flat_home_app();
    let home = tmp.path().join("dotfiles/home");
    write_package(&home, &long_package(), ".long_rc");
    write_package(&home, "deploy", ".deploy_rc");
    write_package(&home, CJK_PACKAGE, ".kanji_rc");
    app.ctx.core.icons = IconSet::Unicode;
    push_screen(&mut app, "Packages");
    let buf = render_updated(&mut app, 80, 24);
    let border = list_border(&buf, "packages 4");
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
    let (mut app, _tmp) = make_flat_test_app();
    push_screen(&mut app, "Packages");
    let colors = ThemeColors::from_theme(app.ctx.core.theme);
    let buf = render_updated(&mut app, 80, 24);

    let title = "packages 1";
    let column = list_border(&buf, title);
    let rows = panel_rows_at(&buf, title, column);
    assert!(rows_with_fg(&buf, column, rows, colors.accent).is_empty());
}

#[test]
fn test_execute_highlighted_row_is_text_on_selection() {
    use andre::components::execute::ExecuteComponent;
    use andre::execute::{ExecutionResult, ExecutionState};

    let (mut app, _tmp) = make_test_app();
    let colors = ThemeColors::from_theme(app.ctx.core.theme);
    let (_tx, rx) = tokio::sync::mpsc::channel(1);
    let results = vec![ExecutionResult {
        name: "home/pkg0".into(),
        success: false,
        error_message: Some("boom".into()),
    }];
    let mut exec = ExecuteComponent::new(rx, results.len(), results);
    exec.state = ExecutionState::Completed;
    app.component_stack.push(Box::new(exec));
    app.update_components();
    // Row 0 is the failed package. Its error is in the detail, not the row.
    let buf = render_app(&mut app, 80, 24);
    let y = (0..buf.area().height)
        .find(|&y| row_text(&buf, y).contains("\u{2717} home/pkg0"))
        .expect("failed row is drawn");
    // The name takes the label colour on the cursor row. The failure mark keeps its own
    // colour when that reads on the fill, so only the name is checked here.
    let text_x = column_of(&buf, y, "home/pkg0").expect("name is drawn");
    for x in text_x..text_x + "home/pkg0".len() as u16 {
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
            name: format!("home/pkg{i}"),
            success: true,
            error_message: None,
        })
        .collect();
    let mut exec = ExecuteComponent::new(rx, results.len(), results);
    exec.state = ExecutionState::Completed;
    app.component_stack.push(Box::new(exec));

    let buf = render_app(&mut app, 80, 24);
    let title = " results ";
    let column = list_border(&buf, title);
    let rows = panel_rows_at(&buf, title, column);
    assert!(
        !rows_with_fg(&buf, column, rows, colors.accent).is_empty(),
        "no scrollbar thumb on the results list's panel border"
    );
}

/// The title of the stow status list. It names the packages, so any count matches.
const STATUS_TITLE: &str = " packages ";

#[test]
fn test_status_list_draws_its_scrollbar_thumb_when_rows_overflow() {
    let (mut app, tmp) = make_flat_status_app();
    // Eight more packages make 14 rows, more than the 13 that fit.
    for i in 0..8 {
        write_package(
            &tmp.path().join("dotfiles/home"),
            &format!("extra{i}"),
            ".x_rc",
        );
    }
    push_screen(&mut app, "Status");
    let colors = ThemeColors::from_theme(app.ctx.core.theme);
    let buf = render_updated(&mut app, 80, 24);

    let column = list_border(&buf, STATUS_TITLE);
    let rows = panel_rows_at(&buf, STATUS_TITLE, column);
    assert_eq!(
        rows.len(),
        13,
        "the status list should show 13 rows at 80x24"
    );
    assert!(
        !rows_with_fg(&buf, column, rows, colors.accent).is_empty(),
        "no scrollbar thumb on the status list's right border"
    );
}

/// A status screen filtered to a profile: the chip names it, the list title counts its
/// packages, and `f` opens the dropdown with every package first and the profile under the bar.
#[test]
fn test_status_profile_chip_title_and_dropdown_follow_the_filter() {
    let (mut app, _tmp) = make_flat_status_app();
    app.ctx.core.icons = IconSet::Unicode;
    app.ctx
        .core
        .profiles
        .create(andre_core::profile::Profile {
            name: "work".into(),
            packages: vec!["test".into()],
            ..Default::default()
        })
        .unwrap();
    app.component_stack.push(Box::new(
        andre::components::status::StatusComponent::for_profile("work"),
    ));

    let buf = render_updated(&mut app, 80, 24);
    let chip = format!("profile [work {}]", UNICODE.caret);
    let chip_y = (0..buf.area().height)
        .find(|&y| row_text(&buf, y).contains(&chip))
        .unwrap_or_else(|| panic!("{chip:?} chip missing"));
    assert!(
        buffer_text(&buf).contains(" work 1 "),
        "filtered title missing"
    );

    app.component_dispatch(key_char('f'));
    let buf = render_updated(&mut app, 80, 24);
    let first = row_text(&buf, chip_y + 2);
    let second = row_text(&buf, chip_y + 3);
    assert!(
        first.contains("all packages"),
        "every packages comes first: {first:?}"
    );
    assert!(
        second.contains("work") && second.contains('\u{258c}'),
        "the profile is second, under the bar: {second:?}"
    );
}

/// The filtered status screen with its profile dropdown open draws at the large sizes too.
#[test]
fn test_status_filter_dropdown_renders_at_120x40_and_200x50_without_panic() {
    let (mut app, _tmp) = make_flat_status_app();
    app.ctx
        .core
        .profiles
        .create(andre_core::profile::Profile {
            name: "work".into(),
            packages: vec!["clash".into()],
            ..Default::default()
        })
        .unwrap();
    app.component_stack.push(Box::new(
        andre::components::status::StatusComponent::for_profile("work"),
    ));
    app.component_dispatch(key_char('f'));
    for (width, height) in [(120, 40), (200, 50)] {
        render_updated(&mut app, width, height);
    }
}

/// The chip row names the filter, and the counts start at a fixed column after the chip, so
/// they line up across profiles: 25 columns after `profile`, as in the 80x24 mockup.
#[test]
fn test_status_chip_row_names_all_and_puts_the_counts_at_a_fixed_column() {
    let (mut app, _tmp) = make_flat_status_app();
    app.ctx.core.icons = IconSet::Unicode;
    push_screen(&mut app, "Status");

    let buf = render_updated(&mut app, 80, 24);
    let y = (0..buf.area().height)
        .find(|&y| row_text(&buf, y).contains("profile [all ▾]"))
        .expect("profile [all ▾] chip row");
    let label = column_of(&buf, y, "profile").unwrap();
    let counts = column_of(&buf, y, UNICODE.status(StowStatus::Stowed)).unwrap();
    assert_eq!(counts - label, 25, "{:?}", row_text(&buf, y));
}

/// A chip too long to leave room keeps three spaces before the counts.
#[test]
fn test_status_chip_row_keeps_three_columns_before_the_counts_after_a_long_profile() {
    let (mut app, _tmp) = make_flat_status_app();
    app.ctx.core.icons = IconSet::Unicode;
    app.ctx
        .core
        .profiles
        .create(andre_core::profile::Profile {
            name: "a-very-long-profile-name".into(),
            packages: vec!["test".into()],
            ..Default::default()
        })
        .unwrap();
    app.component_stack.push(Box::new(
        andre::components::status::StatusComponent::for_profile("a-very-long-profile-name"),
    ));

    let buf = render_updated(&mut app, 80, 24);
    let y = (0..buf.area().height)
        .find(|&y| row_text(&buf, y).contains("profile [a-very-long-profile-name"))
        .expect("long chip row");
    let close = column_of(&buf, y, "▾]").unwrap() + 1;
    // Only the unstowed package is listed, so the counts are its glyph.
    let counts = column_of(&buf, y, UNICODE.status(StowStatus::Unstowed))
        .unwrap_or_else(|| panic!("{:?}", row_text(&buf, y)));
    assert_eq!(counts - close - 1, 3, "{:?}", row_text(&buf, y));
}

/// A package whose target is missing is on disk but unread: its row shows `·`, its detail says
/// the status is not read, and the fix is the target's, not a missing package's.
#[test]
fn test_status_package_with_no_target_reads_unknown() {
    let (mut app, tmp) = make_empty_app();
    app.ctx.core.icons = IconSet::Unicode;
    let source = tmp.path().join("dotfiles/gone");
    std::fs::create_dir_all(source.join("pkg")).unwrap();
    std::fs::write(source.join("pkg/.rc"), "x").unwrap();
    app.ctx
        .core
        .config
        .set_packages(Some(source.display().to_string()));
    app.ctx.core.config.target = Some(tmp.path().join("nowhere").display().to_string());
    push_screen(&mut app, "Status");

    let buf = render_updated(&mut app, 80, 24);
    let border = list_border(&buf, STATUS_TITLE);
    let y = (0..buf.area().height)
        .find(|&y| list_text(&buf, y, border).contains("pkg"))
        .expect("pkg row");
    assert!(
        list_glyph_on_row(&buf, y, border, UNICODE.unknown).is_some(),
        "{:?}",
        list_text(&buf, y, border)
    );
    let text = buffer_text(&buf);
    assert!(text.contains("status not read yet"), "{text}");
    assert!(text.contains("Fix: create the target folder"), "{text}");
    assert!(!text.contains("the package is gone"), "{text}");
    assert!(!text.contains("restore the package folder"), "{text}");
}

/// The status rows start with their glyph in the list's first column, after the cursor bar,
/// and the list draws no group header above them.
#[test]
fn test_status_rows_start_with_their_glyph_and_draw_no_group_header() {
    let (mut app, _tmp) = make_flat_status_app();
    app.ctx.core.icons = IconSet::Unicode;
    push_screen(&mut app, "Status");
    let buf = render_updated(&mut app, 80, 24);
    let border = list_border(&buf, STATUS_TITLE);
    let texts: Vec<String> = panel_rows_at(&buf, STATUS_TITLE, border)
        .map(|y| list_text(&buf, y, border))
        .collect();

    let glyphs = [
        UNICODE.stowed,
        UNICODE.unstowed,
        UNICODE.partial,
        UNICODE.conflict,
        UNICODE.missing,
        UNICODE.empty,
        UNICODE.invalid,
        UNICODE.unknown,
    ];
    for name in ["plain", "linked", "clash", "half", "hollow", "test"] {
        let text = texts
            .iter()
            .find(|text| text.contains(name))
            .unwrap_or_else(|| panic!("no {name} row in {texts:?}"));
        let body = text.trim_start_matches(['│', '▌', ' ']);
        assert!(
            glyphs.iter().any(|glyph| body.starts_with(glyph)),
            "{name} row does not start with its glyph: {body:?}"
        );
    }
    assert!(
        texts
            .iter()
            .all(|text| !text.contains("home") && !text.contains(UNICODE.caret)),
        "a group header is drawn: {texts:?}"
    );
}

/// A conflict's detail names the file in the way, as `real file`, and the fix.
#[test]
fn test_status_conflict_detail_names_the_real_file_in_the_way() {
    let (mut app, _tmp) = make_flat_status_app();
    app.ctx.core.icons = IconSet::Unicode;
    push_screen(&mut app, "Status");
    // Packages sort by name, so `clash` is the first row.

    let buf = render_updated(&mut app, 80, 24);
    let text = buffer_text(&buf);
    assert!(text.contains("✗ .clash_rc"), "{text}");
    assert!(text.contains("real file"), "{text}");
    assert!(text.contains("Fix: move the real file"), "{text}");
}

/// A profile with no packages on disk says why in its detail, with no `Press` line, and the
/// list carries the `f` hint under its message.
#[test]
fn test_status_empty_profile_has_an_f_hint_and_no_press_line() {
    let (mut app, _tmp) = make_test_app();
    app.ctx
        .core
        .profiles
        .create(andre_core::profile::Profile {
            name: "bare".into(),
            packages: vec!["nope".into()],
            ..Default::default()
        })
        .unwrap();
    app.component_stack.push(Box::new(
        andre::components::status::StatusComponent::for_profile("bare"),
    ));

    let buf = render_updated(&mut app, 80, 24);
    let text = buffer_text(&buf);
    assert!(text.contains("No packages on disk."), "{text}");
    assert!(text.contains("f show every package"), "{text}");
    assert!(!text.contains("Press"), "{text}");
}

#[test]
fn test_status_list_draws_no_scrollbar_when_every_row_fits() {
    let (mut app, _tmp) = make_empty_app();
    push_screen(&mut app, "Status");
    let colors = ThemeColors::from_theme(app.ctx.core.theme);
    let buf = render_updated(&mut app, 80, 24);

    let column = list_border(&buf, STATUS_TITLE);
    let rows = panel_rows_at(&buf, STATUS_TITLE, column);
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
packages: dotfiles/home
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
    // Rows: `plain`, then `wide`, in name order.
    app.update_components();
    app.component_dispatch(down_key());
    let buf = render_updated(&mut app, 80, 24);

    let y = (0..buf.area().height)
        .find(|&y| row_text(&buf, y).contains(".wide_configuration_file_with"))
        .expect("long file name row missing");
    let border = panel_right_column(&buf, "wide");
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
    // The cursor starts on `plain`, the first package in name order.
    app.update_components();
    let buf = render_updated(&mut app, 80, 24);
    assert!(
        row_containing(&buf, ".plain_rc").is_some(),
        "a short file name is missing from the detail"
    );
}

#[test]
fn test_packages_screen_has_no_legend_and_counts_the_marks() {
    let (mut app, _tmp) = make_flat_packages_app(&["bash-env", "nvim", "tmux"]);
    push_screen(&mut app, "Packages");
    app.update_components();
    app.component_dispatch(key_char('*')); // marks every package
    let buf = render_updated(&mut app, 80, 24);

    assert!(
        !buffer_text(&buf).contains(" legend "),
        "the legend is back"
    );
    assert!(
        row_containing(&buf, "packages 3 of 3").is_some(),
        "the title does not count the marks"
    );
    assert!(
        line_contains(&buf, "Enter stow 3"),
        "the footer does not follow the marks"
    );
}

#[test]
fn test_a_marked_package_shows_the_checked_box_and_the_title_counts_it() {
    let (mut app, _tmp) = make_flat_packages_app(&["bash-env", "nvim", "tmux"]);
    app.ctx.core.icons = IconSet::Unicode;
    push_screen(&mut app, "Packages");
    app.update_components();
    app.component_dispatch(down_key()); // nvim
    app.component_dispatch(space_key());
    let buf = render_updated(&mut app, 80, 24);

    let icons = Icons::for_set(IconSet::Unicode);
    let border = list_border(&buf, "packages 1 of 3");
    let row_with = |name: &str| {
        (0..buf.area().height)
            .map(|y| list_text(&buf, y, border))
            .find(|text| text.contains(name))
            .unwrap_or_else(|| panic!("{name} row missing"))
    };
    let marked = row_with("nvim");
    assert!(
        marked.contains(icons.checkbox_on),
        "{marked:?}: no checked box"
    );
    let unmarked = row_with("tmux");
    assert!(
        unmarked.contains(icons.checkbox_off),
        "{unmarked:?}: no empty box"
    );
}

#[test]
fn test_footer_run_verb_follows_the_action_and_the_marks() {
    let (mut app, _tmp) = make_flat_packages_app(&["bash-env", "nvim", "tmux"]);
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
    let (mut app, tmp) = make_flat_packages_app(&["test"]);
    app.ctx.core.icons = IconSet::Unicode;
    let dir = tmp.path().join("profiles");
    std::fs::create_dir_all(&dir).unwrap();
    for (name, yaml) in &yamls {
        std::fs::write(dir.join(format!("{name}.yml")), yaml).unwrap();
    }
    app.ctx.core.profiles = andre_core::profile::ProfileSet::load(&dir).unwrap();
    push_screen(&mut app, "Packages");
    app.update_components();
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
fn test_packages_without_a_folder_say_so_and_name_settings_at_80x24() {
    let (mut app, _tmp) = make_empty_app();
    push_screen(&mut app, "Packages");
    let buf = render_updated(&mut app, 80, 24);
    assert!(
        line_contains(&buf, "no packages folder"),
        "empty message missing"
    );
    assert!(line_contains(&buf, "6 settings"), "settings hint missing");
    assert!(
        line_contains(&buf, "sets no `packages:`"),
        "detail does not name the missing key"
    );
}

#[test]
fn test_packages_with_an_empty_folder_name_it_and_offer_new_package_at_80x24() {
    let (mut app, _tmp) = make_flat_packages_app(&[]);
    push_screen(&mut app, "Packages");
    let buf = render_updated(&mut app, 80, 24);
    assert!(
        line_contains(&buf, "no packages in ~/dotfiles"),
        "empty message missing"
    );
    assert!(
        line_contains(&buf, "3 new package"),
        "new package hint missing"
    );
    assert!(
        row_containing(&buf, "packages 0").is_some(),
        "title does not count zero"
    );
}

#[test]
fn test_package_rows_read_mark_status_then_name_and_draw_no_caret_glyphs() {
    let (mut app, _tmp) = make_flat_packages_app(&["nvim", "tmux"]);
    app.ctx.core.icons = IconSet::Unicode;
    push_screen(&mut app, "Packages");
    app.update_components();
    let buf = render_updated(&mut app, 80, 24);

    let icons = Icons::for_set(IconSet::Unicode);
    let border = list_border(&buf, "packages 2");
    let row = (0..buf.area().height)
        .map(|y| list_text(&buf, y, border))
        .find(|text| text.contains("nvim"))
        .expect("nvim row missing");
    let box_at = row
        .find(icons.checkbox_off)
        .expect("no checkbox on the row");
    assert!(
        box_at < row.find("nvim").unwrap(),
        "{row:?}: the name comes first"
    );
    let text = buffer_text(&buf);
    assert!(!text.contains(icons.caret), "a fold glyph is drawn");
}

/// The cursor's row reads bar, checked box, status glyph, then name: `▌■ ◐ shell`.
#[test]
fn test_marked_partial_package_under_the_cursor_reads_bar_box_status_then_name() {
    let (mut app, tmp) = make_flat_packages_app(&["shell"]);
    app.ctx.core.icons = IconSet::Unicode;
    // `shell` has two files and one of them is linked into the home: partly stowed.
    let shell = tmp.path().join("dotfiles/shell");
    std::fs::write(shell.join(".zshrc"), "x").unwrap();
    std::os::unix::fs::symlink(shell.join(".zshrc"), tmp.path().join(".zshrc")).unwrap();
    push_screen(&mut app, "Packages");
    app.update_components();
    app.component_dispatch(space_key());
    let buf = render_updated(&mut app, 80, 24);

    let border = list_border(&buf, "packages 1 of 1");
    let row = (0..buf.area().height)
        .map(|y| list_text(&buf, y, border))
        .find(|text| text.contains("shell"))
        .expect("shell row missing");
    let content = row.trim_start_matches(['│', ' ']);
    assert!(
        content.starts_with("▌■ ◐ shell"),
        "{row:?}: the row does not read bar, box, status, name"
    );
}

#[test]
fn test_menu_packages_facts_name_the_folder_and_the_target_and_no_group() {
    let (mut app, _tmp) = make_flat_test_app();
    menu_mut(&mut app).list.cursor = 1;
    let buf = render_updated(&mut app, 80, 24);
    assert!(
        row_containing(&buf, "folder")
            .expect("folder fact missing")
            .contains("~/dotfiles"),
        "folder is not the packages folder"
    );
    assert!(
        row_containing(&buf, "links to")
            .expect("links to fact missing")
            .contains('~'),
        "links to is not the target"
    );
    assert!(
        !buffer_text(&buf).contains("group"),
        "the menu still says group"
    );
}

/// Every menu item's row and detail, one cursor position at a time, avoid the word group.
#[test]
fn test_no_menu_item_says_group_at_any_cursor() {
    let (mut app, _tmp) = make_flat_test_app();
    for item in 0..menu_mut(&mut app).item_count() {
        menu_mut(&mut app).list.cursor = item;
        let buf = render_updated(&mut app, 80, 24);
        assert!(
            !buffer_text(&buf).contains("group"),
            "menu item {item} still says group"
        );
    }
}

#[test]
fn test_packages_screens_render_at_120x40_and_200x50_without_panic() {
    for (width, height) in [(120, 40), (200, 50)] {
        let (mut app, _tmp) = make_flat_test_app();
        push_screen(&mut app, "Packages");
        app.update_components();
        render_updated(&mut app, width, height);
        app.component_dispatch(key_char('*'));
        render_updated(&mut app, width, height);
        app.component_dispatch(key_char('p'));
        render_updated(&mut app, width, height);

        let (mut app, _tmp) = make_empty_app();
        push_screen(&mut app, "Packages");
        render_updated(&mut app, width, height);

        let (mut app, _tmp) = make_flat_packages_app(&[]);
        push_screen(&mut app, "Packages");
        render_updated(&mut app, width, height);

        let (mut app, _tmp) = make_flat_test_app();
        render_updated(&mut app, width, height);
    }
}

#[test]
fn test_help_packages_topic_describes_marks_and_the_packages_folder() {
    let (mut app, _tmp) = make_flat_test_app();
    push_screen(&mut app, "Help");
    for _ in 0..4 {
        app.component_dispatch(down_key());
    }
    let buf = render_updated(&mut app, 80, 24);
    assert!(
        line_contains(&buf, "packages"),
        "the packages topic is not open"
    );
    assert!(
        line_contains(&buf, "* marks them"),
        "mark-all is not described"
    );
    assert!(
        !buffer_text(&buf).contains("group"),
        "the packages topic still says group"
    );
}

#[test]
fn test_p_profiles_shows_on_a_package_row_only() {
    let (mut app, _tmp) = make_flat_packages_app(&[]);
    push_screen(&mut app, "Packages");
    let empty = render_updated(&mut app, 120, 24);
    assert!(
        !line_contains(&empty, "p profiles"),
        "an empty folder offers p"
    );

    let (mut app, _tmp) = make_flat_packages_app(&["test"]);
    push_screen(&mut app, "Packages");
    let on_package = render_updated(&mut app, 120, 24);
    assert!(
        line_contains(&on_package, "p profiles"),
        "package row lacks p"
    );
}

/// A package whose target does not exist cannot be read, so it shows the unknown dot rather
/// than a status, and nothing is drawn in its place.
#[test]
fn test_package_whose_target_is_missing_shows_the_unknown_dot() {
    let tmp = tempfile::TempDir::new().unwrap();
    let path = tmp.path().join("andre.yml");
    std::fs::write(
        &path,
        "global: {}\npackages: dotfiles/lost\ntarget: \"missing-target\"\n",
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
    let (mut app, _tmp) = make_flat_test_app();
    push_screen(&mut app, "Packages");
    app.update_components();
    app.component_dispatch(key_char('p'));
    let buf = render_updated(&mut app, 80, 24);

    assert!(
        line_contains(&buf, "test in profiles"),
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
        let (mut app, _tmp) = make_flat_test_app();
        push_screen(&mut app, "Packages");
        app.update_components();
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

/// An app whose config folder holds `profiles`, as `(name, yaml)` pairs. The flat packages
/// folder holds `shell` and `test`.
fn make_profiles_app(profiles: &[(&str, &str)]) -> (App, tempfile::TempDir) {
    let (mut app, tmp) = make_flat_packages_app(&["shell", "test"]);
    app.ctx.core.icons = IconSet::Unicode;
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
        ("base", "packages: [shell]\n"),
        (
            "agent",
            "description: Work servers that run agents\nincludes: [base]\npackages: [test]\n",
        ),
    ]);
    open_editor_render(&mut app, "agent");
    let buf = render_updated(&mut app, 80, 24);
    let border = list_border(&buf, "packages 2 of 2");

    assert!(
        line_contains(&buf, "packages 2 of 2"),
        "list title counts marks"
    );
    // Each row is a checkbox, a status glyph, then the name, with no group row above them.
    let body = |name: &str| -> String {
        (0..buf.area().height)
            .map(|y| list_text(&buf, y, border))
            .find(|text| text.contains(name))
            .unwrap_or_else(|| panic!("no {name} row"))
            .trim_start_matches(['│', '▌', ' '])
            .trim_end()
            .to_string()
    };
    assert_eq!(body("test"), "■ → test", "own package");
    let shell = body("shell");
    assert!(
        shell.starts_with("■ → shell") && shell.ends_with("via base"),
        "inherited package: {shell:?}"
    );
    let text = buffer_text(&buf);
    assert!(
        !text.contains('▾') && !text.contains('▸'),
        "a fold header is drawn"
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
        footer_row(&buf).contains("Tab pane  Space toggle  * all  w save  Esc back"),
        "{}",
        footer_row(&buf)
    );
}

#[test]
fn test_profile_edit_title_says_unsaved_after_an_edit() {
    let (mut app, _tmp) = make_profiles_app(&[("agent", "packages: [test]\n")]);
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
    let (mut app, _tmp) = make_profiles_app(&[("agent", "packages: [test]\n")]);
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
    let (mut app, _tmp) = make_profiles_app(&[("agent", "packages: [test]\n")]);
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
    let (mut app, _tmp) = make_profiles_app(&[("agent", "packages: [test]\n")]);
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
        ("base", "packages: [shell]\n"),
        ("agent", "includes: [base]\npackages: [test]\n"),
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
        assert!(
            row_text(&buf, 1).contains("main › profiles › status"),
            "{width}x{height}: {:?}",
            row_text(&buf, 1)
        );
    }
}

#[test]
fn test_profile_edit_form_scrolls_to_keep_the_cursor_row_in_view() {
    let names: Vec<String> = (1..=12).map(|n| format!("p{n:02}")).collect();
    let mut profiles: Vec<(&str, &str)> = vec![("agent", "packages: [test]\n")];
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
    let (mut app, _tmp) = make_profiles_app(&[("agent", "includes: [ghost]\npackages: [test]\n")]);
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
        ("base", "packages: [shell]\n"),
        ("agent", "includes: [base]\npackages: [test]\n"),
    ]);
    open_editor_render(&mut app, "agent");
    // Rows: shell (inherited from base), then test. Unmark test.
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
            "description: Shared by every machine\npackages: [test]\n",
        ),
        (
            "agent-work",
            "description: Work servers that run agents\nincludes: [base]\npackages: [shell]\n",
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
    let (mut app, _tmp) = make_profiles_app(&[("base", "packages: [test]\n")]);
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
    let (mut app, _tmp) = make_profiles_app(&[("base", "packages: [test]\n")]);
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
    let (mut app, _tmp) = make_profiles_app(&[("agent", "includes: [ghost]\npackages: [test]\n")]);
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
    let (mut app, _tmp) = make_profiles_app(&[("base", "packages: [test]\n")]);
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
        ("base", "packages: [test]\n"),
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
    let (mut app, _tmp) = make_profiles_app(&[("agent", "includes: [ghost]\npackages: [test]\n")]);
    open_profiles_render(&mut app);
    let buf = render_updated(&mut app, 80, 24);
    assert!(
        line_contains(&buf, "includes unknown"),
        "the resolution error is the headline"
    );
}

#[test]
fn test_profiles_render_at_120x40_and_200x50_with_a_dialog_open() {
    let (mut app, _tmp) = make_profiles_app(&[("base", "packages: [test]\n")]);
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
        line_contains(&buf, "main › packages › home/test › ignore"),
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
    let (mut app, _tmp) = make_flat_packages_app(&[]);
    app.component_dispatch(key_char('2'));
    app.update_components();
    let empty = render_updated(&mut app, 80, 24);
    assert!(!footer_row(&empty).contains("i ignore"));

    let (mut app, _tmp) = make_flat_packages_app(&["test"]);
    app.component_dispatch(key_char('2'));
    app.update_components();
    let on_package = render_updated(&mut app, 80, 24);
    let footer = footer_row(&on_package);
    assert!(footer.contains("i ignore"), "{footer}");
    assert!(footer.contains("* all"), "{footer}");
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

// --- New-package wizard (steps 1-2 and the live-file picker) ---

/// The wizard on step 1 over a flat app whose packages folder holds `git` and `nvim`, with
/// unicode icons so the mockup glyphs apply. HOME is the temp dir, so the target is the temp dir.
fn wizard_app() -> (App, tempfile::TempDir) {
    let (mut app, tmp) = make_flat_packages_app(&["git", "nvim"]);
    app.ctx.core.icons = IconSet::Unicode;
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

#[test]
fn test_new_package_name_step_is_first_and_lists_the_packages_folder() {
    let (mut app, _tmp) = wizard_app();
    let buf = render_updated(&mut app, 80, 24);

    assert!(line_contains(
        &buf,
        "[1 name] \u{203a} 2 source \u{203a} 3 ignore \u{203a} 4 profiles \u{203a} 5 review"
    ));
    assert!(line_contains(&buf, "\u{256d} name "));
    assert!(line_contains(&buf, "matches 2"));
    assert!(line_contains(&buf, "git"));
    assert!(
        !buffer_text(&buf).contains("group"),
        "the step still says group"
    );
    assert!(footer_row(&buf).contains("Esc cancel"));
}

#[test]
fn test_new_package_without_a_packages_folder_says_so_and_enter_does_nothing() {
    let (mut app, _tmp) = make_empty_app();
    app.component_stack.push(Box::new(
        andre::components::wizard::NewPackageWizard::default(),
    ));
    let buf = render_updated(&mut app, 80, 24);
    assert!(line_contains(&buf, "set a packages folder in settings"));

    press(&mut app, &[key_char('t'), enter_key()]);
    let buf = render_updated(&mut app, 80, 24);
    assert!(line_contains(&buf, "[1 name]"), "the step is kept");
    assert!(!line_contains(&buf, "[2 source]"), "Enter did not go on");
}

#[test]
fn test_new_package_name_step_filters_packages_and_leads_with_the_create_row() {
    let (mut app, _tmp) = wizard_app();
    render_updated(&mut app, 80, 24);
    press(&mut app, &[key_char('n'), key_char('v')]);
    let buf = render_updated(&mut app, 80, 24);

    assert!(line_contains(&buf, "[1 name] \u{203a} 2 source"));
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
    press(&mut app, &[key_char('g'), key_char('i'), key_char('t')]);
    let buf = render_updated(&mut app, 80, 24);

    assert!(line_contains(&buf, "\u{2192} add files to `git`"));
}

#[test]
fn test_new_package_source_step_shows_the_radio_on_the_cursor_row() {
    let (mut app, _tmp) = wizard_app();
    render_updated(&mut app, 80, 24);
    press(&mut app, &[key_char('t'), key_char('m'), enter_key()]);
    let buf = render_updated(&mut app, 80, 24);

    assert!(line_contains(&buf, "\u{2713} name \u{203a} [2 source]"));
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
fn test_new_package_file_step_title_row_adds_files_after_new_package() {
    let (mut app, _tmp) = wizard_app();
    render_updated(&mut app, 80, 24);
    press(&mut app, &[key_char('t'), key_char('m'), enter_key()]);
    let buf = render_updated(&mut app, 80, 24);
    assert!(
        row_text(&buf, 1).contains("andre  main › new package"),
        "source step title row: {:?}",
        row_text(&buf, 1)
    );

    press(&mut app, &[down_key(), enter_key()]);
    let buf = render_updated(&mut app, 80, 24);
    assert!(
        row_text(&buf, 1).contains("andre  main › new package › files"),
        "file picker title row: {:?}",
        row_text(&buf, 1)
    );
}

#[test]
fn test_new_package_file_picker_lists_files_and_marks_them() {
    let (mut app, _tmp) = wizard_app();
    std::fs::write(app.ctx.home_dir.join("notes.txt"), "").unwrap();
    render_updated(&mut app, 80, 24);
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

    assert!(line_contains(&buf, "\u{2713} name \u{203a} [2 source]"));
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
        press(&mut app, &[key_char('t'), key_char('m')]);
        render_updated(&mut app, width, height);
        press(&mut app, &[enter_key()]);
        render_updated(&mut app, width, height);
        press(&mut app, &[down_key(), enter_key()]);
        let buf = render_updated(&mut app, width, height);
        assert!(line_contains(&buf, "notes.txt"));
    }
}

// --- New-package wizard, steps 3-5 (ignore files, profiles, review) ---

/// The wizard over the flat test app with one profile, `personal`, holding `git`.
fn wizard_app_with_profile() -> (App, tempfile::TempDir) {
    let (mut app, tmp) = wizard_app();
    let dir = app.ctx.config_dir.join("profiles");
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("personal.yml"), "packages:\n  - git\n").unwrap();
    app.ctx.core.profiles = andre_core::profile::ProfileSet::load(&dir).unwrap();
    (app, tmp)
}

/// The name `tmux`, then an empty source: the wizard on step 3.
fn wizard_on_ignore_step(app: &mut App) {
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
        "\u{2713} source \u{203a} [3 ignore] \u{203a} 4 profiles"
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

    assert!(line_contains(&buf, "[4 profiles]"));
    assert!(line_contains(&buf, "add to profiles"));
    assert!(line_contains(&buf, "personal"));
    assert!(
        line_contains(&buf, "2 (+ tmux)"),
        "the detail shows the count once the package joins"
    );
    assert!(footer_row(&buf).contains("Enter next"));
}

#[test]
fn test_new_package_profile_detail_counts_the_packages_a_profile_reaches_through_includes() {
    let (mut app, _tmp) = wizard_app_with_profile();
    let dir = app.ctx.config_dir.join("profiles");
    // `laptop` lists no package of its own: it reaches `git` only through `personal`.
    std::fs::write(dir.join("laptop.yml"), "includes:\n  - personal\n").unwrap();
    app.ctx.core.profiles = andre_core::profile::ProfileSet::load(&dir).unwrap();
    wizard_on_ignore_step(&mut app);
    // `laptop` sorts first, so Space on the cursor's row chooses it.
    press(&mut app, &[enter_key(), key_char(' ')]);
    let buf = render_updated(&mut app, 80, 24);

    assert!(
        line_contains(&buf, "2 (+ tmux)"),
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
        line_contains(&buf, "\u{2502} create tmux/.gitignore"),
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

    assert!(line_contains(&buf, "[5 review]"));
    assert!(line_contains(&buf, "review \u{b7} new package tmux"));
    assert!(line_contains(&buf, "create tmux/.gitignore"));
    assert!(line_contains(&buf, "edit   profiles/personal.yml  + tmux"));
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

    assert!(tmp.path().join("dotfiles/tmux/.gitignore").is_file());
    assert!(line_contains(&buf, "created tmux"));
    assert!(footer_row(&buf).contains("Enter done"));

    press(&mut app, &[enter_key()]);
    assert_eq!(
        app.component_stack.len(),
        1,
        "Enter after the result leaves the wizard"
    );
}

#[test]
fn test_new_package_steps_3_to_5_render_at_120x40_and_200x50_without_panic() {
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
        "move   ~/notes.txt \u{2192} tmux/notes.txt"
    ));
    assert!(line_contains(&buf, "link   ~/notes.txt \u{21e2} package"));
    assert!(line_contains(
        &buf,
        "1 move \u{b7} 1 link \u{b7} 1 file \u{b7} 0 profiles"
    ));
}

#[test]
fn test_new_package_esc_with_a_name_typed_draws_the_leave_dialog_at_80x24() {
    let (mut app, _tmp) = wizard_app();
    render_updated(&mut app, 80, 24);
    press(&mut app, &typed_keys("nv"));
    press(&mut app, &[esc_key()]);
    let buf = render_updated(&mut app, 80, 24);

    assert!(line_contains(&buf, "unsaved changes"));
    assert!(line_contains(
        &buf,
        "Leave the new package `nv`? Nothing is written yet."
    ));
    assert!(line_contains(&buf, "[ Esc stay ]"));
    assert!(line_contains(&buf, "[ Enter leave ]"));
    assert!(footer_row(&buf).contains("Enter leave"));
    assert!(
        line_contains(&buf, "[1 name]"),
        "the wizard is still on the name step"
    );
}

#[test]
fn test_new_package_name_detail_says_what_is_created_and_where_it_links_at_80x24() {
    let (mut app, _tmp) = wizard_app();
    render_updated(&mut app, 80, 24);
    press(&mut app, &typed_keys("nv"));
    let buf = render_updated(&mut app, 80, 24);

    assert!(line_contains(&buf, "Creates nv/ in the packages folder,"));
    assert!(line_contains(&buf, "an empty package."));
    let path = row_containing(&buf, "path").expect("the path fact");
    assert!(
        path.split("path")
            .nth(1)
            .unwrap()
            .trim_start()
            .starts_with("~/dotfiles/nv"),
        "path row: {path:?}"
    );
    let links = row_containing(&buf, "links to").expect("the links fact");
    assert!(
        links
            .split("links to")
            .nth(1)
            .unwrap()
            .trim_start()
            .starts_with('~'),
        "links to row: {links:?}"
    );
}

#[test]
fn test_new_package_nested_file_moves_into_its_folder_in_the_package_at_80x24() {
    let (mut app, _tmp) = make_flat_packages_app(&["git"]);
    app.ctx.core.icons = IconSet::Unicode;
    app.component_stack.push(Box::new(
        andre::components::wizard::NewPackageWizard::default(),
    ));
    let nvim = app.ctx.home_dir.join(".config/nvim");
    std::fs::create_dir_all(&nvim).unwrap();
    std::fs::write(nvim.join("init.lua"), "-- config").unwrap();
    render_updated(&mut app, 80, 24);
    press(&mut app, &typed_keys("nvim"));
    // Live files: the picker opens at home, where `.config` is the first entry. Each key is
    // followed by an update, as the event loop does: the picker lists its folder on update.
    press(&mut app, &[enter_key(), down_key(), enter_key()]);
    render_updated(&mut app, 80, 24);
    let right = KeyEvent::new(KeyCode::Right, KeyModifiers::NONE);
    press(&mut app, &[right]);
    render_updated(&mut app, 80, 24);
    press(&mut app, &[right]);
    let buf = render_updated(&mut app, 80, 24);
    assert!(
        line_contains(&buf, "moves to  nvim/.config/nvim/"),
        "the cursor's file lands in the package's folder"
    );

    press(
        &mut app,
        &[space_key(), enter_key(), enter_key(), enter_key()],
    );
    let buf = render_updated(&mut app, 80, 24);
    assert!(line_contains(&buf, "review \u{b7} new package nvim"));
    assert!(line_contains(
        &buf,
        "move   ~/.config/nvim/init.lua \u{2192} nvim/.config/nvim/init.lua"
    ));
}

#[test]
fn test_help_new_package_topic_says_name_and_the_packages_folder_not_group() {
    let (mut app, _tmp) = make_flat_test_app();
    push_screen(&mut app, "Help");
    // The eighth topics, in order: keys, mouse, status glyphs, profiles, packages, new package.
    for _ in 0..5 {
        app.component_dispatch(down_key());
    }
    let buf = render_updated(&mut app, 80, 24);
    assert!(
        line_contains(&buf, "Type a name"),
        "the new package topic is not open"
    );
    assert!(
        !buffer_text(&buf).contains("group"),
        "the new package topic still says group"
    );
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
    assert!(line_contains(&buf, "stow 1 package"), "the review stows");
    assert!(
        line_contains(&buf, "native: config/andre"),
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

/// The review, the run and the unlink screens: the action bar names what the key does, and
/// no screen tells the user to press a key in its text.
#[test]
fn test_review_names_its_action_in_the_action_bar() {
    let (mut app, _tmp) = make_test_app();
    let selected = vec!["home/test".to_string()];
    app.component_stack
        .push(Box::new(andre::components::confirm::ConfirmComponent::new(
            selected, &app.ctx,
        )));
    let buf = render_updated(&mut app, 80, 24);
    assert!(line_contains(&buf, "[ n cancel ]"), "cancel button missing");
    assert!(line_contains(&buf, "[ y stow 1 ]"), "run button missing");
    assert!(
        line_contains(&buf, "stow 1 package"),
        "review title missing"
    );
    assert!(
        !line_contains(&buf, "Press 'y'"),
        "the review must not tell the user to press a key"
    );
}

/// The review's action bar sits on the last content row, right-aligned: one blank column
/// stands between the buttons and the frame, as in the mockup.
#[test]
fn test_review_action_bar_sits_on_the_last_content_row_at_80x24() {
    let (mut app, _tmp) = make_test_app();
    let selected = vec!["home/test".to_string()];
    app.component_stack
        .push(Box::new(andre::components::confirm::ConfirmComponent::new(
            selected, &app.ctx,
        )));
    let buf = render_updated(&mut app, 80, 24);
    assert!(
        row_text(&buf, 20).ends_with("[ n cancel ]  [ y stow 1 ] \u{2502}"),
        "{:?}",
        row_text(&buf, 20)
    );
    assert!(
        row_text(&buf, 21).starts_with("\u{2502}\u{2500}"),
        "footer rule on row 21"
    );
}

/// The review is a flat list: one row per package, no group header, no fold glyph and no
/// checkbox column. The status glyph sits in column one, after the cursor gutter.
#[test]
fn test_review_is_a_flat_list_at_80x24() {
    let (mut app, _tmp) = make_flat_test_app();
    app.ctx.core.icons = IconSet::Unicode;
    app.component_stack
        .push(Box::new(andre::components::confirm::ConfirmComponent::new(
            vec!["test".to_string()],
            &app.ctx,
        )));
    let buf = render_updated(&mut app, 80, 24);
    assert!(line_contains(&buf, "stow 1 package"), "review title");
    assert!(
        line_contains(&buf, "\u{2192} test"),
        "the package row, its status glyph right after the gutter"
    );
    assert!(
        !line_contains(&buf, "\u{25be}"),
        "no fold glyph on a flat list"
    );
}

/// Packages list in name order, whatever order the selection gives them in.
#[test]
fn test_review_lists_packages_in_name_order() {
    let (mut app, tmp) = make_flat_test_app();
    std::fs::create_dir_all(tmp.path().join("dotfiles/shell")).unwrap();
    app.component_stack
        .push(Box::new(andre::components::confirm::ConfirmComponent::new(
            vec!["test".to_string(), "shell".to_string()],
            &app.ctx,
        )));
    let buf = render_updated(&mut app, 80, 24);
    let row_of = |name: &str| (0..buf.area().height).find(|&y| row_text(&buf, y).contains(name));
    let (shell, test) = (row_of("shell").unwrap(), row_of("test").unwrap());
    assert!(
        shell < test,
        "shell (row {shell}) lists before test (row {test})"
    );
    assert!(line_contains(&buf, "stow 2 packages"), "review title");
}

/// The detail names the stow command on a flat fixture: the packages folder and the target.
/// At 80x24 the detail pane is 39 columns, so the command must fit on one row there.
#[test]
fn test_review_detail_shows_the_stow_command_on_a_flat_fixture() {
    let (mut app, _tmp) = make_flat_test_app();
    app.ctx.core.config.global.engine = Some("stow".into());
    app.component_stack
        .push(Box::new(andre::components::confirm::ConfirmComponent::new(
            vec!["test".to_string()],
            &app.ctx,
        )));
    let buf = render_updated(&mut app, 80, 24);
    assert!(
        line_contains(&buf, "$ stow -d ~/dotfiles -t ~"),
        "the detail shows the command the run uses"
    );
}

/// Set-up on a flat config: `0` reviews the package holding the config folder. Its row shows
/// a known status, and the detail resolves the package to a native link.
#[test]
fn test_flat_set_up_review_resolves_its_package_at_80x24() {
    let (mut app, _tmp) = make_flat_bootstrap_app();
    app.ctx.core.icons = IconSet::Unicode;
    render_updated(&mut app, 80, 24);
    app.component_dispatch(key_char('0'));
    let buf = render_updated(&mut app, 80, 24);
    assert!(line_contains(&buf, "stow 1 package"), "the review stows");
    assert!(
        line_contains(&buf, "\u{2192} andre"),
        "the package row shows its status, not `·`"
    );
    assert!(
        line_contains(&buf, "native: andre  (relative symlink)"),
        "the detail resolves the package"
    );
    assert!(
        !line_contains(&buf, "path unresolved"),
        "the package resolves to its target"
    );
}

#[test]
fn test_flat_review_renders_at_120x40_and_200x50_without_panic() {
    for (width, height) in [(120, 40), (200, 50)] {
        let (mut app, _tmp) = make_flat_test_app();
        app.component_stack
            .push(Box::new(andre::components::confirm::ConfirmComponent::new(
                vec!["test".to_string()],
                &app.ctx,
            )));
        let buf = render_updated(&mut app, width, height);
        assert!(line_contains(&buf, "stow 1 package"), "{width}x{height}");
    }
}

/// A failure's message keeps each of its lines: `All operations aborted.` starts its own row
/// in the detail, rather than running on from the line before.
#[test]
fn test_run_failure_keeps_each_line_of_the_engine_message() {
    use andre::components::execute::ExecuteComponent;
    use andre::execute::{ExecutionResult, ExecutionState};

    let (mut app, _tmp) = make_test_app();
    let (_tx, rx) = tokio::sync::mpsc::channel(1);
    let message = "WARNING! stowing shell would cause conflicts:\n  * existing target is neither a link nor a directory: .bashrc\nAll operations aborted.";
    let mut exec = ExecuteComponent::new(
        rx,
        1,
        vec![ExecutionResult {
            name: "home/shell".into(),
            success: false,
            error_message: Some(message.into()),
        }],
    );
    exec.state = ExecutionState::Completed;
    app.component_stack.push(Box::new(exec));

    let buf = render_updated(&mut app, 80, 24);
    assert!(
        line_contains(&buf, "\u{2502} All operations aborted."),
        "the last line starts its own detail row"
    );
    assert!(!line_contains(&buf, "bashrcAll"), "the lines are glued");
}

/// A long failure scrolls with the page keys: the first rows show at the top, the last row
/// at the bottom, and PageUp goes back.
#[test]
fn test_run_detail_pages_through_a_long_failure() {
    use andre::components::execute::ExecuteComponent;
    use andre::execute::{ExecutionResult, ExecutionState};

    let (mut app, _tmp) = make_test_app();
    let (_tx, rx) = tokio::sync::mpsc::channel(1);
    let message = (0..30)
        .map(|i| format!("  * item{i:02}"))
        .collect::<Vec<_>>()
        .join("\n");
    let mut exec = ExecuteComponent::new(
        rx,
        1,
        vec![ExecutionResult {
            name: "home/shell".into(),
            success: false,
            error_message: Some(message),
        }],
    );
    exec.state = ExecutionState::Completed;
    app.component_stack.push(Box::new(exec));

    let buf = render_updated(&mut app, 80, 24);
    assert!(
        line_contains(&buf, "item00"),
        "the top of the message shows"
    );
    assert!(!line_contains(&buf, "item29"), "the bottom does not, yet");

    let page_down = KeyEvent::new(KeyCode::PageDown, KeyModifiers::NONE);
    for _ in 0..5 {
        app.component_dispatch(page_down);
        render_updated(&mut app, 80, 24);
    }
    let buf = render_updated(&mut app, 80, 24);
    assert!(line_contains(&buf, "item29"), "the last line is reachable");
    assert!(!line_contains(&buf, "item00"), "the top has scrolled away");

    let page_up = KeyEvent::new(KeyCode::PageUp, KeyModifiers::NONE);
    app.component_dispatch(page_up);
    let buf = render_updated(&mut app, 80, 24);
    assert!(!line_contains(&buf, "item29"), "PageUp goes back up");
}

/// The run's footer says it is running until every package has finished, then offers Enter.
#[test]
fn test_run_footer_says_running_until_it_finishes() {
    use andre::components::execute::ExecuteComponent;
    use andre::execute::{ExecutionResult, ExecutionState};

    let (mut app, _tmp) = make_test_app();
    let (_tx, rx) = tokio::sync::mpsc::channel(1);
    let mut exec =
        ExecuteComponent::new(rx, 2, Vec::new()).for_run("stow", vec!["home/shell".into()]);
    exec.state = ExecutionState::Running;
    app.component_stack.push(Box::new(exec));
    let buf = render_updated(&mut app, 80, 24);
    assert!(
        footer_row(&buf).contains("\u{2191}\u{2193} inspect  running\u{2026}"),
        "{:?}",
        footer_row(&buf)
    );

    let (mut app, _tmp) = make_test_app();
    let (_tx, rx) = tokio::sync::mpsc::channel(1);
    let results = vec![ExecutionResult {
        name: "home/shell".into(),
        success: true,
        error_message: None,
    }];
    let mut exec = ExecuteComponent::new(rx, 1, results);
    exec.state = ExecutionState::Completed;
    app.component_stack.push(Box::new(exec));
    let buf = render_updated(&mut app, 80, 24);
    assert!(
        footer_row(&buf).contains("Enter done  \u{2191}\u{2193} inspect"),
        "{:?}",
        footer_row(&buf)
    );
    assert!(
        !line_contains(&buf, "running\u{2026}"),
        "no running note once done"
    );
}

/// The run's `[ Enter done ]` sits on the last content row, like the review's buttons.
#[test]
fn test_run_done_button_sits_on_the_last_content_row_at_80x24() {
    use andre::components::execute::ExecuteComponent;
    use andre::execute::{ExecutionResult, ExecutionState};

    let (mut app, _tmp) = make_test_app();
    let (_tx, rx) = tokio::sync::mpsc::channel(1);
    let mut exec = ExecuteComponent::new(
        rx,
        1,
        vec![ExecutionResult {
            name: "home/shell".into(),
            success: true,
            error_message: None,
        }],
    );
    exec.state = ExecutionState::Completed;
    app.component_stack.push(Box::new(exec));
    let buf = render_updated(&mut app, 80, 24);
    assert!(
        row_text(&buf, 20).ends_with("[ Enter done ] \u{2502}"),
        "{:?}",
        row_text(&buf, 20)
    );
}

#[test]
fn test_run_detail_offers_a_fix_for_a_real_file_in_the_way() {
    use andre::components::execute::ExecuteComponent;
    use andre::execute::{ExecutionResult, ExecutionState};

    let (mut app, _tmp) = make_test_app();
    let (_tx, rx) = tokio::sync::mpsc::channel(1);
    let results = vec![ExecutionResult {
        name: "home/shell".into(),
        success: false,
        error_message: Some("target already exists and is not a symlink: /home/u/.bashrc".into()),
    }];
    let mut exec = ExecuteComponent::new(rx, 1, results);
    exec.state = ExecutionState::Completed;
    app.component_stack.push(Box::new(exec));

    let buf = render_updated(&mut app, 80, 24);
    assert!(
        line_contains(&buf, "\u{2717} failed"),
        "failure headline missing"
    );
    assert!(
        line_contains(&buf, "Move it into the package"),
        "fix hint for a real file in the way missing"
    );
    assert!(line_contains(&buf, "[ Enter done ]"), "done button missing");
}

#[test]
fn test_unlink_review_removes_then_says_so() {
    use andre::components::unstow::UnstowConfirmComponent;

    let (mut app, tmp) = make_test_app();
    let source = tmp.path().join("source.txt");
    std::fs::write(&source, "").unwrap();
    let link = tmp.path().join("link.txt");
    std::os::unix::fs::symlink(&source, &link).unwrap();
    app.component_stack
        .push(Box::new(UnstowConfirmComponent::with_paths(
            vec![link.clone()],
            None,
        )));

    let buf = render_updated(&mut app, 80, 24);
    assert!(line_contains(&buf, "remove 1 link"), "list title missing");
    assert!(line_contains(&buf, "points to"), "link target missing");
    assert!(
        line_contains(&buf, "[ y remove 1 ]"),
        "remove button missing"
    );
    assert!(
        !line_contains(&buf, "Press y to remove"),
        "the unlink review must not tell the user to press a key"
    );

    app.component_dispatch(key_char('y'));
    let buf = render_updated(&mut app, 80, 24);
    assert!(!link.is_symlink(), "the link is removed");
    assert!(
        line_contains(&buf, "\u{2713} removed"),
        "removed headline missing"
    );
    assert!(line_contains(&buf, "[ Enter done ]"), "done button missing");
}

/// The screens this batch touches render at the two larger sizes without panicking.
#[test]
fn test_review_run_and_unlink_render_at_120x40_and_200x50() {
    use andre::components::execute::ExecuteComponent;
    use andre::components::unstow::UnstowConfirmComponent;
    use andre::execute::{ExecutionResult, ExecutionState};

    for (width, height) in [(120, 40), (200, 50)] {
        let (mut app, tmp) = make_test_app();
        let selected = vec!["home/test".to_string()];
        app.component_stack
            .push(Box::new(andre::components::confirm::ConfirmComponent::new(
                selected, &app.ctx,
            )));
        render_updated(&mut app, width, height);
        app.component_dispatch(down_key());
        render_updated(&mut app, width, height);

        let (_tx, rx) = tokio::sync::mpsc::channel(1);
        let mut exec = ExecuteComponent::new(
            rx,
            2,
            vec![ExecutionResult {
                name: "home/bash".into(),
                success: true,
                error_message: None,
            }],
        )
        .for_run("stow", vec!["home/vim".into()]);
        exec.state = ExecutionState::Running;
        app.component_stack.push(Box::new(exec));
        render_updated(&mut app, width, height);

        let link = tmp.path().join("missing-link");
        app.component_stack
            .push(Box::new(UnstowConfirmComponent::with_paths(
                vec![link],
                None,
            )));
        render_updated(&mut app, width, height);
    }
}

// --- Settings screen (F6d) ---

#[test]
fn test_settings_list_shows_its_sections_and_no_save_status_text() {
    let (mut app, _tmp) = make_flat_test_app();
    push_screen(&mut app, "Settings");
    let buf = render_updated(&mut app, 80, 24);
    for header in ["stow options", "appearance", "config"] {
        assert!(line_contains(&buf, header), "section {header} not drawn");
    }
    for text in [
        "[Update",
        "[All saved]",
        "Saved.",
        "Unsaved changes",
        "groups",
    ] {
        assert!(
            !line_contains(&buf, text),
            "save status {text:?} still drawn"
        );
    }
    assert!(line_contains(&buf, "packages "), "packages row not drawn");
}

#[test]
fn test_settings_title_marks_unsaved_edits() {
    let (mut app, _tmp) = make_flat_test_app();
    push_screen(&mut app, "Settings");
    let buf = render_updated(&mut app, 80, 24);
    assert!(!line_contains(&buf, "unsaved"));

    app.component_dispatch(key_char('v'));
    let buf = render_app(&mut app, 80, 24);
    assert!(
        line_contains(&buf, "unsaved"),
        "title does not mark the edit"
    );
}

#[test]
fn test_settings_skips_headers_when_the_cursor_moves() {
    let (mut app, _tmp) = make_flat_test_app();
    push_screen(&mut app, "Settings");
    select_setting(&mut app, "icons");
    app.component_dispatch(down_key());
    let buf = render_app(&mut app, 80, 24);
    // Down from `icons` skips the config header: the cursor is on `packages`, whose detail
    // headline is drawn.
    assert!(
        line_contains(&buf, "Every folder here is a package."),
        "cursor did not skip the config header"
    );
}

/// Every settings dialog paints at the design size and at the two larger sizes.
#[test]
fn test_settings_dialogs_render_at_80x24_120x40_and_200x50() {
    type Scenario = (&'static str, fn(&mut App), &'static str);
    let scenarios: &[Scenario] = &[
        (
            "action picker",
            |app| {
                select_setting(app, "action");
                app.component_dispatch(enter_key());
            },
            "action",
        ),
        (
            "theme picker",
            |app| {
                select_setting(app, "theme");
                app.component_dispatch(enter_key());
            },
            "theme",
        ),
        (
            "icons picker",
            |app| {
                select_setting(app, "icons");
                app.component_dispatch(enter_key());
            },
            "icons",
        ),
        (
            "global ignore",
            |app| {
                select_setting(app, "global ignore");
                app.component_dispatch(enter_key());
            },
            "global ignore",
        ),
        (
            "packages folder",
            |app| {
                select_setting(app, "packages");
                app.component_dispatch(enter_key());
            },
            "packages folder",
        ),
        (
            "leave",
            |app| {
                app.component_dispatch(key_char('v'));
                app.component_dispatch(esc_key());
            },
            "unsaved changes",
        ),
    ];
    for (name, open, title) in scenarios {
        for (width, height) in [(80, 24), (120, 40), (200, 50)] {
            let (mut app, _tmp) = make_flat_test_app();
            push_screen(&mut app, "Settings");
            app.update_components();
            open(&mut app);
            let buf = render_app(&mut app, width, height);
            assert!(
                line_contains(&buf, title),
                "{name} at {width}x{height}: title {title:?} not drawn"
            );
        }
    }
}

#[test]
fn test_settings_list_renders_at_120x40_and_200x50() {
    for (width, height) in [(120, 40), (200, 50)] {
        let (mut app, _tmp) = make_flat_test_app();
        push_screen(&mut app, "Settings");
        let buf = render_updated(&mut app, width, height);
        assert!(
            line_contains(&buf, "stow options"),
            "section header missing at {width}x{height}"
        );
        assert!(
            line_contains(&buf, "relative to"),
            "detail of the cursor row missing at {width}x{height}"
        );
    }
}

#[test]
fn test_settings_packages_footer_says_change_and_keeps_the_theme_chip_at_80x24() {
    let (mut app, _tmp) = make_flat_test_app();
    push_screen(&mut app, "Settings");
    select_setting(&mut app, "packages");
    let buf = render_app(&mut app, 80, 24);
    let footer = footer_row(&buf);
    assert!(
        footer.contains("Enter change  w save  Esc back"),
        "packages row footer: {footer:?}"
    );
    assert!(
        footer.contains("theme default"),
        "theme chip crowded out: {footer:?}"
    );
}

#[test]
fn test_settings_unsaved_dot_is_drawn_in_the_warn_colour() {
    let (mut app, _tmp) = make_flat_test_app();
    push_screen(&mut app, "Settings");
    render_updated(&mut app, 80, 24);
    app.component_dispatch(key_char('v'));
    let buf = render_app(&mut app, 80, 24);

    let warn = andre::theme::ThemeColors::from_theme(andre_core::Theme::Default).warn;
    let dot = (0..buf.area().height)
        .flat_map(|y| (0..buf.area().width).map(move |x| (x, y)))
        .find(|&(x, y)| buf[(x, y)].symbol() == "\u{25cf}")
        .expect("unsaved dot drawn");
    assert_eq!(buf[dot].fg, warn, "dot should be in warn");
    assert!(
        line_contains(&buf, "unsaved \u{25cf}"),
        "title should read `unsaved ●`"
    );
}

// --- Help screen, title row and too-small box (F6e) ---

#[test]
fn test_help_title_row_reads_main_then_help() {
    let (mut app, _tmp) = make_test_app();
    push_screen(&mut app, "Help");
    let buf = render_updated(&mut app, 80, 24);
    assert!(
        row_text(&buf, 1).contains("andre  main › help"),
        "title row: {:?}",
        row_text(&buf, 1)
    );
}

#[test]
fn test_help_lists_every_topic_with_the_cursor_on_keys() {
    let (mut app, _tmp) = make_test_app();
    push_screen(&mut app, "Help");
    let buf = render_updated(&mut app, 80, 24);
    assert!(
        line_contains(&buf, " help "),
        "the topic list is titled help"
    );
    assert!(
        row_containing(&buf, "▌keys").is_some(),
        "the cursor bar does not sit on keys"
    );
    for topic in [
        "mouse",
        "status glyphs",
        "profiles",
        "packages",
        "new package",
        "ignore files",
        "about",
    ] {
        assert!(line_contains(&buf, topic), "topic {topic:?} is not listed");
    }
}

#[test]
fn test_help_status_glyph_topic_names_every_status_and_the_unknown_glyph() {
    let (mut app, _tmp) = make_test_app();
    push_screen(&mut app, "Help");
    app.component_dispatch(down_key());
    app.component_dispatch(down_key());
    let buf = render_updated(&mut app, 80, 24);
    assert!(line_contains(&buf, "status glyphs"), "detail title missing");
    for meaning in [
        "stowed — every file is linked",
        "not stowed — nothing is linked",
        "partial — some files linked",
        "conflict — a real file is in place",
        "missing — no such package folder",
        "empty — the package has no files",
        "invalid — ignore list unreadable",
        "unknown — not scanned yet",
        "marked, unmarked",
        "folder, symlink, file",
    ] {
        assert!(
            line_contains(&buf, meaning),
            "status glyph topic lacks {meaning:?}"
        );
    }
    let text = buffer_text(&buf);
    for gone in [
        "▣",
        "▸",
        "▾",
        "open, folded group",
        "marked, unmarked, some",
    ] {
        assert!(
            !text.contains(gone),
            "status glyph topic still has {gone:?}"
        );
    }
}

#[test]
fn test_help_mark_and_kind_legend_lines_up_in_one_column() {
    let (mut app, _tmp) = make_test_app();
    push_screen(&mut app, "Help");
    app.component_dispatch(down_key());
    app.component_dispatch(down_key());
    let buf = render_updated(&mut app, 80, 24);
    let meanings = [
        "marked, unmarked",
        "chosen, not chosen",
        "folder, symlink, file",
    ];
    let column =
        column_of(&buf, row_of(&buf, meanings[0]), meanings[0]).expect("the mark legend is drawn");
    for meaning in meanings {
        let y = row_of(&buf, meaning);
        assert_eq!(
            column_of(&buf, y, meaning),
            Some(column),
            "{meaning:?} does not start in the meaning column"
        );
    }
}

/// A profile that names a package the folder lacks lists it after the packages, as a missing
/// row that says so.
#[test]
fn test_profile_edit_lists_a_missing_ref_after_the_packages_as_missing() {
    let (mut app, _tmp) = make_profiles_app(&[("agent", "packages: [test, gone]\n")]);
    open_editor_render(&mut app, "agent");
    let buf = render_updated(&mut app, 80, 24);

    let rows: Vec<String> = (0..buf.area().height).map(|y| row_text(&buf, y)).collect();
    let test_y = rows
        .iter()
        .position(|row| row.contains("test"))
        .expect("test row");
    let gone_y = rows
        .iter()
        .position(|row| row.contains("? gone"))
        .unwrap_or_else(|| panic!("no missing row in {}", buffer_text(&buf)));
    assert!(gone_y > test_y, "the missing row comes after the packages");
    assert!(rows[gone_y].contains("missing"), "{:?}", rows[gone_y]);
    assert!(
        line_contains(&buf, "packages 2 of 3"),
        "the missing row is counted: {}",
        buffer_text(&buf)
    );
}

/// The status screen of every package, with the cursor on a conflict, draws at 120x40 and
/// 200x50 without panic and keeps the flat rows.
#[test]
fn test_status_of_every_package_renders_at_120x40_and_200x50() {
    let (mut app, _tmp) = make_flat_status_app();
    push_screen(&mut app, "Status");
    for (width, height) in [(120, 40), (200, 50)] {
        let buf = render_updated(&mut app, width, height);
        assert!(line_contains(&buf, "packages 6"), "{width}x{height}");
        assert!(line_contains(&buf, "clash"), "{width}x{height}");
    }
}

/// The help keys list names `*` for mark and unmark all, and no longer names the fold keys.
#[test]
fn test_help_keys_name_the_star_key_and_no_fold_keys() {
    let (mut app, _tmp) = make_test_app();
    push_screen(&mut app, "Help");
    let buf = render_updated(&mut app, 80, 24);
    let text = buffer_text(&buf);
    assert!(text.contains("mark / unmark all"), "{text}");
    assert!(!text.contains("fold / unfold"), "{text}");
}

#[test]
fn test_help_keys_sit_one_column_under_their_heading() {
    let (mut app, _tmp) = make_test_app();
    push_screen(&mut app, "Help");
    let buf = render_updated(&mut app, 80, 24);
    let heading = column_of(&buf, row_of(&buf, "everywhere"), "everywhere")
        .expect("the everywhere heading is drawn");
    let key = column_of(
        &buf,
        row_of(&buf, "\u{2191}\u{2193} / jk"),
        "\u{2191}\u{2193} / jk",
    )
    .expect("the move key is drawn");
    assert_eq!(key, heading + 1, "key rows are indented under the heading");
}

#[test]
fn test_too_small_overlay_names_the_needed_and_the_current_size() {
    let (mut app, _tmp) = make_test_app();
    let buf = render_app(&mut app, 60, 18);
    assert!(line_contains(&buf, "terminal too small"), "title missing");
    assert!(
        line_contains(&buf, "needs   80 × 24"),
        "needed size missing"
    );
    assert!(
        line_contains(&buf, "now     60 × 18"),
        "current size missing"
    );
}

#[test]
fn test_too_small_overlay_colours_each_dimension_by_whether_it_passes() {
    let (mut app, _tmp) = make_test_app();
    let colors = ThemeColors::from_theme(app.ctx.core.theme);
    let buf = render_app(&mut app, 100, 18);
    let y = row_of(&buf, "now     ");
    let cols = column_of(&buf, y, "100").expect("the column count is drawn");
    assert_eq!(buf[(cols, y)].fg, colors.ok, "100 columns passes");
    let rows = column_of(&buf, y, "18").expect("the row count is drawn");
    assert_eq!(buf[(rows, y)].fg, colors.err, "18 rows fails");
}

#[test]
fn test_help_topics_render_at_120x40_and_200x50() {
    for (width, height) in [(120, 40), (200, 50)] {
        let (mut app, _tmp) = make_test_app();
        push_screen(&mut app, "Help");
        for topic in andre::components::help::Topic::ALL {
            let buf = render_updated(&mut app, width, height);
            assert!(
                row_text(&buf, 1).contains("andre  main › help"),
                "{width}x{height}: title row"
            );
            assert!(
                line_contains(&buf, &format!(" {} ─", topic.label())),
                "{width}x{height}: detail titled {:?} missing",
                topic.label()
            );
            assert!(
                row_containing(&buf, "q quit").is_some(),
                "{width}x{height}: footer missing"
            );
            app.component_dispatch(down_key());
        }
    }
}

#[test]
fn test_help_page_down_brings_the_end_of_a_long_topic_into_view_and_stops_there() {
    let (mut app, _tmp) = make_test_app();
    push_screen(&mut app, "Help");
    for _ in 0..6 {
        app.component_dispatch(down_key());
    }
    let page_down = KeyEvent::new(KeyCode::PageDown, KeyModifiers::NONE);
    let top = render_updated(&mut app, 80, 24);
    assert!(
        !line_contains(&top, "w saves"),
        "the end starts out of view"
    );

    app.component_dispatch(page_down);
    let end = render_updated(&mut app, 80, 24);
    assert!(
        line_contains(&end, "w saves"),
        "PgDn shows the end of the text"
    );
    assert!(line_contains(&end, "█"), "a long topic shows its scrollbar");

    app.component_dispatch(page_down);
    let again = render_updated(&mut app, 80, 24);
    assert_eq!(
        buffer_text(&again),
        buffer_text(&end),
        "PgDn past the end stops at the end"
    );
}

/// The title row names the screen and the crumb is `unstow`; the list panel names the folder
/// being browsed.
#[test]
fn test_unstow_browse_title_row_is_unstow_and_the_list_names_the_folder() {
    let (mut app, tmp) = make_unstow_app();
    std::fs::create_dir_all(tmp.path().join(".config")).unwrap();
    push_browse(&mut app, tmp.path());
    let buf = render_updated(&mut app, 80, 24);
    assert!(
        row_text(&buf, 1).contains("andre  main › unstow"),
        "title row: {:?}",
        row_text(&buf, 1)
    );
    assert!(
        line_contains(&buf, "╭ ~ "),
        "the target's list title is missing"
    );

    app.component_stack.pop();
    push_browse(&mut app, &tmp.path().join(".config"));
    let buf = render_updated(&mut app, 80, 24);
    assert!(
        row_text(&buf, 1).contains("andre  main › unstow"),
        "title row after opening a folder: {:?}",
        row_text(&buf, 1)
    );
    assert!(
        line_contains(&buf, "╭ ~/.config "),
        "the browsed folder's list title is missing"
    );
}

#[test]
fn test_review_and_run_title_rows_name_their_screen() {
    let (mut app, _tmp) = make_test_app();
    push_screen(&mut app, "Packages");
    push_screen(&mut app, "Confirm");
    let buf = render_updated(&mut app, 80, 24);
    assert!(
        row_text(&buf, 1).contains("andre  main › packages › review"),
        "review title row: {:?}",
        row_text(&buf, 1)
    );

    app.component_stack.pop();
    push_screen(&mut app, "Execute");
    let buf = render_updated(&mut app, 80, 24);
    assert!(
        row_text(&buf, 1).contains("andre  main › packages › run"),
        "run title row: {:?}",
        row_text(&buf, 1)
    );
}

#[test]
fn test_profile_edit_title_row_names_the_profile() {
    let (mut app, _tmp) = make_profiles_app(&[("agent", "packages: [home/test]\n")]);
    open_editor_render(&mut app, "agent");
    let buf = render_updated(&mut app, 80, 24);
    assert!(
        row_text(&buf, 1).contains("andre  main › profiles › agent"),
        "title row: {:?}",
        row_text(&buf, 1)
    );
}

#[test]
fn test_compact_title_drops_the_leftmost_crumbs_when_the_trail_is_too_wide() {
    let long = "a".repeat(60);
    let (mut app, _tmp) = make_profiles_app(&[(long.as_str(), "packages: [home/test]\n")]);
    open_editor_render(&mut app, &long);
    let buf = render_updated(&mut app, 80, 24);
    let title = row_text(&buf, 1);
    assert!(
        title.contains(&format!("andre  … › {long}")),
        "title row: {title:?}"
    );
}

#[test]
fn test_ignore_editor_title_row_names_the_package_at_120x40_and_200x50() {
    for (width, height) in [(120, 40), (200, 50)] {
        let (mut app, _tmp) = make_test_app();
        write_package_ignore(&app, ".gitignore", "# secrets\n*.env\n");
        open_ignore_render(&mut app);
        let buf = render_updated(&mut app, width, height);
        assert!(
            row_text(&buf, 1).contains("andre  main › packages › home/test › ignore"),
            "{width}x{height}: title row {:?}",
            row_text(&buf, 1)
        );
    }
}

// ---- G2: the run screen names each package by its name ----

#[test]
fn test_run_rows_show_the_package_name_at_80x24() {
    use andre::components::execute::ExecuteComponent;
    use andre::execute::ExecutionResult;

    let (mut app, _tmp) = make_test_app();
    let (_tx, rx) = tokio::sync::mpsc::channel(1);
    app.component_stack.push(Box::new(ExecuteComponent::new(
        rx,
        1,
        vec![ExecutionResult {
            name: "home/shell".into(),
            success: true,
            error_message: None,
        }],
    )));
    let buf = render_updated(&mut app, 80, 24);
    assert!(
        line_contains(&buf, "\u{2713} home/shell"),
        "result row must show `name` after its mark"
    );
}

#[test]
fn test_run_screen_and_set_up_render_at_120x40_and_200x50() {
    use andre::components::execute::ExecuteComponent;
    use andre::execute::ExecutionResult;

    for (width, height) in [(120, 40), (200, 50)] {
        let (mut app, _tmp) = make_bootstrap_app();
        let buf = render_updated(&mut app, width, height);
        assert!(
            line_contains(&buf, "config/andre"),
            "{width}x{height}: set-up detail must name the package"
        );

        let (_tx, rx) = tokio::sync::mpsc::channel(1);
        app.component_stack.push(Box::new(ExecuteComponent::new(
            rx,
            2,
            vec![ExecutionResult {
                name: "config/andre".into(),
                success: false,
                error_message: Some("Skipped: source directory not found".into()),
            }],
        )));
        let buf = render_updated(&mut app, width, height);
        assert!(
            line_contains(&buf, "config/andre"),
            "{width}x{height}: run row must name the package"
        );
    }
}
