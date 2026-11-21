use andre::app::App;
use andre::components::confirm::ConfirmComponent;
use andre::components::menu::MenuItem;
use andre::components::packages::PackagesComponent;
use andre::components::profiles::edit::ProfileEditComponent;
use andre::components::profiles::{NameCheck, Popup, ProfilesComponent};
use andre::components::status::StatusComponent;
use andre::components::InputMode;
use andre::ui::package_list::Mark;
use andre_core::profile::ProfileSet;
use andre_core::StowAction;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::{backend::TestBackend, Terminal};

mod helpers;
use helpers::*;

fn menu(app: &App) -> &andre::components::MainMenuComponent {
    app.component_stack
        .last()
        .unwrap()
        .as_any()
        .downcast_ref()
        .unwrap()
}

fn packages(app: &App) -> &PackagesComponent {
    app.component_stack
        .last()
        .unwrap()
        .as_any()
        .downcast_ref()
        .unwrap()
}

fn settings_mut(app: &mut App) -> &mut andre::components::settings::SettingsComponent {
    app.component_stack
        .last_mut()
        .unwrap()
        .as_any_mut()
        .downcast_mut()
        .unwrap()
}

fn settings(app: &App) -> &andre::components::settings::SettingsComponent {
    app.component_stack
        .last()
        .unwrap()
        .as_any()
        .downcast_ref()
        .unwrap()
}

// --- Menu Navigation ---

#[test]
fn test_menu_up_wraps() {
    let (mut app, _) = make_test_app();
    assert_eq!(menu(&app).list.cursor, 0);
    app.component_dispatch(up_key());
    assert_eq!(menu(&app).list.cursor, 7);
}

#[test]
fn test_menu_down_wraps() {
    let (mut app, _) = make_test_app();
    for _ in 0..8 {
        app.component_dispatch(down_key());
    }
    assert_eq!(menu(&app).list.cursor, 0);
}

#[test]
fn test_menu_j_down() {
    let (mut app, _) = make_test_app();
    app.component_dispatch(key_char('j'));
    assert_eq!(menu(&app).list.cursor, 1);
}

#[test]
fn test_menu_k_up() {
    let (mut app, _) = make_test_app();
    app.component_dispatch(key_char('k'));
    assert_eq!(menu(&app).list.cursor, 7);
}

// --- Screen Transitions ---

#[test]
fn test_enter_on_profiles_goes_to_profiles() {
    let (mut app, _) = make_test_app();
    app.component_dispatch(enter_key());
    assert_eq!(component_id(&app), "Profiles");
}

#[test]
fn test_enter_on_packages_goes_to_packages() {
    let (mut app, _) = make_test_app();
    app.component_dispatch(down_key());
    app.component_dispatch(enter_key());
    assert_eq!(component_id(&app), "Packages");
}

#[test]
fn test_enter_on_new_package_opens_the_wizard() {
    let (mut app, _) = make_two_group_app();
    app.component_dispatch(down_key());
    app.component_dispatch(down_key());
    app.component_dispatch(enter_key());
    assert_eq!(component_id(&app), "NewPackage");
}

#[test]
fn test_enter_on_unstow_goes_to_unstow_target() {
    let (mut app, _) = make_two_group_app();
    for _ in 0..3 {
        app.component_dispatch(down_key());
    }
    app.component_dispatch(enter_key());
    assert_eq!(component_id(&app), "UnstowTargetSelect");
}

#[test]
fn test_enter_on_status_goes_to_status() {
    let (mut app, _) = make_test_app();
    for _ in 0..4 {
        app.component_dispatch(down_key());
    }
    app.component_dispatch(enter_key());
    assert_eq!(component_id(&app), "Status");
}

#[test]
fn test_enter_on_config_goes_to_settings() {
    let (mut app, _) = make_test_app();
    for _ in 0..5 {
        app.component_dispatch(down_key());
    }
    app.component_dispatch(enter_key());
    assert_eq!(component_id(&app), "Settings");
}

#[test]
fn test_enter_on_help_goes_to_help() {
    let (mut app, _) = make_test_app();
    for _ in 0..6 {
        app.component_dispatch(down_key());
    }
    app.component_dispatch(enter_key());
    assert_eq!(component_id(&app), "Help");
}

#[test]
fn test_enter_on_quit_sets_quit() {
    let (mut app, _) = make_test_app();
    for _ in 0..7 {
        app.component_dispatch(down_key());
    }
    app.component_dispatch(enter_key());
    assert!(app.quitting);
}

#[test]
fn test_digit_key_jumps_and_activates() {
    // Each digit 1..=8 should jump straight to the Nth menu item and
    // produce the same transition as Enter on that index. Mirrors the
    // `test_enter_on_*` block above but exercises the digit-key path.
    let cases: [(char, &str); 8] = [
        ('1', "Profiles"),
        ('2', "Packages"),
        ('3', "NewPackage"),
        ('4', "UnstowBrowse"), // one group: unstow goes straight to its browse screen
        ('5', "Status"),
        ('6', "Settings"),
        ('7', "Help"),
        ('8', "MainMenu"), // Quit pops to the still-mounted MainMenu via quitting
    ];
    for (digit, expected) in cases {
        let (mut app, _) = make_test_app();
        app.component_dispatch(key_char(digit));
        // '8' quits; on quit the top remains MainMenu and `quitting` is true.
        if digit == '8' {
            assert!(app.quitting, "digit '8' should set quitting");
            assert_eq!(component_id(&app), expected);
        } else {
            assert_eq!(component_id(&app), expected, "digit '{digit}'");
        }
    }
}

#[test]
fn test_digit_key_out_of_range_is_ignored() {
    // '0' and '9' fall outside the 8-item menu and must not panic
    // or change the highlighted index.
    for c in ['0', '9'] {
        let (mut app, _) = make_test_app();
        let before = menu(&app).list.cursor;
        app.component_dispatch(key_char(c));
        assert_eq!(
            menu(&app).list.cursor,
            before,
            "digit '{c}' should not move the cursor"
        );
        assert_eq!(
            component_id(&app),
            "MainMenu",
            "digit '{c}' should not push a screen"
        );
        assert!(!app.quitting, "digit '{c}' should not quit");
    }
}

#[test]
fn test_digit_key_updates_highlight_for_next_enter() {
    // Pressing a digit must also move the highlight so a subsequent Enter
    // confirms the same item. (Activate-on-press already gives feedback via
    // the transition; this guards the index update itself.)
    let (mut app, _) = make_test_app();
    app.component_dispatch(key_char('5'));
    // '5' would push Status — pop back, then press Enter and confirm Status again.
    app.component_dispatch(esc_key());
    app.component_dispatch(enter_key());
    assert_eq!(component_id(&app), "Status");
}

// --- Esc Back-Navigation ---

#[test]
fn test_esc_packages_to_main_menu() {
    let (mut app, _) = make_test_app();
    push_component(&mut app, "Packages");
    app.component_dispatch(esc_key());
    assert_eq!(component_id(&app), "MainMenu");
}

#[test]
fn test_esc_from_screens_goes_back() {
    let (mut app, _) = make_test_app();
    push_component(&mut app, "Packages");
    app.component_dispatch(esc_key());
    assert_eq!(component_id(&app), "MainMenu");

    push_component(&mut app, "Status");
    app.component_dispatch(esc_key());
    assert_eq!(component_id(&app), "MainMenu");

    push_component(&mut app, "Help");
    app.component_dispatch(esc_key());
    assert_eq!(component_id(&app), "MainMenu");
}

#[test]
fn test_esc_status_to_main_menu() {
    let (mut app, _) = make_test_app();
    push_component(&mut app, "Status");
    app.component_dispatch(esc_key());
    assert_eq!(component_id(&app), "MainMenu");
}

#[test]
fn test_status_profile_dropdown_takes_esc_before_the_screen_does() {
    let (mut app, _) = make_test_app();
    push_component(&mut app, "Status");
    app.component_dispatch(key_char('f'));
    assert_eq!(component_id(&app), "Status");

    app.component_dispatch(esc_key());
    assert_eq!(component_id(&app), "Status", "Esc closes the dropdown only");
    app.component_dispatch(esc_key());
    assert_eq!(component_id(&app), "MainMenu");
}

#[test]
fn test_esc_help_to_main_menu() {
    let (mut app, _) = make_test_app();
    push_component(&mut app, "Help");
    app.component_dispatch(esc_key());
    assert_eq!(component_id(&app), "MainMenu");
}

#[test]
fn test_esc_config_to_main_menu() {
    let (mut app, _) = make_test_app();
    push_component(&mut app, "Settings");
    app.component_dispatch(esc_key());
    assert_eq!(component_id(&app), "MainMenu");
}

// --- Global Flags ---

#[test]
fn test_v_cycles_verbosity() {
    let (mut app, _) = make_test_app();
    app.ctx.core.flags.verbosity = 0;
    app.component_dispatch(key_char('v'));
    assert_eq!(app.ctx.core.flags.verbosity, 1);
    app.component_dispatch(key_char('v'));
    assert_eq!(app.ctx.core.flags.verbosity, 2);
    for _ in 0..4 {
        app.component_dispatch(key_char('v'));
    }
    assert_eq!(app.ctx.core.flags.verbosity, 0);
}

#[test]
fn test_d_toggles_dry_run() {
    let (mut app, _) = make_test_app();
    app.ctx.core.flags.dry_run = false;
    app.component_dispatch(key_char('d'));
    assert!(app.ctx.core.flags.dry_run);
    app.component_dispatch(key_char('d'));
    assert!(!app.ctx.core.flags.dry_run);
}

#[test]
fn test_n_toggles_no_folding() {
    let (mut app, _) = make_test_app();
    app.component_dispatch(key_char('n'));
    assert!(app.ctx.core.flags.no_folding);
    app.component_dispatch(key_char('n'));
    assert!(!app.ctx.core.flags.no_folding);
}

