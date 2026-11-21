use andre::app::App;
use andre::components::settings::{Overlay, SettingsComponent};
use andre_core::{Config, StowAction, Theme};

mod helpers;
use helpers::*;

fn settings_mut(app: &mut App) -> &mut SettingsComponent {
    app.component_stack
        .last_mut()
        .unwrap()
        .as_any_mut()
        .downcast_mut()
        .unwrap()
}

fn settings(app: &App) -> &SettingsComponent {
    app.component_stack
        .last()
        .unwrap()
        .as_any()
        .downcast_ref()
        .unwrap()
}

/// Opens the settings screen and runs one update, as the app's loop does before a paint.
fn open_settings(app: &mut App) {
    push_component(app, "Settings");
    app.update_components();
}

/// Moves the settings cursor onto the row labelled `label`.
fn select(app: &mut App, label: &str) {
    let component = settings_mut(app);
    component.list.cursor = component
        .items()
        .iter()
        .position(|item| item.label == label)
        .unwrap_or_else(|| panic!("no settings row labelled {label}"));
}

fn save(app: &mut App) {
    app.component_dispatch(key_char('w'));
}

fn read_config(app: &App) -> Config {
    Config::load(&app.ctx.config_path).expect("andre.yml should load")
}

fn make_app_with_yaml(yaml: &str) -> (App, tempfile::TempDir) {
    let tmp = tempfile::TempDir::new().unwrap();
    let config_path = tmp.path().join("andre.yml");
    std::fs::write(&config_path, yaml).unwrap();
    create_test_package(&tmp);
    let app = App::with_home(config_path, tmp.path().to_path_buf()).unwrap();
    (app, tmp)
}

#[test]
fn test_enter_cycles_verbosity_and_save_persists() {
    let (mut app, _tmp) = make_empty_app();
    open_settings(&mut app);
    select(&mut app, "verbose");
    app.component_dispatch(enter_key());
    assert!(settings(&app).dirty);
    save(&mut app);

    assert!(!settings(&app).dirty);
    assert_eq!(app.ctx.core.flags.verbosity, 1);
    assert_eq!(read_config(&app).stow_flags().verbosity, 1);
}

#[test]
fn test_space_on_dry_run_then_save_persists() {
    let (mut app, _tmp) = make_empty_app();
    open_settings(&mut app);
    select(&mut app, "dry-run");
    app.component_dispatch(space_key());
    assert!(!app.ctx.core.flags.dry_run, "saved only on w");
    save(&mut app);

    assert!(app.ctx.core.flags.dry_run);
    assert!(read_config(&app).stow_flags().dry_run);
}

/// `[dry_run, no_folding, adopt, dotfiles]` from the stow flags.
fn bool_flags(flags: &andre_core::StowFlags) -> [bool; 4] {
    [flags.dry_run, flags.no_folding, flags.adopt, flags.dotfiles]
}

#[test]
fn test_each_bool_flag_row_toggles_only_its_own_flag_and_saves() {
    let rows = ["dry-run", "no-fold", "adopt", "dotfiles"];
    for (i, label) in rows.iter().enumerate() {
        let (mut app, _tmp) = make_empty_app();
        open_settings(&mut app);
        select(&mut app, label);
        app.component_dispatch(enter_key());
        save(&mut app);

        let mut expected = [false; 4];
        expected[i] = true;
        assert_eq!(bool_flags(&app.ctx.core.flags), expected, "row {label}");
        assert_eq!(bool_flags(&read_config(&app).stow_flags()), expected);
    }
}

#[test]
fn test_flag_shortcuts_a_and_o_in_settings_then_save_persists() {
    let (mut app, _tmp) = make_empty_app();
    open_settings(&mut app);
    app.component_dispatch(key_char('a'));
    app.component_dispatch(key_char('o'));
    assert!(settings(&app).dirty);
    save(&mut app);

    let flags = read_config(&app).stow_flags();
    assert!(flags.adopt);
    assert!(flags.dotfiles);
}

#[test]
fn test_verbosity_wraps_at_six() {
    let (mut app, _tmp) = make_empty_app();
    open_settings(&mut app);
    select(&mut app, "verbose");
    for _ in 0..6 {
        app.component_dispatch(enter_key());
    }
    save(&mut app);
    assert_eq!(app.ctx.core.flags.verbosity, 0);
    assert_eq!(read_config(&app).stow_flags().verbosity, 0);
}

