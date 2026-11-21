use std::path::Path;

use crossterm::event::{KeyCode, KeyEvent};

use crate::components::browser::{dir_entries, Browser};
use crate::components::settings::pending::PendingSettings;
use crate::components::settings::{GroupDialog, GroupEditField, SettingsComponent};
use crate::components::{AppContext, Transition};
use crate::input::step_cursor;

/// What a dialog key press leaves on screen.
enum Next {
    /// Keep the current dialog, possibly edited in place.
    Stay,
    /// Show a different dialog.
    Goto(GroupDialog),
    /// Close the dialog stack.
    Close,
}

/// The settings a dialog may change, borrowed from the component.
struct Env<'a> {
    pending: &'a mut PendingSettings,
    dirty: &'a mut bool,
    home_dir: &'a Path,
}

/// Outcome of a key on a list of groups.
enum Pick {
    Stay,
    Cancel,
    Chosen(usize),
}

pub fn handle_dialog_key(
    component: &mut SettingsComponent,
    key: KeyEvent,
    ctx: &mut AppContext,
) -> Transition {
    let pending = component.pending.as_mut().unwrap();
    let Some(dialog) = component.dialog.as_mut() else {
        return Transition::Handled;
    };
    let mut env = Env {
        pending,
        dirty: &mut component.config_dirty,
        home_dir: &ctx.home_dir,
    };

    let next = match dialog {
        GroupDialog::Actions { cursor } => actions_key(cursor, &key, &env),
        GroupDialog::AddName { name } => add_name_key(name, &key, &env),
        GroupDialog::AddBrowse {
            name,
            source,
            browser,
        } => add_browse_key(name, source, browser, &key, &mut env),
        GroupDialog::RemoveSelect { cursor } => group_picker_key(cursor, &key, &env, 1, |name| {
            GroupDialog::RemoveConfirm { name }
        }),
        GroupDialog::RemoveConfirm { name } => remove_confirm_key(name, &key, &mut env),
        GroupDialog::EditSelect { cursor } => group_picker_key(cursor, &key, &env, 2, |group| {
            GroupDialog::EditField { group, cursor: 0 }
        }),
        GroupDialog::EditField { group, cursor } => edit_field_key(group, cursor, &key, &env),
        GroupDialog::EditName { original, name } => edit_name_key(original, name, &key, &mut env),
        GroupDialog::EditBrowse {
            group,
            field,
            browser,
        } => edit_browse_key(group, *field, browser, &key, &mut env),
        GroupDialog::IgnoreManage { group, cursor } => {
            ignore_manage_key(group, cursor, &key, &mut env)
        }
        GroupDialog::IgnoreAdd { group, pattern } => ignore_add_key(group, pattern, &key, &mut env),
    };

    match next {
        Next::Stay => {}
        Next::Goto(dialog) => component.dialog = Some(dialog),
        Next::Close => component.dialog = None,
    }
    Transition::Handled
}

/// Appends a typed character or removes the last one. Other keys are ignored.
fn edit_text(buf: &mut String, key: &KeyEvent) {
    match key.code {
        KeyCode::Char(c) => buf.push(c),
        KeyCode::Backspace => {
            buf.pop();
        }
        _ => {}
    }
}

/// A directory picker at the home directory, listed now so the first paint has entries.
fn home_browser(home_dir: &Path) -> Browser {
    Browser::open(home_dir.to_path_buf(), dir_entries)
}

/// Up/Down (and `k`/`j`) move a list cursor with wrap-around. Returns false for any other key.
fn list_cursor_key(cursor: &mut usize, len: usize, key: &KeyEvent) -> bool {
    match key.code {
        KeyCode::Up | KeyCode::Char('k') => *cursor = step_cursor(*cursor, len, false),
        KeyCode::Down | KeyCode::Char('j') => *cursor = step_cursor(*cursor, len, true),
        _ => return false,
    }
    true
}

/// Up/Down/Left/Right on a directory picker. Returns false for any other key.
/// The listing is refreshed after every move so the popup paints the directory
/// the cursor is in.
fn navigate_dirs(browser: &mut Browser, key: &KeyEvent) -> bool {
    match key.code {
        KeyCode::Up | KeyCode::Char('k') => browser.step(false),
        KeyCode::Down | KeyCode::Char('j') => browser.step(true),
        KeyCode::Right => browser.enter_dir(),
        KeyCode::Left => browser.go_up(),
        _ => return false,
    }
    browser.refresh();
    true
}