#[test]
fn test_a_toggles_adopt() {
    let (mut app, _) = make_test_app();
    app.component_dispatch(key_char('a'));
    assert!(app.ctx.core.flags.adopt);
    app.component_dispatch(key_char('a'));
    assert!(!app.ctx.core.flags.adopt);
}

#[test]
fn test_o_toggles_dotfiles() {
    let (mut app, _) = make_test_app();
    app.component_dispatch(key_char('o'));
    assert!(app.ctx.core.flags.dotfiles);
    app.component_dispatch(key_char('o'));
    assert!(!app.ctx.core.flags.dotfiles);
}

#[test]
fn test_t_cycles_theme() {
    let (mut app, _) = make_test_app();
    let initial = app.ctx.core.theme;
    app.component_dispatch(key_char('t'));
    assert_ne!(app.ctx.core.theme, initial);
}

#[test]
fn test_t_cycles_all_eight_themes_and_wraps() {
    let (mut app, _) = make_test_app();
    let start = app.ctx.core.theme;
    for _ in 0..8 {
        app.component_dispatch(key_char('t'));
    }
    assert_eq!(app.ctx.core.theme, start);
}

// --- Packages browser ---

/// Opens the packages browser from the menu and runs one update, as the main loop does.
fn open_packages(app: &mut App) {
    app.component_dispatch(key_char('2'));
    app.update_components();
}

fn key_left() -> KeyEvent {
    KeyEvent::new(KeyCode::Left, KeyModifiers::NONE)
}

fn key_right() -> KeyEvent {
    KeyEvent::new(KeyCode::Right, KeyModifiers::NONE)
}

/// A flat app with three packages, in name order: `bash-env`, `nvim`, `tmux`.
fn make_app_with_three_packages() -> (App, tempfile::TempDir) {
    make_flat_packages_app(&["tmux", "nvim", "bash-env"])
}

/// The packages the confirm screen runs, by name, sorted.
fn confirmed(app: &App) -> Vec<String> {
    let review: &ConfirmComponent = top_component(app);
    let mut packages = review.selected_packages.clone();
    packages.sort();
    packages
}

#[test]
fn test_space_marks_the_cursor_package_and_again_unmarks_it() {
    let (mut app, _tmp) = make_app_with_three_packages();
    open_packages(&mut app);
    // The cursor starts on the first package in name order.
    assert!(packages(&app).marked().is_empty());
    app.component_dispatch(space_key());
    assert_eq!(packages(&app).marked(), ["bash-env"]);
    app.component_dispatch(space_key());
    assert!(packages(&app).marked().is_empty());
}

#[test]
fn test_space_on_a_package_marks_only_that_package() {
    let (mut app, _tmp) = make_app_with_three_packages();
    open_packages(&mut app);
    app.component_dispatch(down_key()); // nvim
    app.component_dispatch(space_key());
    assert_eq!(packages(&app).marked(), ["nvim"]);
    app.component_dispatch(down_key()); // tmux
    app.component_dispatch(space_key());
    assert_eq!(packages(&app).marked(), ["nvim", "tmux"]);
}

#[test]
fn test_star_marks_every_package_then_unmarks_them_all() {
    let (mut app, _tmp) = make_app_with_three_packages();
    open_packages(&mut app);
    app.component_dispatch(key_char('*'));
    assert_eq!(packages(&app).marked(), ["bash-env", "nvim", "tmux"]);
    app.component_dispatch(key_char('*'));
    assert!(packages(&app).marked().is_empty());
}

#[test]
fn test_star_with_some_marked_marks_the_rest_and_keeps_the_cursor() {
    let (mut app, _tmp) = make_app_with_three_packages();
    open_packages(&mut app);
    app.component_dispatch(space_key()); // bash-env
    app.component_dispatch(key_char('*'));
    assert_eq!(packages(&app).marked(), ["bash-env", "nvim", "tmux"]);
    assert_eq!(packages(&app).list.cursor, 0, "the cursor stays put");
}

#[test]
fn test_navigation_wraps_through_the_rows() {
    let (mut app, _tmp) = make_app_with_three_packages();
    open_packages(&mut app);
    for _ in 0..packages(&app).rows().len() {
        app.component_dispatch(down_key());
    }
    assert_eq!(packages(&app).list.cursor, 0);
    app.component_dispatch(up_key());
    assert_eq!(packages(&app).list.cursor, packages(&app).rows().len() - 1);
}

#[test]
fn test_enter_runs_the_marked_packages_through_the_confirm_screen() {
    let (mut app, _tmp) = make_app_with_three_packages();
    open_packages(&mut app);
    app.component_dispatch(down_key()); // nvim
    app.component_dispatch(space_key());
    app.component_dispatch(down_key()); // tmux, unmarked: the cursor does not pick what runs
    app.component_dispatch(enter_key());
    assert_eq!(component_id(&app), "Confirm");
    assert_eq!(confirmed(&app), ["nvim"]);
    // Esc returns to the browser, with the mark still there.
    app.component_dispatch(esc_key());
    assert_eq!(component_id(&app), "Packages");
    assert_eq!(packages(&app).marked().len(), 1);
}

#[test]
fn test_enter_on_a_package_with_no_marks_runs_that_package() {
    let (mut app, _tmp) = make_app_with_three_packages();
    open_packages(&mut app);
    app.component_dispatch(down_key());
    app.component_dispatch(down_key()); // tmux
    app.component_dispatch(enter_key());
    assert_eq!(component_id(&app), "Confirm");
    assert_eq!(confirmed(&app), ["tmux"]);
}

#[test]
fn test_keys_on_an_empty_folder_change_nothing_and_enter_runs_nothing() {
    let (mut app, _tmp) = make_flat_packages_app(&[]);
    open_packages(&mut app);
    app.component_dispatch(enter_key());
    app.component_dispatch(key_char('*'));
    app.component_dispatch(space_key());
    assert_eq!(component_id(&app), "Packages");
    assert!(packages(&app).marked().is_empty());
}

#[test]
fn test_six_on_a_packages_screen_with_no_folder_opens_settings() {
    let (mut app, _tmp) = make_empty_app();
    open_packages(&mut app);
    app.component_dispatch(key_char('6'));
    assert_eq!(component_id(&app), "Settings");
}

#[test]
fn test_three_on_an_empty_packages_folder_opens_the_new_package_wizard() {
    let (mut app, _tmp) = make_flat_packages_app(&[]);
    open_packages(&mut app);
    app.component_dispatch(key_char('3'));
    assert_eq!(component_id(&app), "NewPackage");
}

#[test]
fn test_fix_digits_do_nothing_while_the_packages_list_has_rows() {
    let (mut app, _tmp) = make_app_with_three_packages();
    open_packages(&mut app);
    app.component_dispatch(key_char('6'));
    app.component_dispatch(key_char('3'));
    assert_eq!(component_id(&app), "Packages");
    assert!(packages(&app).marked().is_empty());
}

#[test]
fn test_left_and_right_leave_the_cursor_where_it_is() {
    let (mut app, _tmp) = make_app_with_three_packages();
    open_packages(&mut app);
    app.component_dispatch(down_key()); // nvim
    app.component_dispatch(key_left());
    app.component_dispatch(key_right());
    assert_eq!(packages(&app).list.cursor, 1);
    assert_eq!(packages(&app).rows().len(), 3, "nothing folds");
}

#[test]
fn test_i_opens_the_ignore_editor_on_the_cursor_package() {
    let (mut app, _tmp) = make_app_with_three_packages();
    open_packages(&mut app);
    app.component_dispatch(down_key()); // nvim
    app.component_dispatch(key_char('i'));
    assert_eq!(component_id(&app), "IgnoreEdit");
}

/// A flat app with the package `test`, and the profile files written.
fn make_flat_app_with_profiles(profiles: &[(&str, &str)]) -> (App, tempfile::TempDir) {
    let (mut app, tmp) = make_flat_packages_app(&["test"]);
    let dir = tmp.path().join("profiles");
    std::fs::create_dir_all(&dir).unwrap();
    for (name, yaml) in profiles {
        std::fs::write(dir.join(format!("{name}.yml")), yaml).unwrap();
    }
    app.ctx.core.profiles = ProfileSet::load(&dir).unwrap();
    (app, tmp)
}

#[test]
fn test_p_opens_the_membership_dialog_and_enter_saves_the_profile_file() {
    let (mut app, tmp) = make_flat_app_with_profiles(&[
        ("base", "description: Shared by every machine\n"),
        ("work", "packages: [test]\n"),
    ]);
    open_packages(&mut app);
    app.component_dispatch(key_char('p'));

    let popup = packages(&app).popup().expect("the dialog is open");
    assert_eq!(popup.package, "test");
    let listed: Vec<(&str, bool)> = popup
        .profiles
        .iter()
        .map(|row| (row.name.as_str(), row.now))
        .collect();
    assert_eq!(listed, vec![("base", false), ("work", true)]);
    assert_eq!(
        app.component_stack.last().unwrap().input_mode(),
        InputMode::Modal
    );

    app.component_dispatch(space_key()); // base is the cursor's row
    app.component_dispatch(enter_key());
    assert!(packages(&app).popup().is_none());
    assert_eq!(stored_packages(&tmp, "base"), vec!["test".to_string()]);
    assert_eq!(stored_packages(&tmp, "work"), vec!["test".to_string()]);
}

#[test]
fn test_esc_in_the_membership_dialog_keeps_the_profile_files() {
    let (mut app, tmp) = make_flat_app_with_profiles(&[("work", "packages: [test]\n")]);
    open_packages(&mut app);
    app.component_dispatch(key_char('p'));
    app.component_dispatch(space_key()); // now off in the dialog
    app.component_dispatch(esc_key());
    assert!(packages(&app).popup().is_none());
    assert_eq!(stored_packages(&tmp, "work"), vec!["test".to_string()]);
}

