use andre::app::App;

mod helpers;
use helpers::*;

fn settings_mut(app: &mut App) -> &mut andre::components::settings::SettingsComponent {
    app.component_stack
        .last_mut()
        .unwrap()
        .as_any_mut()
        .downcast_mut()
        .unwrap()
}

fn navigate_to_config(app: &mut andre::app::App) {
    push_component(app, "Settings");
    app.component_dispatch(down_key());
    let comp = settings_mut(app);
    comp.cursor = 2;
}

fn save_and_exit(app: &mut andre::app::App) {
    settings_mut(app).cursor = 12;
    app.component_dispatch(enter_key());
    app.component_dispatch(esc_key());
}

fn assert_config_verbosity(app: &App, expected: u8) {
    assert_eq!(
        app.ctx.core.verbosity, expected,
        "AppState verbosity mismatch"
    );
    assert_eq!(
        app.ctx.core.config.effective_verbosity(),
        expected,
        "Config verbosity out of sync with AppState"
    );
}

#[test]
fn test_enter_cycles_verbosity_and_save_persists() {
    let (mut app, tmp) = make_test_app();
    let config_path = tmp.path().join("andre.yml");
    navigate_to_config(&mut app);
    app.component_dispatch(enter_key());
    app.component_dispatch(enter_key());
    save_and_exit(&mut app);
    assert_eq!(component_id(&app), "MainMenu");
    assert_config_verbosity(&app, 2);
    let reloaded = andre_core::Config::load(&config_path).unwrap();
    assert_eq!(reloaded.effective_verbosity(), 2);
}

#[test]
fn test_space_on_dry_run_then_save_persists() {
    let (mut app, tmp) = make_test_app();
    navigate_to_config(&mut app);
    settings_mut(&mut app).cursor = 3;
    app.component_dispatch(space_key());
    save_and_exit(&mut app);
    assert!(app.ctx.core.dry_run);
    assert!(app.ctx.core.config.effective_dry_run());
    let reloaded = andre_core::Config::load(&tmp.path().join("andre.yml")).unwrap();
    assert!(reloaded.effective_dry_run());
}

#[test]
fn test_enter_toggles_dry_run_and_save_persists() {
    let (mut app, tmp) = make_test_app();
    navigate_to_config(&mut app);
    settings_mut(&mut app).cursor = 3;
    app.component_dispatch(enter_key());
    save_and_exit(&mut app);
    assert!(app.ctx.core.dry_run);
    let reloaded = andre_core::Config::load(&tmp.path().join("andre.yml")).unwrap();
    assert!(reloaded.effective_dry_run());
}

#[test]
fn test_verbosity_wraps_at_six() {
    let (mut app, tmp) = make_test_app();
    let config_path = tmp.path().join("andre.yml");
    navigate_to_config(&mut app);
    for _ in 0..6 {
        app.component_dispatch(enter_key());
    }
    // Nothing applied yet — verbosity still 0 in ctx.core
    assert_eq!(app.ctx.core.verbosity, 0);
    save_and_exit(&mut app);
    // After save, verbosity should be 0 (wrapped around)
    assert_config_verbosity(&app, 0);
    let reloaded = andre_core::Config::load(&config_path).unwrap();
    assert_eq!(reloaded.effective_verbosity(), 0);
}

#[test]
fn test_add_group_then_save_persists() {
    let (mut app, tmp) = make_test_app();
    let config_path = tmp.path().join("andre.yml");
    navigate_to_config(&mut app);
    let groups_before = app.ctx.core.config.groups.len();
    // Open group actions, select Add, type name, confirm
    settings_mut(&mut app).cursor = 11;
    app.component_dispatch(enter_key()); // Opens Actions dialog
    app.component_dispatch(enter_key()); // Select Add (cursor=0)
                                         // Now at AddName dialog
    app.component_dispatch(key_char('m'));
    app.component_dispatch(key_char('y'));
    app.component_dispatch(key_char('g'));
    app.component_dispatch(key_char('r'));
    app.component_dispatch(key_char('p'));
    app.component_dispatch(enter_key()); // Confirm name -> AddBrowse source
                                         // Pick source dir (first dir in home_dir)
    app.component_dispatch(enter_key()); // Select source dir -> AddBrowse target
    app.component_dispatch(enter_key()); // Select target dir -> group added
                                         // Not yet applied
    assert_eq!(app.ctx.core.config.groups.len(), groups_before);
    save_and_exit(&mut app);
    assert_eq!(app.ctx.core.config.groups.len(), groups_before + 1);
    let reloaded = andre_core::Config::load(&config_path).unwrap();
    assert_eq!(reloaded.groups.len(), groups_before + 1);
}

