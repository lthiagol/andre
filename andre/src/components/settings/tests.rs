//! Behaviour of the settings screen: the list, the pending copy, the dialogs and the save
//! flow. Each test runs on a temp folder, so none touches the real config.

use andre_core::config::Config;
use andre_core::{IconSet, StowAction, Theme};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers, MouseEvent, MouseEventKind};
use ratatui::{layout::Rect, text::Line};

use crate::ui::detail::{detail_inner, detail_rows, Detail, WHEEL_ROWS};
use crate::ui::layout::{screen_rects, Shape};

use super::pending::GroupChange;
use super::*;
use crate::components::browser::Browser;
use crate::components::{AppContext, Component, InputMode, Transition};

/// A config with one group `dots` whose source and target exist, plus a `pick` folder in the
/// home directory. The config file is `andre.yml` in the temp folder.
fn fixture() -> (tempfile::TempDir, AppContext) {
    let tmp = tempfile::TempDir::new().unwrap();
    for dir in ["src", "tgt", "pick"] {
        std::fs::create_dir_all(tmp.path().join(dir)).unwrap();
    }
    let mut config = Config::default_empty();
    config.add_group(
        "dots".into(),
        tmp.path().join("src").display().to_string(),
        tmp.path().join("tgt").display().to_string(),
    );
    let ctx = AppContext::for_tests(config, tmp.path());
    (tmp, ctx)
}

/// A settings screen after its first update, as the app runs it.
fn open(ctx: &mut AppContext) -> SettingsComponent {
    let mut component = SettingsComponent::default();
    component.update(ctx);
    component
}

fn press(component: &mut SettingsComponent, ctx: &mut AppContext, code: KeyCode) -> Transition {
    component.handle_key(KeyEvent::new(code, KeyModifiers::NONE), ctx)
}

fn type_text(component: &mut SettingsComponent, ctx: &mut AppContext, text: &str) {
    for c in text.chars() {
        press(component, ctx, KeyCode::Char(c));
    }
}

/// Moves the cursor onto the row labelled `label`.
fn select(component: &mut SettingsComponent, label: &str) {
    component.list.cursor = component
        .items
        .iter()
        .position(|item| item.label == label)
        .unwrap_or_else(|| panic!("no row labelled {label}"));
}

fn pending(component: &SettingsComponent) -> &PendingSettings {
    component.pending.as_ref().unwrap()
}

fn saved_yaml(ctx: &AppContext) -> Config {
    Config::load(&ctx.config_path).unwrap()
}

#[test]
fn test_cursor_skips_headers_in_both_directions() {
    let (_tmp, mut ctx) = fixture();
    let mut component = open(&mut ctx);
    assert_eq!(component.selected().unwrap().label, "action");

    // Up from the first row wraps to the last row, the group; the headers above it are
    // passed over.
    press(&mut component, &mut ctx, KeyCode::Up);
    assert!(matches!(
        component.selected().unwrap().row,
        Row::Group(ref name) if name == "dots"
    ));
    press(&mut component, &mut ctx, KeyCode::Down);
    assert_eq!(component.selected().unwrap().label, "action");

    // Down from the config section skips the groups header and lands on the group.
    select(&mut component, "global ignore");
    press(&mut component, &mut ctx, KeyCode::Down);
    assert_eq!(component.selected().unwrap().label, "dots");
}

#[test]
fn test_list_has_the_section_headers_in_order() {
    let (_tmp, mut ctx) = fixture();
    let component = open(&mut ctx);
    let headers: Vec<&str> = component
        .items
        .iter()
        .filter(|item| item.row == Row::Header)
        .map(|item| item.label.as_str())
        .collect();
    assert_eq!(headers, ["stow options", "appearance", "config", "groups"]);
}

#[test]
fn test_space_on_dry_run_changes_only_the_pending_copy() {
    let (_tmp, mut ctx) = fixture();
    let mut component = open(&mut ctx);
    select(&mut component, "dry-run");
    press(&mut component, &mut ctx, KeyCode::Char(' '));

    assert!(pending(&component).flags.dry_run);
    assert!(!ctx.core.flags.dry_run, "saved flags changed before w");
    assert!(component.dirty);
}