#[test]
fn test_s_cycles_action_and_save_persists() {
    let (mut app, _tmp) = make_empty_app();
    open_settings(&mut app);
    app.component_dispatch(key_char('s'));
    save(&mut app);
    assert_eq!(app.ctx.core.flags.action, StowAction::Restow);
    assert_eq!(read_config(&app).stow_flags().action, StowAction::Restow);
}

#[test]
fn test_add_group_then_save_persists() {
    let (mut app, tmp) = make_empty_app();
    let pick = tmp.path().join("pick");
    std::fs::create_dir_all(&pick).unwrap();
    open_settings(&mut app);

    app.component_dispatch(key_char('+'));
    type_text(&mut app, "work");
    app.component_dispatch(enter_key());
    choose_in_browser(&mut app, "pick");
    app.component_dispatch(enter_key());
    choose_in_browser(&mut app, "pick");
    app.component_dispatch(enter_key());
    save(&mut app);

    let config = read_config(&app);
    let group = config.groups.get("work").expect("group written");
    assert_eq!(group.source, pick.display().to_string());
}

/// Puts the top folder picker's cursor on the folder called `name`.
fn choose_in_browser(app: &mut App, name: &str) {
    let Some(Overlay::Browse { browser, .. }) = settings_mut(app).overlays.last_mut() else {
        panic!("no folder picker open");
    };
    browser.cursor = browser
        .entries()
        .iter()
        .position(|entry| entry.name == name)
        .unwrap();
}

#[test]
fn test_remove_group_via_dialog_then_save_persists() {
    let (mut app, _tmp) = make_app_with_yaml(
        "global:\n  stow:\n    verbosity: 0\ngroups:\n  home:\n    source: dotfiles/home\n    target: \"~\"\n",
    );
    open_settings(&mut app);
    select(&mut app, "home");
    app.component_dispatch(key_char('x'));
    app.component_dispatch(key_char('y'));
    save(&mut app);

    assert!(!read_config(&app).groups.contains_key("home"));
}

#[test]
fn test_rename_group_then_save_persists() {
    let (mut app, _tmp) = make_app_with_yaml(
        "global:\n  stow:\n    verbosity: 0\ngroups:\n  home:\n    source: dotfiles/home\n    target: \"~\"\n",
    );
    open_settings(&mut app);
    select(&mut app, "home");
    app.component_dispatch(enter_key()); // group menu
    app.component_dispatch(enter_key()); // name row: rename
    for _ in 0..4 {
        app.component_dispatch(backspace_key());
    }
    type_text(&mut app, "dots");
    app.component_dispatch(enter_key()); // rename
    for _ in 0..4 {
        app.component_dispatch(down_key());
    }
    app.component_dispatch(enter_key()); // done: keep the rename
    save(&mut app);

    let config = read_config(&app);
    assert!(config.groups.contains_key("dots"));
    assert!(!config.groups.contains_key("home"));
}

#[test]
fn test_config_dirty_set_on_modification() {
    let (mut app, _tmp) = make_empty_app();
    open_settings(&mut app);
    assert!(!settings(&app).dirty);
    app.component_dispatch(key_char('v'));
    assert!(settings(&app).dirty);
}

#[test]
fn test_esc_dirty_then_y_saves_and_leaves() {
    let (mut app, _tmp) = make_empty_app();
    open_settings(&mut app);
    app.component_dispatch(key_char('v'));
    app.component_dispatch(esc_key());
    assert!(matches!(
        settings(&app).overlays.last(),
        Some(Overlay::Leave)
    ));
    app.component_dispatch(key_char('y'));

    assert_eq!(component_id(&app), "MainMenu");
    assert_eq!(read_config(&app).stow_flags().verbosity, 1);
}

#[test]
fn test_esc_dirty_then_n_discards_and_leaves() {
    let (mut app, _tmp) = make_empty_app();
    open_settings(&mut app);
    app.component_dispatch(key_char('v'));
    app.component_dispatch(esc_key());
    app.component_dispatch(key_char('n'));

    assert_eq!(component_id(&app), "MainMenu");
    assert_eq!(app.ctx.core.flags.verbosity, 0);
    assert_eq!(
        read_config(&app).stow_flags().verbosity,
        0,
        "discard must not write"
    );
}

