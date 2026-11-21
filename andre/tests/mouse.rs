//! Mouse input through `App::mouse_dispatch`, as the event loop sends it. Each test draws
//! first, so the hit rects it clicks are the ones on screen.

mod helpers;

use andre::app::App;
use andre::components::ignore::IgnoreEditComponent;
use andre::components::menu::{MainMenuComponent, MenuItem};
use andre::components::packages::PackagesComponent;
use andre::components::profiles::edit::ProfileEditComponent;
use andre::components::profiles::{Popup, ProfilesComponent};
use andre::components::settings::SettingsComponent;
use andre::components::unstow::UnstowBrowseComponent;
use andre::components::InputMode;
use andre::input;
use andre::ui::icons::Icons;
use andre::ui::tree::TreeRow;
use andre_core::profile::ProfileSet;
use crossterm::event::KeyCode;
use ratatui::{buffer::Buffer, layout::Rect};
use std::path::PathBuf;
use tempfile::TempDir;

use helpers::{
    click, component_id, down_key, draw, enter_key, esc_key, find_text, key_char, make_empty_app,
    make_test_app, make_two_group_app, make_unstow_app, push_browse, push_component, row_text,
    up_key, wheel,
};

/// Pushes the named screen and brings it up to date, as the loop does before the first frame.
fn open(app: &mut App, name: &str) {
    push_component(app, name);
    app.update_components();
}

fn menu(app: &App) -> &MainMenuComponent {
    app.component_stack[0]
        .as_any()
        .downcast_ref()
        .expect("main menu at the bottom")
}

fn packages(app: &App) -> &PackagesComponent {
    app.component_stack
        .last()
        .and_then(|c| c.as_any().downcast_ref())
        .expect("packages on top")
}

/// Pushes the profiles screen and brings it up to date, as the loop does before the first frame.
fn open_profiles(app: &mut App) {
    app.component_stack
        .push(Box::new(ProfilesComponent::default()));
    app.update_components();
}

fn profiles(app: &App) -> &ProfilesComponent {
    app.component_stack
        .last()
        .and_then(|c| c.as_any().downcast_ref())
        .expect("profiles on top")
}

fn settings(app: &App) -> &SettingsComponent {
    app.component_stack
        .last()
        .and_then(|c| c.as_any().downcast_ref())
        .expect("settings on top")
}

fn ignore_editor(app: &App) -> &IgnoreEditComponent {
    app.component_stack
        .last()
        .and_then(|c| c.as_any().downcast_ref())
        .expect("ignore editor on top")
}

fn browse(app: &App) -> &UnstowBrowseComponent {
    app.component_stack
        .last()
        .and_then(|c| c.as_any().downcast_ref())
        .expect("unstow browse on top")
}

fn editor(app: &App) -> &ProfileEditComponent {
    app.component_stack
        .last()
        .and_then(|c| c.as_any().downcast_ref())
        .expect("profile edit on top")
}

/// `make_test_app` with one profile, `agent`, written from `yaml`, and the `shell` package
/// beside `test` in the `home` group.
fn make_editor_app(yaml: &str) -> (App, TempDir) {
    let (mut app, tmp) = make_test_app();
    std::fs::create_dir_all(tmp.path().join("dotfiles/home/shell")).unwrap();
    let dir = tmp.path().join("profiles");
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("agent.yml"), yaml).unwrap();
    app.ctx.core.profiles = ProfileSet::load(&dir).unwrap();
    (app, tmp)
}

/// Opens the editor on the only profile, `agent`.
fn open_editor(app: &mut App) {
    open_profiles(app);
    app.component_dispatch(key_char('e'));
    app.update_components();
}

/// The name typed into the open `+` dialog.
fn name_value(app: &App) -> String {
    match profiles(app).popup() {
        Some(Popup::New(name)) => name.field.value.clone(),
        other => panic!("the new profile dialog is open: {other:?}"),
    }
}

/// The column of the unmarked checkbox on row `y`.
fn checkbox_column(buf: &Buffer, icons: &Icons, y: u16) -> u16 {
    (0..buf.area().width)
        .find(|&x| buf[(x, y)].symbol() == icons.checkbox_off)
        .expect("a checkbox on the row")
}