#[test]
fn test_w_writes_andre_yml_and_clears_dirty() {
    let (_tmp, mut ctx) = fixture();
    let mut component = open(&mut ctx);
    select(&mut component, "verbose");
    press(&mut component, &mut ctx, KeyCode::Enter);
    press(&mut component, &mut ctx, KeyCode::Char('w'));

    assert!(!component.dirty);
    assert_eq!(saved_yaml(&ctx).stow_flags().verbosity, 1);
    assert_eq!(ctx.core.flags.verbosity, 1);
}

#[test]
fn test_esc_when_clean_leaves_the_screen() {
    let (_tmp, mut ctx) = fixture();
    let mut component = open(&mut ctx);
    assert!(matches!(
        press(&mut component, &mut ctx, KeyCode::Esc),
        Transition::Pop
    ));
}

#[test]
fn test_esc_when_dirty_asks_and_n_discards() {
    let (_tmp, mut ctx) = fixture();
    let mut component = open(&mut ctx);
    select(&mut component, "dry-run");
    press(&mut component, &mut ctx, KeyCode::Enter);

    assert!(matches!(
        press(&mut component, &mut ctx, KeyCode::Esc),
        Transition::Handled
    ));
    assert!(matches!(component.overlays.last(), Some(Overlay::Leave)));
    assert_eq!(component.input_mode(), InputMode::Modal);

    assert!(matches!(
        press(&mut component, &mut ctx, KeyCode::Char('n')),
        Transition::Pop
    ));
    assert!(!ctx.core.flags.dry_run);
    assert!(!ctx.config_path.exists(), "discard must not write the file");
}

#[test]
fn test_esc_when_dirty_then_y_saves_and_leaves() {
    let (_tmp, mut ctx) = fixture();
    let mut component = open(&mut ctx);
    select(&mut component, "dry-run");
    press(&mut component, &mut ctx, KeyCode::Enter);
    press(&mut component, &mut ctx, KeyCode::Esc);

    assert!(matches!(
        press(&mut component, &mut ctx, KeyCode::Char('y')),
        Transition::Pop
    ));
    assert!(ctx.core.flags.dry_run);
    assert!(saved_yaml(&ctx).stow_flags().dry_run);
}

#[test]
fn test_esc_on_leave_dialog_stays_and_keeps_edits() {
    let (_tmp, mut ctx) = fixture();
    let mut component = open(&mut ctx);
    select(&mut component, "dry-run");
    press(&mut component, &mut ctx, KeyCode::Enter);
    press(&mut component, &mut ctx, KeyCode::Esc);
    press(&mut component, &mut ctx, KeyCode::Esc);

    assert!(component.overlays.is_empty());
    assert!(component.dirty);
    assert!(pending(&component).flags.dry_run);
}

#[test]
fn test_theme_picker_previews_and_esc_reverts() {
    let (_tmp, mut ctx) = fixture();
    let mut component = open(&mut ctx);
    select(&mut component, "theme");
    press(&mut component, &mut ctx, KeyCode::Enter);
    press(&mut component, &mut ctx, KeyCode::Down);
    let previewed = pending(&component).theme;
    assert_ne!(
        previewed,
        Theme::Default,
        "moving should preview the next theme"
    );

    press(&mut component, &mut ctx, KeyCode::Esc);
    assert!(component.overlays.is_empty());
    assert_eq!(pending(&component).theme, Theme::Default);
    assert!(!component.dirty);
}

#[test]
fn test_theme_picker_enter_keeps_the_previewed_theme() {
    let (_tmp, mut ctx) = fixture();
    let mut component = open(&mut ctx);
    select(&mut component, "theme");
    press(&mut component, &mut ctx, KeyCode::Enter);
    press(&mut component, &mut ctx, KeyCode::Down);
    let chosen = pending(&component).theme;
    press(&mut component, &mut ctx, KeyCode::Enter);

    assert!(component.overlays.is_empty());
    assert_eq!(pending(&component).theme, chosen);
    assert!(component.dirty);
}

