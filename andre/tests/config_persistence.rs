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
        app.ctx.core.flags.verbosity, expected,
        "AppState verbosity mismatch"
    );
    assert_eq!(
        app.ctx.core.config.stow_flags().verbosity,
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
    assert_eq!(reloaded.stow_flags().verbosity, 2);
}

#[test]
fn test_space_on_dry_run_then_save_persists() {
    let (mut app, tmp) = make_test_app();
    navigate_to_config(&mut app);
    settings_mut(&mut app).cursor = 3;
    app.component_dispatch(space_key());
    save_and_exit(&mut app);
    assert!(app.ctx.core.flags.dry_run);
    assert!(app.ctx.core.config.stow_flags().dry_run);
    let reloaded = andre_core::Config::load(&tmp.path().join("andre.yml")).unwrap();
    assert!(reloaded.stow_flags().dry_run);
}

#[test]
fn test_enter_toggles_dry_run_and_save_persists() {
    let (mut app, tmp) = make_test_app();
    navigate_to_config(&mut app);
    settings_mut(&mut app).cursor = 3;
    app.component_dispatch(enter_key());
    save_and_exit(&mut app);
    assert!(app.ctx.core.flags.dry_run);
    let reloaded = andre_core::Config::load(&tmp.path().join("andre.yml")).unwrap();
    assert!(reloaded.stow_flags().dry_run);
}

#[test]
fn test_flag_rows_enter_and_space_then_save_persists() {
    let (mut app, tmp) = make_test_app();
    navigate_to_config(&mut app);
    settings_mut(&mut app).cursor = 4;
    app.component_dispatch(enter_key()); // no-folding
    settings_mut(&mut app).cursor = 5;
    app.component_dispatch(space_key()); // adopt
    settings_mut(&mut app).cursor = 6;
    app.component_dispatch(enter_key()); // dotfiles
                                         // Staged in Settings only until save
    assert!(!app.ctx.core.flags.no_folding);
    assert!(!app.ctx.core.flags.adopt);
    assert!(!app.ctx.core.flags.dotfiles);
    save_and_exit(&mut app);
    assert!(app.ctx.core.flags.no_folding);
    assert!(app.ctx.core.flags.adopt);
    assert!(app.ctx.core.flags.dotfiles);
    let reloaded = andre_core::Config::load(&tmp.path().join("andre.yml")).unwrap();
    let flags = reloaded.stow_flags();
    assert!(flags.no_folding);
    assert!(flags.adopt);
    assert!(flags.dotfiles);
}

/// `[dry_run, no_folding, adopt, dotfiles]`: the bool flags behind settings rows 3..=6.
fn bool_flags(flags: &andre_core::StowFlags) -> [bool; 4] {
    [flags.dry_run, flags.no_folding, flags.adopt, flags.dotfiles]
}

#[test]
fn test_each_bool_flag_row_toggles_only_its_own_flag_and_saves() {
    // Toggling one row at a time: a swap between two rows shows up as the wrong bit set.
    for (cursor, idx) in [(3, 0), (4, 1), (5, 2), (6, 3)] {
        let (mut app, tmp) = make_test_app();
        navigate_to_config(&mut app);
        settings_mut(&mut app).cursor = cursor;
        app.component_dispatch(enter_key());
        save_and_exit(&mut app);

        let mut expected = [false; 4];
        expected[idx] = true;
        assert_eq!(
            bool_flags(&app.ctx.core.flags),
            expected,
            "row {cursor} in AppState"
        );
        let reloaded = andre_core::Config::load(&tmp.path().join("andre.yml")).unwrap();
        assert_eq!(
            bool_flags(&reloaded.stow_flags()),
            expected,
            "row {cursor} on disk"
        );
    }
}

#[test]
fn test_flag_shortcuts_a_and_o_in_settings_then_save_persists() {
    let (mut app, tmp) = make_test_app();
    navigate_to_config(&mut app);
    app.component_dispatch(key_char('a'));
    app.component_dispatch(key_char('o'));
    assert!(settings(&app).config_dirty);
    assert!(!app.ctx.core.flags.adopt);
    assert!(!app.ctx.core.flags.dotfiles);
    save_and_exit(&mut app);
    assert!(app.ctx.core.flags.adopt);
    assert!(app.ctx.core.flags.dotfiles);
    let reloaded = andre_core::Config::load(&tmp.path().join("andre.yml")).unwrap();
    let flags = reloaded.stow_flags();
    assert!(flags.adopt);
    assert!(flags.dotfiles);
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
    assert_eq!(app.ctx.core.flags.verbosity, 0);
    save_and_exit(&mut app);
    // After save, verbosity should be 0 (wrapped around)
    assert_config_verbosity(&app, 0);
    let reloaded = andre_core::Config::load(&config_path).unwrap();
    assert_eq!(reloaded.stow_flags().verbosity, 0);
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
    assert_eq!(reloaded.stow_flags().verbosity, 2);
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
    let no_folding_before = app.ctx.core.flags.no_folding;
    app.component_dispatch(key_char('n'));
    assert_eq!(
        settings(&app).config_message,
        andre::components::settings::ConfigMessage::Hidden
    );
    assert_eq!(app.ctx.core.flags.no_folding, no_folding_before);
}