/// Cursor and Enter/Esc handling shared by the group pickers. `back` is the
/// Actions row that Esc and an empty list return to; `chosen` builds the dialog
/// for the picked group.
fn group_picker_key(
    cursor: &mut usize,
    key: &KeyEvent,
    env: &Env,
    back: usize,
    chosen: fn(String) -> GroupDialog,
) -> Next {
    let names: Vec<String> = env.pending.config.groups.keys().cloned().collect();
    if list_cursor_key(cursor, names.len(), key) {
        return Next::Stay;
    }
    let pick = match key.code {
        KeyCode::Esc => Pick::Cancel,
        KeyCode::Enter => Pick::Chosen(*cursor),
        _ => Pick::Stay,
    };
    match pick {
        Pick::Stay => Next::Stay,
        Pick::Cancel => Next::Goto(GroupDialog::Actions { cursor: back }),
        Pick::Chosen(i) => match names.get(i) {
            Some(name) => Next::Goto(chosen(name.clone())),
            None => Next::Goto(GroupDialog::Actions { cursor: back }),
        },
    }
}

fn actions_key(cursor: &mut usize, key: &KeyEvent, env: &Env) -> Next {
    let has_groups = !env.pending.config.groups.is_empty();
    if list_cursor_key(cursor, 3, key) {
        return Next::Stay;
    }
    match key.code {
        KeyCode::Esc => Next::Close,
        KeyCode::Enter => match *cursor {
            0 => Next::Goto(GroupDialog::AddName {
                name: String::new(),
            }),
            1 if has_groups => Next::Goto(GroupDialog::RemoveSelect { cursor: 0 }),
            2 if has_groups => Next::Goto(GroupDialog::EditSelect { cursor: 0 }),
            _ => Next::Close,
        },
        _ => Next::Stay,
    }
}

fn add_name_key(name: &mut String, key: &KeyEvent, env: &Env) -> Next {
    match key.code {
        KeyCode::Esc => Next::Goto(GroupDialog::Actions { cursor: 0 }),
        KeyCode::Enter if !name.is_empty() => Next::Goto(GroupDialog::AddBrowse {
            name: std::mem::take(name),
            source: None,
            browser: home_browser(env.home_dir),
        }),
        _ => {
            edit_text(name, key);
            Next::Stay
        }
    }
}

fn add_browse_key(
    name: &mut String,
    source: &mut Option<String>,
    browser: &mut Browser,
    key: &KeyEvent,
    env: &mut Env,
) -> Next {
    if navigate_dirs(browser, key) {
        return Next::Stay;
    }
    match key.code {
        KeyCode::Esc if source.is_some() => Next::Goto(GroupDialog::AddBrowse {
            name: std::mem::take(name),
            source: None,
            browser: home_browser(env.home_dir),
        }),
        KeyCode::Esc => Next::Goto(GroupDialog::AddName {
            name: std::mem::take(name),
        }),
        KeyCode::Enter => {
            let Some(entry) = browser.current() else {
                return Next::Stay;
            };
            let selected = browser.path.join(&entry.name).to_string_lossy().to_string();
            match source.take() {
                None => Next::Goto(GroupDialog::AddBrowse {
                    name: std::mem::take(name),
                    source: Some(selected),
                    browser: home_browser(env.home_dir),
                }),
                Some(src) => {
                    env.pending
                        .config
                        .add_group(std::mem::take(name), src, selected);
                    *env.dirty = true;
                    Next::Close
                }
            }
        }
        _ => Next::Stay,
    }
}

fn remove_confirm_key(name: &str, key: &KeyEvent, env: &mut Env) -> Next {
    match key.code {
        KeyCode::Esc | KeyCode::Char('n') | KeyCode::Char('N') => {
            Next::Goto(GroupDialog::Actions { cursor: 1 })
        }
        KeyCode::Char('y') | KeyCode::Char('Y') => {
            env.pending.config.remove_group(name);
            *env.dirty = true;
            Next::Close
        }
        _ => Next::Stay,
    }
}