#[test]
fn test_icons_picker_previews_and_esc_reverts() {
    let (_tmp, mut ctx) = fixture();
    let mut component = open(&mut ctx);
    select(&mut component, "icons");
    press(&mut component, &mut ctx, KeyCode::Enter);
    assert!(matches!(
        component.overlays.last(),
        Some(Overlay::Icons { .. })
    ));

    press(&mut component, &mut ctx, KeyCode::Down);
    assert_eq!(
        pending(&component).icons,
        IconSet::Unicode,
        "moving previews"
    );
    press(&mut component, &mut ctx, KeyCode::Esc);
    assert_eq!(pending(&component).icons, IconSet::Nerd);
    assert!(!component.dirty);
}

#[test]
fn test_icons_picker_enter_keeps_the_choice() {
    let (_tmp, mut ctx) = fixture();
    let mut component = open(&mut ctx);
    select(&mut component, "icons");
    press(&mut component, &mut ctx, KeyCode::Enter);
    press(&mut component, &mut ctx, KeyCode::Down);
    press(&mut component, &mut ctx, KeyCode::Enter);

    assert!(component.overlays.is_empty());
    assert_eq!(pending(&component).icons, IconSet::Unicode);
    assert!(component.dirty);
}

#[test]
fn test_action_picker_enter_sets_the_action() {
    let (_tmp, mut ctx) = fixture();
    let mut component = open(&mut ctx);
    select(&mut component, "action");
    press(&mut component, &mut ctx, KeyCode::Enter);
    press(&mut component, &mut ctx, KeyCode::Down);
    press(&mut component, &mut ctx, KeyCode::Enter);

    assert_eq!(pending(&component).flags.action, StowAction::Restow);
    assert!(component.dirty);
}

#[test]
fn test_s_cycles_the_action_on_the_pending_copy() {
    let (_tmp, mut ctx) = fixture();
    let mut component = open(&mut ctx);
    press(&mut component, &mut ctx, KeyCode::Char('s'));
    assert_eq!(pending(&component).flags.action, StowAction::Restow);
    assert_eq!(ctx.core.flags.action, StowAction::Stow);
}

#[test]
fn test_plus_adds_a_group_through_name_source_and_target() {
    let (tmp, mut ctx) = fixture();
    let mut component = open(&mut ctx);
    press(&mut component, &mut ctx, KeyCode::Char('+'));
    assert_eq!(component.input_mode(), InputMode::TextInput);
    type_text(&mut component, &mut ctx, "work");
    press(&mut component, &mut ctx, KeyCode::Enter);

    // The source picker opens at home; choose `pick`.
    pick_entry(&mut component, "pick");
    press(&mut component, &mut ctx, KeyCode::Enter);
    pick_entry(&mut component, "pick");
    press(&mut component, &mut ctx, KeyCode::Enter);

    assert!(component.overlays.is_empty());
    let group = &pending(&component).config.groups["work"];
    assert_eq!(group.source, tmp.path().join("pick").display().to_string());
    assert!(component.dirty);
}

/// Sets the top browser's cursor onto the folder named `name`.
fn pick_entry(component: &mut SettingsComponent, name: &str) {
    let Some(Overlay::Browse { browser, .. }) = component.overlays.last_mut() else {
        panic!("no folder picker open");
    };
    browser.cursor = browser
        .entries()
        .iter()
        .position(|entry| entry.name == name)
        .unwrap();
}

#[test]
fn test_name_dialog_refuses_empty_taken_and_slashed_names() {
    let (_tmp, mut ctx) = fixture();
    let mut component = open(&mut ctx);
    press(&mut component, &mut ctx, KeyCode::Char('+'));

    press(&mut component, &mut ctx, KeyCode::Enter);
    assert!(matches!(
        component.overlays.last(),
        Some(Overlay::Name { .. })
    ));

    type_text(&mut component, &mut ctx, "dots");
    press(&mut component, &mut ctx, KeyCode::Enter);
    let Some(Overlay::Name { field, .. }) = component.overlays.last() else {
        panic!("name dialog closed on a taken name");
    };
    assert!(field.error.as_deref().unwrap().contains("already"));

    for _ in 0..4 {
        press(&mut component, &mut ctx, KeyCode::Backspace);
    }
    type_text(&mut component, &mut ctx, "a/b");
    press(&mut component, &mut ctx, KeyCode::Enter);
    let Some(Overlay::Name { field, .. }) = component.overlays.last() else {
        panic!("name dialog closed on a slashed name");
    };
    assert!(field.error.as_deref().unwrap().contains('/'));
}

