//! Keys for the dialogs on the settings list. Only the top overlay takes keys. A key either
//! keeps the overlay (possibly edited in place) or closes it.

use std::path::{Path, PathBuf};

use andre_core::{IconSet, StowAction, Theme};
use crossterm::event::{KeyCode, KeyEvent};

use super::pending::PendingSettings;
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
}

/// The settings a dialog may change, borrowed from the component.
struct Env<'a> {
    pending: &'a mut PendingSettings,
    dirty: &'a mut bool,
    ctx: &'a AppContext,
}

/// Routes a key to the top overlay and applies what it leaves on the stack. `Overlay::Leave`
/// is not handled here: the screen handles it, since saving or discarding pops the screen.
pub fn handle_key(component: &mut SettingsComponent, key: KeyEvent, ctx: &AppContext) {
    let SettingsComponent {
        overlays,
        pending,
        dirty,
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
        ctx,
    };
    let next = match &mut top {
        Overlay::Action { cursor } => action_key(cursor, &key, &mut env),
        Overlay::Theme { cursor, original } => theme_key(cursor, *original, &key, &mut env),
        Overlay::Icons { cursor, original } => icons_key(cursor, *original, &key, &mut env),
        Overlay::Browse { browser } => browse_key(browser, &key, &mut env),
        Overlay::Ignore { cursor, edit } => ignore_key(cursor, edit, &key, &mut env),
        Overlay::Leave => Next::Keep,
    };
    if let Next::Keep = next {
        overlays.push(top);
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
        Overlay::Browse { .. } => vec![
            Hint::new("\u{2192}", "open", PRIO_NAV),
            Hint::new("\u{2190}", "up", PRIO_NAV),
            Hint::new("Enter", "choose", PRIO_PRIMARY),
            Hint::new("Esc", "cancel", PRIO_BACK),
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

/// The packages folder picker. Enter chooses the highlighted folder, as the path the picker
/// shows it at (absolute), and closes; Esc closes with nothing changed.
fn browse_key(browser: &mut Browser, key: &KeyEvent, env: &mut Env) -> Next {
    if navigate_dirs(browser, key) {
        return Next::Keep;
    }
    match key.code {
        KeyCode::Esc => Next::Close,
        KeyCode::Enter => {
            let Some(entry) = browser.current() else {
                return Next::Keep;
            };
            let chosen = browser.path.join(&entry.name);
            // Choosing the folder that is already set keeps the text in andre.yml as it is.
            let current = env
                .pending
                .config
                .packages_dir(&env.ctx.config_dir, &env.ctx.home_dir);
            if current.as_ref() != Some(&chosen) {
                env.pending
                    .config
                    .set_packages(Some(chosen.to_string_lossy().to_string()));
                *env.dirty = true;
            }
            Next::Close
        }
        _ => Next::Keep,
    }
}

/// The pattern list. The row after the last pattern adds one. While a line is being
/// typed, the keys go to it.
fn ignore_key(
    cursor: &mut usize,
    edit: &mut Option<LineEdit>,
    key: &KeyEvent,
    env: &mut Env,
) -> Next {
    if edit.is_some() {
        return edit_line_key(cursor, edit, key, env);
    }
    let len = env.pending.config.global_ignore().len();
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
            let list = env.pending.config.global_ignore();
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
            let mut list = env.pending.config.global_ignore();
            list.remove(*cursor);
            set_patterns(env.pending, list);
            *env.dirty = true;
            *cursor = (*cursor).min(len - 1);
            Next::Keep
        }
        _ => Next::Keep,
    }
}

fn set_patterns(pending: &mut PendingSettings, list: Vec<String>) {
    pending.config.global.ignore = Some(list);
}

fn new_line() -> LineEdit {
    LineEdit {
        index: None,
        field: TextField::default(),
    }
}

fn edit_line_key(
    cursor: &mut usize,
    edit: &mut Option<LineEdit>,
    key: &KeyEvent,
    env: &mut Env,
) -> Next {
    match key.code {
        KeyCode::Esc => *edit = None,
        KeyCode::Enter => {
            if let Some(line) = edit.take() {
                commit_line(cursor, line, env);
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
fn commit_line(cursor: &mut usize, line: LineEdit, env: &mut Env) {
    let text = line.field.value.trim().to_string();
    let mut list = env.pending.config.global_ignore();
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
    set_patterns(env.pending, list);
    *env.dirty = true;
}
