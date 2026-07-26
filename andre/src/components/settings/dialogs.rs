use crossterm::event::{KeyCode, KeyEvent};

use crate::components::settings::{GroupDialog, GroupEditField, SettingsComponent};
use crate::components::{AppContext, Transition};

pub fn handle_dialog_key(
    component: &mut SettingsComponent,
    key: KeyEvent,
    ctx: &mut AppContext,
) -> Transition {
    let pending = component.pending.as_mut().unwrap();

    match component.dialog.take() {
        Some(GroupDialog::Actions { cursor }) => match key.code {
            KeyCode::Esc => {
                component.dialog = None;
            }
            KeyCode::Up | KeyCode::Char('k') => {
                let new_cursor = if cursor == 0 { 2 } else { cursor - 1 };
                component.dialog = Some(GroupDialog::Actions { cursor: new_cursor });
            }
            KeyCode::Down | KeyCode::Char('j') => {
                let new_cursor = (cursor + 1) % 3;
                component.dialog = Some(GroupDialog::Actions { cursor: new_cursor });
            }
            KeyCode::Enter => match cursor {
                0 => {
                    component.dialog = Some(GroupDialog::AddName {
                        name: String::new(),
                    });
                }
                1 if !pending.config.groups.is_empty() => {
                    component.dialog = Some(GroupDialog::RemoveSelect { cursor: 0 });
                }
                1 => {
                    component.dialog = None;
                }
                2 if !pending.config.groups.is_empty() => {
                    component.dialog = Some(GroupDialog::EditSelect { cursor: 0 });
                }
                2 => {
                    component.dialog = None;
                }
                _ => {
                    component.dialog = None;
                }
            },
            _ => {
                component.dialog = Some(GroupDialog::Actions { cursor });
            }
        },
        Some(GroupDialog::AddName { name }) => match key.code {
            KeyCode::Esc => {
                component.dialog = Some(GroupDialog::Actions { cursor: 0 });
            }
            KeyCode::Enter if !name.is_empty() => {
                component.dialog = Some(GroupDialog::AddBrowse {
                    name,
                    source: None,
                    path: ctx.home_dir.clone(),
                    cursor: 0,
                });
            }
            KeyCode::Enter => {
                component.dialog = Some(GroupDialog::AddName { name });
            }
            KeyCode::Char(c) => {
                let mut new_name = name;
                new_name.push(c);
                component.dialog = Some(GroupDialog::AddName { name: new_name });
            }
            KeyCode::Backspace => {
                let mut new_name = name;
                new_name.pop();
                component.dialog = Some(GroupDialog::AddName { name: new_name });
            }
            _ => {
                component.dialog = Some(GroupDialog::AddName { name });
            }
        },
        Some(GroupDialog::AddBrowse {
            name,
            source,
            path,
            cursor,
        }) => match key.code {
            KeyCode::Esc => {
                if source.is_some() {
                    component.dialog = Some(GroupDialog::AddBrowse {
                        name,
                        source: None,
                        path: ctx.home_dir.clone(),
                        cursor: 0,
                    });
                } else {
                    component.dialog = Some(GroupDialog::AddName { name });
                }
            }
            KeyCode::Up | KeyCode::Char('k') => {
                let entries = andre_core::adopt::list_target_dirs(&path).unwrap_or_default();
                if !entries.is_empty() {
                    let new_cursor = if cursor == 0 {
                        entries.len() - 1
                    } else {
                        cursor - 1
                    };
                    component.dialog = Some(GroupDialog::AddBrowse {
                        name,
                        source,
                        path,
                        cursor: new_cursor,
                    });
                } else {
                    component.dialog = Some(GroupDialog::AddBrowse {
                        name,
                        source,
                        path,
                        cursor,
                    });
                }
            }
            KeyCode::Down | KeyCode::Char('j') => {
                let entries = andre_core::adopt::list_target_dirs(&path).unwrap_or_default();
                if !entries.is_empty() {
                    let new_cursor = (cursor + 1) % entries.len();
                    component.dialog = Some(GroupDialog::AddBrowse {
                        name,
                        source,
                        path,
                        cursor: new_cursor,
                    });
                } else {
                    component.dialog = Some(GroupDialog::AddBrowse {
                        name,
                        source,
                        path,
                        cursor,
                    });
                }
            }
            KeyCode::Right => {
                let entries = andre_core::adopt::list_target_dirs(&path).unwrap_or_default();
                if let Some(entry) = entries.get(cursor) {
                    if entry.is_dir {
                        let new_path = path.join(&entry.name);
                        component.dialog = Some(GroupDialog::AddBrowse {
                            name,
                            source,
                            path: new_path,
                            cursor: 0,
                        });
                    } else {
                        component.dialog = Some(GroupDialog::AddBrowse {
                            name,
                            source,
                            path,
                            cursor,
                        });
                    }
                } else {
                    component.dialog = Some(GroupDialog::AddBrowse {
                        name,
                        source,
                        path,
                        cursor,
                    });
                }
            }
            KeyCode::Left => {
                if let Some(parent) = path.parent() {
                    let parent = parent.to_path_buf();
                    component.dialog = Some(GroupDialog::AddBrowse {
                        name,
                        source,
                        path: parent,
                        cursor: 0,
                    });
                } else {
                    component.dialog = Some(GroupDialog::AddBrowse {
                        name,
                        source,
                        path,
                        cursor,
                    });
                }
            }
            KeyCode::Enter => {
                let entries = andre_core::adopt::list_target_dirs(&path).unwrap_or_default();
                if let Some(entry) = entries.get(cursor) {
                    let selected_path = path.join(&entry.name);
                    let selected_str = selected_path.to_string_lossy().to_string();
                    match source {
                        None => {
                            component.dialog = Some(GroupDialog::AddBrowse {
                                name,
                                source: Some(selected_str),
                                path: ctx.home_dir.clone(),
                                cursor: 0,
                            });
                        }
                        Some(src) => {
                            pending.config.add_group(name.clone(), src, selected_str);
                            component.config_dirty = true;
                            component.dialog = None;
                        }
                    }
                } else {
                    component.dialog = Some(GroupDialog::AddBrowse {
                        name,
                        source,
                        path,
                        cursor,
                    });
                }
            }
            _ => {
                component.dialog = Some(GroupDialog::AddBrowse {
                    name,
                    source,
                    path,
                    cursor,
                });
            }
        },
        Some(GroupDialog::RemoveSelect { cursor }) => {
            let names: Vec<String> = pending.config.groups.keys().cloned().collect();
            match key.code {
                KeyCode::Esc => {
                    component.dialog = Some(GroupDialog::Actions { cursor: 1 });
                }
                KeyCode::Up | KeyCode::Char('k') => {
                    let new_cursor = if cursor == 0 {
                        names.len() - 1
                    } else {
                        cursor - 1
                    };
                    component.dialog = Some(GroupDialog::RemoveSelect { cursor: new_cursor });
                }
                KeyCode::Down | KeyCode::Char('j') => {
                    let new_cursor = (cursor + 1) % names.len();
                    component.dialog = Some(GroupDialog::RemoveSelect { cursor: new_cursor });
                }
                KeyCode::Enter => {
                    if let Some(name) = names.get(cursor) {
                        component.dialog = Some(GroupDialog::RemoveConfirm { name: name.clone() });
                    } else {
                        component.dialog = Some(GroupDialog::Actions { cursor: 1 });
                    }
                }
                _ => {
                    component.dialog = Some(GroupDialog::RemoveSelect { cursor });
                }
            }
        }
        Some(GroupDialog::RemoveConfirm { name }) => match key.code {
            KeyCode::Esc | KeyCode::Char('n') | KeyCode::Char('N') => {
                component.dialog = Some(GroupDialog::Actions { cursor: 1 });
            }
            KeyCode::Char('y') | KeyCode::Char('Y') => {
                pending.config.remove_group(&name);
                component.config_dirty = true;
                component.dialog = None;
            }
            _ => {
                component.dialog = Some(GroupDialog::RemoveConfirm { name });
            }
        },
        Some(GroupDialog::EditSelect { cursor }) => {
            let names: Vec<String> = pending.config.groups.keys().cloned().collect();
            match key.code {
                KeyCode::Esc => {
                    component.dialog = Some(GroupDialog::Actions { cursor: 2 });
                }
                KeyCode::Up | KeyCode::Char('k') => {
                    let new_cursor = if cursor == 0 {
                        names.len() - 1
                    } else {
                        cursor - 1
                    };
                    component.dialog = Some(GroupDialog::EditSelect { cursor: new_cursor });
                }
                KeyCode::Down | KeyCode::Char('j') => {
                    let new_cursor = (cursor + 1) % names.len();
                    component.dialog = Some(GroupDialog::EditSelect { cursor: new_cursor });
                }
                KeyCode::Enter => {
                    if let Some(name) = names.get(cursor) {
                        component.dialog = Some(GroupDialog::EditField {
                            group: name.clone(),
                            cursor: 0,
                        });
                    } else {
                        component.dialog = Some(GroupDialog::Actions { cursor: 2 });
                    }
                }
                _ => {
                    component.dialog = Some(GroupDialog::EditSelect { cursor });
                }
            }
        }
        Some(GroupDialog::EditField { group, cursor }) => match key.code {
            KeyCode::Esc => {
                component.dialog = Some(GroupDialog::EditSelect { cursor: 0 });
            }
            KeyCode::Up | KeyCode::Char('k') => {
                let new_cursor = if cursor == 0 { 3 } else { cursor - 1 };
                component.dialog = Some(GroupDialog::EditField {
                    group,
                    cursor: new_cursor,
                });
            }
            KeyCode::Down | KeyCode::Char('j') => {
                let new_cursor = (cursor + 1) % 4;
                component.dialog = Some(GroupDialog::EditField {
                    group,
                    cursor: new_cursor,
                });
            }
            KeyCode::Enter => match cursor {
                0 => {
                    component.dialog = Some(GroupDialog::EditName { group });
                }
                1 | 2 => {
                    let field = if cursor == 1 {
                        GroupEditField::Source
                    } else {
                        GroupEditField::Target
                    };
                    component.dialog = Some(GroupDialog::EditBrowse {
                        group,
                        field,
                        path: ctx.home_dir.clone(),
                        cursor: 0,
                    });
                }
                3 => {
                    component.dialog = Some(GroupDialog::IgnoreManage {
                        group: Some(group.clone()),
                        cursor: 0,
                    });
                }
                _ => {
                    component.dialog = Some(GroupDialog::EditField { group, cursor });
                }
            },
            _ => {
                component.dialog = Some(GroupDialog::EditField { group, cursor });
            }
        },
        Some(GroupDialog::EditName { group }) => match key.code {
            KeyCode::Esc => {
                component.dialog = Some(GroupDialog::EditField { group, cursor: 0 });
            }
            KeyCode::Enter if !group.is_empty() => {
                if let Some(old_group) = pending.config.groups.get(&group) {
                    let source = old_group.source.clone();
                    let target = old_group.target.clone();
                    pending.config.remove_group(&group);
                    pending.config.add_group(group.clone(), source, target);
                    component.config_dirty = true;
                }
                component.dialog = None;
            }
            KeyCode::Enter => {
                component.dialog = Some(GroupDialog::EditName { group });
            }
            KeyCode::Char(c) => {
                let mut new_name = group;
                new_name.push(c);
                component.dialog = Some(GroupDialog::EditName { group: new_name });
            }
            KeyCode::Backspace => {
                let mut new_name = group;
                new_name.pop();
                component.dialog = Some(GroupDialog::EditName { group: new_name });
            }
            _ => {
                component.dialog = Some(GroupDialog::EditName { group });
            }
        },
        Some(GroupDialog::EditBrowse {
            group,
            field,
            path,
            cursor,
        }) => match key.code {
            KeyCode::Esc => {
                component.dialog = Some(GroupDialog::EditField {
                    group,
                    cursor: (if matches!(field, GroupEditField::Source) {
                        1
                    } else {
                        2
                    }),
                });
            }
            KeyCode::Up | KeyCode::Char('k') => {
                let entries = andre_core::adopt::list_target_dirs(&path).unwrap_or_default();
                if !entries.is_empty() {
                    let new_cursor = if cursor == 0 {
                        entries.len() - 1
                    } else {
                        cursor - 1
                    };
                    component.dialog = Some(GroupDialog::EditBrowse {
                        group,
                        field,
                        path,
                        cursor: new_cursor,
                    });
                } else {
                    component.dialog = Some(GroupDialog::EditBrowse {
                        group,
                        field,
                        path,
                        cursor,
                    });
                }
            }
            KeyCode::Down | KeyCode::Char('j') => {
                let entries = andre_core::adopt::list_target_dirs(&path).unwrap_or_default();
                if !entries.is_empty() {
                    let new_cursor = (cursor + 1) % entries.len();
                    component.dialog = Some(GroupDialog::EditBrowse {
                        group,
                        field,
                        path,
                        cursor: new_cursor,
                    });
                } else {
                    component.dialog = Some(GroupDialog::EditBrowse {
                        group,
                        field,
                        path,
                        cursor,
                    });
                }
            }
            KeyCode::Right => {
                let entries = andre_core::adopt::list_target_dirs(&path).unwrap_or_default();
                if let Some(entry) = entries.get(cursor) {
                    if entry.is_dir {
                        component.dialog = Some(GroupDialog::EditBrowse {
                            group,
                            field,
                            path: path.join(&entry.name),
                            cursor: 0,
                        });
                    } else {
                        component.dialog = Some(GroupDialog::EditBrowse {
                            group,
                            field,
                            path,
                            cursor,
                        });
                    }
                } else {
                    component.dialog = Some(GroupDialog::EditBrowse {
                        group,
                        field,
                        path,
                        cursor,
                    });
                }
            }
            KeyCode::Left => {
                if let Some(parent) = path.parent() {
                    component.dialog = Some(GroupDialog::EditBrowse {
                        group,
                        field,
                        path: parent.to_path_buf(),
                        cursor: 0,
                    });
                } else {
                    component.dialog = Some(GroupDialog::EditBrowse {
                        group,
                        field,
                        path,
                        cursor,
                    });
                }
            }
            KeyCode::Enter => {
                let entries = andre_core::adopt::list_target_dirs(&path).unwrap_or_default();
                if let Some(entry) = entries.get(cursor) {
                    let selected_path = path.join(&entry.name).to_string_lossy().to_string();
                    if let Some(g) = pending.config.groups.get_mut(&group) {
                        match field {
                            GroupEditField::Source => g.source = selected_path,
                            GroupEditField::Target => g.target = selected_path,
                            GroupEditField::Ignore => unreachable!(),
                        }
                        component.config_dirty = true;
                    }
                    component.dialog = None;
                } else {
                    component.dialog = Some(GroupDialog::EditBrowse {
                        group,
                        field,
                        path,
                        cursor,
                    });
                }
            }
            _ => {
                component.dialog = Some(GroupDialog::EditBrowse {
                    group,
                    field,
                    path,
                    cursor,
                });
            }
        },
        Some(GroupDialog::IgnoreManage { group, cursor }) => {
            let patterns: Vec<String> = match &group {
                Some(g) => pending
                    .config
                    .get_group_ignore(g)
                    .map(|v| v.to_vec())
                    .unwrap_or_default(),
                None => pending.config.global_ignore(),
            };
            match key.code {
                KeyCode::Esc => {
                    if let Some(g) = group {
                        component.dialog = Some(GroupDialog::EditField {
                            group: g,
                            cursor: 3,
                        });
                    } else {
                        component.dialog = None;
                    }
                }
                KeyCode::Up | KeyCode::Char('k') => {
                    let total = patterns.len() + 1;
                    let new_cursor = if cursor == 0 { total - 1 } else { cursor - 1 };
                    component.dialog = Some(GroupDialog::IgnoreManage {
                        group,
                        cursor: new_cursor,
                    });
                }
                KeyCode::Down | KeyCode::Char('j') => {
                    let total = patterns.len() + 1;
                    let new_cursor = (cursor + 1) % total;
                    component.dialog = Some(GroupDialog::IgnoreManage {
                        group,
                        cursor: new_cursor,
                    });
                }
                KeyCode::Char('r') | KeyCode::Enter if cursor < patterns.len() => {
                    let target_group = group.clone();
                    let pat = patterns[cursor].clone();
                    match &target_group {
                        Some(g) => {
                            if let Some(g) = pending.config.groups.get_mut(g) {
                                g.ignore = g.ignore.as_mut().map(|v| {
                                    v.retain(|p| p != &pat);
                                    v.clone()
                                });
                            }
                        }
                        None => {
                            if let Some(ref mut global) = pending.config.global.ignore {
                                global.retain(|p| p != &pat);
                            }
                        }
                    }
                    component.config_dirty = true;
                    component.dialog = Some(GroupDialog::IgnoreManage {
                        group,
                        cursor: cursor.min(patterns.len().saturating_sub(1)),
                    });
                }
                KeyCode::Enter if cursor >= patterns.len() => {
                    component.dialog = Some(GroupDialog::IgnoreAdd {
                        group,
                        pattern: String::new(),
                    });
                }
                _ => {
                    component.dialog = Some(GroupDialog::IgnoreManage { group, cursor });
                }
            }
        }
        Some(GroupDialog::IgnoreAdd { group, pattern }) => match key.code {
            KeyCode::Esc => {
                component.dialog = Some(GroupDialog::IgnoreManage { group, cursor: 0 });
            }
            KeyCode::Enter if !pattern.is_empty() => {
                let target_group = group.clone();
                let new_pat = pattern.clone();
                match &target_group {
                    Some(g) => {
                        if let Some(g) = pending.config.groups.get_mut(g) {
                            g.ignore.get_or_insert_with(Vec::new).push(new_pat);
                        }
                    }
                    None => {
                        pending
                            .config
                            .global
                            .ignore
                            .get_or_insert_with(Vec::new)
                            .push(new_pat);
                    }
                }
                component.config_dirty = true;
                component.dialog = Some(GroupDialog::IgnoreManage { group, cursor: 0 });
            }
            KeyCode::Enter => {
                component.dialog = Some(GroupDialog::IgnoreAdd { group, pattern });
            }
            KeyCode::Char(c) => {
                let mut new_pattern = pattern;
                new_pattern.push(c);
                component.dialog = Some(GroupDialog::IgnoreAdd {
                    group,
                    pattern: new_pattern,
                });
            }
            KeyCode::Backspace => {
                let mut new_pattern = pattern;
                new_pattern.pop();
                component.dialog = Some(GroupDialog::IgnoreAdd {
                    group,
                    pattern: new_pattern,
                });
            }
            _ => {
                component.dialog = Some(GroupDialog::IgnoreAdd { group, pattern });
            }
        },
        None => {}
    }

    Transition::Handled
}
