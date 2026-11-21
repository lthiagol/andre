//! Keys for the dialogs on the settings list. Only the top overlay takes keys. Each handler
//! says what the stack becomes, so a sub-dialog returns to the one that opened it.

use std::path::{Path, PathBuf};

use andre_core::{IconSet, StowAction, Theme};
use crossterm::event::{KeyCode, KeyEvent};

use super::pending::{GroupChange, PendingSettings};
use super::types::*;
use super::SettingsComponent;
use crate::components::browser::{dir_entries, Browser};
use crate::components::AppContext;
use crate::input::step_cursor;
use crate::ui::footer::{Hint, PRIO_BACK, PRIO_NAV, PRIO_PRIMARY, PRIO_VERB};
use crate::ui::text_field::TextField;

/// What the overlay stack becomes after a key.
enum Next {
    /// Keep the overlay, possibly edited in place.
    Keep,
    /// Close it; the overlay below, if any, takes keys again.
    Close,
    /// Close it and show this one instead.
    Replace(Overlay),
    /// Show this one on top of it.
    Open(Overlay),
    /// Close the rename dialog. The group menu below now edits `name`.
    Renamed(String),
}

/// The settings a dialog may change, borrowed from the component.
struct Env<'a> {
    pending: &'a mut PendingSettings,
    dirty: &'a mut bool,
    focus: &'a mut Option<Row>,
    ctx: &'a AppContext,
}

/// Routes a key to the top overlay and applies what it leaves on the stack. `Overlay::Leave`
/// is not handled here: the screen handles it, since saving or discarding pops the screen.
pub fn handle_key(component: &mut SettingsComponent, key: KeyEvent, ctx: &AppContext) {
    let SettingsComponent {
        overlays,
        pending,
        dirty,
        focus,
        ..
    } = component;
    let Some(pending) = pending.as_mut() else {
        return;
    };
    let Some(mut top) = overlays.pop() else {
        return;
    };
    let mut env = Env {
        pending,
        dirty,
        focus,
        ctx,
    };
    let next = match &mut top {
        Overlay::Action { cursor } => action_key(cursor, &key, &mut env),
        Overlay::Theme { cursor, original } => theme_key(cursor, *original, &key, &mut env),
        Overlay::Icons { cursor, original } => icons_key(cursor, *original, &key, &mut env),
        Overlay::Group {
            group,
            cursor,
            snapshot,
        } => group_key(group, cursor, snapshot, &key, &mut env),
        Overlay::Name { target, field } => name_key(target, field, &key, &mut env),
        Overlay::Browse { pick, browser } => browse_key(pick, browser, &key, &mut env),
        Overlay::Remove { group, .. } => remove_key(group, &key, &mut env),
        Overlay::Ignore {
            scope,
            cursor,
            edit,
        } => ignore_key(scope, cursor, edit, &key, &mut env),
        Overlay::Leave => Next::Keep,
    };
    match next {
        Next::Keep => overlays.push(top),
        Next::Close => {}
        Next::Replace(overlay) => overlays.push(overlay),
        Next::Open(overlay) => {
            overlays.push(top);
            overlays.push(overlay);
        }
        Next::Renamed(name) => {
            if let Some(Overlay::Group { group, .. }) = overlays.last_mut() {
                *group = name;
            }
        }
    }
}

/// The keys the overlay takes: a verb for each, the same order as the footer.
pub fn hints(overlay: &Overlay) -> Vec<Hint> {
    match overlay {
        Overlay::Action { .. } => vec![
            Hint::new("\u{2191}\u{2193}", "move", PRIO_NAV),
            Hint::new("Enter", "keep", PRIO_PRIMARY),
            Hint::new("Esc", "cancel", PRIO_BACK),
        ],
        Overlay::Theme { .. } | Overlay::Icons { .. } => vec![
            Hint::new("\u{2191}\u{2193}", "preview", PRIO_NAV),
            Hint::new("Enter", "keep", PRIO_PRIMARY),
            Hint::new("Esc", "revert", PRIO_BACK),
        ],
        Overlay::Group { .. } => vec![
            Hint::new("\u{2191}\u{2193}", "field", PRIO_NAV),
            Hint::new("Enter", "change", PRIO_PRIMARY),
            Hint::new("Esc", "cancel", PRIO_BACK),
        ],
        Overlay::Name { target, .. } => {
            let verb = match target {
                NameTarget::New => "create",
                NameTarget::Rename { .. } => "rename",
            };
            vec![
                Hint::new("Enter", verb, PRIO_PRIMARY),
                Hint::new("Esc", "cancel", PRIO_BACK),
            ]
        }
        Overlay::Browse { .. } => vec![
            Hint::new("\u{2192}", "open", PRIO_NAV),
            Hint::new("\u{2190}", "up", PRIO_NAV),
            Hint::new("Enter", "choose", PRIO_PRIMARY),
            Hint::new("Esc", "cancel", PRIO_BACK),
        ],
        Overlay::Remove { .. } => vec![
            Hint::new("y", "remove", PRIO_PRIMARY),
            Hint::new("n", "keep", PRIO_BACK),
        ],
        Overlay::Ignore { edit: Some(_), .. } => vec![
            Hint::new("Enter", "done", PRIO_PRIMARY),
            Hint::new("Esc", "cancel", PRIO_BACK),
        ],
        Overlay::Ignore { .. } => vec![
            Hint::new("\u{2191}\u{2193}", "move", PRIO_NAV),
            Hint::new("+", "add", PRIO_VERB),
            Hint::new("Enter", "edit", PRIO_PRIMARY),
            Hint::new("x", "delete", PRIO_VERB),
            Hint::new("Esc", "back", PRIO_BACK),
        ],
        Overlay::Leave => vec![
            Hint::new("y", "save", PRIO_PRIMARY),
            Hint::new("n", "discard", PRIO_BACK),
            Hint::new("Esc", "stay", PRIO_BACK),
        ],
    }
}

