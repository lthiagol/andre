//! Behaviour of the settings screen: the list, the pending copy, the dialogs and the save
//! flow. Each test runs on a temp folder, so none touches the real config.

use andre_core::config::Config;
use andre_core::{IconSet, StowAction, StowStatus, Theme};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers, MouseEvent, MouseEventKind};
use ratatui::{layout::Rect, text::Line};

use crate::ui::detail::{detail_inner, detail_rows, Detail, WHEEL_ROWS};
use crate::ui::layout::{screen_rects, Shape};

use super::*;
use crate::components::browser::Browser;
use crate::components::{AppContext, Component, InputMode, ToastKind, Transition};

/// A config whose packages folder is `dotfiles`, holding one package `nvim`, and a `pick`
/// folder beside it, all in the temp folder, which is also the home folder. The config file
/// is `andre.yml` in the temp folder.
fn fixture() -> (tempfile::TempDir, AppContext) {
    let tmp = tempfile::TempDir::new().unwrap();
    std::fs::create_dir_all(tmp.path().join("dotfiles/nvim")).unwrap();
    std::fs::create_dir_all(tmp.path().join("pick")).unwrap();
    let mut config = Config::default_empty();
    config.set_packages(Some(tmp.path().join("dotfiles").display().to_string()));
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

/// Puts the folder picker's cursor on the folder called `name`.
fn pick_entry(component: &mut SettingsComponent, name: &str) {
    let Some(Overlay::Browse { browser }) = component.overlays.last_mut() else {
        panic!("no folder picker open");
    };
    browser.cursor = browser
        .entries()
        .iter()
        .position(|entry| entry.name == name)
        .unwrap();
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

    // Up from the first row wraps to the last row; the headers above it are passed over.
    press(&mut component, &mut ctx, KeyCode::Up);
    assert_eq!(component.selected().unwrap().label, "global ignore");
    press(&mut component, &mut ctx, KeyCode::Down);
    assert_eq!(component.selected().unwrap().label, "action");

    // Down from `icons` skips the config header and lands on `packages`.
    select(&mut component, "icons");
    press(&mut component, &mut ctx, KeyCode::Down);
    assert_eq!(component.selected().unwrap().label, "packages");
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
    assert_eq!(headers, ["stow options", "appearance", "config"]);
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
fn test_x_on_a_setting_does_nothing() {
    let (_tmp, mut ctx) = fixture();
    let mut component = open(&mut ctx);
    select(&mut component, "theme");
    press(&mut component, &mut ctx, KeyCode::Char('x'));
    assert!(component.overlays.is_empty());
}

#[test]
fn test_plus_and_x_do_nothing_on_the_packages_row() {
    let (_tmp, mut ctx) = fixture();
    let mut component = open(&mut ctx);
    select(&mut component, "packages");
    press(&mut component, &mut ctx, KeyCode::Char('+'));
    press(&mut component, &mut ctx, KeyCode::Char('x'));
    assert!(component.overlays.is_empty());
    assert!(!component.dirty);
}

#[test]
fn test_enter_on_packages_opens_the_folder_picker() {
    let (_tmp, mut ctx) = fixture();
    let mut component = open(&mut ctx);
    select(&mut component, "packages");
    press(&mut component, &mut ctx, KeyCode::Enter);

    assert!(matches!(
        component.overlays.last(),
        Some(Overlay::Browse { .. })
    ));
    assert_eq!(component.input_mode(), InputMode::Modal);
}

#[test]
fn test_choosing_a_folder_sets_the_packages_folder_on_the_pending_copy() {
    let (tmp, mut ctx) = fixture();
    let mut component = open(&mut ctx);
    select(&mut component, "packages");
    press(&mut component, &mut ctx, KeyCode::Enter);
    pick_entry(&mut component, "pick");
    press(&mut component, &mut ctx, KeyCode::Enter);

    let chosen = tmp.path().join("pick").display().to_string();
    assert!(component.overlays.is_empty());
    assert_eq!(
        pending(&component).config.packages.as_deref(),
        Some(chosen.as_str())
    );
    assert!(component.dirty);
    assert_eq!(
        ctx.core.config.packages.as_deref(),
        Some(tmp.path().join("dotfiles").display().to_string().as_str()),
        "saved packages folder changed before w"
    );
}

#[test]
fn test_esc_in_the_folder_picker_changes_nothing() {
    let (_tmp, mut ctx) = fixture();
    let mut component = open(&mut ctx);
    select(&mut component, "packages");
    press(&mut component, &mut ctx, KeyCode::Enter);
    pick_entry(&mut component, "pick");
    press(&mut component, &mut ctx, KeyCode::Esc);

    assert!(component.overlays.is_empty());
    assert!(!component.dirty);
    assert_eq!(
        pending(&component).config.packages,
        ctx.core.config.packages
    );
}

#[test]
fn test_choosing_the_folder_already_set_is_not_a_change() {
    let (_tmp, mut ctx) = fixture();
    let mut component = open(&mut ctx);
    select(&mut component, "packages");
    press(&mut component, &mut ctx, KeyCode::Enter);
    // The picker opens on the set folder, `dotfiles`, so Enter keeps it.
    press(&mut component, &mut ctx, KeyCode::Enter);

    assert!(component.overlays.is_empty());
    assert!(!component.dirty);
}

#[test]
fn test_choosing_a_relative_folder_that_is_already_set_keeps_its_text() {
    let (_tmp, mut ctx) = fixture();
    ctx.core.config.set_packages(Some("dotfiles".to_string()));
    let mut component = open(&mut ctx);
    select(&mut component, "packages");
    press(&mut component, &mut ctx, KeyCode::Enter);
    press(&mut component, &mut ctx, KeyCode::Enter);

    assert!(!component.dirty, "the same folder is not a change");
    assert_eq!(
        pending(&component).config.packages.as_deref(),
        Some("dotfiles")
    );
}

#[test]
fn test_packages_row_says_not_set_until_a_folder_is_chosen() {
    let tmp = tempfile::TempDir::new().unwrap();
    let mut ctx = AppContext::for_tests(Config::default_empty(), tmp.path());
    let component = open(&mut ctx);
    let row = component
        .items
        .iter()
        .find(|item| item.row == Row::Packages)
        .unwrap();
    assert_eq!(row.value, "not set");
}

#[test]
fn test_packages_detail_counts_the_packages_and_says_where_links_go() {
    let (_tmp, mut ctx) = fixture();
    let component = open(&mut ctx);
    let detail = build::detail(&Row::Packages, pending(&component), &ctx);

    let labels: Vec<&str> = detail.facts.iter().map(|(l, _)| l.as_ref()).collect();
    assert_eq!(labels, ["now", "saved", "packages", "links to"]);
    let value = |label: &str| {
        detail
            .facts
            .iter()
            .find(|(l, _)| l == label)
            .map(|(_, v)| v.as_str())
            .unwrap()
    };
    assert_eq!(value("packages"), "1", "the one package, nvim");
    assert_eq!(value("links to"), "~", "the home folder is the temp folder");
}

#[test]
fn test_packages_detail_marks_a_target_set_in_andre_yml() {
    let (_tmp, mut ctx) = fixture();
    ctx.core.config.target = Some("~/elsewhere".to_string());
    let component = open(&mut ctx);
    let detail = build::detail(&Row::Packages, pending(&component), &ctx);

    let links = detail
        .facts
        .iter()
        .find(|(l, _)| l == "links to")
        .map(|(_, v)| v.as_str())
        .unwrap();
    assert!(links.ends_with("(target: in andre.yml)"), "{links}");
}

#[test]
fn test_on_packages_starts_the_cursor_on_the_packages_row() {
    let (_tmp, mut ctx) = fixture();
    let mut component = SettingsComponent::on_packages();
    component.update(&mut ctx);
    assert_eq!(component.selected().unwrap().row, Row::Packages);
}

#[test]
fn test_saving_a_new_packages_folder_drops_the_status_cache() {
    let (_tmp, mut ctx) = fixture();
    ctx.status_cache
        .borrow_mut()
        .insert("nvim".to_string(), StowStatus::Stowed);
    let mut component = open(&mut ctx);
    select(&mut component, "packages");
    press(&mut component, &mut ctx, KeyCode::Enter);
    pick_entry(&mut component, "pick");
    press(&mut component, &mut ctx, KeyCode::Enter);
    press(&mut component, &mut ctx, KeyCode::Char('w'));

    assert!(!component.dirty);
    assert_eq!(
        ctx.status_cache.borrow().get("nvim"),
        None,
        "the old folder's status must not show for the new folder"
    );
}

#[test]
fn test_save_refuses_the_home_folder_as_the_packages_folder() {
    let (tmp, mut ctx) = fixture();
    let before = "global: {}\n";
    std::fs::write(&ctx.config_path, before).unwrap();
    let home_name = tmp
        .path()
        .file_name()
        .unwrap()
        .to_string_lossy()
        .to_string();
    let mut component = open(&mut ctx);
    select(&mut component, "packages");
    press(&mut component, &mut ctx, KeyCode::Enter);
    // Left to the folder above the temp (home) folder, then choose the home folder.
    press(&mut component, &mut ctx, KeyCode::Left);
    pick_entry(&mut component, &home_name);
    press(&mut component, &mut ctx, KeyCode::Enter);
    press(&mut component, &mut ctx, KeyCode::Char('w'));

    assert_eq!(
        std::fs::read_to_string(&ctx.config_path).unwrap(),
        before,
        "a file that would not load is not written"
    );
    assert!(component.dirty, "the edit stays pending");
    assert_eq!(
        ctx.core.config.packages.as_deref(),
        Some(tmp.path().join("dotfiles").display().to_string().as_str()),
        "the saved packages folder is unchanged"
    );
    let toast = ctx.toast.as_ref().expect("error toast");
    assert!(matches!(toast.kind, ToastKind::Error));
    assert!(
        toast.message.starts_with("Save failed:"),
        "unexpected toast: {}",
        toast.message
    );
}

#[test]
fn test_save_refuses_a_packages_folder_holding_the_target() {
    let (tmp, mut ctx) = fixture();
    ctx.core.config.target = Some(tmp.path().join("pick/links").display().to_string());
    let mut component = open(&mut ctx);
    select(&mut component, "packages");
    press(&mut component, &mut ctx, KeyCode::Enter);
    pick_entry(&mut component, "pick");
    press(&mut component, &mut ctx, KeyCode::Enter);
    press(&mut component, &mut ctx, KeyCode::Char('w'));

    assert!(
        !ctx.config_path.exists(),
        "a file that would not load is not written"
    );
    assert!(component.dirty, "the edit stays pending");
    let toast = ctx.toast.as_ref().expect("error toast");
    assert!(
        toast.message.starts_with("Save failed:"),
        "unexpected toast: {}",
        toast.message
    );
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

    select(&mut component, "action");
    press(&mut component, &mut ctx, KeyCode::Enter);
    assert_eq!(component.input_mode(), InputMode::Modal);

    press(&mut component, &mut ctx, KeyCode::Esc);
    select(&mut component, "global ignore");
    press(&mut component, &mut ctx, KeyCode::Enter);
    press(&mut component, &mut ctx, KeyCode::Char('+'));
    assert_eq!(component.input_mode(), InputMode::TextInput);
}

#[test]
fn test_browser_helper_opens_on_the_current_packages_folder() {
    let (tmp, _ctx) = fixture();
    let browser: Browser = overlays::browser_at(Some(tmp.path().join("dotfiles")), tmp.path());
    assert_eq!(browser.path, tmp.path());
    assert_eq!(browser.entries()[browser.cursor].name, "dotfiles");
}

#[test]
fn test_hints_follow_the_selected_row_and_the_open_dialog() {
    let (_tmp, mut ctx) = fixture();
    let mut component = open(&mut ctx);
    let labels = |c: &SettingsComponent| -> Vec<String> {
        c.hints(&ctx_for_hints())
            .iter()
            .map(|h| h.label.to_string())
            .collect()
    };
    select(&mut component, "packages");
    assert_eq!(labels(&component), ["change", "save", "back"]);

    press(&mut component, &mut ctx, KeyCode::Enter);
    assert!(labels(&component).contains(&"choose".to_string()));
}

/// `hints` takes a context it does not read; any context will do.
fn ctx_for_hints() -> AppContext {
    AppContext::for_tests(Config::default_empty(), std::path::Path::new("/"))
}

#[test]
fn test_hints_on_the_packages_row_say_change_save_back() {
    let (_tmp, mut ctx) = fixture();
    let mut component = open(&mut ctx);
    select(&mut component, "packages");
    let hints: Vec<(String, String)> = component
        .hints(&ctx)
        .iter()
        .map(|h| (h.key.to_string(), h.label.to_string()))
        .collect();
    let expected = [("Enter", "change"), ("w", "save"), ("Esc", "back")];
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