fn backspace_key() -> crossterm::event::KeyEvent {
    crossterm::event::KeyEvent::new(
        crossterm::event::KeyCode::Backspace,
        crossterm::event::KeyModifiers::NONE,
    )
}

fn type_text(app: &mut App, text: &str) {
    for c in text.chars() {
        app.component_dispatch(key_char(c));
    }
}

fn make_app_with_yaml(yaml: &str) -> (App, tempfile::TempDir) {
    let tmp = tempfile::TempDir::new().unwrap();
    let config_path = tmp.path().join("andre.yml");
    std::fs::write(&config_path, yaml).unwrap();
    create_test_package(&tmp);
    let app = App::with_home(config_path, tmp.path().to_path_buf()).unwrap();
    (app, tmp)
}

/// Opens the Edit > Name prompt for the only group ("home").
fn open_edit_name_prompt(app: &mut App) {
    navigate_to_config(app);
    settings_mut(app).cursor = 11; // group line
    app.component_dispatch(enter_key()); // group actions
    app.component_dispatch(down_key());
    app.component_dispatch(down_key()); // Edit
    app.component_dispatch(enter_key()); // pick group
    app.component_dispatch(enter_key()); // edit fields
    app.component_dispatch(enter_key()); // Name
}

#[test]
fn test_rename_group_in_settings_then_save_persists() {
    let (mut app, tmp) = make_app_with_yaml(
        r#"
global:
  stow:
    verbosity: 0
groups:
  home:
    source: dotfiles/home
    target: "~"
    ignore:
      - .git
"#,
    );
    let config_path = tmp.path().join("andre.yml");

    open_edit_name_prompt(&mut app);
    for _ in 0.."home".len() {
        app.component_dispatch(backspace_key());
    }
    type_text(&mut app, "work");
    app.component_dispatch(enter_key());
    assert!(
        settings_mut(&mut app).dialog.is_none(),
        "rename should close the dialog"
    );

    save_and_exit(&mut app);
    assert_eq!(component_id(&app), "MainMenu");
    let reloaded = andre_core::Config::load(&config_path).unwrap();
    assert!(!reloaded.groups.contains_key("home"));
    let renamed = &reloaded.groups["work"];
    assert_eq!(renamed.source, "dotfiles/home");
    assert_eq!(renamed.target, "~");
    assert_eq!(renamed.ignore, Some(vec![".git".to_string()]));
}

#[test]
fn test_rename_group_escape_keeps_original_name() {
    let (mut app, _tmp) = make_test_app();
    open_edit_name_prompt(&mut app);
    type_text(&mut app, "x");
    app.component_dispatch(esc_key());

    let dialog = settings_mut(&mut app).dialog.clone();
    assert!(
        matches!(
            dialog,
            Some(andre::components::settings::GroupDialog::EditField { ref group, .. })
                if group == "home"
        ),
        "Esc should return to the field menu for the original name, got {dialog:?}"
    );
    assert!(!settings_mut(&mut app).config_dirty);
}

#[test]
fn test_theme_picked_in_settings_persists_to_disk() {
    let (mut app, tmp) = make_test_app();
    let config_path = tmp.path().join("andre.yml");
    navigate_to_config(&mut app);
    settings_mut(&mut app).cursor = 8; // theme row
    app.component_dispatch(enter_key()); // open picker on "default"
    app.component_dispatch(down_key());
    app.component_dispatch(down_key()); // "catppuccin-mocha"
    app.component_dispatch(enter_key());
    assert_eq!(app.ctx.core.theme, andre_core::Theme::Default);
    save_and_exit(&mut app);

    assert_eq!(app.ctx.core.theme, andre_core::Theme::CatppuccinMocha);
    let reloaded = andre_core::Config::load(&config_path).unwrap();
    assert_eq!(reloaded.global.theme.as_deref(), Some("catppuccin-mocha"));
    assert_eq!(
        andre_core::AppState::from_config(reloaded).theme,
        andre_core::Theme::CatppuccinMocha
    );
}

#[test]
fn test_theme_cycled_with_t_key_persists_to_disk() {
    let (mut app, tmp) = make_test_app();
    let config_path = tmp.path().join("andre.yml");
    navigate_to_config(&mut app);
    app.component_dispatch(key_char('t'));
    save_and_exit(&mut app);
    let reloaded = andre_core::Config::load(&config_path).unwrap();
    assert_eq!(reloaded.global.theme.as_deref(), Some("dracula"));
}