#[test]
fn test_enter_in_the_membership_dialog_removes_the_package_when_unticked() {
    let (mut app, tmp) = make_flat_app_with_profiles(&[("work", "packages: [test]\n")]);
    open_packages(&mut app);
    app.component_dispatch(key_char('p'));
    app.component_dispatch(space_key());
    app.component_dispatch(enter_key());
    assert!(stored_packages(&tmp, "work").is_empty());
}

#[test]
fn test_p_with_no_package_does_nothing() {
    let (mut app, _tmp) = make_flat_packages_app(&[]);
    open_packages(&mut app);
    app.component_dispatch(key_char('p'));
    assert!(packages(&app).popup().is_none());
}

#[test]
fn test_the_membership_dialog_is_modal() {
    let (mut app, _tmp) = make_flat_app_with_profiles(&[("work", "packages: []\n")]);
    open_packages(&mut app);
    app.component_dispatch(key_char('p'));
    assert!(packages(&app).popup().is_some());
    let verbosity = app.ctx.core.flags.verbosity;
    app.component_dispatch(key_char('v'));
    assert_eq!(
        app.ctx.core.flags.verbosity, verbosity,
        "dialog blocks globals"
    );
}

#[test]
fn test_global_keys_still_work_on_the_packages_screen() {
    let (mut app, _tmp) = make_app_with_three_packages();
    open_packages(&mut app);
    let initial_verbosity = app.ctx.core.flags.verbosity;
    app.component_dispatch(key_char('v'));
    assert_ne!(
        app.ctx.core.flags.verbosity, initial_verbosity,
        "global v should toggle verbosity on the packages screen"
    );
}

// --- Global Quit ---

#[test]
fn test_q_quits() {
    let (mut app, _) = make_test_app();
    app.component_dispatch(key_char('q'));
    assert!(app.quitting);
}

#[test]
fn test_ctrl_c_quits() {
    let (mut app, _) = make_test_app();
    app.component_dispatch(ctrl_c());
    assert!(app.quitting);
}

// --- Edge Cases ---

#[test]
fn test_empty_groups_handled_gracefully() {
    let (mut app, _) = make_empty_app();
    push_component(&mut app, "Packages");
    app.component_dispatch(up_key());
    app.component_dispatch(down_key());
    app.component_dispatch(space_key());
    app.component_dispatch(enter_key());
    assert_eq!(component_id(&app), "Packages");
}

#[test]
fn test_quit_screen_ignores_keys() {
    let (mut app, _) = make_test_app();
    app.component_dispatch(key_char('q'));
    assert!(app.quitting);
}

// --- Help Topics ---

fn help(app: &App) -> &andre::components::help::HelpComponent {
    app.component_stack
        .last()
        .unwrap()
        .as_any()
        .downcast_ref()
        .unwrap()
}

#[test]
fn test_help_topic_cursor_starts_on_keys_and_wraps_through_the_topics() {
    let (mut app, _) = make_test_app();
    push_component(&mut app, "Help");
    assert_eq!(help(&app).list.cursor, 0);
    for _ in 0..andre::components::help::Topic::ALL.len() {
        app.component_dispatch(down_key());
    }
    assert_eq!(help(&app).list.cursor, 0, "eight steps wrap back to keys");
    for _ in 0..3 {
        app.component_dispatch(down_key());
    }
    app.component_dispatch(up_key());
    assert_eq!(help(&app).list.cursor, 2);
}

#[test]
fn test_help_esc_goes_back() {
    let (mut app, _) = make_test_app();
    push_component(&mut app, "Help");
    app.component_dispatch(down_key());
    app.component_dispatch(esc_key());
    assert_eq!(component_id(&app), "MainMenu");
}

#[test]
fn test_help_enter_stays_on_help() {
    let (mut app, _) = make_test_app();
    push_component(&mut app, "Help");
    app.component_dispatch(enter_key());
    assert_eq!(
        component_id(&app),
        "Help",
        "Enter does nothing on help; Esc leaves"
    );
}

/// The wizard on its name step, which is text input: group `config` is the first row.
fn wizard_on_name_step(app: &mut App) {
    app.component_dispatch(down_key());
    app.component_dispatch(down_key());
    app.component_dispatch(enter_key());
    app.component_dispatch(enter_key());
    assert_eq!(
        app.component_stack.last().unwrap().input_mode(),
        andre::components::InputMode::TextInput
    );
}

// --- Q Quit Blocked During Text Input ---

#[test]
fn test_q_quit_blocked_during_new_package_name_step() {
    let (mut app, _) = make_two_group_app();
    wizard_on_name_step(&mut app);
    app.component_dispatch(key_char('q'));
    assert!(!app.quitting, "q should not quit during text input");
}

// --- Global Keys Blocked During Text Input ---

#[test]
fn test_global_keys_blocked_during_new_package_name_step() {
    let (mut app, _) = make_two_group_app();
    wizard_on_name_step(&mut app);
    let initial_theme = app.ctx.core.theme;
    let initial_verbosity = app.ctx.core.flags.verbosity;
    let initial_dry_run = app.ctx.core.flags.dry_run;
    let initial_no_folding = app.ctx.core.flags.no_folding;
    let initial_adopt = app.ctx.core.flags.adopt;
    let initial_dotfiles = app.ctx.core.flags.dotfiles;
    // All global shortcuts should be absorbed — nothing changes
    app.component_dispatch(key_char('t'));
    assert_eq!(app.ctx.core.theme, initial_theme);
    app.component_dispatch(key_char('v'));
    assert_eq!(app.ctx.core.flags.verbosity, initial_verbosity);
    app.component_dispatch(key_char('d'));
    assert_eq!(app.ctx.core.flags.dry_run, initial_dry_run);
    app.component_dispatch(key_char('n'));
    assert_eq!(app.ctx.core.flags.no_folding, initial_no_folding);
    app.component_dispatch(key_char('a'));
    assert_eq!(app.ctx.core.flags.adopt, initial_adopt);
    app.component_dispatch(key_char('o'));
    assert_eq!(app.ctx.core.flags.dotfiles, initial_dotfiles);
}

// --- InputMode Audit ---

#[test]
fn test_input_mode_audit_normal_components() {
    let (mut app, _) = make_test_app();
    use andre::components::InputMode;

    // All list/menu screens should be Normal
    for name in &["MainMenu", "Packages", "Status", "Help"] {
        push_component(&mut app, name);
        let mode = app.component_stack.last().unwrap().input_mode();
        assert_eq!(mode, InputMode::Normal, "{} should be Normal", name);
        app.component_stack.pop();
    }

    // Settings starts Normal
    push_component(&mut app, "Settings");
    assert_eq!(
        app.component_stack.last().unwrap().input_mode(),
        InputMode::Normal
    );
    app.component_stack.pop();
}

#[test]
fn test_input_mode_audit_text_input() {
    use andre::components::InputMode;
    let (mut app, _) = make_two_group_app();
    push_component(&mut app, "NewPackage");
    assert_eq!(
        app.component_stack.last().unwrap().input_mode(),
        InputMode::Normal,
        "the group step takes no text"
    );
    wizard_on_name_step(&mut app);
    assert_eq!(
        app.component_stack.last().unwrap().input_mode(),
        InputMode::TextInput
    );
}

// --- Toast (M04) ---

#[test]
fn test_toast_shows_and_auto_dismisses() {
    use std::time::{Duration, Instant};

    let (mut app, _) = make_test_app();
    app.ctx.show_toast("hello toast");
    assert_eq!(
        app.ctx.toast.as_ref().map(|t| t.message.as_str()),
        Some("hello toast")
    );

    // Force expiry then tick.
    if let Some(ref mut t) = app.ctx.toast {
        t.deadline = Instant::now() - Duration::from_secs(1);
    }
    app.update_components();
    assert!(app.ctx.toast.is_none(), "toast should clear after deadline");
}

#[test]
fn test_toast_does_not_capture_input() {
    let (mut app, _) = make_test_app();
    app.ctx.show_toast("visible");
    let before = app.ctx.core.flags.verbosity;
    app.component_dispatch(key_char('v'));
    assert_ne!(
        app.ctx.core.flags.verbosity, before,
        "global keys must still work while toast is visible"
    );
    assert!(app.ctx.toast.is_some(), "toast remains until deadline");
}

#[test]
fn test_config_save_shows_toast() {
    use andre::components::ToastKind;

    let (mut app, _tmp) = make_test_app();
    push_component(&mut app, "Settings");
    app.component_dispatch(key_char('v'));
    app.component_dispatch(key_char('w'));
    let toast = app.ctx.toast.as_ref().expect("toast after save");
    assert_eq!(toast.message, "Config saved");
    assert_eq!(toast.kind, ToastKind::Success);
}

#[test]
fn test_newer_toast_replaces_older() {
    let (mut app, _) = make_test_app();
    app.ctx.show_toast("first");
    app.ctx.show_toast("second");
    assert_eq!(
        app.ctx.toast.as_ref().map(|t| t.message.as_str()),
        Some("second")
    );
}

#[test]
fn test_empty_toast_is_noop() {
    let (mut app, _) = make_test_app();
    app.ctx.show_toast("   ");
    assert!(app.ctx.toast.is_none());
    app.ctx.show_toast("kept");
    app.ctx.show_toast("");
    assert_eq!(
        app.ctx.toast.as_ref().map(|t| t.message.as_str()),
        Some("kept"),
        "empty message must not clear or replace an existing toast"
    );
}

// --- File Browser Cursor Reset ---
// These are covered by unit tests in the component modules.

// --- App-owned execute drain (M05 AC-03) ---