#[test]
fn test_esc_in_the_source_picker_goes_back_to_the_name() {
    let (_tmp, mut ctx) = fixture();
    let mut component = open(&mut ctx);
    press(&mut component, &mut ctx, KeyCode::Char('+'));
    type_text(&mut component, &mut ctx, "work");
    press(&mut component, &mut ctx, KeyCode::Enter);
    press(&mut component, &mut ctx, KeyCode::Esc);

    let Some(Overlay::Name { field, .. }) = component.overlays.last() else {
        panic!("esc should return to the name dialog");
    };
    assert_eq!(field.value, "work");
}

#[test]
fn test_group_menu_enter_on_name_renames_and_the_menu_follows() {
    let (_tmp, mut ctx) = fixture();
    let mut component = open(&mut ctx);
    select(&mut component, "dots");
    press(&mut component, &mut ctx, KeyCode::Enter);
    assert!(matches!(
        component.overlays.last(),
        Some(Overlay::Group { .. })
    ));

    press(&mut component, &mut ctx, KeyCode::Enter); // name row: rename dialog
    for _ in 0..4 {
        press(&mut component, &mut ctx, KeyCode::Backspace);
    }
    type_text(&mut component, &mut ctx, "files");
    press(&mut component, &mut ctx, KeyCode::Enter);

    let Some(Overlay::Group { group, .. }) = component.overlays.last() else {
        panic!("group menu should be back on top");
    };
    assert_eq!(group, "files");
    assert!(pending(&component).config.groups.contains_key("files"));
}

#[test]
fn test_group_menu_esc_restores_the_group_as_it_was() {
    let (_tmp, mut ctx) = fixture();
    let mut component = open(&mut ctx);
    select(&mut component, "dots");
    press(&mut component, &mut ctx, KeyCode::Enter);
    press(&mut component, &mut ctx, KeyCode::Enter); // rename
    press(&mut component, &mut ctx, KeyCode::Backspace);
    press(&mut component, &mut ctx, KeyCode::Enter);
    press(&mut component, &mut ctx, KeyCode::Esc); // cancel the group menu

    assert!(component.overlays.is_empty());
    assert!(pending(&component).config.groups.contains_key("dots"));
    assert!(!component.dirty);
}

#[test]
fn test_group_menu_done_keeps_the_edits() {
    let (_tmp, mut ctx) = fixture();
    let mut component = open(&mut ctx);
    select(&mut component, "dots");
    press(&mut component, &mut ctx, KeyCode::Enter);
    press(&mut component, &mut ctx, KeyCode::Enter); // rename
    press(&mut component, &mut ctx, KeyCode::Backspace);
    type_text(&mut component, &mut ctx, "x");
    press(&mut component, &mut ctx, KeyCode::Enter); // Name done
    for _ in 0..GROUP_DONE {
        press(&mut component, &mut ctx, KeyCode::Down);
    }
    press(&mut component, &mut ctx, KeyCode::Enter); // done row

    assert!(component.overlays.is_empty());
    assert!(pending(&component).config.groups.contains_key("dotx"));
    assert!(component.dirty);
}

#[test]
fn test_x_on_a_group_confirms_then_y_removes_it() {
    let (_tmp, mut ctx) = fixture();
    let mut component = open(&mut ctx);
    select(&mut component, "dots");
    press(&mut component, &mut ctx, KeyCode::Char('x'));
    assert!(matches!(
        component.overlays.last(),
        Some(Overlay::Remove { .. })
    ));
    press(&mut component, &mut ctx, KeyCode::Char('n'));
    assert!(pending(&component).config.groups.contains_key("dots"));

    press(&mut component, &mut ctx, KeyCode::Char('x'));
    press(&mut component, &mut ctx, KeyCode::Char('y'));
    assert!(!pending(&component).config.groups.contains_key("dots"));
    assert!(component.dirty);
}

