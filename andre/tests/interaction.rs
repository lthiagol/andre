use andre::app::App;
use andre::components::stow::GroupSelectComponent;

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
    assert_eq!(menu(&app).index, 0);
    app.component_dispatch(up_key());
    assert_eq!(menu(&app).index, 6);
}

#[test]
fn test_menu_down_wraps() {
    let (mut app, _) = make_test_app();
    for _ in 0..7 {
        app.component_dispatch(down_key());
    }
    assert_eq!(menu(&app).index, 0);
}

#[test]
fn test_menu_j_down() {
    let (mut app, _) = make_test_app();
    app.component_dispatch(key_char('j'));
    assert_eq!(menu(&app).index, 1);
}

#[test]
fn test_menu_k_up() {
    let (mut app, _) = make_test_app();
    app.component_dispatch(key_char('k'));
    assert_eq!(menu(&app).index, 6);
}

// --- Screen Transitions ---

#[test]
fn test_enter_on_stow_goes_to_group_select() {
    let (mut app, _) = make_test_app();
    app.component_dispatch(enter_key());
    assert_eq!(component_id(&app), "GroupSelect");
}

#[test]
fn test_enter_on_adopt_goes_to_adopt_target() {
    let (mut app, _) = make_two_group_app();
    app.component_dispatch(down_key());
    app.component_dispatch(enter_key());
    assert_eq!(component_id(&app), "AdoptTargetSelect");
}

#[test]
fn test_enter_on_unstow_goes_to_unstow_target() {
    let (mut app, _) = make_two_group_app();
    app.component_dispatch(down_key());
    app.component_dispatch(down_key());
    app.component_dispatch(enter_key());
    assert_eq!(component_id(&app), "UnstowTargetSelect");
}

#[test]
fn test_enter_on_status_goes_to_status() {
    let (mut app, _) = make_test_app();
    for _ in 0..3 {
        app.component_dispatch(down_key());
    }
    app.component_dispatch(enter_key());
    assert_eq!(component_id(&app), "Status");
}

#[test]
fn test_enter_on_config_goes_to_settings() {
    let (mut app, _) = make_test_app();
    for _ in 0..4 {
        app.component_dispatch(down_key());
    }
    app.component_dispatch(enter_key());
    assert_eq!(component_id(&app), "Settings");
}

#[test]
fn test_enter_on_help_goes_to_help() {
    let (mut app, _) = make_test_app();
    for _ in 0..5 {
        app.component_dispatch(down_key());
    }
    app.component_dispatch(enter_key());
    assert_eq!(component_id(&app), "Help");
}

#[test]
fn test_enter_on_quit_sets_quit() {
    let (mut app, _) = make_test_app();
    for _ in 0..6 {
        app.component_dispatch(down_key());
    }
    app.component_dispatch(enter_key());
    assert!(app.quitting);
}

#[test]
fn test_digit_key_jumps_and_activates() {
    // Each digit 1..=7 should jump straight to the Nth menu item and
    // produce the same transition as Enter on that index. Mirrors the
    // `test_enter_on_*` block above but exercises the digit-key path.
    let cases: [(char, &str); 7] = [
        ('1', "GroupSelect"),
        ('2', "AdoptTargetSelect"),
        ('3', "UnstowTargetSelect"),
        ('4', "Status"),
        ('5', "Settings"),
        ('6', "Help"),
        ('7', "MainMenu"), // Quit pops to the still-mounted MainMenu via quitting
    ];
    for (digit, expected) in cases {
        let (mut app, _) = make_test_app();
        app.component_dispatch(key_char(digit));
        // '7' quits; on quit the top remains MainMenu and `quitting` is true.
        if digit == '7' {
            assert!(app.quitting, "digit '7' should set quitting");
            assert_eq!(component_id(&app), expected);
        } else {
            assert_eq!(component_id(&app), expected, "digit '{digit}'");
        }
    }
}