/// The top bar's clickable part for `key`, as drawn by the last frame.
fn top_bar_hit(app: &App, key: char) -> Rect {
    app.chrome_hits
        .top_bar
        .iter()
        .find(|(_, hit)| hit.code == KeyCode::Char(key))
        .map(|(rect, _)| *rect)
        .unwrap_or_else(|| panic!("no top bar hit for {key:?}"))
}

/// The footer's clickable part for `code`, as drawn by the last frame.
fn footer_hit(app: &App, code: KeyCode) -> Rect {
    app.chrome_hits
        .footer
        .iter()
        .find(|(_, hit)| hit.code == code)
        .map(|(rect, _)| *rect)
        .unwrap_or_else(|| panic!("no footer hit for {code:?}"))
}

/// The text the buffer shows inside `rect`, on its row.
fn text_in(buf: &Buffer, rect: Rect) -> String {
    (rect.x..rect.right())
        .map(|x| buf[(x, rect.y)].symbol())
        .collect()
}

#[test]
fn test_top_bar_hits_cover_the_chips_they_name() {
    let (mut app, _tmp) = make_test_app();
    let buf = draw(&mut app, 80, 24);
    for (key, label) in [
        ('d', "dry-run"),
        ('n', "no-fold"),
        ('a', "adopt"),
        ('o', "dotfiles"),
        ('v', "verbose"),
    ] {
        let text = text_in(&buf, top_bar_hit(&app, key));
        assert!(text.contains(label), "{key}: {text:?}");
    }
}

#[test]
fn test_click_on_a_flag_chip_toggles_its_flag() {
    let (mut app, _tmp) = make_test_app();
    let buf = draw(&mut app, 80, 24);
    let rect = top_bar_hit(&app, 'd');
    assert!(text_in(&buf, rect).contains("dry-run"));
    let before = app.ctx.core.flags.dry_run;
    click(&mut app, rect.x + 3, rect.y);
    assert_ne!(app.ctx.core.flags.dry_run, before);
}

#[test]
fn test_click_on_the_action_pill_cycles_the_action() {
    let (mut app, _tmp) = make_test_app();
    draw(&mut app, 80, 24);
    let rect = top_bar_hit(&app, 's');
    let before = app.ctx.core.flags.action.as_str();
    click(&mut app, rect.x, rect.y);
    assert_ne!(app.ctx.core.flags.action.as_str(), before);
}

#[test]
fn test_top_bar_click_does_nothing_while_a_text_field_has_focus() {
    let (mut app, _tmp) = make_test_app();
    app.component_stack
        .push(Box::new(ProfilesComponent::default()));
    app.component_dispatch(key_char('+'));
    let focused = app.component_stack.last().unwrap().input_mode();
    assert_eq!(focused, InputMode::TextInput, "the name dialog is open");
    let rect = {
        draw(&mut app, 80, 24);
        top_bar_hit(&app, 'd')
    };
    let before = app.ctx.core.flags.dry_run;
    click(&mut app, rect.x + 3, rect.y);
    assert_eq!(app.ctx.core.flags.dry_run, before);
}

#[test]
fn test_wheel_over_a_chip_or_a_hint_does_nothing() {
    let (mut app, _tmp) = make_two_group_app();
    open(&mut app, "Packages");
    draw(&mut app, 80, 24);
    let chip = top_bar_hit(&app, 'd');
    let hint = footer_hit(&app, KeyCode::Esc);
    let (flag, screen) = (app.ctx.core.flags.dry_run, component_id(&app).to_string());
    wheel(&mut app, chip.x + 3, chip.y, true);
    wheel(&mut app, hint.x, hint.y, false);
    assert_eq!(app.ctx.core.flags.dry_run, flag);
    assert_eq!(component_id(&app), screen);
}

#[test]
fn test_footer_esc_click_pops_the_screen() {
    let (mut app, _tmp) = make_two_group_app();
    open(&mut app, "Packages");
    draw(&mut app, 80, 24);
    let rect = footer_hit(&app, KeyCode::Esc);
    click(&mut app, rect.x, rect.y);
    assert_eq!(component_id(&app), "MainMenu");
}