/// Up/Down (and `k`/`j`) move a list cursor with wrap-around. Returns false for any other key.
fn move_cursor(cursor: &mut usize, len: usize, key: &KeyEvent) -> bool {
    match key.code {
        KeyCode::Up | KeyCode::Char('k') => *cursor = step_cursor(*cursor, len, false),
        KeyCode::Down | KeyCode::Char('j') => *cursor = step_cursor(*cursor, len, true),
        _ => return false,
    }
    true
}

/// Up/Down/Left/Right on a directory picker. Returns false for any other key. The listing
/// is refreshed after every move, so the dialog paints the directory the cursor is in.
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

/// A text field holding `value`, with the cursor at its end.
pub fn text_field(value: &str) -> TextField {
    TextField {
        value: value.to_string(),
        cursor: value.chars().count(),
        error: None,
    }
}

/// A directory picker opened at `current` when it exists (on its name, in its parent), or
/// at `home` otherwise.
pub fn browser_at(current: Option<PathBuf>, home: &Path) -> Browser {
    let Some(path) = current.filter(|path| path.is_dir()) else {
        return Browser::open(home.to_path_buf(), dir_entries);
    };
    let parent = path.parent().unwrap_or(&path).to_path_buf();
    let name = path
        .file_name()
        .and_then(|n| n.to_str())
        .map(str::to_string);
    let mut browser = Browser::open(parent, dir_entries);
    if let Some(index) = browser
        .entries()
        .iter()
        .position(|e| Some(&e.name) == name.as_ref())
    {
        browser.cursor = index;
    }
    browser
}

fn action_key(cursor: &mut usize, key: &KeyEvent, env: &mut Env) -> Next {
    if move_cursor(cursor, StowAction::ALL.len(), key) {
        return Next::Keep;
    }
    match key.code {
        KeyCode::Esc => Next::Close,
        KeyCode::Enter => {
            let action = StowAction::ALL[*cursor];
            if env.pending.flags.action != action {
                env.pending.flags.action = action;
                *env.dirty = true;
            }
            Next::Close
        }
        _ => Next::Keep,
    }
}

/// Moving the cursor sets the pending theme, so the screen previews it. Esc puts the theme
/// back as it was when the picker opened.
fn theme_key(cursor: &mut usize, original: Theme, key: &KeyEvent, env: &mut Env) -> Next {
    if move_cursor(cursor, Theme::ALL.len(), key) {
        env.pending.theme = Theme::ALL[*cursor];
        return Next::Keep;
    }
    match key.code {
        KeyCode::Esc => {
            env.pending.theme = original;
            Next::Close
        }
        KeyCode::Enter => {
            if env.pending.theme != original {
                *env.dirty = true;
            }
            Next::Close
        }
        _ => Next::Keep,
    }
}

/// The icon set picker: as for the theme, moving previews and Esc reverts.
fn icons_key(cursor: &mut usize, original: IconSet, key: &KeyEvent, env: &mut Env) -> Next {
    if move_cursor(cursor, IconSet::ALL.len(), key) {
        env.pending.icons = IconSet::ALL[*cursor];
        return Next::Keep;
    }
    match key.code {
        KeyCode::Esc => {
            env.pending.icons = original;
            Next::Close
        }
        KeyCode::Enter => {
            if env.pending.icons != original {
                *env.dirty = true;
            }
            Next::Close
        }
        _ => Next::Keep,
    }
}