#[test]
fn test_app_update_drains_execution_update_into_execute() {
    use andre::components::execute::ExecuteComponent;
    use andre::execute::ExecutionUpdate;

    let (mut app, _tmp) = make_test_app();
    let (tx, rx) = tokio::sync::mpsc::channel(4);
    app.component_stack
        .push(Box::new(ExecuteComponent::new(rx, 2, Vec::new())));

    // Toast is visible; its tick also runs inside update_components.
    app.ctx.show_toast_kind(
        "working",
        andre::components::ToastKind::Success,
        std::time::Duration::from_secs(0),
    );
    assert!(app.ctx.toast.is_some());

    tx.try_send(ExecutionUpdate::Progress { current: 1 })
        .unwrap();

    // App::update_components is the driver that drains ExecutionUpdate into
    // the active Execute component.
    app.update_components();

    let exec = app
        .component_stack
        .last()
        .unwrap()
        .as_any()
        .downcast_ref::<ExecuteComponent>()
        .unwrap();
    assert_eq!(
        exec.current, 1,
        "App::update_components must drain ExecutionUpdate into Execute"
    );
    assert!(
        app.ctx.toast.is_none(),
        "toast tick must still run alongside the execute drain in update_components"
    );
}

// --- Target selection with several groups (groups are listed by name) ---

fn top_component<T: 'static>(app: &App) -> &T {
    app.component_stack
        .last()
        .unwrap()
        .as_any()
        .downcast_ref()
        .unwrap()
}

#[test]
fn test_unstow_target_enter_opens_highlighted_group() {
    let (mut app, tmp) = make_two_group_app();
    for _ in 0..3 {
        app.component_dispatch(down_key());
    }
    app.component_dispatch(enter_key());
    assert_eq!(component_id(&app), "UnstowTargetSelect");

    // "config" sorts before "home", so it is the first row the user sees.
    app.component_dispatch(enter_key());
    let browse: &andre::components::unstow::UnstowBrowseComponent = top_component(&app);
    assert_eq!(browse.group_name, "config");
    assert_eq!(
        browse.browser.path,
        andre_core::path::resolve_path("~/.config", tmp.path(), tmp.path())
    );
}

#[test]
fn test_unstow_target_second_row_opens_second_group() {
    let (mut app, tmp) = make_two_group_app();
    for _ in 0..3 {
        app.component_dispatch(down_key());
    }
    app.component_dispatch(enter_key());
    app.component_dispatch(down_key());
    app.component_dispatch(enter_key());
    let browse: &andre::components::unstow::UnstowBrowseComponent = top_component(&app);
    assert_eq!(browse.group_name, "home");
    assert_eq!(
        browse.browser.path,
        andre_core::path::resolve_path("~", tmp.path(), tmp.path())
    );
}

#[test]
fn test_new_package_enter_on_a_group_opens_its_name_step() {
    let (mut app, _) = make_two_group_app();
    app.component_dispatch(down_key());
    app.component_dispatch(down_key());
    app.component_dispatch(enter_key());
    assert_eq!(component_id(&app), "NewPackage");

    app.component_dispatch(enter_key());
    assert_eq!(
        app.component_stack.last().unwrap().input_mode(),
        andre::components::InputMode::TextInput,
        "Enter on the first group (config) opens the name step"
    );
}

#[test]
fn test_new_package_second_group_row_opens_its_name_step() {
    let (mut app, _) = make_two_group_app();
    app.component_dispatch(down_key());
    app.component_dispatch(down_key());
    app.component_dispatch(enter_key());
    app.component_dispatch(down_key());
    app.component_dispatch(enter_key());
    assert_eq!(
        app.component_stack.last().unwrap().input_mode(),
        andre::components::InputMode::TextInput
    );
}

/// Save the open Settings screen (Save row) and reload the config file from disk.
/// The whole screen as text, one line per buffer row.
fn screen_text(app: &mut App) -> String {
    app.set_terminal_size(120, 24);
    let mut terminal = Terminal::new(TestBackend::new(120, 24)).unwrap();
    terminal.draw(|f| andre::ui::render(app, f)).unwrap();
    let buf = terminal.backend().buffer();
    (0..buf.area().height)
        .map(|y| {
            (0..buf.area().width)
                .map(|x| buf[(x, y)].symbol())
                .collect::<String>()
        })
        .collect::<Vec<_>>()
        .join("\n")
}

#[test]
fn test_review_stow_preview_follows_dry_run_toggle() {
    // The review list is built once, so a global flag toggled while it is open
    // must rebuild the stow command preview before the next paint.
    let (mut app, _tmp) = make_test_app();
    app.ctx.core.config.global.engine = Some("stow".into());
    let selected = vec!["home/test".to_string()];
    app.component_stack
        .push(Box::new(ConfirmComponent::new(selected, &app.ctx)));
    app.update_components();
    assert!(
        !screen_text(&mut app).contains(" -n "),
        "dry-run is off, so the preview has no -n"
    );

    app.component_dispatch(key_char('d'));
    app.update_components();
    assert!(
        screen_text(&mut app).contains(" -n "),
        "dry-run toggled on the review screen must show in the stow preview"
    );
}

// --- Profiles and the action key ---

fn profiles(app: &App) -> &ProfilesComponent {
    app.component_stack
        .last()
        .unwrap()
        .as_any()
        .downcast_ref()
        .unwrap()
}

/// An app whose config folder holds `profiles`, as `(name, yaml)` pairs.
fn make_app_with_profiles(profiles: &[(&str, &str)]) -> (App, tempfile::TempDir) {
    let (mut app, tmp) = make_test_app();
    let dir = tmp.path().join("profiles");
    std::fs::create_dir_all(&dir).unwrap();
    for (name, yaml) in profiles {
        std::fs::write(dir.join(format!("{name}.yml")), yaml).unwrap();
    }
    app.ctx.core.profiles = ProfileSet::load(&dir).unwrap();
    (app, tmp)
}

/// Opens the profiles screen from the menu and runs one update, as the main loop does.
fn open_profiles(app: &mut App) {
    app.component_dispatch(key_char('1'));
    app.update_components();
}

fn type_text(app: &mut App, text: &str) {
    for c in text.chars() {
        app.component_dispatch(key_char(c));
    }
}

fn backspace(app: &mut App, times: usize) {
    for _ in 0..times {
        app.component_dispatch(KeyEvent::new(KeyCode::Backspace, KeyModifiers::NONE));
    }
}

fn profile_file(tmp: &tempfile::TempDir, name: &str) -> std::path::PathBuf {
    tmp.path().join("profiles").join(format!("{name}.yml"))
}

const BASE: &str = "description: Shared by every machine\npackages: [home/test]\n";

/// `BASE` for the flat packages folder the editor tests use.
const FLAT_BASE: &str = "description: Shared by every machine\npackages: [test]\n";

#[test]
fn test_s_cycles_the_action_stow_restow_unstow() {
    let (mut app, _) = make_test_app();
    assert_eq!(app.ctx.core.flags.action, StowAction::Stow);
    app.component_dispatch(key_char('s'));
    assert_eq!(app.ctx.core.flags.action, StowAction::Restow);
    app.component_dispatch(key_char('s'));
    assert_eq!(app.ctx.core.flags.action, StowAction::Unstow);
    app.component_dispatch(key_char('s'));
    assert_eq!(app.ctx.core.flags.action, StowAction::Stow);
}

#[test]
fn test_s_is_typed_into_a_name_not_cycled() {
    let (mut app, _tmp) = make_app_with_profiles(&[]);
    open_profiles(&mut app);
    app.component_dispatch(key_char('+'));
    type_text(&mut app, "s");
    assert_eq!(app.ctx.core.flags.action, StowAction::Stow);
    assert!(matches!(profiles(&app).popup(), Some(Popup::New(f)) if f.field.value == "s"));
}

#[test]
fn test_q_is_typed_into_a_name_and_does_not_quit() {
    let (mut app, _tmp) = make_app_with_profiles(&[]);
    open_profiles(&mut app);
    app.component_dispatch(key_char('+'));
    type_text(&mut app, "q");
    assert!(!app.quitting);
}

#[test]
fn test_plus_creates_a_profile_and_selects_it() {
    let (mut app, tmp) = make_app_with_profiles(&[("base", BASE)]);
    open_profiles(&mut app);
    app.component_dispatch(key_char('+'));
    type_text(&mut app, "laptop");
    app.component_dispatch(enter_key());

    assert!(profiles(&app).popup().is_none());
    assert!(profile_file(&tmp, "laptop").exists());
    app.update_components();
    assert_eq!(profiles(&app).list.cursor, 1, "base sorts before laptop");
}

#[test]
fn test_an_invalid_or_taken_name_keeps_the_new_dialog_open() {
    let (mut app, tmp) = make_app_with_profiles(&[("base", BASE)]);
    open_profiles(&mut app);
    app.component_dispatch(key_char('+'));
    type_text(&mut app, "Bad Name");
    app.component_dispatch(enter_key());
    assert!(matches!(profiles(&app).popup(), Some(Popup::New(f)) if f.field.value == "Bad Name"));

    backspace(&mut app, 8);
    type_text(&mut app, "base");
    app.component_dispatch(enter_key());
    assert!(matches!(profiles(&app).popup(), Some(Popup::New(_))));
    assert!(!profile_file(&tmp, "Bad Name").exists());
}

#[test]
fn test_esc_closes_the_dialog_and_keeps_the_screen() {
    let (mut app, _tmp) = make_app_with_profiles(&[]);
    open_profiles(&mut app);
    app.component_dispatch(key_char('+'));
    app.component_dispatch(esc_key());
    assert!(profiles(&app).popup().is_none());
    assert_eq!(component_id(&app), "Profiles");
}