#[test]
fn test_esc_clean_leaves_without_asking() {
    let (mut app, _tmp) = make_empty_app();
    open_settings(&mut app);
    app.component_dispatch(esc_key());
    assert_eq!(component_id(&app), "MainMenu");
}

#[test]
fn test_theme_picked_in_settings_persists_to_disk() {
    let (mut app, _tmp) = make_empty_app();
    open_settings(&mut app);
    select(&mut app, "theme");
    app.component_dispatch(enter_key());
    app.component_dispatch(down_key());
    app.component_dispatch(enter_key());
    save(&mut app);

    let chosen = Theme::ALL[1];
    assert_eq!(app.ctx.core.theme, chosen);
    assert_eq!(
        read_config(&app).global.theme.as_deref(),
        Some(chosen.name())
    );
}

#[test]
fn test_theme_esc_reverts_the_preview_and_saves_nothing() {
    let (mut app, _tmp) = make_empty_app();
    open_settings(&mut app);
    select(&mut app, "theme");
    app.component_dispatch(enter_key());
    app.component_dispatch(down_key());
    app.component_dispatch(esc_key());
    save(&mut app);

    assert_eq!(app.ctx.core.theme, Theme::Default);
    assert_eq!(settings(&app).pending().unwrap().theme, Theme::Default);
}

#[test]
fn test_theme_cycled_with_t_key_persists_to_disk() {
    let (mut app, _tmp) = make_empty_app();
    open_settings(&mut app);
    app.component_dispatch(key_char('t'));
    save(&mut app);
    assert_eq!(app.ctx.core.theme, Theme::Default.next());
    assert_eq!(
        read_config(&app).global.theme.as_deref(),
        Some(Theme::Default.next().name())
    );
}

#[test]
fn test_icons_picked_in_settings_persists_to_disk() {
    let (mut app, _tmp) = make_empty_app();
    open_settings(&mut app);
    select(&mut app, "icons");
    app.component_dispatch(enter_key());
    app.component_dispatch(down_key());
    app.component_dispatch(enter_key());
    save(&mut app);

    assert_eq!(app.ctx.core.icons, andre_core::IconSet::Unicode);
    assert_eq!(read_config(&app).global.icons.as_deref(), Some("unicode"));
}

#[test]
fn test_global_ignore_pattern_saved_to_andre_yml() {
    let (mut app, _tmp) = make_empty_app();
    open_settings(&mut app);
    select(&mut app, "global ignore");
    app.component_dispatch(enter_key());
    app.component_dispatch(key_char('+'));
    type_text(&mut app, "*.bak");
    app.component_dispatch(enter_key());
    app.component_dispatch(esc_key());
    save(&mut app);

    assert_eq!(read_config(&app).global_ignore(), ["*.bak"]);
    assert_eq!(app.ctx.core.config.global_ignore(), ["*.bak"]);
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

const HOME_GROUP_YAML: &str =
    "global:\n  stow:\n    verbosity: 0\ngroups:\n  home:\n    source: dotfiles/home\n    target: \"~\"\n";

/// Writes `profiles/<name>.yml` beside the config and loads it into the app.
fn load_profile(app: &mut App, tmp: &tempfile::TempDir, name: &str, yaml: &str) {
    let dir = tmp.path().join("profiles");
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join(format!("{name}.yml")), yaml).unwrap();
    app.ctx.core.profiles = andre_core::ProfileSet::load(&dir).unwrap();
}

fn profile_packages(app: &App, name: &str) -> Vec<String> {
    app.ctx
        .core
        .profiles
        .get(name)
        .expect("profile loaded")
        .packages
        .clone()
}

fn toast_message(app: &App) -> String {
    app.ctx
        .toast
        .as_ref()
        .map(|toast| toast.message.clone())
        .unwrap_or_default()
}