#[test]
fn test_click_on_an_unselected_menu_row_selects_and_a_second_click_opens_it() {
    let (mut app, _tmp) = make_test_app();
    let buf = draw(&mut app, 80, 24);
    let label = MenuItem::Settings.label();
    let (x, y) = find_text(&buf, label).expect("settings row");
    click(&mut app, x, y);
    assert_eq!(
        component_id(&app),
        "MainMenu",
        "the first click only selects"
    );
    let selected = menu(&app).items()[menu(&app).list.cursor].label();
    assert_eq!(selected, label);

    draw(&mut app, 80, 24);
    click(&mut app, x, y);
    assert_eq!(component_id(&app), "Settings", "the second click opens it");
}

#[test]
fn test_wheel_moves_the_menu_cursor_one_row_per_notch() {
    let (mut app, _tmp) = make_test_app();
    let buf = draw(&mut app, 80, 24);
    let (x, y) = find_text(&buf, MenuItem::Settings.label()).expect("settings row");
    wheel(&mut app, x, y, true);
    assert_eq!(menu(&app).list.cursor, 1);
    wheel(&mut app, x, y, true);
    assert_eq!(menu(&app).list.cursor, 2);
    wheel(&mut app, x, y, false);
    assert_eq!(menu(&app).list.cursor, 1);
}

#[test]
fn test_click_outside_the_list_changes_nothing() {
    let (mut app, _tmp) = make_test_app();
    draw(&mut app, 80, 24);
    let before = menu(&app).list.cursor;
    // Column 70 is in the detail pane, well right of the list.
    click(&mut app, 70, 12);
    assert_eq!(menu(&app).list.cursor, before);
    assert_eq!(component_id(&app), "MainMenu");
}

#[test]
fn test_second_click_on_a_package_marks_it_and_does_not_run() {
    let (mut app, _tmp) = make_two_group_app();
    open(&mut app, "Packages");
    let buf = draw(&mut app, 80, 24);
    let (x, y) = find_text(&buf, "bash-env").expect("package row");
    click(&mut app, x, y);
    assert!(
        packages(&app).marked().is_empty(),
        "the first click selects"
    );
    click(&mut app, x, y);
    assert_eq!(packages(&app).marked().len(), 1, "the second click marks");
    assert_eq!(component_id(&app), "Packages", "nothing was run");
}

#[test]
fn test_a_click_on_a_checkbox_marks_the_package_in_one_click() {
    let (mut app, _tmp) = make_two_group_app();
    open(&mut app, "Packages");
    let buf = draw(&mut app, 80, 24);
    let icons = Icons::for_set(app.ctx.core.icons);
    let (x_name, y) = find_text(&buf, "bash-env").expect("package row");
    // The name starts 9 columns into the row; its checkbox is 4 columns before it.
    let x_box = x_name - 4;
    assert_eq!(buf[(x_box, y)].symbol(), icons.checkbox_off);
    click(&mut app, x_box, y);
    assert_eq!(packages(&app).marked().len(), 1);
}

#[test]
fn test_a_click_on_a_fold_triangle_folds_its_group() {
    let (mut app, _tmp) = make_two_group_app();
    open(&mut app, "Packages");
    let buf = draw(&mut app, 80, 24);
    let icons = Icons::for_set(app.ctx.core.icons);
    let (x, y) = find_text(&buf, icons.fold_open).expect("a group row");
    let before = packages(&app).rows().len();
    click(&mut app, x, y);
    assert!(
        packages(&app).rows().len() < before,
        "the group's packages are hidden"
    );
    let row = &packages(&app).rows()[packages(&app).list.cursor];
    assert!(
        matches!(row, TreeRow::Group { folded: true, .. }),
        "{row:?}"
    );
}

#[test]
fn test_wheel_over_the_packages_list_moves_the_cursor() {
    let (mut app, _tmp) = make_two_group_app();
    open(&mut app, "Packages");
    let buf = draw(&mut app, 80, 24);
    let (x, y) = find_text(&buf, "bash-env").expect("package row");
    wheel(&mut app, x, y, true);
    assert_eq!(packages(&app).list.cursor, 1);
}