fn group_key(
    group: &str,
    cursor: &mut usize,
    snapshot: &Snapshot,
    key: &KeyEvent,
    env: &mut Env,
) -> Next {
    if move_cursor(cursor, GROUP_ROWS, key) {
        return Next::Keep;
    }
    match key.code {
        KeyCode::Esc => {
            env.pending.config = snapshot.config.clone();
            env.pending.group_changes = snapshot.changes.clone();
            *env.dirty = snapshot.dirty;
            *env.focus = Some(Row::Group(snapshot.group.clone()));
            Next::Close
        }
        KeyCode::Enter => {
            let config = &env.pending.config;
            let (ctx, name) = (env.ctx, group.to_string());
            match *cursor {
                GROUP_NAME => Next::Open(Overlay::Name {
                    target: NameTarget::Rename {
                        original: name.clone(),
                    },
                    field: text_field(&name),
                }),
                GROUP_SOURCE => {
                    let current =
                        config.resolve_group_source(&name, &ctx.config_dir, &ctx.home_dir);
                    Next::Open(Overlay::Browse {
                        pick: Pick::Source { group: name },
                        browser: browser_at(current, &ctx.home_dir),
                    })
                }
                GROUP_TARGET => {
                    let current =
                        config.resolve_group_target(&name, &ctx.config_dir, &ctx.home_dir);
                    Next::Open(Overlay::Browse {
                        pick: Pick::Target { group: name },
                        browser: browser_at(current, &ctx.home_dir),
                    })
                }
                GROUP_IGNORE => Next::Open(Overlay::Ignore {
                    scope: IgnoreScope::Group(name),
                    cursor: 0,
                    edit: None,
                }),
                // Done keeps the edits.
                _ => Next::Close,
            }
        }
        _ => Next::Keep,
    }
}

/// The group name dialog. A new name must be unused and free of `/`, which separates the
/// group from its packages in profile refs.
fn name_key(target: &NameTarget, field: &mut TextField, key: &KeyEvent, env: &mut Env) -> Next {
    match key.code {
        KeyCode::Esc => Next::Close,
        KeyCode::Enter => {
            let name = field.value.trim().to_string();
            let taken = match target {
                NameTarget::New => env.pending.config.groups.contains_key(&name),
                NameTarget::Rename { original } => {
                    name != *original && env.pending.config.groups.contains_key(&name)
                }
            };
            if name.is_empty() {
                field.error = Some("type a name".to_string());
                return Next::Keep;
            }
            if name.contains('/') {
                field.error = Some("no `/` in a group name".to_string());
                return Next::Keep;
            }
            if taken {
                field.error = Some(format!("`{name}` is already a group"));
                return Next::Keep;
            }
            match target {
                NameTarget::New => Next::Replace(Overlay::Browse {
                    pick: Pick::NewSource { name },
                    browser: browser_at(None, &env.ctx.home_dir),
                }),
                NameTarget::Rename { original } => {
                    if name == *original {
                        return Next::Close;
                    }
                    env.pending.config.rename_group(original, &name);
                    env.pending.group_changes.push(GroupChange::Rename {
                        from: original.clone(),
                        to: name.clone(),
                    });
                    *env.dirty = true;
                    *env.focus = Some(Row::Group(name.clone()));
                    Next::Renamed(name)
                }
            }
        }
        _ => {
            field.handle_key(key);
            field.error = None;
            Next::Keep
        }
    }
}

fn browse_key(pick: &mut Pick, browser: &mut Browser, key: &KeyEvent, env: &mut Env) -> Next {
    if navigate_dirs(browser, key) {
        return Next::Keep;
    }
    match key.code {
        KeyCode::Esc => match pick {
            Pick::NewSource { name } => Next::Replace(Overlay::Name {
                target: NameTarget::New,
                field: text_field(name),
            }),
            Pick::NewTarget { name, .. } => Next::Replace(Overlay::Browse {
                pick: Pick::NewSource { name: name.clone() },
                browser: browser_at(None, &env.ctx.home_dir),
            }),
            Pick::Source { .. } | Pick::Target { .. } => Next::Close,
        },
        KeyCode::Enter => {
            let Some(entry) = browser.current() else {
                return Next::Keep;
            };
            let chosen = browser.path.join(&entry.name).to_string_lossy().to_string();
            match pick {
                Pick::NewSource { name } => Next::Replace(Overlay::Browse {
                    pick: Pick::NewTarget {
                        name: name.clone(),
                        source: chosen,
                    },
                    browser: browser_at(None, &env.ctx.home_dir),
                }),
                Pick::NewTarget { name, source } => {
                    env.pending
                        .config
                        .add_group(name.clone(), source.clone(), chosen);
                    // A group removed and added back in one session keeps its refs.
                    let removed = GroupChange::Remove(name.clone());
                    env.pending
                        .group_changes
                        .retain(|change| *change != removed);
                    *env.dirty = true;
                    *env.focus = Some(Row::Group(name.clone()));
                    Next::Close
                }
                Pick::Source { group } => {
                    if env.pending.config.update_group_source(group, chosen) {
                        *env.dirty = true;
                    }
                    Next::Close
                }
                Pick::Target { group } => {
                    if env.pending.config.update_group_target(group, chosen) {
                        *env.dirty = true;
                    }
                    Next::Close
                }
            }
        }
        _ => Next::Keep,
    }
}