/// Writes `profiles/work.yml` under `tmp` with `packages` and loads it into `ctx`.
fn load_work_profile(ctx: &mut AppContext, tmp: &tempfile::TempDir, packages: &str) {
    let dir = tmp.path().join("profiles");
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("work.yml"), format!("packages: [{packages}]\n")).unwrap();
    ctx.core.profiles = andre_core::ProfileSet::load(&dir).unwrap();
}

/// Renames group `dots` to `files` through the group menu, then leaves the menu with done.
fn rename_dots_to_files(component: &mut SettingsComponent, ctx: &mut AppContext) {
    select(component, "dots");
    press(component, ctx, KeyCode::Enter); // group menu
    press(component, ctx, KeyCode::Enter); // name row: rename
    for _ in 0..4 {
        press(component, ctx, KeyCode::Backspace);
    }
    type_text(component, ctx, "files");
    press(component, ctx, KeyCode::Enter);
    for _ in 0..GROUP_DONE {
        press(component, ctx, KeyCode::Down);
    }
    press(component, ctx, KeyCode::Enter); // done
}

#[test]
fn test_x_after_a_rename_counts_the_refs_the_saved_profiles_hold() {
    let (tmp, mut ctx) = fixture();
    load_work_profile(&mut ctx, &tmp, "dots/git, other/vim");
    let mut component = open(&mut ctx);
    rename_dots_to_files(&mut component, &mut ctx);
    select_group(&mut component, "files");
    press(&mut component, &mut ctx, KeyCode::Char('x'));

    let Some(Overlay::Remove { group, refs }) = component.overlays.last() else {
        panic!("x should open the remove dialog");
    };
    assert_eq!(group, "files");
    assert_eq!(*refs, 1);
}

#[test]
fn test_group_detail_after_a_rename_counts_the_saved_profile_refs() {
    let (tmp, mut ctx) = fixture();
    load_work_profile(&mut ctx, &tmp, "dots/git, other/vim");
    let mut component = open(&mut ctx);
    rename_dots_to_files(&mut component, &mut ctx);

    let detail = build::detail(&Row::Group("files".into()), pending(&component), &ctx);
    let profiles = detail
        .facts
        .iter()
        .find(|(label, _)| label.as_ref() == "profiles")
        .map(|(_, value)| value.as_str());
    assert_eq!(profiles, Some("used by 1"));
}

#[test]
fn test_refs_into_undoes_renames_newest_first() {
    let (tmp, mut ctx) = fixture();
    load_work_profile(&mut ctx, &tmp, "dots/git, other/vim");
    let mut pending = PendingSettings::from_core(&ctx.core);
    pending.group_changes = vec![
        GroupChange::Rename {
            from: "dots".into(),
            to: "files".into(),
        },
        GroupChange::Rename {
            from: "files".into(),
            to: "home".into(),
        },
    ];
    assert_eq!(build::refs_into(&ctx, &pending, "home"), 1);

    pending.group_changes.push(GroupChange::Rename {
        from: "home".into(),
        to: "dots".into(),
    });
    assert_eq!(build::refs_into(&ctx, &pending, "dots"), 1);
}

#[test]
fn test_x_on_a_setting_does_nothing() {
    let (_tmp, mut ctx) = fixture();
    let mut component = open(&mut ctx);
    select(&mut component, "theme");
    press(&mut component, &mut ctx, KeyCode::Char('x'));
    assert!(component.overlays.is_empty());
}

#[test]
fn test_global_ignore_add_edit_and_delete_a_pattern() {
    let (_tmp, mut ctx) = fixture();
    let mut component = open(&mut ctx);
    select(&mut component, "global ignore");
    press(&mut component, &mut ctx, KeyCode::Enter);

    press(&mut component, &mut ctx, KeyCode::Char('+'));
    type_text(&mut component, &mut ctx, "*.bak");
    press(&mut component, &mut ctx, KeyCode::Enter);
    assert_eq!(pending(&component).config.global_ignore(), ["*.bak"]);

    // The cursor is on the new pattern; Enter edits it in place.
    press(&mut component, &mut ctx, KeyCode::Enter);
    for _ in 0..5 {
        press(&mut component, &mut ctx, KeyCode::Backspace);
    }
    type_text(&mut component, &mut ctx, "*.tmp");
    press(&mut component, &mut ctx, KeyCode::Enter);
    assert_eq!(pending(&component).config.global_ignore(), ["*.tmp"]);

    press(&mut component, &mut ctx, KeyCode::Char('x'));
    assert!(pending(&component).config.global_ignore().is_empty());
    assert!(component.dirty);
}