#[test]
fn test_a_click_behind_an_open_dialog_changes_nothing() {
    let (mut app, _tmp) = make_two_group_app();
    open(&mut app, "Packages");
    let buf = draw(&mut app, 80, 24);
    let (x, y) = find_text(&buf, "bash-env").expect("package row");
    click(&mut app, x, y);
    app.component_dispatch(key_char('p'));
    assert!(
        packages(&app).popup().is_some(),
        "the membership dialog is open"
    );
    let before = (packages(&app).list.cursor, packages(&app).marked().len());
    draw(&mut app, 80, 24);
    // The row is the cursor's own, so an unblocked click would mark it.
    click(&mut app, x, y);
    assert_eq!(
        (packages(&app).list.cursor, packages(&app).marked().len()),
        before
    );
    assert!(packages(&app).popup().is_some());
}

#[test]
fn test_the_touched_screens_render_at_120x40_and_200x50() {
    for (width, height) in [(120, 40), (200, 50)] {
        for name in [
            "MainMenu",
            "Packages",
            "Profiles",
            "Status",
            "Help",
            "Settings",
            "UnstowTargetSelect",
            "NewPackage",
        ] {
            let (mut app, _tmp): (App, TempDir) = make_two_group_app();
            if name == "Profiles" {
                app.component_stack
                    .push(Box::new(ProfilesComponent::default()));
                app.update_components();
            } else if name != "MainMenu" {
                open(&mut app, name);
            }
            draw(&mut app, width, height);
            click(&mut app, width / 2, height / 2);
        }
        let (mut app, _tmp) = make_unstow_app();
        let home = app.ctx.home_dir.clone();
        push_browse(&mut app, &home);
        draw(&mut app, width, height);
    }
}

#[test]
fn test_footer_quit_and_theme_chips_type_nothing_into_an_open_name_field() {
    let (mut app, _tmp) = make_test_app();
    open_profiles(&mut app);
    app.component_dispatch(key_char('+'));
    draw(&mut app, 80, 24);
    let theme = app.ctx.core.theme;
    let quit = footer_hit(&app, KeyCode::Char('q'));
    click(&mut app, quit.x, quit.y);
    let chip = footer_hit(&app, KeyCode::Char('t'));
    click(&mut app, chip.x, chip.y);
    assert!(!app.quitting, "`q quit` does nothing while a name is typed");
    assert_eq!(app.ctx.core.theme, theme, "the theme chip does nothing");
    assert_eq!(name_value(&app), "", "neither chip types into the name");

    app.component_dispatch(esc_key());
    draw(&mut app, 80, 24);
    let quit = footer_hit(&app, KeyCode::Char('q'));
    click(&mut app, quit.x, quit.y);
    assert!(app.quitting, "with no text field focused, `q quit` quits");
}

/// The settings cursor after ↑ from the first selectable row, on a fresh screen. Up skips the
/// header above it and wraps to the last selectable row; a wheel notch must land there too.
fn settings_cursor_after_up() -> usize {
    let (mut app, _tmp) = make_test_app();
    open(&mut app, "Settings");
    app.component_dispatch(up_key());
    settings(&app).list.cursor
}

#[test]
fn test_wheel_over_settings_moves_to_the_next_selectable_row() {
    let (mut app, _tmp) = make_test_app();
    open(&mut app, "Settings");
    let buf = draw(&mut app, 80, 24);
    let header = settings(&app).items()[0].label.clone();
    assert!(!settings(&app).items()[0].selectable());
    assert_eq!(settings(&app).list.cursor, 1, "the first selectable row");
    let (x, y) = find_text(&buf, &header).expect("the header row");
    // A notch over the header row moves the cursor as ↑ does.
    wheel(&mut app, x, y, false);
    let cursor = settings(&app).list.cursor;
    assert_eq!(
        cursor,
        settings_cursor_after_up(),
        "the wheel moves as ↑ does"
    );
    assert_ne!(cursor, 1, "the cursor moves off the first selectable row");
    assert_ne!(cursor, 0, "the cursor never rests on a header");
    assert!(settings(&app).items()[cursor].selectable());
}

