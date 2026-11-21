//! Mouse input through `App::mouse_dispatch`, as the event loop sends it. Each test draws
//! first, so the hit rects it clicks are the ones on screen.

mod helpers;

use andre::app::App;
use andre::components::ignore::IgnoreEditComponent;
use andre::components::menu::{MainMenuComponent, MenuItem};
use andre::components::packages::PackagesComponent;
use andre::components::profiles::edit::ProfileEditComponent;
use andre::components::profiles::{Popup, ProfilesComponent};
use andre::components::settings::{Overlay, SettingsComponent};
use andre::components::status::StatusComponent;
use andre::components::unstow::UnstowBrowseComponent;
use andre::components::InputMode;
use andre::input;
use andre::ui::icons::Icons;
use andre_core::profile::ProfileSet;
use andre_core::Theme;
use crossterm::event::KeyCode;
use ratatui::{buffer::Buffer, layout::Rect};
use std::path::PathBuf;
use tempfile::TempDir;

use helpers::{
    click, component_id, down_key, draw, enter_key, esc_key, find_text, key_char, make_empty_app,
    make_flat_packages_app, make_test_app, make_unstow_app, push_browse, push_component, row_text,
    space_key, up_key, wheel,
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

/// A flat app with the packages `shell` and `test`, and the profile `agent` written from `yaml`.
fn make_flat_membership_app(yaml: &str) -> (App, TempDir) {
    let (mut app, tmp) = make_flat_packages_app(&["shell", "test"]);
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
    let (mut app, _tmp) = make_flat_packages_app(&["bash-env", "nvim", "tmux"]);
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
    let (mut app, _tmp) = make_flat_packages_app(&["bash-env", "nvim", "tmux"]);
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
    let (mut app, _tmp) = make_flat_packages_app(&["bash-env", "nvim", "tmux"]);
    open(&mut app, "Packages");
    let buf = draw(&mut app, 80, 24);
    // The cursor starts on bash-env, so nvim is the row the first click selects.
    let (x, y) = find_text(&buf, "nvim").expect("package row");
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
    let (mut app, _tmp) = make_flat_packages_app(&["bash-env", "nvim", "tmux"]);
    open(&mut app, "Packages");
    let buf = draw(&mut app, 80, 24);
    let icons = Icons::for_set(app.ctx.core.icons);
    let (x_name, y) = find_text(&buf, "nvim").expect("package row");
    // The name starts 5 columns into the row; its checkbox is 4 columns before it.
    let x_box = x_name - 4;
    assert_eq!(buf[(x_box, y)].symbol(), icons.checkbox_off);
    click(&mut app, x_box, y);
    assert_eq!(packages(&app).marked().len(), 1);
}

#[test]
fn test_wheel_over_the_packages_list_moves_the_cursor() {
    let (mut app, _tmp) = make_flat_packages_app(&["bash-env", "nvim", "tmux"]);
    open(&mut app, "Packages");
    let buf = draw(&mut app, 80, 24);
    let (x, y) = find_text(&buf, "nvim").expect("package row");
    wheel(&mut app, x, y, true);
    assert_eq!(packages(&app).list.cursor, 1);
}

#[test]
fn test_a_click_behind_an_open_dialog_changes_nothing() {
    let (mut app, _tmp) = make_flat_packages_app(&["bash-env", "nvim", "tmux"]);
    open(&mut app, "Packages");
    let buf = draw(&mut app, 80, 24);
    let (x, y) = find_text(&buf, "nvim").expect("package row");
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

/// Clicks the empty state's hint: two rows under its message, `hint_gap` columns in from the
/// message's first column. The message and the hint are both centred, and the hint starts
/// four columns left of the message.
fn click_empty_hint(app: &mut App, message: &str, hint_gap: u16) {
    let buf = draw(app, 80, 24);
    let (x, y) = find_text(&buf, message).expect("the empty message");
    click(app, x + hint_gap, y + 2);
}

#[test]
fn test_click_on_the_settings_hint_of_an_unset_folder_opens_settings() {
    let (mut app, _tmp) = make_empty_app();
    open(&mut app, "Packages");
    click_empty_hint(&mut app, "no packages folder", 6);
    assert_eq!(component_id(&app), "Settings");
}

#[test]
fn test_click_on_the_new_package_hint_of_an_empty_folder_opens_the_wizard() {
    let (mut app, _tmp) = make_flat_packages_app(&[]);
    open(&mut app, "Packages");
    click_empty_hint(&mut app, "no packages in", 8);
    assert_eq!(component_id(&app), "NewPackage");
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
            "NewPackage",
        ] {
            let (mut app, _tmp): (App, TempDir) = make_flat_packages_app(&["bash-env", "nvim"]);
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
    std::fs::write(tmp.path().join("dotfiles/test/.gitignore"), "aaa\nbbb\n").unwrap();
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
    let (mut app, _tmp) = make_flat_packages_app(&["git", "nvim"]);
    push_component(&mut app, "NewPackage");
    app.update_components();
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
fn test_click_on_the_unstow_row_twice_opens_the_browse_at_the_target() {
    let (mut app, tmp) = make_test_app();
    let buf = draw(&mut app, 80, 24);
    let (x, y) = find_text(&buf, "unstow").expect("the unstow row");
    click(&mut app, x, y);
    assert_eq!(component_id(&app), "MainMenu", "the first click selects");
    draw(&mut app, 80, 24);
    click(&mut app, x, y);
    assert_eq!(
        component_id(&app),
        "UnstowBrowse",
        "the second click opens it"
    );
    assert_eq!(browse(&app).browser.path, tmp.path());
}

#[test]
fn test_click_on_a_package_row_focuses_the_packages_then_a_second_click_toggles_it() {
    let (mut app, _tmp) = make_flat_membership_app("packages: [test]\n");
    open_editor(&mut app);
    // The cursor starts on shell; move to test so the first click on shell only moves there.
    app.component_dispatch(down_key());
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
    assert!(!editor(&app).draft().packages.contains(&"shell".to_string()));
    draw(&mut app, 80, 24);
    click(&mut app, x, y);
    assert!(editor(&app).draft().packages.contains(&"shell".to_string()));
}

#[test]
fn test_a_file_checkbox_click_marks_the_file_and_a_link_cannot_be_marked() {
    let (mut app, tmp) = make_test_app();
    std::fs::write(tmp.path().join("notes.txt"), "hi").unwrap();
    std::os::unix::fs::symlink("dotfiles/test/.bashrc", tmp.path().join(".bashrc")).unwrap();
    let icons = Icons::for_set(app.ctx.core.icons);
    push_component(&mut app, "NewPackage");
    app.update_components();
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

fn status(app: &App) -> &StatusComponent {
    app.component_stack
        .last()
        .and_then(|c| c.as_any().downcast_ref())
        .expect("status on top")
}

#[test]
fn test_click_on_the_delete_button_deletes_the_profile() {
    let (mut app, _tmp) = make_flat_membership_app("packages: [test]\n");
    open_profiles(&mut app);
    app.component_dispatch(key_char('x'));
    let buf = draw(&mut app, 80, 24);
    let (x, y) = find_text(&buf, "y delete").expect("the delete button");
    click(&mut app, x + 2, y);
    assert!(
        app.ctx.core.profiles.get("agent").is_none(),
        "the profile is gone"
    );
    assert!(profiles(&app).popup().is_none(), "the dialog closed");
}

#[test]
fn test_click_on_the_keep_button_closes_the_delete_dialog_and_keeps_the_profile() {
    let (mut app, _tmp) = make_flat_membership_app("packages: [test]\n");
    open_profiles(&mut app);
    app.component_dispatch(key_char('x'));
    let buf = draw(&mut app, 80, 24);
    let (x, y) = find_text(&buf, "n keep").expect("the keep button");
    click(&mut app, x + 2, y);
    assert!(profiles(&app).popup().is_none());
    assert!(app.ctx.core.profiles.get("agent").is_some());
}

#[test]
fn test_click_on_the_list_behind_the_delete_dialog_does_not_enter_the_profile() {
    let (mut app, _tmp) = make_flat_membership_app("packages: [test]\n");
    open_profiles(&mut app);
    app.component_dispatch(key_char('x'));
    let buf = draw(&mut app, 80, 24);
    let (x, y) = find_text(&buf, "agent").expect("the profile row");
    click(&mut app, x, y);
    assert!(
        matches!(profiles(&app).popup(), Some(Popup::Delete { .. })),
        "the dialog is still open"
    );
    assert_eq!(component_id(&app), "Profiles", "no review was pushed");
}

#[test]
fn test_click_on_the_create_button_makes_the_profile() {
    let (mut app, _tmp) = make_empty_app();
    open_profiles(&mut app);
    app.component_dispatch(key_char('+'));
    for c in "work".chars() {
        app.component_dispatch(key_char(c));
    }
    let buf = draw(&mut app, 80, 24);
    let (x, y) = find_text(&buf, "Enter create").expect("the create button");
    click(&mut app, x + 1, y);
    assert!(
        app.ctx.core.profiles.get("work").is_some(),
        "the profile exists"
    );
    assert!(profiles(&app).popup().is_none());
}

#[test]
fn test_click_on_the_stow_local_ignore_tab_switches_the_file() {
    let (mut app, tmp) = make_test_app();
    std::fs::write(tmp.path().join("dotfiles/test/.gitignore"), "aaa\n").unwrap();
    open(&mut app, "Packages");
    app.component_dispatch(down_key());
    app.component_dispatch(key_char('i'));
    let buf = draw(&mut app, 80, 24);
    assert_eq!(ignore_editor(&app).active(), 0);
    let (x, y) = find_text(&buf, ".stow-local-ignore").expect("the tab");
    click(&mut app, x, y);
    assert_eq!(ignore_editor(&app).active(), 1);
}

#[test]
fn test_click_on_the_discard_button_of_the_ignore_leave_dialog_leaves_the_editor() {
    let (mut app, tmp) = make_test_app();
    std::fs::write(tmp.path().join("dotfiles/test/.gitignore"), "aaa\n").unwrap();
    open(&mut app, "Packages");
    app.component_dispatch(down_key());
    app.component_dispatch(key_char('i'));
    app.component_dispatch(key_char('+'));
    for c in "bbb".chars() {
        app.component_dispatch(key_char(c));
    }
    app.component_dispatch(enter_key());
    app.component_dispatch(esc_key());
    let buf = draw(&mut app, 80, 24);
    let (x, y) = find_text(&buf, "n discard").expect("the discard button");
    click(&mut app, x + 2, y);
    assert_ne!(component_id(&app), "IgnoreEdit", "the editor closed");
}

#[test]
fn test_click_on_the_leave_button_of_the_wizard_dialog_leaves_the_wizard() {
    let (mut app, _tmp) = make_flat_packages_app(&["git", "nvim"]);
    open(&mut app, "NewPackage");
    for c in "tmux".chars() {
        app.component_dispatch(key_char(c));
    }
    app.component_dispatch(esc_key());
    let buf = draw(&mut app, 80, 24);
    let (x, y) = find_text(&buf, "Enter leave").expect("the leave button");
    click(&mut app, x + 2, y);
    assert_ne!(component_id(&app), "NewPackage", "the wizard was left");
}

#[test]
fn test_click_on_a_done_step_in_the_bar_goes_back_to_it() {
    let (mut app, _tmp) = make_flat_packages_app(&["git", "nvim"]);
    open(&mut app, "NewPackage");
    for c in "tmux".chars() {
        app.component_dispatch(key_char(c));
    }
    // The name, then the empty source: the wizard is on step 3, ignore.
    app.component_dispatch(enter_key());
    app.component_dispatch(enter_key());
    let buf = draw(&mut app, 80, 24);
    assert!(
        find_text(&buf, "[3 ignore]").is_some(),
        "on the ignore step"
    );
    // The done step's check is the icon set's glyph, so the step is found by its label.
    let (x, y) = find_text(&buf, "name \u{203a}").expect("the done name step");
    click(&mut app, x, y);
    let buf = draw(&mut app, 80, 24);
    assert!(
        find_text(&buf, "[1 name]").is_some(),
        "back on the name step"
    );
}

#[test]
fn test_click_on_the_cancel_button_of_a_review_goes_back() {
    let (mut app, _tmp) = make_flat_packages_app(&["bash-env", "nvim"]);
    open(&mut app, "Packages");
    app.component_dispatch(down_key());
    app.component_dispatch(enter_key());
    assert_eq!(component_id(&app), "Confirm", "the review is open");
    let buf = draw(&mut app, 80, 24);
    let (x, y) = find_text(&buf, "n cancel").expect("the cancel button");
    click(&mut app, x + 2, y);
    assert_ne!(component_id(&app), "Confirm", "the review closed");
}

/// Opens the profile dropdown on the status screen, which must sit at `width` x `height`.
fn open_status_dropdown(app: &mut App, width: u16, height: u16) {
    let buf = draw(app, width, height);
    let icons = Icons::for_set(app.ctx.core.icons);
    let chip = format!("all {}", icons.caret);
    let (x, y) = find_text(&buf, &chip).expect("the profile chip");
    click(app, x, y);
    let buf = draw(app, width, height);
    assert!(
        find_text(&buf, "all packages").is_some(),
        "the dropdown is open"
    );
}

#[test]
fn test_click_on_the_profile_chip_opens_the_dropdown_and_a_second_click_picks_a_profile() {
    let (mut app, _tmp) = make_flat_membership_app("packages: [test]\n");
    open(&mut app, "Status");
    open_status_dropdown(&mut app, 80, 24);
    // The first click on an option moves the cursor there; the second picks it.
    let buf = draw(&mut app, 80, 24);
    let (x, y) = find_text(&buf, "agent").expect("the profile option");
    click(&mut app, x, y);
    assert_eq!(
        status(&app).profile(),
        None,
        "one click only moves the cursor"
    );
    let buf = draw(&mut app, 80, 24);
    let (x, y) = find_text(&buf, "agent").expect("the profile option");
    click(&mut app, x, y);
    assert_eq!(status(&app).profile(), Some("agent"));
}

#[test]
fn test_wheel_over_the_dropdown_moves_its_cursor_and_enter_picks_the_option() {
    let (mut app, _tmp) = make_flat_membership_app("packages: [test]\n");
    open(&mut app, "Status");
    open_status_dropdown(&mut app, 80, 24);
    let buf = draw(&mut app, 80, 24);
    let (x, y) = find_text(&buf, "all packages").expect("the first option");
    wheel(&mut app, x, y, true);
    app.component_dispatch(enter_key());
    assert_eq!(
        status(&app).profile(),
        Some("agent"),
        "the wheel moved to agent"
    );
}

#[test]
fn test_help_mouse_topic_says_shift_selects_text() {
    let (mut app, _tmp) = make_test_app();
    open(&mut app, "Help");
    let buf = draw(&mut app, 80, 24);
    let (x, y) = find_text(&buf, "mouse").expect("the mouse topic");
    click(&mut app, x, y);
    app.update_components();
    let buf = draw(&mut app, 80, 24);
    assert!(find_text(&buf, "Shift").is_some(), "the topic names Shift");
}

#[test]
fn test_the_dialogs_and_bars_this_batch_touched_render_at_120x40_and_200x50() {
    for (width, height) in [(120, 40), (200, 50)] {
        let (mut app, _tmp) = make_flat_membership_app("packages: [test]\n");
        open_profiles(&mut app);
        app.component_dispatch(key_char('x'));
        draw(&mut app, width, height);

        let (mut app, _tmp) = make_flat_membership_app("packages: [test]\n");
        open(&mut app, "Status");
        open_status_dropdown(&mut app, width, height);

        let (mut app, _tmp) = make_flat_packages_app(&["bash-env", "nvim"]);
        open(&mut app, "Packages");
        app.component_dispatch(down_key());
        app.component_dispatch(enter_key());
        draw(&mut app, width, height);

        let (mut app, _tmp) = make_flat_packages_app(&["git", "nvim"]);
        open(&mut app, "NewPackage");
        draw(&mut app, width, height);
    }
}

/// The settings theme picker, opened from its row: the first click selects the row, the
/// second opens it.
fn open_theme_picker(app: &mut App) {
    open(app, "Settings");
    let buf = draw(app, 80, 24);
    let (x, y) = find_text(&buf, "theme").expect("the theme row");
    click(app, x, y);
    click(app, x, y);
    assert!(
        matches!(settings(app).overlays.last(), Some(Overlay::Theme { .. })),
        "the theme picker is open"
    );
}

#[test]
fn test_click_on_the_keep_button_of_the_theme_picker_closes_it() {
    let (mut app, _tmp) = make_test_app();
    open_theme_picker(&mut app);
    let buf = draw(&mut app, 80, 24);
    let (x, y) = find_text(&buf, "Enter keep").expect("the keep button");
    click(&mut app, x + 2, y);
    assert!(settings(&app).overlays.is_empty(), "the picker closed");
}

#[test]
fn test_click_on_an_option_of_the_theme_picker_previews_it_and_a_second_click_keeps_it() {
    let (mut app, _tmp) = make_test_app();
    open_theme_picker(&mut app);
    let buf = draw(&mut app, 80, 24);
    let name = Theme::ALL[2].name();
    let (x, y) = find_text(&buf, name).expect("the third theme");
    click(&mut app, x, y);
    assert_eq!(
        settings(&app).pending().map(|p| p.theme),
        Some(Theme::ALL[2]),
        "the click previews the theme"
    );
    assert!(
        !settings(&app).overlays.is_empty(),
        "one click keeps the picker open"
    );
    let buf = draw(&mut app, 80, 24);
    let (x, y) = find_text(&buf, name).expect("the third theme");
    click(&mut app, x, y);
    assert!(
        settings(&app).overlays.is_empty(),
        "the second click keeps it"
    );
}

#[test]
fn test_wheel_over_the_theme_picker_moves_its_cursor_one_row() {
    let (mut app, _tmp) = make_test_app();
    open_theme_picker(&mut app);
    let buf = draw(&mut app, 80, 24);
    let (x, y) = find_text(&buf, Theme::ALL[0].name()).expect("the first theme");
    wheel(&mut app, x, y, true);
    assert_eq!(
        settings(&app).pending().map(|p| p.theme),
        Some(Theme::ALL[1]),
        "one notch previews the next theme"
    );
}

#[test]
fn test_click_behind_the_theme_picker_changes_nothing() {
    let (mut app, _tmp) = make_test_app();
    open_theme_picker(&mut app);
    let theme = settings(&app).pending().map(|p| p.theme);
    let buf = draw(&mut app, 80, 24);
    // The theme row is on the list, behind the picker.
    let (x, y) = find_text(&buf, "theme").expect("the theme row");
    click(&mut app, x, y);
    assert!(
        matches!(settings(&app).overlays.last(), Some(Overlay::Theme { .. })),
        "the picker is still open"
    );
    assert_eq!(settings(&app).pending().map(|p| p.theme), theme);
}

#[test]
fn test_click_on_the_discard_button_of_the_settings_leave_dialog_leaves_settings() {
    let (mut app, _tmp) = make_test_app();
    open(&mut app, "Settings");
    app.component_dispatch(key_char('t'));
    app.component_dispatch(esc_key());
    assert!(
        matches!(settings(&app).overlays.last(), Some(Overlay::Leave)),
        "the leave dialog is open"
    );
    let buf = draw(&mut app, 80, 24);
    let (x, y) = find_text(&buf, "n discard").expect("the discard button");
    click(&mut app, x + 2, y);
    assert_ne!(component_id(&app), "Settings", "settings was left");
}

/// The membership dialog of the only package, `test`, opened from the packages screen.
fn open_membership(app: &mut App) {
    open(app, "Packages");
    app.component_dispatch(key_char('p'));
    assert!(
        packages(app).popup().is_some(),
        "the membership dialog is open"
    );
}

#[test]
fn test_click_on_a_profile_row_then_save_writes_the_membership() {
    let (mut app, _tmp) = make_flat_membership_app("packages: []\n");
    open_membership(&mut app);
    let buf = draw(&mut app, 80, 24);
    let (x, y) = find_text(&buf, "agent").expect("the profile row");
    click(&mut app, x, y);
    let buf = draw(&mut app, 80, 24);
    let (x, y) = find_text(&buf, "Enter save").expect("the save button");
    click(&mut app, x + 2, y);
    assert!(packages(&app).popup().is_none(), "the dialog closed");
    let agent = app.ctx.core.profiles.get("agent").expect("the profile");
    assert!(
        agent.packages.iter().any(|p| p == "shell"),
        "the profile lists the package under the cursor: {:?}",
        agent.packages
    );
}

#[test]
fn test_click_on_the_cancel_button_of_the_membership_dialog_changes_nothing() {
    let (mut app, _tmp) = make_flat_membership_app("packages: []\n");
    open_membership(&mut app);
    let buf = draw(&mut app, 80, 24);
    let (x, y) = find_text(&buf, "agent").expect("the profile row");
    click(&mut app, x, y);
    let buf = draw(&mut app, 80, 24);
    let (x, y) = find_text(&buf, "Esc cancel").expect("the cancel button");
    click(&mut app, x + 2, y);
    assert!(packages(&app).popup().is_none(), "the dialog closed");
    let agent = app.ctx.core.profiles.get("agent").expect("the profile");
    assert!(agent.packages.is_empty(), "nothing was saved");
}

#[test]
fn test_a_click_behind_the_membership_dialog_changes_nothing() {
    let (mut app, _tmp) = make_flat_membership_app("packages: []\n");
    open_membership(&mut app);
    let cursor = packages(&app).list.cursor;
    let buf = draw(&mut app, 80, 24);
    let (x, y) = find_text(&buf, "test").expect("the package row behind the dialog");
    click(&mut app, x, y);
    assert!(packages(&app).popup().is_some(), "the dialog is still open");
    assert_eq!(packages(&app).list.cursor, cursor, "the list did not move");
}

#[test]
fn test_wheel_over_the_membership_dialog_moves_its_cursor() {
    let (mut app, tmp) = make_flat_membership_app("packages: []\n");
    std::fs::write(tmp.path().join("profiles/alpha.yml"), "packages: []\n").unwrap();
    app.ctx.core.profiles = ProfileSet::load(&tmp.path().join("profiles")).unwrap();
    open_membership(&mut app);
    let buf = draw(&mut app, 80, 24);
    let (x, y) = find_text(&buf, "alpha").expect("the second profile row");
    wheel(&mut app, x, y, true);
    assert_eq!(
        packages(&app).popup().map(|m| m.cursor),
        Some(1),
        "one notch moves to the next profile"
    );
}

/// The profile editor with `test` marked, left with Esc so its leave dialog is open.
fn open_leave_dialog_on_the_profile_edit(app: &mut App) {
    open_editor(app);
    // The cursor starts on `shell`, so Space marks it.
    app.component_dispatch(space_key());
    app.component_dispatch(esc_key());
    assert!(
        find_text(&draw(app, 80, 24), "unsaved changes").is_some(),
        "the leave dialog is open"
    );
}

#[test]
fn test_click_on_the_save_button_of_the_profile_edit_leave_dialog_writes_the_profile() {
    let (mut app, _tmp) = make_flat_membership_app("packages: [test]\n");
    open_leave_dialog_on_the_profile_edit(&mut app);
    let buf = draw(&mut app, 80, 24);
    let (x, y) = find_text(&buf, "y save").expect("the save button");
    click(&mut app, x + 2, y);
    assert_ne!(component_id(&app), "ProfileEdit", "the editor closed");
    let agent = app.ctx.core.profiles.get("agent").expect("the profile");
    assert!(
        agent.packages.iter().any(|p| p == "shell"),
        "the mark was saved: {:?}",
        agent.packages
    );
    assert!(
        agent.packages.iter().any(|p| p == "test"),
        "the package it was saved with is kept: {:?}",
        agent.packages
    );
}

#[test]
fn test_click_on_the_discard_button_of_the_profile_edit_leave_dialog_keeps_the_profile() {
    let (mut app, _tmp) = make_flat_membership_app("packages: [test]\n");
    open_leave_dialog_on_the_profile_edit(&mut app);
    let buf = draw(&mut app, 80, 24);
    let (x, y) = find_text(&buf, "n discard").expect("the discard button");
    click(&mut app, x + 2, y);
    assert_ne!(component_id(&app), "ProfileEdit", "the editor closed");
    let agent = app.ctx.core.profiles.get("agent").expect("the profile");
    assert_eq!(
        agent.packages,
        vec!["test".to_string()],
        "nothing was saved"
    );
}

#[test]
fn test_a_click_off_the_profile_edit_leave_dialog_changes_nothing() {
    let (mut app, _tmp) = make_flat_membership_app("packages: [test]\n");
    open_leave_dialog_on_the_profile_edit(&mut app);
    let buf = draw(&mut app, 80, 24);
    let (x, y) = find_text(&buf, "agent").expect("a row behind the dialog");
    click(&mut app, x, y);
    assert_eq!(
        component_id(&app),
        "ProfileEdit",
        "the editor is still open"
    );
    let buf = draw(&mut app, 80, 24);
    assert!(
        find_text(&buf, "unsaved changes").is_some(),
        "the leave dialog is still open"
    );
}

#[test]
fn test_wheel_over_a_long_profile_detail_moves_its_sections() {
    let packages: Vec<String> = (0..14).map(|i| format!("pkg{i:02}")).collect();
    let yaml = format!("packages: [{}]\n", packages.join(", "));
    let (mut app, _tmp) = make_flat_membership_app(&yaml);
    open_profiles(&mut app);
    let before = draw(&mut app, 80, 24);
    let (x, y) = find_text(&before, "more").expect("the sections overflow");
    wheel(&mut app, x, y, true);
    let after = draw(&mut app, 80, 24);
    assert_ne!(before, after, "the sections moved");
    wheel(&mut app, x, y, false);
    let back = draw(&mut app, 80, 24);
    assert_eq!(before, back, "a notch back restores the top");
}

#[test]
fn test_click_on_the_cancel_button_of_the_packages_folder_picker_closes_it() {
    let (mut app, _tmp) = make_test_app();
    open(&mut app, "Settings");
    let buf = draw(&mut app, 80, 24);
    // `packages ` with its padding: the detail of `action` also says "packages."
    let (x, y) = find_text(&buf, "packages ").expect("the packages row");
    click(&mut app, x, y);
    app.component_dispatch(enter_key());
    assert!(
        matches!(settings(&app).overlays.last(), Some(Overlay::Browse { .. })),
        "the folder picker is open"
    );
    let buf = draw(&mut app, 80, 24);
    let (x, y) = find_text(&buf, "Esc cancel").expect("the cancel button");
    click(&mut app, x + 2, y);
    assert!(settings(&app).overlays.is_empty(), "the dialog closed");
}