/// Renames the group under the cursor from `from` to `to` through its menu, then takes
/// `done`, which keeps the edit.
fn rename_selected_group(app: &mut App, from: &str, to: &str) {
    app.component_dispatch(enter_key()); // group menu
    app.component_dispatch(enter_key()); // name row: rename
    for _ in 0..from.len() {
        app.component_dispatch(backspace_key());
    }
    type_text(app, to);
    app.component_dispatch(enter_key()); // rename
    for _ in 0..4 {
        app.component_dispatch(down_key());
    }
    app.component_dispatch(enter_key()); // done
}

#[test]
fn test_rename_group_then_save_rewrites_profile_refs() {
    let (mut app, tmp) = make_app_with_yaml(HOME_GROUP_YAML);
    load_profile(&mut app, &tmp, "work", "packages: [home/git, other/vim]\n");
    open_settings(&mut app);
    select(&mut app, "home");
    rename_selected_group(&mut app, "home", "dots");
    save(&mut app);

    assert_eq!(profile_packages(&app, "work"), ["dots/git", "other/vim"]);
    let on_disk = std::fs::read_to_string(tmp.path().join("profiles/work.yml")).unwrap();
    assert!(on_disk.contains("dots/git"));
    assert!(!on_disk.contains("home/git"));
    assert_eq!(toast_message(&app), "Config saved \u{b7} 1 profile updated");
}

#[test]
fn test_remove_group_then_save_drops_its_profile_refs() {
    let (mut app, tmp) = make_app_with_yaml(HOME_GROUP_YAML);
    load_profile(&mut app, &tmp, "work", "packages: [home/git, other/vim]\n");
    open_settings(&mut app);
    select(&mut app, "home");
    app.component_dispatch(key_char('x'));
    app.component_dispatch(key_char('y'));
    save(&mut app);

    assert_eq!(profile_packages(&app, "work"), ["other/vim"]);
    assert!(!read_config(&app).groups.contains_key("home"));
}

#[test]
fn test_rename_then_remove_then_save_drops_the_renamed_refs() {
    let (mut app, tmp) = make_app_with_yaml(HOME_GROUP_YAML);
    load_profile(&mut app, &tmp, "work", "packages: [home/git, other/vim]\n");
    open_settings(&mut app);
    select(&mut app, "home");
    rename_selected_group(&mut app, "home", "dots");
    select(&mut app, "dots");
    app.component_dispatch(key_char('x'));

    assert!(matches!(
        settings(&app).overlays.last(),
        Some(Overlay::Remove { refs: 1, .. })
    ));
    app.component_dispatch(key_char('y'));
    save(&mut app);

    assert_eq!(profile_packages(&app, "work"), ["other/vim"]);
}

#[test]
fn test_rename_then_leave_without_saving_leaves_profiles_alone() {
    let (mut app, tmp) = make_app_with_yaml(HOME_GROUP_YAML);
    load_profile(&mut app, &tmp, "work", "packages: [home/git]\n");
    open_settings(&mut app);
    select(&mut app, "home");
    rename_selected_group(&mut app, "home", "dots");
    app.component_dispatch(esc_key());
    app.component_dispatch(key_char('n'));

    assert_eq!(profile_packages(&app, "work"), ["home/git"]);
    let on_disk = std::fs::read_to_string(tmp.path().join("profiles/work.yml")).unwrap();
    assert!(on_disk.contains("home/git"));
}

#[test]
fn test_group_removed_then_added_back_keeps_its_profile_refs() {
    let (mut app, tmp) = make_app_with_yaml(HOME_GROUP_YAML);
    load_profile(&mut app, &tmp, "work", "packages: [home/git]\n");
    std::fs::create_dir_all(tmp.path().join("pick")).unwrap();
    open_settings(&mut app);
    select(&mut app, "home");
    app.component_dispatch(key_char('x'));
    app.component_dispatch(key_char('y'));

    app.component_dispatch(key_char('+'));
    type_text(&mut app, "home");
    app.component_dispatch(enter_key());
    choose_in_browser(&mut app, "pick");
    app.component_dispatch(enter_key());
    choose_in_browser(&mut app, "pick");
    app.component_dispatch(enter_key());
    save(&mut app);

    assert!(read_config(&app).groups.contains_key("home"));
    assert_eq!(profile_packages(&app, "work"), ["home/git"]);
}