#[test]
fn test_wheel_up_over_a_selectable_row_moves_as_up_does() {
    let (mut app, _tmp) = make_test_app();
    open(&mut app, "Settings");
    let buf = draw(&mut app, 80, 24);
    let header = settings(&app).items()[0].label.clone();
    assert_eq!(settings(&app).list.cursor, 1, "the first selectable row");
    // The header is row 0, so the row under it is row 1, the cursor row.
    let (x, header_y) = find_text(&buf, &header).expect("the header row");
    let y = header_y + 1;
    assert_eq!(
        settings(&app).list.row_at(x, y),
        Some(1),
        "the pointer is over row 1"
    );
    wheel(&mut app, x, y, false);
    let cursor = settings(&app).list.cursor;
    assert_eq!(
        cursor,
        settings_cursor_after_up(),
        "the wheel moves as ↑ does"
    );
    assert_ne!(cursor, 0, "the cursor never rests on a header");
    assert!(settings(&app).items()[cursor].selectable());
}

#[test]
fn test_click_on_a_settings_header_changes_nothing() {
    let (mut app, _tmp) = make_test_app();
    open(&mut app, "Settings");
    let buf = draw(&mut app, 80, 24);
    let header = settings(&app).items()[0].label.clone();
    let (x, y) = find_text(&buf, &header).expect("the header row");
    click(&mut app, x, y);
    assert_eq!(settings(&app).list.cursor, 1);
}

#[test]
fn test_click_on_an_ignore_line_refreshes_the_detail_to_that_line() {
    let (mut app, tmp) = make_test_app();
    std::fs::write(
        tmp.path().join("dotfiles/home/test/.gitignore"),
        "aaa\nbbb\n",
    )
    .unwrap();
    open(&mut app, "Packages");
    app.component_dispatch(down_key());
    app.component_dispatch(key_char('i'));
    let buf = draw(&mut app, 80, 24);
    assert_eq!(ignore_editor(&app).detail_title(), "aaa");
    let (x, y) = find_text(&buf, "bbb").expect("the second line");
    click(&mut app, x, y);
    assert_eq!(ignore_editor(&app).cursor(), 1);
    assert_eq!(ignore_editor(&app).detail_title(), "bbb");
}

#[test]
fn test_click_on_a_name_step_row_works_while_the_name_field_has_focus() {
    let (mut app, _tmp) = make_two_group_app();
    push_component(&mut app, "NewPackage");
    app.update_components();
    app.component_dispatch(enter_key());
    let mode = |app: &App| app.component_stack.last().unwrap().input_mode();
    assert_eq!(mode(&app), InputMode::TextInput, "the name step");
    let buf = draw(&mut app, 80, 24);
    let (x, y) = find_text(&buf, "nvim").expect("a package the name matches");
    click(&mut app, x, y);
    assert_eq!(mode(&app), InputMode::TextInput, "the first click selects");
    click(&mut app, x, y);
    assert_eq!(mode(&app), InputMode::Normal, "the second click picks it");
}

#[test]
fn test_click_on_the_empty_profiles_hint_opens_the_new_profile_dialog() {
    let (mut app, _tmp) = make_empty_app();
    open_profiles(&mut app);
    let buf = draw(&mut app, 80, 24);
    let (x, y) = find_text(&buf, "new profile").expect("the hint line");
    click(&mut app, x, y);
    assert!(
        matches!(profiles(&app).popup(), Some(Popup::New(_))),
        "the hint opens the name dialog"
    );
}

#[test]
fn test_click_on_the_selected_folder_opens_it() {
    let (mut app, _tmp) = make_unstow_app();
    let home: PathBuf = app.ctx.home_dir.clone();
    push_browse(&mut app, &home);
    let buf = draw(&mut app, 80, 24);
    // The top bar's `dotfiles` chip is the first match of the bare word, so match the row.
    let (x, y) = find_text(&buf, "/ dotfiles").expect("the folder row");
    assert_eq!(
        browse(&app).list.cursor,
        0,
        "the folder is the cursor's row"
    );
    click(&mut app, x, y);
    assert_eq!(browse(&app).browser.path, home.join("dotfiles"));
}

#[test]
fn test_second_click_on_a_link_marks_it() {
    let (mut app, _tmp) = make_unstow_app();
    let home: PathBuf = app.ctx.home_dir.clone();
    push_browse(&mut app, &home);
    let buf = draw(&mut app, 80, 24);
    let (x, y) = find_text(&buf, ".bashrc").expect("the link row");
    click(&mut app, x, y);
    assert!(
        browse(&app).marked().is_empty(),
        "the first click only selects"
    );
    draw(&mut app, 80, 24);
    click(&mut app, x, y);
    assert_eq!(browse(&app).marked(), [home.join(".bashrc")]);
}