#[test]
fn test_remove_group_via_dialog_then_save_persists() {
    let (mut app, tmp) = make_test_app();
    let config_path = tmp.path().join("andre.yml");
    navigate_to_config(&mut app);
    settings_mut(&mut app).cursor = 11;
    let groups_before = app.ctx.core.config.groups.len();
    // Open group actions, select Remove
    app.component_dispatch(enter_key());
    app.component_dispatch(down_key()); // cursor=1 (Remove)
    app.component_dispatch(enter_key()); // Opens RemoveSelect
    app.component_dispatch(enter_key()); // Select first group
    app.component_dispatch(key_char('y')); // Confirm removal
                                           // Not yet applied
    assert_eq!(app.ctx.core.config.groups.len(), groups_before);
    save_and_exit(&mut app);
    assert_eq!(app.ctx.core.config.groups.len(), groups_before - 1);
    let reloaded = andre_core::Config::load(&config_path).unwrap();
    assert_eq!(reloaded.groups.len(), groups_before - 1);
}

#[test]
fn test_config_dirty_set_on_modification() {
    let (mut app, _tmp) = make_test_app();
    navigate_to_config(&mut app);
    assert!(!settings(&app).config_dirty);
    app.component_dispatch(enter_key());
    assert!(settings(&app).config_dirty);
}

fn settings(app: &App) -> &andre::components::settings::SettingsComponent {
    app.component_stack
        .last()
        .unwrap()
        .as_any()
        .downcast_ref()
        .unwrap()
}

#[test]
fn test_esc_dirty_shows_warning_then_discards() {
    let (mut app, _tmp) = make_test_app();
    navigate_to_config(&mut app);
    app.component_dispatch(enter_key());
    assert!(settings(&app).config_dirty);
    app.component_dispatch(esc_key());
    assert_eq!(component_id(&app), "Settings");
    assert_eq!(
        settings(&app).config_message,
        andre::components::settings::ConfigMessage::ConfirmDiscard
    );
    app.component_dispatch(esc_key());
    assert_eq!(component_id(&app), "MainMenu");
}

#[test]
fn test_esc_dirty_then_y_saves() {
    let (mut app, tmp) = make_test_app();
    let config_path = tmp.path().join("andre.yml");
    navigate_to_config(&mut app);
    app.component_dispatch(enter_key());
    app.component_dispatch(enter_key());
    assert!(settings(&app).config_dirty);
    app.component_dispatch(esc_key());
    assert_eq!(component_id(&app), "Settings");
    assert_eq!(
        settings(&app).config_message,
        andre::components::settings::ConfigMessage::ConfirmDiscard
    );
    app.component_dispatch(key_char('y'));
    assert_eq!(component_id(&app), "MainMenu");
    assert_config_verbosity(&app, 2);
    let reloaded = andre_core::Config::load(&config_path).unwrap();
    assert_eq!(reloaded.effective_verbosity(), 2);
}

#[test]
fn test_esc_dirty_then_n_discards() {
    let (mut app, _tmp) = make_test_app();
    navigate_to_config(&mut app);
    app.component_dispatch(enter_key());
    app.component_dispatch(enter_key());
    app.component_dispatch(esc_key());
    assert_eq!(component_id(&app), "Settings");
    assert_eq!(
        settings(&app).config_message,
        andre::components::settings::ConfigMessage::ConfirmDiscard
    );
    app.component_dispatch(key_char('n'));
    assert_eq!(component_id(&app), "MainMenu");
}

#[test]
fn test_esc_with_clean_state_navigates_normally() {
    let (mut app, _tmp) = make_test_app();
    navigate_to_config(&mut app);
    assert!(!settings(&app).config_dirty);
    app.component_dispatch(esc_key());
    assert_eq!(component_id(&app), "MainMenu");
}

#[test]
fn test_dirty_then_save_then_esc_works() {
    let (mut app, _tmp) = make_test_app();
    navigate_to_config(&mut app);
    app.component_dispatch(enter_key());
    settings_mut(&mut app).cursor = 12;
    app.component_dispatch(enter_key());
    app.component_dispatch(esc_key());
    assert_eq!(component_id(&app), "MainMenu");
}

#[test]
fn test_n_dismisses_saved_message_without_toggling_setting() {
    let (mut app, _tmp) = make_test_app();
    navigate_to_config(&mut app);
    app.component_dispatch(enter_key());
    settings_mut(&mut app).cursor = 12;
    app.component_dispatch(enter_key());
    assert_eq!(
        settings(&app).config_message,
        andre::components::settings::ConfigMessage::Saved
    );
    let no_folding_before = app.ctx.core.no_folding;
    app.component_dispatch(key_char('n'));
    assert_eq!(
        settings(&app).config_message,
        andre::components::settings::ConfigMessage::Hidden
    );
    assert_eq!(app.ctx.core.no_folding, no_folding_before);
}