#[test]
fn test_blank_line_removes_a_pattern_and_an_empty_add_adds_nothing() {
    let (_tmp, mut ctx) = fixture();
    let mut component = open(&mut ctx);
    select(&mut component, "global ignore");
    press(&mut component, &mut ctx, KeyCode::Enter);

    // Enter on the add row, then Enter on the blank line: nothing is added.
    press(&mut component, &mut ctx, KeyCode::Enter);
    press(&mut component, &mut ctx, KeyCode::Enter);
    assert!(pending(&component).config.global_ignore().is_empty());

    press(&mut component, &mut ctx, KeyCode::Char('+'));
    type_text(&mut component, &mut ctx, ".git");
    press(&mut component, &mut ctx, KeyCode::Enter);
    press(&mut component, &mut ctx, KeyCode::Enter);
    for _ in 0..4 {
        press(&mut component, &mut ctx, KeyCode::Backspace);
    }
    press(&mut component, &mut ctx, KeyCode::Enter);
    assert!(pending(&component).config.global_ignore().is_empty());
}

#[test]
fn test_group_ignore_is_edited_from_the_group_menu() {
    let (_tmp, mut ctx) = fixture();
    let mut component = open(&mut ctx);
    select(&mut component, "dots");
    press(&mut component, &mut ctx, KeyCode::Enter);
    for _ in 0..GROUP_IGNORE {
        press(&mut component, &mut ctx, KeyCode::Down);
    }
    press(&mut component, &mut ctx, KeyCode::Enter); // ignore row
    assert!(matches!(
        component.overlays.last(),
        Some(Overlay::Ignore { scope: IgnoreScope::Group(group), .. }) if group == "dots"
    ));

    press(&mut component, &mut ctx, KeyCode::Char('+'));
    type_text(&mut component, &mut ctx, "*.log");
    press(&mut component, &mut ctx, KeyCode::Enter);
    press(&mut component, &mut ctx, KeyCode::Esc); // back to the group menu
    assert!(matches!(
        component.overlays.last(),
        Some(Overlay::Group { .. })
    ));

    let own = pending(&component).config.get_group_ignore("dots").unwrap();
    assert_eq!(own, ["*.log"]);
    assert!(component.dirty);
}

#[test]
fn test_cancelling_the_group_menu_also_drops_its_ignore_edits() {
    let (_tmp, mut ctx) = fixture();
    let mut component = open(&mut ctx);
    select(&mut component, "dots");
    press(&mut component, &mut ctx, KeyCode::Enter);
    for _ in 0..GROUP_IGNORE {
        press(&mut component, &mut ctx, KeyCode::Down);
    }
    press(&mut component, &mut ctx, KeyCode::Enter);
    press(&mut component, &mut ctx, KeyCode::Char('+'));
    type_text(&mut component, &mut ctx, "*.log");
    press(&mut component, &mut ctx, KeyCode::Enter);
    press(&mut component, &mut ctx, KeyCode::Esc);
    press(&mut component, &mut ctx, KeyCode::Esc); // cancel the group menu

    assert!(pending(&component)
        .config
        .get_group_ignore("dots")
        .is_none());
    assert!(!component.dirty);
}

#[test]
fn test_theme_key_cycles_and_marks_dirty() {
    let (_tmp, mut ctx) = fixture();
    let mut component = open(&mut ctx);
    press(&mut component, &mut ctx, KeyCode::Char('t'));
    assert_eq!(pending(&component).theme, Theme::Default.next());
    assert_eq!(ctx.core.theme, Theme::Default);
    assert!(component.dirty);
}