#[test]
fn test_rename_moves_the_file_and_repoints_includes() {
    let (mut app, tmp) = make_app_with_profiles(&[
        ("agent", "includes: [base]\npackages: [home/test]\n"),
        ("base", BASE),
    ]);
    open_profiles(&mut app);
    app.component_dispatch(down_key());
    app.component_dispatch(key_char('r'));
    assert!(matches!(profiles(&app).popup(), Some(Popup::Rename { from, .. }) if from == "base"));

    backspace(&mut app, 4);
    type_text(&mut app, "shared");
    app.component_dispatch(enter_key());

    assert!(!profile_file(&tmp, "base").exists());
    assert!(profile_file(&tmp, "shared").exists());
    let agent = std::fs::read_to_string(profile_file(&tmp, "agent")).unwrap();
    assert!(
        agent.contains("shared"),
        "agent now includes shared: {agent}"
    );
}

#[test]
fn test_delete_removes_the_profile_and_the_includes_that_name_it() {
    let (mut app, tmp) = make_app_with_profiles(&[
        ("agent", "includes: [base]\npackages: [home/test]\n"),
        ("base", BASE),
    ]);
    open_profiles(&mut app);
    app.component_dispatch(down_key());
    app.component_dispatch(key_char('x'));
    assert!(matches!(
        profiles(&app).popup(),
        Some(Popup::Delete { included_by, .. }) if included_by == &["agent".to_string()]
    ));

    app.component_dispatch(key_char('y'));
    assert!(profiles(&app).popup().is_none());
    assert!(!profile_file(&tmp, "base").exists());
    let agent = std::fs::read_to_string(profile_file(&tmp, "agent")).unwrap();
    assert!(
        !agent.contains("base"),
        "agent no longer includes base: {agent}"
    );
    let toast = app.ctx.toast.as_ref().expect("a toast after the delete");
    assert_eq!(
        toast.message,
        "deleted profile base; removed it from 1 profile"
    );
}

#[test]
fn test_n_keeps_an_included_profile() {
    let (mut app, tmp) = make_app_with_profiles(&[
        ("agent", "includes: [base]\npackages: [home/test]\n"),
        ("base", BASE),
    ]);
    open_profiles(&mut app);
    app.component_dispatch(down_key());
    app.component_dispatch(key_char('x'));
    app.component_dispatch(key_char('n'));
    assert!(profiles(&app).popup().is_none());
    assert!(profile_file(&tmp, "base").exists());
}

#[test]
fn test_a_name_is_checked_when_it_changes() {
    let (mut app, _tmp) = make_app_with_profiles(&[("base", BASE)]);
    open_profiles(&mut app);
    app.component_dispatch(key_char('+'));
    type_text(&mut app, "base");
    assert!(matches!(
        profiles(&app).popup(),
        Some(Popup::New(name)) if name.check == NameCheck::Exists
    ));
    backspace(&mut app, 1);
    assert!(matches!(
        profiles(&app).popup(),
        Some(Popup::New(name)) if name.check == NameCheck::Valid
    ));
}

#[test]
fn test_c_opens_the_status_of_the_selected_profile_only() {
    let (mut app, _tmp) = make_app_with_profiles(&[("base", BASE)]);
    open_profiles(&mut app);
    app.component_dispatch(key_char('c'));
    assert_eq!(component_id(&app), "Status");
    let status: &StatusComponent = app
        .component_stack
        .last()
        .unwrap()
        .as_any()
        .downcast_ref()
        .unwrap();
    assert_eq!(status.profile(), Some("base"));
}

#[test]
fn test_c_on_a_profile_that_does_not_resolve_says_why_and_stays() {
    let (mut app, _tmp) = make_app_with_profiles(&[("agent", "includes: [ghost]\n")]);
    open_profiles(&mut app);
    app.component_dispatch(key_char('c'));
    assert_eq!(component_id(&app), "Profiles");
    assert!(app
        .ctx
        .toast
        .as_ref()
        .is_some_and(|t| t.message.contains("ghost")));
}

// --- The profile editor ---

/// A flat app with packages `shell` and `test`, so the list has two rows to move between, and
/// the profiles written to its config folder.
fn make_editor_app(profiles: &[(&str, &str)]) -> (App, tempfile::TempDir) {
    let (mut app, tmp) = make_flat_packages_app(&["shell", "test"]);
    let dir = tmp.path().join("profiles");
    std::fs::create_dir_all(&dir).unwrap();
    for (name, yaml) in profiles {
        std::fs::write(dir.join(format!("{name}.yml")), yaml).unwrap()
    }
    app.ctx.core.profiles = ProfileSet::load(&dir).unwrap();
    (app, tmp)
}

fn editor(app: &App) -> &ProfileEditComponent {
    app.component_stack
        .last()
        .unwrap()
        .as_any()
        .downcast_ref()
        .unwrap()
}