#[test]
fn test_digit_key_out_of_range_is_ignored() {
    // '0' and '8'..='9' fall outside the 7-item menu and must not panic
    // or change the highlighted index.
    for c in ['0', '8', '9'] {
        let (mut app, _) = make_test_app();
        let before = menu(&app).index;
        app.component_dispatch(key_char(c));
        assert_eq!(
            menu(&app).index,
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
    app.component_dispatch(key_char('4'));
    // '4' would push Status — pop back, then press Enter and confirm Status again.
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
    app.ctx.core.verbosity = 0;
    app.component_dispatch(key_char('v'));
    assert_eq!(app.ctx.core.verbosity, 1);
    app.component_dispatch(key_char('v'));
    assert_eq!(app.ctx.core.verbosity, 2);
    for _ in 0..4 {
        app.component_dispatch(key_char('v'));
    }
    assert_eq!(app.ctx.core.verbosity, 0);
}

#[test]
fn test_d_toggles_dry_run() {
    let (mut app, _) = make_test_app();
    app.ctx.core.dry_run = false;
    app.component_dispatch(key_char('d'));
    assert!(app.ctx.core.dry_run);
    app.component_dispatch(key_char('d'));
    assert!(!app.ctx.core.dry_run);
}

#[test]
fn test_n_toggles_no_folding() {
    let (mut app, _) = make_test_app();
    app.component_dispatch(key_char('n'));
    assert!(app.ctx.core.no_folding);
    app.component_dispatch(key_char('n'));
    assert!(!app.ctx.core.no_folding);
}

#[test]
fn test_a_toggles_adopt() {
    let (mut app, _) = make_test_app();
    app.component_dispatch(key_char('a'));
    assert!(app.ctx.core.adopt);
    app.component_dispatch(key_char('a'));
    assert!(!app.ctx.core.adopt);
}

#[test]
fn test_o_toggles_dotfiles() {
    let (mut app, _) = make_test_app();
    app.component_dispatch(key_char('o'));
    assert!(app.ctx.core.dotfiles);
    app.component_dispatch(key_char('o'));
    assert!(!app.ctx.core.dotfiles);
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
    settings_mut(&mut app).cursor = 2;
    app.component_dispatch(up_key());
    assert!(
        settings(&app).cursor >= 2,
        "cursor at {}",
        settings(&app).cursor
    );
}

// --- Group Actions Dialog ---

fn navigate_to_group_row(app: &mut App) {
    settings_mut(app).cursor = 11;
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
    let initial_verbosity = app.ctx.core.verbosity;
    let initial_dry_run = app.ctx.core.dry_run;
    let initial_no_folding = app.ctx.core.no_folding;
    let initial_adopt = app.ctx.core.adopt;
    let initial_dotfiles = app.ctx.core.dotfiles;
    // All global shortcuts should be absorbed — nothing changes
    app.component_dispatch(key_char('t'));
    assert_eq!(app.ctx.core.theme, initial_theme);
    app.component_dispatch(key_char('v'));
    assert_eq!(app.ctx.core.verbosity, initial_verbosity);
    app.component_dispatch(key_char('d'));
    assert_eq!(app.ctx.core.dry_run, initial_dry_run);
    app.component_dispatch(key_char('n'));
    assert_eq!(app.ctx.core.no_folding, initial_no_folding);
    app.component_dispatch(key_char('a'));
    assert_eq!(app.ctx.core.adopt, initial_adopt);
    app.component_dispatch(key_char('o'));
    assert_eq!(app.ctx.core.dotfiles, initial_dotfiles);
}

#[test]
fn test_global_keys_blocked_during_settings_picker() {
    let (mut app, _) = make_test_app();
    push_component(&mut app, "Settings");
    // Navigate to Action row (ROW_ACTION = 7)
    settings_mut(&mut app).cursor = 7;
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
    settings_mut(&mut app).cursor = 7;
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
    settings_mut(&mut app).cursor = 11;
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
    settings_mut(&mut app).cursor = 11;
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
    let initial_verbosity = app.ctx.core.verbosity;
    // Global v should still work on PackageSelect (Normal mode)
    app.component_dispatch(key_char('v'));
    assert_ne!(
        app.ctx.core.verbosity, initial_verbosity,
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
    let before = app.ctx.core.verbosity;
    app.component_dispatch(key_char('v'));
    assert_ne!(
        app.ctx.core.verbosity, before,
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
    settings.cursor = 12;
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
        s.cursor = 2;
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
        s.cursor = 12;
    }
    let verbosity_before = app.ctx.core.verbosity;
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
        app.ctx.core.verbosity, verbosity_before,
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
        s.cursor = 2;
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
    app.ctx
        .show_toast_for("working", std::time::Duration::from_secs(0));
    assert!(app.ctx.toast.is_some());

    tx.try_send(ExecutionUpdate::Progress {
        current: 1,
        total: 2,
        _package: "home / bash".into(),
    })
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