#[test]
fn test_settings_dialog_keys_take_the_input_mode_of_their_dialog() {
    let (_tmp, mut ctx) = fixture();
    let mut component = open(&mut ctx);
    assert_eq!(component.input_mode(), InputMode::Normal);

    press(&mut component, &mut ctx, KeyCode::Char('+'));
    assert_eq!(component.input_mode(), InputMode::TextInput);

    press(&mut component, &mut ctx, KeyCode::Esc);
    select(&mut component, "action");
    press(&mut component, &mut ctx, KeyCode::Enter);
    assert_eq!(component.input_mode(), InputMode::Modal);
}

#[test]
fn test_browser_helper_opens_on_the_current_source() {
    let (tmp, _ctx) = fixture();
    let browser: Browser = overlays::browser_at(Some(tmp.path().join("src")), tmp.path());
    assert_eq!(browser.path, tmp.path());
    assert_eq!(browser.entries()[browser.cursor].name, "src");
}

#[test]
fn test_hints_follow_the_selected_row_and_the_open_dialog() {
    let (_tmp, mut ctx) = fixture();
    let mut component = open(&mut ctx);
    let keys = |c: &SettingsComponent| -> Vec<String> {
        c.hints(&ctx_for_hints())
            .iter()
            .map(|h| h.label.to_string())
            .collect()
    };
    select(&mut component, "dots");
    assert!(keys(&component).contains(&"remove".to_string()));
    select(&mut component, "theme");
    assert!(!keys(&component).contains(&"remove".to_string()));

    press(&mut component, &mut ctx, KeyCode::Enter);
    assert!(keys(&component).contains(&"keep".to_string()));
}

/// `hints` takes a context it does not read; any context will do.
fn ctx_for_hints() -> AppContext {
    AppContext::for_tests(Config::default_empty(), std::path::Path::new("/"))
}

/// Moves the cursor onto the row of group `name`. A group and a section can share a label.
fn select_group(component: &mut SettingsComponent, name: &str) {
    component.list.cursor = component
        .items
        .iter()
        .position(|item| item.row == Row::Group(name.to_string()))
        .unwrap_or_else(|| panic!("no group row {name}"));
}

/// Two groups, `config` and `home`, so a rename can change the sort order.
fn two_group_fixture() -> (tempfile::TempDir, AppContext) {
    let tmp = tempfile::TempDir::new().unwrap();
    let mut config = Config::default_empty();
    config.add_group("config".into(), "src".into(), "tgt".into());
    config.add_group("home".into(), "src".into(), "tgt".into());
    let ctx = AppContext::for_tests(config, tmp.path());
    (tmp, ctx)
}

#[test]
fn test_rename_that_changes_sort_order_moves_the_cursor_to_the_group() {
    let (_tmp, mut ctx) = two_group_fixture();
    let mut component = open(&mut ctx);
    select_group(&mut component, "config");
    press(&mut component, &mut ctx, KeyCode::Enter); // group menu
    press(&mut component, &mut ctx, KeyCode::Enter); // name row: rename
    for _ in 0..6 {
        press(&mut component, &mut ctx, KeyCode::Backspace);
    }
    type_text(&mut component, &mut ctx, "zz");
    press(&mut component, &mut ctx, KeyCode::Enter); // rename: `zz` sorts after `home`

    assert_eq!(
        component.selected().unwrap().row,
        Row::Group("zz".to_string())
    );
    assert_eq!(component.detail_title, "zz");

    for _ in 0..GROUP_DONE {
        press(&mut component, &mut ctx, KeyCode::Down);
    }
    press(&mut component, &mut ctx, KeyCode::Enter); // done
    assert_eq!(
        component.selected().unwrap().row,
        Row::Group("zz".to_string())
    );
    assert_eq!(component.detail_title, "zz");
}