#[test]
fn test_click_on_a_group_selects_it_and_a_second_click_opens_its_browse() {
    let (mut app, _tmp) = make_two_group_app();
    push_component(&mut app, "UnstowTargetSelect");
    app.update_components();
    let buf = draw(&mut app, 80, 24);
    let (x, y) = find_text(&buf, "home").expect("the home group row");
    click(&mut app, x, y);
    assert_eq!(
        component_id(&app),
        "UnstowTargetSelect",
        "the first click selects"
    );
    draw(&mut app, 80, 24);
    click(&mut app, x, y);
    assert_eq!(
        component_id(&app),
        "UnstowBrowse",
        "the second click opens it"
    );
}

#[test]
fn test_a_click_on_a_status_fold_triangle_folds_its_group() {
    let (mut app, _tmp) = make_two_group_app();
    push_component(&mut app, "Status");
    app.update_components();
    let icons = Icons::for_set(app.ctx.core.icons);
    let buf = draw(&mut app, 80, 24);
    assert!(
        find_text(&buf, icons.fold_closed).is_none(),
        "every group starts open"
    );
    // The cursor's row is the first group; its fold cell sits just after the cursor bar.
    let (x, y) = find_text(&buf, "▌").expect("the cursor row");
    click(&mut app, x + 1, y);
    let buf = draw(&mut app, 80, 24);
    assert!(
        find_text(&buf, icons.fold_closed).is_some(),
        "the group folded"
    );
}

#[test]
fn test_click_on_a_package_row_focuses_the_packages_then_a_second_click_toggles_it() {
    let (mut app, _tmp) = make_editor_app("packages: [home/test]\n");
    open_editor(&mut app);
    app.component_dispatch(input::key(KeyCode::Tab));
    let buf = draw(&mut app, 80, 24);
    assert!(
        !editor(&app).on_packages(),
        "Tab leaves the packages pane focused"
    );
    let (x, y) = find_text(&buf, "shell").expect("the shell row");
    click(&mut app, x, y);
    assert!(
        editor(&app).on_packages(),
        "the click focuses the packages pane"
    );
    assert!(!editor(&app)
        .draft()
        .packages
        .contains(&"home/shell".to_string()));
    draw(&mut app, 80, 24);
    click(&mut app, x, y);
    assert!(editor(&app)
        .draft()
        .packages
        .contains(&"home/shell".to_string()));
}

#[test]
fn test_a_file_checkbox_click_marks_the_file_and_a_link_cannot_be_marked() {
    let (mut app, tmp) = make_unstow_app();
    std::fs::write(tmp.path().join("notes.txt"), "hi").unwrap();
    let icons = Icons::for_set(app.ctx.core.icons);
    push_component(&mut app, "NewPackage");
    app.update_components();
    app.component_dispatch(enter_key());
    for c in "pkg".chars() {
        app.component_dispatch(key_char(c));
    }
    app.component_dispatch(enter_key());
    app.component_dispatch(down_key());
    app.component_dispatch(enter_key());
    assert_eq!(
        app.component_stack.last().unwrap().input_mode(),
        InputMode::Normal,
        "the files step takes no text"
    );
    app.update_components();
    let buf = draw(&mut app, 80, 24);
    let (_, notes) = find_text(&buf, "notes.txt").expect("the file row");
    let (_, link) = find_text(&buf, ".bashrc").expect("the link row");
    let box_x = checkbox_column(&buf, icons, notes);
    assert!(!row_text(&buf, notes).contains(icons.checkbox_on));

    click(&mut app, box_x, link);
    let buf = draw(&mut app, 80, 24);
    assert!(
        !row_text(&buf, link).contains(icons.checkbox_on),
        "a link is not marked"
    );

    click(&mut app, box_x, notes);
    let buf = draw(&mut app, 80, 24);
    assert!(
        row_text(&buf, notes).contains(icons.checkbox_on),
        "the file is marked"
    );
}
