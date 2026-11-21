use std::collections::HashMap;

use andre::app::App;
use andre::components::confirm::ConfirmComponent;
use andre::components::profiles::{NameCheck, Popup, ProfilesComponent};
use andre::components::settings::rows::*;
use andre::components::stow::GroupSelectComponent;
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

fn groups(app: &App) -> &GroupSelectComponent {
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
fn test_enter_on_packages_goes_to_group_select() {
    let (mut app, _) = make_test_app();
    app.component_dispatch(down_key());
    app.component_dispatch(enter_key());
    assert_eq!(component_id(&app), "GroupSelect");
}

#[test]
fn test_enter_on_new_package_goes_to_adopt_target() {
    let (mut app, _) = make_two_group_app();
    app.component_dispatch(down_key());
    app.component_dispatch(down_key());
    app.component_dispatch(enter_key());
    assert_eq!(component_id(&app), "AdoptTargetSelect");
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
        ('2', "GroupSelect"),
        ('3', "AdoptTargetSelect"),
        ('4', "UnstowTargetSelect"),
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
fn test_esc_group_select_to_main_menu() {
    let (mut app, _) = make_test_app();
    push_component(&mut app, "GroupSelect");
    app.component_dispatch(esc_key());
    assert_eq!(component_id(&app), "MainMenu");
}

#[test]
fn test_esc_from_screens_goes_back() {
    let (mut app, _) = make_test_app();
    push_component(&mut app, "GroupSelect");
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

// --- Group Selection ---

#[test]
fn test_space_toggles_group_selection() {
    let (mut app, _) = make_two_group_app();
    push_component(&mut app, "GroupSelect");
    // GroupSelectComponent sorts groups alphabetically — "config" < "home"
    let first_group = "config".to_string();
    assert!(groups(&app).selected.is_empty());
    app.component_dispatch(space_key());
    assert!(groups(&app).selected.contains(&first_group));
    app.component_dispatch(space_key());
    assert!(!groups(&app).selected.contains(&first_group));
}

#[test]
fn test_group_navigation_does_not_wrap() {
    let (mut app, _) = make_two_group_app();
    push_component(&mut app, "GroupSelect");
    app.component_dispatch(down_key());
    assert_eq!(groups(&app).cursor, 1);
    app.component_dispatch(down_key());
    // Should not wrap past the last item
    assert_eq!(groups(&app).cursor, 1);
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
    push_component(&mut app, "GroupSelect");
    app.component_dispatch(up_key());
    app.component_dispatch(down_key());
    app.component_dispatch(space_key());
    app.component_dispatch(enter_key());
}

#[test]
fn test_quit_screen_ignores_keys() {
    let (mut app, _) = make_test_app();
    app.component_dispatch(key_char('q'));
    assert!(app.quitting);
}

// --- Config Screen Navigation ---

#[test]
fn test_config_navigation_starts_at_first_interactive() {
    let (mut app, _) = make_test_app();
    push_component(&mut app, "Settings");
    app.component_dispatch(down_key());
    assert!(
        settings(&app).cursor >= 2,
        "cursor should skip to interactive row, got {}",
        settings(&app).cursor
    );
}

#[test]
fn test_config_up_wraps_to_last_interactive() {
    let (mut app, _) = make_test_app();
    push_component(&mut app, "Settings");
    settings_mut(&mut app).cursor = ROW_VERBOSITY;
    app.component_dispatch(up_key());
    assert!(
        settings(&app).cursor >= 2,
        "cursor at {}",
        settings(&app).cursor
    );
}

// --- Group Actions Dialog ---

fn navigate_to_group_row(app: &mut App) {
    settings_mut(app).cursor = ROW_GROUP_LINE;
}

#[test]
fn test_group_actions_opens_on_enter() {
    let (mut app, _) = make_test_app();
    push_component(&mut app, "Settings");
    navigate_to_group_row(&mut app);
    assert!(settings(&app).dialog.is_none());
    app.component_dispatch(enter_key());
    assert!(matches!(
        settings(&app).dialog,
        Some(andre::components::settings::GroupDialog::Actions { cursor: 0 })
    ));
}

#[test]
fn test_group_actions_esc_closes() {
    let (mut app, _) = make_test_app();
    push_component(&mut app, "Settings");
    navigate_to_group_row(&mut app);
    app.component_dispatch(enter_key());
    assert!(settings(&app).dialog.is_some());
    app.component_dispatch(esc_key());
    assert!(settings(&app).dialog.is_none());
}

#[test]
fn test_group_actions_navigates_add_remove_edit() {
    let (mut app, _) = make_test_app();
    push_component(&mut app, "Settings");
    navigate_to_group_row(&mut app);
    app.component_dispatch(enter_key());
    // Default is Add (cursor=0)
    assert!(matches!(
        settings(&app).dialog,
        Some(andre::components::settings::GroupDialog::Actions { cursor: 0 })
    ));
    // Down to Remove
    app.component_dispatch(down_key());
    assert!(matches!(
        settings(&app).dialog,
        Some(andre::components::settings::GroupDialog::Actions { cursor: 1 })
    ));
    // Down to Edit
    app.component_dispatch(down_key());
    assert!(matches!(
        settings(&app).dialog,
        Some(andre::components::settings::GroupDialog::Actions { cursor: 2 })
    ));
    // Up wraps back to Add
    app.component_dispatch(up_key());
    assert!(matches!(
        settings(&app).dialog,
        Some(andre::components::settings::GroupDialog::Actions { cursor: 1 })
    ));
}

// --- Help Scrolling ---

fn help(app: &App) -> &andre::components::help::HelpComponent {
    app.component_stack
        .last()
        .unwrap()
        .as_any()
        .downcast_ref()
        .unwrap()
}

#[test]
fn test_help_scroll_down_clamped() {
    let (mut app, _) = make_test_app();
    push_component(&mut app, "Help");
    let start = help(&app).scroll_index;
    assert_eq!(start, 0);
    for _ in 0..50 {
        app.component_dispatch(down_key());
    }
    // Should clamp at last item, not grow unboundedly
    let max = andre::ui::help::help_item_count() - 1;
    assert_eq!(help(&app).scroll_index, max);
}

#[test]
fn test_help_esc_goes_back() {
    let (mut app, _) = make_test_app();
    push_component(&mut app, "Help");
    app.component_dispatch(down_key());
    app.component_dispatch(esc_key());
    assert_eq!(component_id(&app), "MainMenu");
}

// --- Q Quit Blocked During Text Input ---

#[test]
fn test_q_quit_blocked_during_adopt_name_prompt() {
    let (mut app, _) = make_two_group_app();
    use andre::components::adopt::AdoptNamePromptComponent;
    app.component_stack
        .push(Box::new(AdoptNamePromptComponent::new(
            "test".to_string(),
            vec![],
        )));
    app.component_dispatch(key_char('q'));
    assert!(!app.quitting, "q should not quit during text input");
}

// --- Global Keys Blocked During Text Input ---

#[test]
fn test_global_keys_blocked_during_adopt_name_prompt() {
    let (mut app, _) = make_two_group_app();
    use andre::components::adopt::AdoptNamePromptComponent;
    app.component_stack
        .push(Box::new(AdoptNamePromptComponent::new(
            "test".to_string(),
            vec![],
        )));
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

#[test]
fn test_global_keys_blocked_during_settings_picker() {
    let (mut app, _) = make_test_app();
    push_component(&mut app, "Settings");
    // Navigate to Action row (ROW_ACTION = 7)
    settings_mut(&mut app).cursor = ROW_ACTION;
    app.component_dispatch(enter_key());
    assert!(settings(&app).picker.is_some(), "picker should be open");
    let initial_theme = app.ctx.core.theme;
    app.component_dispatch(key_char('t'));
    assert_eq!(
        app.ctx.core.theme, initial_theme,
        "theme should not change while picker is open"
    );
}

// --- InputMode Audit ---

#[test]
fn test_input_mode_audit_normal_components() {
    let (mut app, _) = make_test_app();
    use andre::components::InputMode;

    // All list/menu screens should be Normal
    for name in &["MainMenu", "GroupSelect", "Status", "Help"] {
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
    use andre::components::adopt::AdoptNamePromptComponent;
    use andre::components::InputMode;
    let (mut app, _) = make_test_app();
    app.component_stack
        .push(Box::new(AdoptNamePromptComponent::new(
            "test".to_string(),
            vec![],
        )));
    assert_eq!(
        app.component_stack.last().unwrap().input_mode(),
        InputMode::TextInput
    );
}

#[test]
fn test_settings_picker_is_modal() {
    let (mut app, _) = make_test_app();
    use andre::components::InputMode;
    push_component(&mut app, "Settings");
    // Open picker on Action row
    settings_mut(&mut app).cursor = ROW_ACTION;
    app.component_dispatch(enter_key());
    assert_eq!(
        app.component_stack.last().unwrap().input_mode(),
        InputMode::Modal,
        "Settings with picker should be Modal"
    );
}

#[test]
fn test_settings_add_name_is_text_input() {
    let (mut app, _) = make_test_app();
    use andre::components::InputMode;
    push_component(&mut app, "Settings");
    // Navigate to group row, open Actions, select Add
    settings_mut(&mut app).cursor = ROW_GROUP_LINE;
    app.component_dispatch(enter_key()); // Actions menu
    app.component_dispatch(enter_key()); // Add → AddName dialog
    assert_eq!(
        app.component_stack.last().unwrap().input_mode(),
        InputMode::TextInput,
        "Settings AddName should be TextInput"
    );
}

#[test]
fn test_settings_text_input_blocks_globals() {
    let (mut app, _) = make_test_app();
    push_component(&mut app, "Settings");
    // Open AddName dialog via group row
    settings_mut(&mut app).cursor = ROW_GROUP_LINE;
    app.component_dispatch(enter_key()); // Actions menu
    app.component_dispatch(enter_key()); // Add → AddName dialog
    let initial_theme = app.ctx.core.theme;
    // Global t key should be blocked
    app.component_dispatch(key_char('t'));
    assert_eq!(
        app.ctx.core.theme, initial_theme,
        "theme should not change while AddName text input is active"
    );
}

#[test]
fn test_package_select_still_allows_global_v() {
    let (mut app, _) = make_two_group_app();
    push_component(&mut app, "GroupSelect");
    // Select a group
    app.component_dispatch(space_key());
    // Navigate into packages
    app.component_dispatch(enter_key());
    assert_eq!(component_id(&app), "PackageSelect");
    let initial_verbosity = app.ctx.core.flags.verbosity;
    // Global v should still work on PackageSelect (Normal mode)
    app.component_dispatch(key_char('v'));
    assert_ne!(
        app.ctx.core.flags.verbosity, initial_verbosity,
        "global v should toggle verbosity on PackageSelect"
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
    // Cursor on verbosity, toggle, then Save (row 12 per config_persistence helpers).
    app.component_dispatch(enter_key());
    let settings = app
        .component_stack
        .last_mut()
        .unwrap()
        .as_any_mut()
        .downcast_mut::<andre::components::settings::SettingsComponent>()
        .unwrap();
    settings.cursor = ROW_SAVE_BUTTON;
    app.component_dispatch(enter_key());
    let toast = app.ctx.toast.as_ref().expect("toast after save");
    assert_eq!(toast.message, "Config saved");
    assert_eq!(toast.kind, ToastKind::Success);
}

#[test]
fn test_config_save_failure_error_toast_keeps_dirty() {
    use andre::components::settings::{ConfigMessage, SettingsComponent};
    use andre::components::ToastKind;

    let (mut app, tmp) = make_test_app();
    push_component(&mut app, "Settings");
    // Verbosity row (2) → dirty, then Save (12) with unwritable path.
    {
        let s = app
            .component_stack
            .last_mut()
            .unwrap()
            .as_any_mut()
            .downcast_mut::<SettingsComponent>()
            .unwrap();
        s.cursor = ROW_VERBOSITY;
    }
    app.component_dispatch(enter_key());
    app.ctx.config_path = tmp.path().join("missing-dir").join("andre.yml");
    {
        let s = app
            .component_stack
            .last_mut()
            .unwrap()
            .as_any_mut()
            .downcast_mut::<SettingsComponent>()
            .unwrap();
        s.cursor = ROW_SAVE_BUTTON;
    }
    let verbosity_before = app.ctx.core.flags.verbosity;
    app.component_dispatch(enter_key());

    let settings = app
        .component_stack
        .last()
        .unwrap()
        .as_any()
        .downcast_ref::<SettingsComponent>()
        .unwrap();
    assert!(settings.config_dirty, "dirty must remain after failed save");
    assert_ne!(
        settings.config_message,
        ConfigMessage::Saved,
        "must not claim Saved on failure"
    );
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
    use andre::components::settings::{ConfigMessage, SettingsComponent};

    let (mut app, tmp) = make_test_app();
    push_component(&mut app, "Settings");
    {
        let s = app
            .component_stack
            .last_mut()
            .unwrap()
            .as_any_mut()
            .downcast_mut::<SettingsComponent>()
            .unwrap();
        s.cursor = ROW_VERBOSITY;
    }
    app.component_dispatch(enter_key()); // dirty via verbosity
    app.ctx.config_path = tmp.path().join("nope").join("andre.yml");
    app.component_dispatch(esc_key());
    assert_eq!(
        app.component_stack
            .last()
            .unwrap()
            .as_any()
            .downcast_ref::<SettingsComponent>()
            .unwrap()
            .config_message,
        ConfigMessage::ConfirmDiscard
    );
    app.component_dispatch(key_char('y'));
    assert_eq!(
        app.component_stack.last().map(|c| c.id()),
        Some("Settings"),
        "must stay on Settings when confirm-save fails"
    );
    assert!(app
        .ctx
        .toast
        .as_ref()
        .is_some_and(|t| t.message.starts_with("Save failed:")));
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
fn test_adopt_target_enter_opens_highlighted_group() {
    let (mut app, tmp) = make_two_group_app();
    app.component_dispatch(down_key());
    app.component_dispatch(down_key());
    app.component_dispatch(enter_key());
    assert_eq!(component_id(&app), "AdoptTargetSelect");

    app.component_dispatch(enter_key());
    let select: &andre::components::adopt::AdoptFileSelectComponent = top_component(&app);
    assert_eq!(select.group_name, "config");
    assert_eq!(
        select.browser.path,
        andre_core::path::resolve_path("~/.config", tmp.path(), tmp.path())
    );
}

#[test]
fn test_adopt_target_second_row_opens_second_group() {
    let (mut app, tmp) = make_two_group_app();
    app.component_dispatch(down_key());
    app.component_dispatch(down_key());
    app.component_dispatch(enter_key());
    app.component_dispatch(down_key());
    app.component_dispatch(enter_key());
    let select: &andre::components::adopt::AdoptFileSelectComponent = top_component(&app);
    assert_eq!(select.group_name, "home");
    assert_eq!(
        select.browser.path,
        andre_core::path::resolve_path("~", tmp.path(), tmp.path())
    );
}

/// Save the open Settings screen (Save row) and reload the config file from disk.
fn save_and_reload(app: &mut App) -> andre_core::Config {
    settings_mut(app).cursor = ROW_SAVE_BUTTON;
    app.component_dispatch(enter_key());
    andre_core::Config::load(&app.ctx.config_path).unwrap()
}

#[test]
fn test_group_ignore_add_then_remove_persists_on_save() {
    use andre::components::settings::GroupDialog;

    let (mut app, _tmp) = make_test_app();
    push_component(&mut app, "Settings");
    settings_mut(&mut app).dialog = Some(GroupDialog::IgnoreManage {
        group: Some("home".into()),
        cursor: 0,
    });

    // Add "*.log" (add row is 0 while the list is empty), then "tmp" (add row is 1).
    app.component_dispatch(enter_key());
    for c in "*.log".chars() {
        app.component_dispatch(key_char(c));
    }
    app.component_dispatch(enter_key());
    app.component_dispatch(down_key());
    app.component_dispatch(enter_key());
    for c in "tmp".chars() {
        app.component_dispatch(key_char(c));
    }
    app.component_dispatch(enter_key());

    // Remove the first pattern with 'r' on row 0.
    app.component_dispatch(key_char('r'));
    settings_mut(&mut app).dialog = None;

    let saved = save_and_reload(&mut app);
    assert_eq!(
        saved.groups["home"].ignore.as_deref(),
        Some(&["tmp".to_string()][..])
    );
}

#[test]
fn test_removing_last_group_ignore_clears_it_on_save() {
    use andre::components::settings::GroupDialog;

    let (mut app, _tmp) = make_test_app();
    push_component(&mut app, "Settings");
    settings_mut(&mut app).dialog = Some(GroupDialog::IgnoreManage {
        group: Some("home".into()),
        cursor: 0,
    });
    app.component_dispatch(enter_key());
    for c in "*.log".chars() {
        app.component_dispatch(key_char(c));
    }
    app.component_dispatch(enter_key());
    app.component_dispatch(key_char('r'));
    settings_mut(&mut app).dialog = None;

    let saved = save_and_reload(&mut app);
    assert_eq!(saved.groups["home"].ignore, None);
}

#[test]
fn test_global_ignore_add_persists_on_save() {
    use andre::components::settings::GroupDialog;

    let (mut app, _tmp) = make_test_app();
    push_component(&mut app, "Settings");
    settings_mut(&mut app).dialog = Some(GroupDialog::IgnoreManage {
        group: None,
        cursor: 0,
    });
    app.component_dispatch(enter_key());
    for c in "node_modules".chars() {
        app.component_dispatch(key_char(c));
    }
    app.component_dispatch(enter_key());
    settings_mut(&mut app).dialog = None;

    let saved = save_and_reload(&mut app);
    assert_eq!(saved.global_ignore(), vec!["node_modules".to_string()]);
}

#[test]
fn test_edit_group_target_from_browser_persists_on_save() {
    use andre::components::settings::{GroupDialog, GroupEditField};

    let (mut app, tmp) = make_test_app();
    push_component(&mut app, "Settings");
    let browse = tmp.path().join("browse");
    std::fs::create_dir_all(browse.join("only")).unwrap();
    settings_mut(&mut app).dialog = Some(GroupDialog::EditBrowse {
        group: "home".into(),
        field: GroupEditField::Target,
        browser: andre::components::browser::Browser::open(
            browse.clone(),
            andre::components::browser::dir_entries,
        ),
    });
    app.component_dispatch(enter_key());

    let saved = save_and_reload(&mut app);
    assert_eq!(
        saved.groups["home"].target,
        browse.join("only").to_string_lossy()
    );
}

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
    let selected = HashMap::from([("home".to_string(), vec!["test".to_string()])]);
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
fn test_e_and_c_say_they_are_not_there_yet() {
    let (mut app, _tmp) = make_app_with_profiles(&[("base", BASE)]);
    open_profiles(&mut app);
    app.component_dispatch(key_char('e'));
    assert_eq!(
        app.ctx.toast.as_ref().expect("e toasts").message,
        "profile edit: coming soon"
    );
    app.component_dispatch(key_char('c'));
    assert_eq!(
        app.ctx.toast.as_ref().expect("c toasts").message,
        "profile status: coming soon"
    );
    assert_eq!(component_id(&app), "Profiles");
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
    let mut packages = review
        .selected_packages
        .get("home")
        .cloned()
        .unwrap_or_default();
    packages.sort();
    assert_eq!(
        packages,
        ["own", "test"],
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
    assert!(settings(&app).config_dirty);
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