#[test]
fn test_cancelled_group_menu_puts_the_cursor_back_on_its_group() {
    let (_tmp, mut ctx) = two_group_fixture();
    let mut component = open(&mut ctx);
    select_group(&mut component, "config");
    press(&mut component, &mut ctx, KeyCode::Enter); // group menu
    press(&mut component, &mut ctx, KeyCode::Enter); // name row: rename
    for _ in 0..6 {
        press(&mut component, &mut ctx, KeyCode::Backspace);
    }
    type_text(&mut component, &mut ctx, "zz");
    press(&mut component, &mut ctx, KeyCode::Enter); // rename
    press(&mut component, &mut ctx, KeyCode::Esc); // cancel the menu: undo the rename

    assert!(component.overlays.is_empty());
    assert!(pending(&component).group_changes.is_empty());
    assert_eq!(
        component.selected().unwrap().row,
        Row::Group("config".to_string())
    );
    assert_eq!(component.detail_title, "config");
}

#[test]
fn test_group_row_hints_say_edit_add_remove_save_back() {
    let (_tmp, mut ctx) = fixture();
    let mut component = open(&mut ctx);
    select(&mut component, "dots");
    let hints: Vec<(String, String)> = component
        .hints(&ctx)
        .iter()
        .map(|h| (h.key.to_string(), h.label.to_string()))
        .collect();
    let expected = [
        ("Enter", "edit"),
        ("+", "add"),
        ("x", "remove"),
        ("w", "save"),
        ("Esc", "back"),
    ];
    let expected: Vec<(String, String)> = expected
        .iter()
        .map(|(k, v)| (k.to_string(), v.to_string()))
        .collect();
    assert_eq!(hints, expected);
}

#[test]
fn test_setting_detail_puts_file_and_paths_in_a_block_after_the_facts() {
    let (_tmp, mut ctx) = fixture();
    let component = open(&mut ctx);
    let detail = build::detail(&Row::Theme, pending(&component), &ctx);

    let labels: Vec<&str> = detail.facts.iter().map(|(l, _)| l.as_ref()).collect();
    assert_eq!(labels, ["now", "saved", "key"]);
    assert_eq!(detail.sections.len(), 2, "file and paths are one block");
}

#[test]
fn test_wheel_over_the_detail_scrolls_it_three_rows() {
    let (_tmp, mut ctx) = fixture();
    let mut component = SettingsComponent::default();
    component.ensure(&ctx);
    component.detail = Detail {
        sections: (0..40).map(|i| Line::raw(format!("row{i}"))).collect(),
        ..Detail::default()
    };
    let area = Rect::new(0, 0, 80, 24);
    component.prepare(area);
    let detail = screen_rects(area, Shape::TwoPane).detail;
    let notch = |kind| MouseEvent {
        kind,
        column: detail.x + 1,
        row: detail.y + 1,
        modifiers: KeyModifiers::NONE,
    };
    component.handle_mouse(notch(MouseEventKind::ScrollDown), &mut ctx);
    assert_eq!(component.scroll, WHEEL_ROWS, "one notch is three rows");
    component.handle_mouse(notch(MouseEventKind::ScrollUp), &mut ctx);
    assert_eq!(component.scroll, 0, "and a notch up takes them back");
}

#[test]
fn test_moving_the_cursor_starts_the_detail_at_the_top() {
    let (_tmp, mut ctx) = fixture();
    let mut component = open(&mut ctx);
    let area = Rect::new(0, 0, 80, 4);
    component.prepare(area);
    let pane = screen_rects(area, Shape::TwoPane).detail;
    component.detail = Detail {
        sections: (0..40).map(|i| Line::raw(format!("row{i}"))).collect(),
        ..Detail::default()
    };
    let notch = MouseEvent {
        kind: MouseEventKind::ScrollDown,
        column: pane.x + 1,
        row: pane.y + 1,
        modifiers: KeyModifiers::NONE,
    };
    component.handle_mouse(notch, &mut ctx);
    assert_eq!(component.scroll, WHEEL_ROWS, "the first detail scrolls");

    press(&mut component, &mut ctx, KeyCode::Down);
    assert_ne!(
        component.selected().unwrap().label,
        "action",
        "the cursor moved"
    );
    let colors = ThemeColors::from_theme(ctx.core.theme);
    assert!(
        detail_rows(&component.detail, pane, &colors)
            > usize::from(WHEEL_ROWS) + usize::from(detail_inner(pane).height),
        "the next detail overflows the pane"
    );
    assert_eq!(component.scroll, 0, "the next detail starts at its top");
}