fn edit_field_key(group: &mut String, cursor: &mut usize, key: &KeyEvent, env: &Env) -> Next {
    if list_cursor_key(cursor, 4, key) {
        return Next::Stay;
    }
    match key.code {
        KeyCode::Esc => Next::Goto(GroupDialog::EditSelect { cursor: 0 }),
        KeyCode::Enter => match *cursor {
            0 => Next::Goto(GroupDialog::EditName {
                name: group.clone(),
                original: std::mem::take(group),
            }),
            1 | 2 => Next::Goto(GroupDialog::EditBrowse {
                group: std::mem::take(group),
                field: if *cursor == 1 {
                    GroupEditField::Source
                } else {
                    GroupEditField::Target
                },
                browser: home_browser(env.home_dir),
            }),
            3 => Next::Goto(GroupDialog::IgnoreManage {
                group: Some(std::mem::take(group)),
                cursor: 0,
            }),
            _ => Next::Stay,
        },
        _ => Next::Stay,
    }
}

fn edit_name_key(original: &mut String, name: &mut String, key: &KeyEvent, env: &mut Env) -> Next {
    match key.code {
        KeyCode::Esc => Next::Goto(GroupDialog::EditField {
            group: std::mem::take(original),
            cursor: 0,
        }),
        KeyCode::Enter => {
            // An empty or already-taken name keeps the prompt open so the user can fix it.
            let renamed = !name.is_empty() && env.pending.config.rename_group(original, name);
            if renamed {
                *env.dirty |= *name != *original;
                Next::Close
            } else {
                Next::Stay
            }
        }
        _ => {
            edit_text(name, key);
            Next::Stay
        }
    }
}

fn edit_browse_key(
    group: &mut String,
    field: GroupEditField,
    browser: &mut Browser,
    key: &KeyEvent,
    env: &mut Env,
) -> Next {
    if navigate_dirs(browser, key) {
        return Next::Stay;
    }
    match key.code {
        KeyCode::Esc => Next::Goto(GroupDialog::EditField {
            group: std::mem::take(group),
            cursor: match field {
                GroupEditField::Source => 1,
                GroupEditField::Target => 2,
            },
        }),
        KeyCode::Enter => {
            let Some(entry) = browser.current() else {
                return Next::Stay;
            };
            let selected = browser.path.join(&entry.name).to_string_lossy().to_string();
            let updated = match field {
                GroupEditField::Source => env.pending.config.update_group_source(group, selected),
                GroupEditField::Target => env.pending.config.update_group_target(group, selected),
            };
            if updated {
                *env.dirty = true;
            }
            Next::Close
        }
        _ => Next::Stay,
    }
}

fn ignore_manage_key(
    group: &mut Option<String>,
    cursor: &mut usize,
    key: &KeyEvent,
    env: &mut Env,
) -> Next {
    let patterns: Vec<String> = match group.as_deref() {
        Some(g) => env
            .pending
            .config
            .get_group_ignore(g)
            .map(<[String]>::to_vec)
            .unwrap_or_default(),
        None => env.pending.config.global_ignore(),
    };
    if list_cursor_key(cursor, patterns.len() + 1, key) {
        return Next::Stay;
    }
    match key.code {
        KeyCode::Esc => match group.take() {
            Some(g) => Next::Goto(GroupDialog::EditField {
                group: g,
                cursor: 3,
            }),
            None => Next::Close,
        },
        KeyCode::Char('r') | KeyCode::Enter if *cursor < patterns.len() => {
            let pat = patterns[*cursor].clone();
            match group.as_deref() {
                Some(g) => {
                    let remaining: Vec<String> =
                        patterns.iter().filter(|p| *p != &pat).cloned().collect();
                    env.pending.config.set_group_ignore(g, remaining);
                }
                None => env.pending.config.remove_global_ignore(&pat),
            }
            *env.dirty = true;
            *cursor = (*cursor).min(patterns.len().saturating_sub(1));
            Next::Stay
        }
        KeyCode::Enter => Next::Goto(GroupDialog::IgnoreAdd {
            group: group.take(),
            pattern: String::new(),
        }),
        _ => Next::Stay,
    }
}