/// Opens the profiles screen, selects `name` and presses `e`.
fn open_editor(app: &mut App, name: &str) {
    open_profiles(app);
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

/// The names of the editor's package rows, in list order.
fn row_names(app: &App) -> Vec<String> {
    editor(app)
        .rows()
        .iter()
        .map(|row| row.name.clone())
        .collect()
}

fn stored_packages(tmp: &tempfile::TempDir, name: &str) -> Vec<String> {
    let set = ProfileSet::load(&tmp.path().join("profiles")).unwrap();
    set.get(name).unwrap().packages.clone()
}

#[test]
fn test_e_opens_the_editor_on_the_selected_profile() {
    let (mut app, _tmp) = make_editor_app(&[("base", FLAT_BASE)]);
    open_editor(&mut app, "base");
    assert_eq!(component_id(&app), "ProfileEdit");
    assert_eq!(editor(&app).draft().name, "base");
    assert!(!editor(&app).is_dirty());
}

#[test]
fn test_space_marks_a_package_and_w_writes_packages_in_order() {
    let (mut app, tmp) = make_editor_app(&[("agent", "packages: [test]\n")]);
    open_editor(&mut app, "agent");

    // Rows: shell, then test. The cursor starts on shell, so Space marks it.
    app.component_dispatch(space_key());
    assert!(editor(&app).is_dirty());
    assert_eq!(editor(&app).draft().packages, ["test", "shell"]);

    app.component_dispatch(key_char('w'));
    assert_eq!(
        stored_packages(&tmp, "agent"),
        ["test", "shell"],
        "kept refs stay first, new marks follow"
    );
    assert!(!editor(&app).is_dirty(), "a save clears the unsaved mark");
}

#[test]
fn test_star_marks_every_package_and_a_second_star_unmarks_them() {
    let (mut app, tmp) = make_editor_app(&[("agent", "packages: []\n")]);
    open_editor(&mut app, "agent");
    app.component_dispatch(key_char('*'));
    app.component_dispatch(key_char('w'));
    assert_eq!(stored_packages(&tmp, "agent"), ["shell", "test"]);

    app.component_dispatch(key_char('*'));
    app.component_dispatch(key_char('w'));
    assert!(stored_packages(&tmp, "agent").is_empty());
}

#[test]
fn test_star_leaves_an_inherited_package_alone() {
    let (mut app, tmp) = make_editor_app(&[
        ("base", "packages: [shell]\n"),
        ("agent", "includes: [base]\n"),
    ]);
    open_editor(&mut app, "agent");
    app.component_dispatch(key_char('*'));
    app.component_dispatch(key_char('w'));
    assert_eq!(stored_packages(&tmp, "agent"), ["test"]);

    // shell still comes from base: it stays inherited and is not written into agent.
    let shell = editor(&app)
        .rows()
        .iter()
        .find(|row| row.name == "shell")
        .expect("shell row");
    assert_eq!(shell.mark, Some(Mark::Inherited("base".to_string())));

    app.component_dispatch(key_char('*'));
    app.component_dispatch(key_char('w'));
    assert!(stored_packages(&tmp, "agent").is_empty());
}

#[test]
fn test_unmarking_a_missing_ref_drops_it_on_save() {
    // `gone` is in the profile but not in the packages folder: it is listed after the packages.
    let (mut app, tmp) = make_editor_app(&[("agent", "packages: [test, gone]\n")]);
    open_editor(&mut app, "agent");
    app.component_dispatch(down_key());
    app.component_dispatch(down_key());
    app.component_dispatch(space_key());
    assert_eq!(editor(&app).draft().packages, ["test"]);

    app.component_dispatch(key_char('w'));
    assert_eq!(stored_packages(&tmp, "agent"), ["test"]);
    // The save dropped `gone`, so its row leaves the list too.
    assert_eq!(row_names(&app), ["shell", "test"]);
}

#[test]
fn test_unmarking_a_missing_ref_keeps_its_row_until_save() {
    let (mut app, tmp) = make_editor_app(&[("agent", "packages: [test, gone]\n")]);
    open_editor(&mut app, "agent");
    app.component_dispatch(down_key());
    app.component_dispatch(down_key());
    app.component_dispatch(space_key());
    assert_eq!(editor(&app).draft().packages, ["test"]);
    assert_eq!(row_names(&app), ["shell", "test", "gone"]);

    // Space again marks `gone` back: the draft keeps it, and `w` writes it.
    app.component_dispatch(space_key());
    assert_eq!(editor(&app).draft().packages, ["test", "gone"]);
    app.component_dispatch(key_char('w'));
    assert_eq!(stored_packages(&tmp, "agent"), ["test", "gone"]);
}

#[test]
fn test_star_marks_and_unmarks_a_missing_ref_with_the_rest() {
    let (mut app, _tmp) = make_editor_app(&[("agent", "packages: [test, gone]\n")]);
    open_editor(&mut app, "agent");
    app.component_dispatch(key_char('*'));
    assert_eq!(editor(&app).draft().packages, ["test", "gone", "shell"]);

    app.component_dispatch(key_char('*'));
    assert!(editor(&app).draft().packages.is_empty());
    assert_eq!(row_names(&app), ["shell", "test", "gone"]);

    app.component_dispatch(key_char('*'));
    assert_eq!(editor(&app).draft().packages, ["shell", "test", "gone"]);
}

#[test]
fn test_left_and_right_do_nothing_in_the_package_list() {
    let (mut app, _tmp) = make_editor_app(&[("agent", "packages: []\n")]);
    open_editor(&mut app, "agent");
    app.component_dispatch(KeyEvent::new(KeyCode::Left, KeyModifiers::NONE));
    app.component_dispatch(KeyEvent::new(KeyCode::Right, KeyModifiers::NONE));

    // Still on the first row, `shell`: Space marks it.
    app.component_dispatch(space_key());
    assert_eq!(editor(&app).draft().packages, ["shell"]);
}

#[test]
fn test_an_inherited_package_does_not_toggle_and_says_where_it_comes_from() {
    let (mut app, tmp) = make_editor_app(&[
        ("base", "packages: [test]\n"),
        ("agent", "includes: [base]\n"),
    ]);
    open_editor(&mut app, "agent");

    // test comes from base: it is the second row.
    app.component_dispatch(down_key());
    app.component_dispatch(space_key());

    assert!(editor(&app).draft().packages.is_empty());
    assert!(!editor(&app).is_dirty());
    assert_eq!(
        app.ctx.toast.as_ref().expect("toast").message,
        "comes from base — edit base or drop the include"
    );
    assert!(stored_packages(&tmp, "agent").is_empty());
}

#[test]
fn test_esc_with_unsaved_edits_asks_and_n_discards() {
    let (mut app, tmp) = make_editor_app(&[("agent", "packages: [test]\n")]);
    open_editor(&mut app, "agent");
    app.component_dispatch(space_key());

    app.component_dispatch(esc_key());
    assert_eq!(component_id(&app), "ProfileEdit", "Esc stays while it asks");
    assert_eq!(
        app.component_stack.last().unwrap().input_mode(),
        InputMode::Modal
    );

    app.component_dispatch(key_char('n'));
    assert_eq!(component_id(&app), "Profiles");
    assert_eq!(stored_packages(&tmp, "agent"), ["test"]);
}

#[test]
fn test_esc_then_esc_stays_and_y_saves_then_leaves() {
    let (mut app, tmp) = make_editor_app(&[("agent", "packages: [test]\n")]);
    open_editor(&mut app, "agent");
    app.component_dispatch(space_key());

    app.component_dispatch(esc_key());
    app.component_dispatch(esc_key());
    assert_eq!(
        component_id(&app),
        "ProfileEdit",
        "a second Esc only closes the dialog"
    );

    app.component_dispatch(esc_key());
    app.component_dispatch(key_char('y'));
    assert_eq!(component_id(&app), "Profiles");
    assert_eq!(stored_packages(&tmp, "agent"), ["test", "shell"]);
}

#[test]
fn test_esc_without_edits_leaves_at_once() {
    let (mut app, _tmp) = make_editor_app(&[("base", FLAT_BASE)]);
    open_editor(&mut app, "base");
    app.component_dispatch(esc_key());
    assert_eq!(component_id(&app), "Profiles");
}

#[test]
fn test_a_cycle_through_an_include_is_refused_with_a_toast() {
    let (mut app, tmp) = make_editor_app(&[("base", FLAT_BASE), ("agent", "includes: [base]\n")]);
    // Editing base: its only other profile is agent, which already includes base.
    open_editor(&mut app, "base");
    app.component_dispatch(tab_key());
    app.component_dispatch(down_key());
    app.component_dispatch(space_key());

    assert!(editor(&app).draft().includes.is_empty());
    assert!(!editor(&app).is_dirty());
    assert_eq!(
        app.ctx.toast.as_ref().expect("toast").message,
        "can't include agent: profile include cycle: base -> agent -> base"
    );
    let stored = ProfileSet::load(&tmp.path().join("profiles")).unwrap();
    assert!(stored.get("base").unwrap().includes.is_empty());
}

#[test]
fn test_a_problem_that_was_already_there_does_not_block_another_include() {
    let (mut app, tmp) = make_editor_app(&[
        ("base", "packages: [test]\n"),
        ("agent", "includes: [ghost]\n"),
    ]);
    // The draft has an unknown include, so it does not resolve. Adding base is still allowed.
    open_editor(&mut app, "agent");
    app.component_dispatch(tab_key());
    // The form lists base, then ghost (the draft's own include).
    app.component_dispatch(down_key());
    app.component_dispatch(space_key());
    app.component_dispatch(key_char('w'));

    let stored = ProfileSet::load(&tmp.path().join("profiles")).unwrap();
    assert_eq!(stored.get("agent").unwrap().includes, ["ghost", "base"]);
}

#[test]
fn test_including_a_profile_brings_its_packages_in_and_w_saves_the_include() {
    let (mut app, tmp) =
        make_editor_app(&[("base", "packages: [test]\n"), ("agent", "packages: []\n")]);
    open_editor(&mut app, "agent");
    app.component_dispatch(tab_key());
    app.component_dispatch(down_key());
    app.component_dispatch(space_key());

    assert_eq!(editor(&app).draft().includes, ["base"]);
    assert_eq!(editor(&app).resolved_names(), ["test"]);

    app.component_dispatch(key_char('w'));
    let stored = ProfileSet::load(&tmp.path().join("profiles")).unwrap();
    assert_eq!(stored.get("agent").unwrap().includes, ["base"]);
}

#[test]
fn test_enter_edits_the_description_and_w_saves_it() {
    let (mut app, tmp) = make_editor_app(&[("agent", "packages: []\n")]);
    open_editor(&mut app, "agent");
    app.component_dispatch(tab_key());
    app.component_dispatch(enter_key());
    assert_eq!(
        app.component_stack.last().unwrap().input_mode(),
        InputMode::TextInput
    );

    // While typing, letters are text, not global keys: `s` must not cycle the action.
    let action = app.ctx.core.flags.action;
    type_text(&mut app, "Laptop s");
    assert_eq!(app.ctx.core.flags.action, action);
    app.component_dispatch(enter_key());
    assert_eq!(
        app.component_stack.last().unwrap().input_mode(),
        InputMode::Normal
    );

    app.component_dispatch(key_char('w'));
    let stored = ProfileSet::load(&tmp.path().join("profiles")).unwrap();
    assert_eq!(
        stored.get("agent").unwrap().description.as_deref(),
        Some("Laptop s")
    );
}

#[test]
fn test_a_blank_description_is_saved_as_no_description() {
    let (mut app, tmp) = make_editor_app(&[("agent", "description: Work\npackages: []\n")]);
    open_editor(&mut app, "agent");
    app.component_dispatch(tab_key());
    app.component_dispatch(enter_key());
    backspace(&mut app, 4);
    app.component_dispatch(enter_key());
    app.component_dispatch(key_char('w'));

    let stored = ProfileSet::load(&tmp.path().join("profiles")).unwrap();
    assert_eq!(stored.get("agent").unwrap().description, None);
}

#[test]
fn test_an_unresolved_profile_opens_so_its_broken_include_can_be_removed() {
    let (mut app, tmp) = make_editor_app(&[("agent", "includes: [ghost]\npackages: [test]\n")]);
    open_editor(&mut app, "agent");
    assert_eq!(component_id(&app), "ProfileEdit");

    // The form lists the broken include, so it can be unchecked.
    app.component_dispatch(tab_key());
    app.component_dispatch(down_key());
    app.component_dispatch(space_key());
    app.component_dispatch(key_char('w'));

    let stored = ProfileSet::load(&tmp.path().join("profiles")).unwrap();
    assert!(stored.get("agent").unwrap().includes.is_empty());
}

#[test]
fn test_tab_switches_the_pane_that_takes_keys() {
    let (mut app, _tmp) = make_editor_app(&[("agent", "packages: []\n")]);
    open_editor(&mut app, "agent");
    assert!(editor(&app).on_packages());
    app.component_dispatch(tab_key());
    assert!(!editor(&app).on_packages());
    app.component_dispatch(tab_key());
    assert!(editor(&app).on_packages());
}

#[test]
fn test_x_then_y_deletes_an_unincluded_profile_file() {
    let (mut app, tmp) = make_app_with_profiles(&[("base", BASE)]);
    open_profiles(&mut app);
    app.component_dispatch(key_char('x'));
    app.component_dispatch(key_char('y'));
    assert!(!profile_file(&tmp, "base").exists());
    assert!(profiles(&app).popup().is_none());
}

#[test]
fn test_enter_runs_the_own_and_included_packages_and_skips_the_missing_ones() {
    let (mut app, tmp) = make_app_with_profiles(&[
        (
            "agent",
            "includes: [base]\npackages: [home/own, home/ghost]\n",
        ),
        ("base", BASE),
    ]);
    std::fs::create_dir_all(tmp.path().join("dotfiles/home/own")).unwrap();
    open_profiles(&mut app);
    // `agent` sorts first, so Enter runs it.
    app.component_dispatch(enter_key());
    assert_eq!(component_id(&app), "Confirm");
    let review: &ConfirmComponent = top_component(&app);
    let mut packages = review.selected_packages.clone();
    packages.sort();
    assert_eq!(
        packages,
        ["home/own", "home/test"],
        "own and included packages run; home/ghost names no package and does not"
    );
}

#[test]
fn test_s_on_settings_waits_for_save_like_the_other_settings() {
    let (mut app, _tmp) = make_test_app();
    push_component(&mut app, "Settings");
    app.component_dispatch(key_char('s'));
    assert_eq!(
        app.ctx.core.flags.action,
        StowAction::Stow,
        "the core action is unchanged until save"
    );
    assert!(settings(&app).dirty);
    assert!(
        screen_text(&mut app).contains("restow"),
        "the Settings action row shows the pending action"
    );

    app.component_dispatch(esc_key());
    app.component_dispatch(key_char('y'));
    assert_eq!(component_id(&app), "MainMenu");
    assert_eq!(app.ctx.core.flags.action, StowAction::Restow);
}

#[test]
fn test_n_still_toggles_no_fold_on_the_profiles_screen() {
    let (mut app, _tmp) = make_app_with_profiles(&[("base", BASE)]);
    open_profiles(&mut app);
    app.component_dispatch(key_char('n'));
    assert!(app.ctx.core.flags.no_folding);
    assert_eq!(component_id(&app), "Profiles");
}

#[test]
fn test_enter_on_a_profile_with_no_packages_says_so_and_stays() {
    let (mut app, _tmp) = make_app_with_profiles(&[("empty", "description: nothing yet\n")]);
    open_profiles(&mut app);
    app.component_dispatch(enter_key());
    assert_eq!(component_id(&app), "Profiles");
    let toast = app
        .ctx
        .toast
        .as_ref()
        .expect("a toast explains the refusal");
    assert!(toast.message.contains("no packages"), "{}", toast.message);
}

fn ignore_editor(app: &App) -> &andre::components::ignore::IgnoreEditComponent {
    app.component_stack
        .last()
        .unwrap()
        .as_any()
        .downcast_ref()
        .unwrap()
}

/// Opens the ignore editor on `home/test`, the package in `make_test_app`.
fn open_ignore_on_test_package(app: &mut App) {
    open_packages(app);
    app.component_dispatch(down_key());
    app.component_dispatch(key_char('i'));
}

fn package_file(tmp: &tempfile::TempDir, name: &str) -> std::path::PathBuf {
    tmp.path().join("dotfiles/home/test").join(name)
}

#[test]
fn test_i_opens_the_ignore_editor_on_the_cursor_package_with_no_files_yet() {
    let (mut app, _tmp) = make_test_app();
    open_ignore_on_test_package(&mut app);
    assert_eq!(component_id(&app), "IgnoreEdit");
    let editor = ignore_editor(&app);
    assert!(editor.files().iter().all(|draft| !draft.present()));
    assert!(!editor.is_dirty());
}

#[test]
fn test_i_on_an_empty_folder_does_nothing() {
    let (mut app, _tmp) = make_flat_packages_app(&[]);
    open_packages(&mut app);
    app.component_dispatch(key_char('i'));
    assert_eq!(component_id(&app), "Packages");
}

#[test]
fn test_plus_creates_the_gitignore_from_its_starter_lines_and_w_writes_it() {
    let (mut app, tmp) = make_test_app();
    open_ignore_on_test_package(&mut app);
    app.component_dispatch(key_char('+'));
    assert_eq!(
        app.component_stack.last().unwrap().input_mode(),
        InputMode::TextInput
    );
    type_text(&mut app, "*.env");
    app.component_dispatch(enter_key());
    app.component_dispatch(key_char('w'));

    let written = std::fs::read_to_string(package_file(&tmp, ".gitignore")).unwrap();
    assert!(written.lines().any(|line| line == "*.env"), "{written}");
    assert!(written.contains("# Written by andre"), "{written}");
    assert!(!ignore_editor(&app).is_dirty());
}

#[test]
fn test_x_deletes_the_cursor_line_and_esc_then_n_discards_the_change() {
    let (mut app, tmp) = make_test_app();
    std::fs::write(package_file(&tmp, ".gitignore"), "a\nb\n").unwrap();
    open_ignore_on_test_package(&mut app);
    app.component_dispatch(key_char('x'));
    assert_eq!(ignore_editor(&app).files()[0].lines(), ["b".to_string()]);
    assert!(ignore_editor(&app).is_dirty());

    app.component_dispatch(esc_key());
    assert_eq!(
        app.component_stack.last().unwrap().input_mode(),
        InputMode::Modal,
        "Esc with unsaved lines opens the leave dialog"
    );
    app.component_dispatch(key_char('n'));
    assert_eq!(component_id(&app), "Packages");
    assert_eq!(
        std::fs::read_to_string(package_file(&tmp, ".gitignore")).unwrap(),
        "a\nb\n"
    );
}

#[test]
fn test_esc_then_y_in_the_leave_dialog_saves_and_returns() {
    let (mut app, tmp) = make_test_app();
    std::fs::write(package_file(&tmp, ".gitignore"), "a\nb\n").unwrap();
    open_ignore_on_test_package(&mut app);
    app.component_dispatch(key_char('x'));
    app.component_dispatch(esc_key());
    app.component_dispatch(key_char('y'));
    assert_eq!(component_id(&app), "Packages");
    assert_eq!(
        std::fs::read_to_string(package_file(&tmp, ".gitignore")).unwrap(),
        "b\n"
    );
}

#[test]
fn test_esc_on_an_unchanged_editor_goes_back_without_asking() {
    let (mut app, _tmp) = make_test_app();
    open_ignore_on_test_package(&mut app);
    app.component_dispatch(esc_key());
    assert_eq!(component_id(&app), "Packages");
}

#[test]
fn test_tab_switches_files_and_a_stow_line_that_does_not_compile_stays_open() {
    let (mut app, tmp) = make_test_app();
    std::fs::write(package_file(&tmp, ".stow-local-ignore"), "# mine\n").unwrap();
    open_ignore_on_test_package(&mut app);
    app.component_dispatch(KeyEvent::new(KeyCode::Tab, KeyModifiers::NONE));
    assert_eq!(ignore_editor(&app).active(), 1);

    app.component_dispatch(key_char('+'));
    type_text(&mut app, "bad(");
    app.component_dispatch(enter_key());
    assert_eq!(ignore_editor(&app).editing_text(), Some("bad("));

    app.component_dispatch(esc_key());
    assert_eq!(ignore_editor(&app).editing_text(), None);
    assert!(!ignore_editor(&app).is_dirty());
    assert_eq!(
        std::fs::read_to_string(package_file(&tmp, ".stow-local-ignore")).unwrap(),
        "# mine\n"
    );
}

#[test]
fn test_e_edits_the_cursor_line_and_a_blank_line_removes_it() {
    let (mut app, tmp) = make_test_app();
    std::fs::write(package_file(&tmp, ".gitignore"), "a\nb\n").unwrap();
    open_ignore_on_test_package(&mut app);
    app.component_dispatch(down_key());
    app.component_dispatch(key_char('e'));
    assert_eq!(ignore_editor(&app).editing_text(), Some("b"));
    app.component_dispatch(KeyEvent::new(KeyCode::Backspace, KeyModifiers::NONE));
    type_text(&mut app, "c");
    app.component_dispatch(enter_key());
    assert_eq!(ignore_editor(&app).files()[0].lines(), ["a", "c"]);

    app.component_dispatch(up_key());
    app.component_dispatch(key_char('e'));
    for _ in 0..3 {
        app.component_dispatch(KeyEvent::new(KeyCode::Backspace, KeyModifiers::NONE));
    }
    app.component_dispatch(enter_key());
    assert_eq!(ignore_editor(&app).files()[0].lines(), ["c"]);
}

#[test]
fn test_enter_on_the_add_row_adds_a_line_and_esc_drops_an_empty_one() {
    let (mut app, tmp) = make_test_app();
    std::fs::write(package_file(&tmp, ".gitignore"), "a\n").unwrap();
    open_ignore_on_test_package(&mut app);
    app.component_dispatch(down_key()); // the add row
    app.component_dispatch(enter_key());
    app.component_dispatch(esc_key());
    assert_eq!(ignore_editor(&app).files()[0].lines(), ["a"]);
    assert!(!ignore_editor(&app).is_dirty());
}

#[test]
fn test_global_keys_still_work_in_the_ignore_editor() {
    let (mut app, _tmp) = make_test_app();
    open_ignore_on_test_package(&mut app);
    let before = app.ctx.core.theme;
    app.component_dispatch(key_char('t'));
    assert_ne!(app.ctx.core.theme, before);
    assert_eq!(component_id(&app), "IgnoreEdit");
}

#[test]
fn test_plus_then_esc_on_a_missing_file_leaves_no_file_and_no_dialog() {
    let (mut app, tmp) = make_test_app();
    open_ignore_on_test_package(&mut app);
    app.component_dispatch(key_char('+'));
    app.component_dispatch(esc_key());
    assert!(!ignore_editor(&app).files()[0].present());
    assert!(!ignore_editor(&app).is_dirty());
    assert_eq!(
        app.component_stack.last().unwrap().input_mode(),
        InputMode::Normal,
        "Esc cancels the line; it does not open the leave dialog"
    );

    app.component_dispatch(esc_key());
    assert_eq!(component_id(&app), "Packages");
    assert!(!package_file(&tmp, ".gitignore").exists());
}

#[test]
fn test_plus_then_blank_enter_on_a_missing_file_leaves_no_file() {
    let (mut app, tmp) = make_test_app();
    open_ignore_on_test_package(&mut app);
    app.component_dispatch(key_char('+'));
    app.component_dispatch(enter_key());
    assert!(!ignore_editor(&app).files()[0].present());
    assert!(!ignore_editor(&app).is_dirty());
    assert!(!package_file(&tmp, ".gitignore").exists());
}

#[test]
fn test_stow_local_ignore_line_added_in_the_editor_is_written_on_w() {
    let (mut app, tmp) = make_test_app();
    open_ignore_on_test_package(&mut app);
    app.component_dispatch(KeyEvent::new(KeyCode::Tab, KeyModifiers::NONE));
    app.component_dispatch(key_char('+'));
    type_text(&mut app, "^/README.*");
    app.component_dispatch(enter_key());
    app.component_dispatch(key_char('w'));

    let written = std::fs::read_to_string(package_file(&tmp, ".stow-local-ignore")).unwrap();
    assert!(
        written.lines().any(|line| line == "^/README.*"),
        "{written}"
    );
    assert!(!ignore_editor(&app).is_dirty());
}

#[test]
fn test_saving_the_ignore_files_invalidates_the_status_cache() {
    let (mut app, _tmp) = make_test_app();
    open_ignore_on_test_package(&mut app);
    app.ctx
        .status_cache
        .borrow_mut()
        .insert("home/test".to_string(), andre_core::StowStatus::Stowed);
    app.component_dispatch(key_char('+'));
    type_text(&mut app, "*.env");
    app.component_dispatch(enter_key());
    assert!(
        app.ctx.status_cache.borrow().get("home/test").is_some(),
        "the cache is untouched until a save"
    );

    app.component_dispatch(key_char('w'));
    assert!(
        app.ctx.status_cache.borrow().get("home/test").is_none(),
        "a save makes the packages screen read statuses again"
    );
}

// ---- main menu digits and the set-up item (F5) ----

#[test]
fn test_zero_does_nothing_without_the_set_up_item() {
    let (mut app, _tmp) = make_test_app();
    app.update_components();
    assert_eq!(menu(&app).items().len(), 8);
    app.component_dispatch(key_char('0'));
    assert_eq!(app.component_stack.len(), 1, "no screen opens on 0");
}

#[test]
fn test_zero_opens_the_set_up_review_and_digits_keep_their_items() {
    let (mut app, _tmp) = make_bootstrap_app();
    app.update_components();
    assert_eq!(menu(&app).items().len(), 9);
    app.component_dispatch(key_char('1'));
    assert_eq!(
        app.component_stack.last().unwrap().id(),
        "Profiles",
        "1 still opens profiles, the first of the eight"
    );
    app.component_dispatch(esc_key());

    app.component_dispatch(key_char('0'));
    let confirm = app
        .component_stack
        .last()
        .unwrap()
        .as_any()
        .downcast_ref::<ConfirmComponent>()
        .expect("0 opens the review");
    assert_eq!(confirm.selected_packages, ["config/andre"]);
}

#[test]
fn test_zero_on_a_flat_config_reviews_the_package_holding_the_config_folder() {
    let (mut app, _tmp) = make_flat_bootstrap_app();
    app.update_components();
    assert_eq!(menu(&app).items().first(), Some(&MenuItem::Setup));
    app.component_dispatch(key_char('0'));
    let confirm = app
        .component_stack
        .last()
        .unwrap()
        .as_any()
        .downcast_ref::<ConfirmComponent>()
        .expect("0 opens the review");
    assert_eq!(confirm.selected_packages, ["andre"]);
}

#[test]
fn test_enter_on_the_set_up_item_of_a_flat_config_reviews_its_package() {
    let (mut app, _tmp) = make_flat_bootstrap_app();
    app.update_components();
    app.component_dispatch(enter_key());
    let confirm = app
        .component_stack
        .last()
        .unwrap()
        .as_any()
        .downcast_ref::<ConfirmComponent>()
        .expect("Enter on set-up opens the review");
    assert_eq!(confirm.selected_packages, ["andre"]);
}

#[test]
fn test_profiles_opened_at_puts_the_cursor_on_the_named_profile() {
    let (mut app, tmp) = make_test_app();
    let dir = tmp.path().join("profiles");
    std::fs::create_dir_all(&dir).unwrap();
    for name in ["alpha", "beta", "gamma"] {
        std::fs::write(dir.join(format!("{name}.yml")), "packages: [home/test]\n").unwrap();
    }
    app.ctx.core.profiles = ProfileSet::load(&dir).unwrap();
    app.component_stack
        .push(Box::new(ProfilesComponent::opened_at("beta")));
    app.update_components();

    let profiles = app
        .component_stack
        .last()
        .unwrap()
        .as_any()
        .downcast_ref::<ProfilesComponent>()
        .unwrap();
    assert_eq!(profiles.list.cursor, 1);
}

#[test]
fn test_unstow_browse_marks_links_and_enter_reviews_all_of_them() {
    let (mut app, tmp) = make_unstow_app();
    push_browse(&mut app, tmp.path());
    browse_cursor_to(&mut app, ".bashrc");
    app.component_dispatch(space_key());
    browse_cursor_to(&mut app, ".profile");
    app.component_dispatch(space_key());
    app.component_dispatch(enter_key());

    assert_eq!(component_id(&app), "UnstowConfirm");
    let review: &andre::components::unstow::UnstowConfirmComponent = top_component(&app);
    assert_eq!(review.links.len(), 2);

    app.component_dispatch(key_char('y'));
    assert!(!tmp.path().join(".bashrc").is_symlink());
    assert!(!tmp.path().join(".profile").is_symlink());
    assert!(
        tmp.path().join("dotfiles/home/test/.bashrc").is_file(),
        "the package keeps its files"
    );
}

#[test]
fn test_unstow_browse_enter_without_marks_reviews_the_cursor_link_only() {
    let (mut app, tmp) = make_unstow_app();
    push_browse(&mut app, tmp.path());
    browse_cursor_to(&mut app, ".profile");
    app.component_dispatch(enter_key());

    assert_eq!(component_id(&app), "UnstowConfirm");
    let review: &andre::components::unstow::UnstowConfirmComponent = top_component(&app);
    assert_eq!(review.links.len(), 1);
    assert!(review.links[0].path.ends_with(".profile"));
}

#[test]
fn test_space_on_a_folder_is_refused_with_a_toast_and_marks_nothing() {
    use andre::components::Component as _;

    let (mut app, tmp) = make_unstow_app();
    push_browse(&mut app, tmp.path());
    browse_cursor_to(&mut app, "dotfiles");
    app.component_dispatch(space_key());

    let toast = app.ctx.toast.as_ref().expect("the refusal is shown");
    assert!(
        toast.message.contains("dotfiles is not a link"),
        "{}",
        toast.message
    );
    let browse: &andre::components::unstow::UnstowBrowseComponent = top_component(&app);
    assert!(
        browse
            .hints(&app.ctx)
            .iter()
            .any(|hint| hint.label == "review"),
        "no link was marked"
    );
}

// --- Settings screen: dialogs, modal keys and the save flow ---

/// Opens the settings screen and runs one update, as the app's loop does before a paint.
fn open_settings(app: &mut App) {
    push_component(app, "Settings");
    app.update_components();
}

/// Moves the settings cursor onto the row labelled `label`.
fn select_setting(app: &mut App, label: &str) {
    let settings = settings_mut(app);
    settings.list.cursor = settings
        .items()
        .iter()
        .position(|item| item.label == label)
        .unwrap_or_else(|| panic!("no settings row labelled {label}"));
}

#[test]
fn test_settings_picker_is_modal_and_blocks_global_keys() {
    use andre::components::InputMode;

    let (mut app, _) = make_test_app();
    open_settings(&mut app);
    select_setting(&mut app, "action");
    app.component_dispatch(enter_key());
    assert_eq!(
        app.component_stack.last().unwrap().input_mode(),
        InputMode::Modal,
        "an open picker is Modal"
    );
    app.component_dispatch(key_char('s'));
    assert_eq!(
        app.ctx.core.flags.action,
        StowAction::Stow,
        "the action key must not cycle the action while the picker is open"
    );
    assert_eq!(settings(&app).overlays.len(), 1);
}

#[test]
fn test_settings_name_dialog_is_text_input_and_blocks_globals() {
    use andre::components::settings::Overlay;
    use andre::components::InputMode;

    let (mut app, _) = make_test_app();
    open_settings(&mut app);
    app.component_dispatch(key_char('+'));
    assert_eq!(
        app.component_stack.last().unwrap().input_mode(),
        InputMode::TextInput,
        "the new group name is typed text"
    );
    let theme = app.ctx.core.theme;
    app.component_dispatch(key_char('t'));
    assert_eq!(
        app.ctx.core.theme, theme,
        "t types into the name, not the theme"
    );
    let Some(Overlay::Name { field, .. }) = settings(&app).overlays.last() else {
        panic!("name dialog should still be open");
    };
    assert_eq!(field.value, "t");
}

#[test]
fn test_config_save_failure_error_toast_keeps_dirty() {
    use andre::components::ToastKind;

    let (mut app, tmp) = make_test_app();
    open_settings(&mut app);
    select_setting(&mut app, "verbose");
    app.component_dispatch(enter_key());
    app.ctx.config_path = tmp.path().join("missing-dir").join("andre.yml");
    let verbosity_before = app.ctx.core.flags.verbosity;
    app.component_dispatch(key_char('w'));

    assert!(settings(&app).dirty, "dirty must remain after failed save");
    let toast = app.ctx.toast.as_ref().expect("error toast");
    assert!(
        toast.message.starts_with("Save failed:"),
        "unexpected toast: {}",
        toast.message
    );
    assert_eq!(toast.kind, ToastKind::Error);
    assert_eq!(
        app.ctx.core.flags.verbosity, verbosity_before,
        "core must not commit pending changes when disk write fails"
    );
}

#[test]
fn test_discard_save_y_stays_on_failure() {
    let (mut app, tmp) = make_test_app();
    open_settings(&mut app);
    select_setting(&mut app, "verbose");
    app.component_dispatch(enter_key());
    app.ctx.config_path = tmp.path().join("nope").join("andre.yml");
    app.component_dispatch(esc_key());
    assert!(matches!(
        settings(&app).overlays.last(),
        Some(andre::components::settings::Overlay::Leave)
    ));
    app.component_dispatch(key_char('y'));

    assert_eq!(
        component_id(&app),
        "Settings",
        "must stay on Settings when the save fails"
    );
    assert!(settings(&app).dirty);
    assert!(app
        .ctx
        .toast
        .as_ref()
        .is_some_and(|t| t.message.starts_with("Save failed:")));
}