fn remove_key(group: &str, key: &KeyEvent, env: &mut Env) -> Next {
    match key.code {
        KeyCode::Char('y') | KeyCode::Char('Y') => {
            env.pending.config.remove_group(group);
            env.pending
                .group_changes
                .push(GroupChange::Remove(group.to_string()));
            *env.dirty = true;
            Next::Close
        }
        KeyCode::Char('n') | KeyCode::Char('N') | KeyCode::Esc => Next::Close,
        _ => Next::Keep,
    }
}

/// The patterns an ignore editor lists: the global list, or one group's own list.
fn patterns(pending: &PendingSettings, scope: &IgnoreScope) -> Vec<String> {
    match scope {
        IgnoreScope::Global => pending.config.global_ignore(),
        IgnoreScope::Group(group) => pending
            .config
            .get_group_ignore(group)
            .map(<[String]>::to_vec)
            .unwrap_or_default(),
    }
}

fn set_patterns(pending: &mut PendingSettings, scope: &IgnoreScope, list: Vec<String>) {
    match scope {
        IgnoreScope::Global => pending.config.global.ignore = Some(list),
        IgnoreScope::Group(group) => {
            pending.config.set_group_ignore(group, list);
        }
    }
}

/// The pattern list. The row after the last pattern adds one. While a line is being
/// typed, the keys go to it.
fn ignore_key(
    scope: &IgnoreScope,
    cursor: &mut usize,
    edit: &mut Option<LineEdit>,
    key: &KeyEvent,
    env: &mut Env,
) -> Next {
    if edit.is_some() {
        return edit_line_key(scope, cursor, edit, key, env);
    }
    let len = patterns(env.pending, scope).len();
    if move_cursor(cursor, len + 1, key) {
        return Next::Keep;
    }
    match key.code {
        KeyCode::Esc => Next::Close,
        KeyCode::Char('+') => {
            *edit = Some(new_line());
            Next::Keep
        }
        KeyCode::Enter if *cursor < len => {
            let list = patterns(env.pending, scope);
            *edit = Some(LineEdit {
                index: Some(*cursor),
                field: text_field(&list[*cursor]),
            });
            Next::Keep
        }
        KeyCode::Enter => {
            *edit = Some(new_line());
            Next::Keep
        }
        KeyCode::Char('x') if *cursor < len => {
            let mut list = patterns(env.pending, scope);
            list.remove(*cursor);
            set_patterns(env.pending, scope, list);
            *env.dirty = true;
            *cursor = (*cursor).min(len - 1);
            Next::Keep
        }
        _ => Next::Keep,
    }
}

fn new_line() -> LineEdit {
    LineEdit {
        index: None,
        field: TextField::default(),
    }
}

fn edit_line_key(
    scope: &IgnoreScope,
    cursor: &mut usize,
    edit: &mut Option<LineEdit>,
    key: &KeyEvent,
    env: &mut Env,
) -> Next {
    match key.code {
        KeyCode::Esc => *edit = None,
        KeyCode::Enter => {
            if let Some(line) = edit.take() {
                commit_line(scope, cursor, line, env);
            }
        }
        _ => {
            if let Some(line) = edit.as_mut() {
                line.field.handle_key(key);
                line.field.error = None;
            }
        }
    }
    Next::Keep
}

/// Writes a typed line. A blank line removes an existing pattern and adds nothing.
fn commit_line(scope: &IgnoreScope, cursor: &mut usize, line: LineEdit, env: &mut Env) {
    let text = line.field.value.trim().to_string();
    let mut list = patterns(env.pending, scope);
    match line.index {
        None if text.is_empty() => return,
        None => {
            list.push(text);
            *cursor = list.len() - 1;
        }
        Some(index) if text.is_empty() => {
            list.remove(index);
        }
        Some(index) => list[index] = text,
    }
    *cursor = (*cursor).min(list.len());
    set_patterns(env.pending, scope, list);
    *env.dirty = true;
}