fn ignore_add_key(
    group: &mut Option<String>,
    pattern: &mut String,
    key: &KeyEvent,
    env: &mut Env,
) -> Next {
    match key.code {
        KeyCode::Esc => Next::Goto(GroupDialog::IgnoreManage {
            group: group.take(),
            cursor: 0,
        }),
        KeyCode::Enter if !pattern.is_empty() => {
            let new_pat = std::mem::take(pattern);
            match group.as_deref() {
                Some(g) => {
                    let mut ignores = env
                        .pending
                        .config
                        .get_group_ignore(g)
                        .map(<[String]>::to_vec)
                        .unwrap_or_default();
                    ignores.push(new_pat);
                    env.pending.config.set_group_ignore(g, ignores);
                }
                None => env.pending.config.add_global_ignore(new_pat),
            }
            *env.dirty = true;
            Next::Goto(GroupDialog::IgnoreManage {
                group: group.take(),
                cursor: 0,
            })
        }
        _ => {
            edit_text(pattern, key);
            Next::Stay
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::components::settings::pending::PendingSettings;
    use crate::components::Component;
    use andre_core::config::Config;
    use crossterm::event::KeyModifiers;

    /// Config with one group `dots` (source and target exist) and a home directory with a `pick` subdirectory.
    fn fixture() -> (tempfile::TempDir, AppContext) {
        let tmp = tempfile::TempDir::new().unwrap();
        std::fs::create_dir_all(tmp.path().join("src")).unwrap();
        std::fs::create_dir_all(tmp.path().join("tgt")).unwrap();
        std::fs::create_dir_all(tmp.path().join("pick")).unwrap();
        let mut config = Config::default_empty();
        config.add_group(
            "dots".into(),
            tmp.path().join("src").display().to_string(),
            tmp.path().join("tgt").display().to_string(),
        );
        let ctx = AppContext::for_tests(config, tmp.path());
        (tmp, ctx)
    }

    fn press(component: &mut SettingsComponent, ctx: &mut AppContext, code: KeyCode) {
        component.handle_key(KeyEvent::new(code, KeyModifiers::NONE), ctx);
    }

    fn type_text(component: &mut SettingsComponent, ctx: &mut AppContext, text: &str) {
        for c in text.chars() {
            press(component, ctx, KeyCode::Char(c));
        }
    }

    /// Shows `dialog` over a settings screen whose pending copy has been taken from `ctx`.
    fn open(component: &mut SettingsComponent, ctx: &mut AppContext, dialog: GroupDialog) {
        component.pending = Some(PendingSettings::from_core(&ctx.core));
        component.dialog = Some(dialog);
    }

    #[test]
    fn test_actions_enter_opens_add_name() {
        let (_tmp, mut ctx) = fixture();
        let mut component = SettingsComponent::default();
        open(&mut component, &mut ctx, GroupDialog::Actions { cursor: 0 });
        press(&mut component, &mut ctx, KeyCode::Enter);
        assert!(
            matches!(component.dialog, Some(GroupDialog::AddName { ref name }) if name.is_empty())
        );
    }

    #[test]
    fn test_actions_wraps_cursor_and_closes_on_esc() {
        let (_tmp, mut ctx) = fixture();
        let mut component = SettingsComponent::default();
        open(&mut component, &mut ctx, GroupDialog::Actions { cursor: 0 });
        press(&mut component, &mut ctx, KeyCode::Up);
        assert!(matches!(
            component.dialog,
            Some(GroupDialog::Actions { cursor: 2 })
        ));
        press(&mut component, &mut ctx, KeyCode::Esc);
        assert!(component.dialog.is_none());
    }

    #[test]
    fn test_add_group_walks_name_then_source_then_target() {
        let (tmp, mut ctx) = fixture();
        let mut component = SettingsComponent::default();
        open(&mut component, &mut ctx, GroupDialog::Actions { cursor: 0 });
        press(&mut component, &mut ctx, KeyCode::Enter);
        type_text(&mut component, &mut ctx, "work");
        press(&mut component, &mut ctx, KeyCode::Backspace);
        type_text(&mut component, &mut ctx, "k");
        press(&mut component, &mut ctx, KeyCode::Enter);

        // Browse starts in home; the only directory there is `pick`.
        let Some(GroupDialog::AddBrowse {
            name,
            source,
            browser,
        }) = &component.dialog
        else {
            panic!("expected source picker, got {:?}", component.dialog);
        };
        assert_eq!(name, "work");
        assert!(source.is_none());
        assert_eq!(browser.path, tmp.path());

        press(&mut component, &mut ctx, KeyCode::Enter);
        press(&mut component, &mut ctx, KeyCode::Enter);
        assert!(component.dialog.is_none());
        assert!(component.config_dirty);
        assert!(component
            .pending
            .as_ref()
            .unwrap()
            .config
            .groups
            .contains_key("work"));
    }

    #[test]
    fn test_add_browse_esc_from_target_returns_to_source() {
        let (tmp, mut ctx) = fixture();
        let mut component = SettingsComponent::default();
        open(
            &mut component,
            &mut ctx,
            GroupDialog::AddBrowse {
                name: "work".into(),
                source: Some("/somewhere".into()),
                browser: Browser::open(tmp.path().to_path_buf(), dir_entries),
            },
        );
        press(&mut component, &mut ctx, KeyCode::Esc);
        assert!(matches!(
            component.dialog,
            Some(GroupDialog::AddBrowse { ref source, .. }) if source.is_none()
        ));
    }

    #[test]
    fn test_edit_name_renames_group_and_keeps_it_on_empty_name() {
        let (_tmp, mut ctx) = fixture();
        let mut component = SettingsComponent::default();
        open(
            &mut component,
            &mut ctx,
            GroupDialog::EditName {
                original: "dots".into(),
                name: String::new(),
            },
        );
        press(&mut component, &mut ctx, KeyCode::Enter);
        assert!(matches!(
            component.dialog,
            Some(GroupDialog::EditName { .. })
        ));

        type_text(&mut component, &mut ctx, "renamed");
        press(&mut component, &mut ctx, KeyCode::Enter);
        assert!(component.dialog.is_none());
        let groups = &component.pending.as_ref().unwrap().config.groups;
        assert!(groups.contains_key("renamed"));
        assert!(!groups.contains_key("dots"));
    }

    #[test]
    fn test_edit_browse_enter_sets_target_from_highlighted_directory() {
        let (tmp, mut ctx) = fixture();
        let mut component = SettingsComponent::default();
        open(
            &mut component,
            &mut ctx,
            GroupDialog::EditBrowse {
                group: "dots".into(),
                field: GroupEditField::Target,
                browser: Browser::open(tmp.path().to_path_buf(), dir_entries),
            },
        );
        // Highlight `pick` (the only directory besides none) and choose it.
        let index = match &component.dialog {
            Some(GroupDialog::EditBrowse { browser, .. }) => browser
                .entries()
                .iter()
                .position(|e| e.name == "pick")
                .expect("pick listed"),
            _ => unreachable!(),
        };
        if let Some(GroupDialog::EditBrowse { browser, .. }) = component.dialog.as_mut() {
            browser.cursor = index;
        }
        press(&mut component, &mut ctx, KeyCode::Enter);

        assert!(component.dialog.is_none());
        assert!(component.config_dirty);
        let target = &component.pending.as_ref().unwrap().config.groups["dots"].target;
        assert_eq!(*target, tmp.path().join("pick").display().to_string());
    }

    #[test]
    fn test_edit_browse_esc_returns_to_field_list() {
        let (tmp, mut ctx) = fixture();
        let mut component = SettingsComponent::default();
        open(
            &mut component,
            &mut ctx,
            GroupDialog::EditBrowse {
                group: "dots".into(),
                field: GroupEditField::Source,
                browser: Browser::open(tmp.path().to_path_buf(), dir_entries),
            },
        );
        press(&mut component, &mut ctx, KeyCode::Esc);
        assert!(matches!(
            component.dialog,
            Some(GroupDialog::EditField { ref group, cursor: 1 }) if group == "dots"
        ));
    }

    #[test]
    fn test_remove_confirm_y_removes_group() {
        let (_tmp, mut ctx) = fixture();
        let mut component = SettingsComponent::default();
        open(
            &mut component,
            &mut ctx,
            GroupDialog::RemoveConfirm {
                name: "dots".into(),
            },
        );
        press(&mut component, &mut ctx, KeyCode::Char('y'));
        assert!(component.dialog.is_none());
        assert!(component.pending.as_ref().unwrap().config.groups.is_empty());
    }

    #[test]
    fn test_global_ignore_add_then_remove() {
        let (_tmp, mut ctx) = fixture();
        let mut component = SettingsComponent::default();
        open(
            &mut component,
            &mut ctx,
            GroupDialog::IgnoreAdd {
                group: None,
                pattern: String::new(),
            },
        );
        type_text(&mut component, &mut ctx, "node_modules");
        press(&mut component, &mut ctx, KeyCode::Enter);
        let pending = component.pending.as_ref().unwrap();
        assert_eq!(
            pending.config.global_ignore(),
            vec!["node_modules".to_string()]
        );
        assert!(matches!(
            component.dialog,
            Some(GroupDialog::IgnoreManage {
                group: None,
                cursor: 0
            })
        ));

        press(&mut component, &mut ctx, KeyCode::Char('r'));
        assert!(component
            .pending
            .as_ref()
            .unwrap()
            .config
            .global_ignore()
            .is_empty());
    }

    #[test]
    fn test_group_ignore_esc_returns_to_edit_field_ignore_row() {
        let (_tmp, mut ctx) = fixture();
        let mut component = SettingsComponent::default();
        open(
            &mut component,
            &mut ctx,
            GroupDialog::IgnoreManage {
                group: Some("dots".into()),
                cursor: 0,
            },
        );
        press(&mut component, &mut ctx, KeyCode::Esc);
        assert!(matches!(
            component.dialog,
            Some(GroupDialog::EditField { ref group, cursor: 3 }) if group == "dots"
        ));
    }

    fn picker_browser(component: &SettingsComponent) -> &Browser {
        match component.dialog.as_ref() {
            Some(
                GroupDialog::AddBrowse { browser, .. } | GroupDialog::EditBrowse { browser, .. },
            ) => browser,
            other => panic!("expected a directory picker, got {:?}", other),
        }
    }

    fn picker_browser_mut(component: &mut SettingsComponent) -> &mut Browser {
        match component.dialog.as_mut() {
            Some(
                GroupDialog::AddBrowse { browser, .. } | GroupDialog::EditBrowse { browser, .. },
            ) => browser,
            other => panic!("expected a directory picker, got {:?}", other),
        }
    }

    /// Moves the picker's cursor onto the entry called `name`.
    fn select_entry(component: &mut SettingsComponent, name: &str) {
        let browser = picker_browser_mut(component);
        let index = browser
            .entries()
            .iter()
            .position(|e| e.name == name)
            .expect("entry listed");
        browser.cursor = index;
    }

    /// Sorted names of the entries the open picker lists.
    fn picker_names(component: &SettingsComponent) -> Vec<String> {
        let mut names: Vec<String> = picker_browser(component)
            .entries()
            .iter()
            .map(|e| e.name.clone())
            .collect();
        names.sort();
        names
    }

    #[test]
    fn test_dir_picker_relists_after_right_and_left() {
        let (tmp, mut ctx) = fixture();
        std::fs::create_dir_all(tmp.path().join("a/inner")).unwrap();
        std::fs::create_dir_all(tmp.path().join("b")).unwrap();
        let mut component = SettingsComponent::default();
        open(
            &mut component,
            &mut ctx,
            GroupDialog::EditBrowse {
                group: "dots".into(),
                field: GroupEditField::Target,
                browser: Browser::open(tmp.path().to_path_buf(), dir_entries),
            },
        );
        select_entry(&mut component, "a");

        press(&mut component, &mut ctx, KeyCode::Right);
        assert_eq!(picker_browser(&component).path, tmp.path().join("a"));
        assert_eq!(picker_names(&component), vec!["inner".to_string()]);

        press(&mut component, &mut ctx, KeyCode::Left);
        assert_eq!(picker_browser(&component).path, tmp.path());
        let names = picker_names(&component);
        assert!(names.contains(&"a".to_string()));
        assert!(names.contains(&"b".to_string()));
        assert!(!names.contains(&"inner".to_string()));
    }

    #[test]
    fn test_dir_picker_enter_selects_directory_entered_with_right() {
        let (tmp, mut ctx) = fixture();
        std::fs::create_dir_all(tmp.path().join("a/inner")).unwrap();
        let mut component = SettingsComponent::default();
        open(
            &mut component,
            &mut ctx,
            GroupDialog::EditBrowse {
                group: "dots".into(),
                field: GroupEditField::Target,
                browser: Browser::open(tmp.path().to_path_buf(), dir_entries),
            },
        );
        select_entry(&mut component, "a");

        press(&mut component, &mut ctx, KeyCode::Right);
        press(&mut component, &mut ctx, KeyCode::Enter);

        assert!(component.dialog.is_none());
        let target = &component.pending.as_ref().unwrap().config.groups["dots"].target;
        assert_eq!(*target, tmp.path().join("a/inner").display().to_string());
    }
}
