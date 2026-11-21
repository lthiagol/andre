//! The settings screen: a list of stow options, appearance, the global ignore list and the
//! groups, with the selected row's detail beside it. Edits go to a pending copy. `w` writes
//! it to `andre.yml`. Esc with unsaved edits asks first. Dialogs for pickers, group edits and
//! removals sit on top of the list (see `overlays`).

use andre_core::{IconSet, StowFlag, Theme};
use crossterm::event::{KeyCode, KeyEvent};
use ratatui::{layout::Rect, Frame};

use crate::components::{AppContext, Component, InputMode, Transition};
use crate::input::{flag_for_key, Input};
use crate::theme::ThemeColors;
use crate::ui;
use crate::ui::detail::{render_detail, Detail};
use crate::ui::footer::{self, Hint};
use crate::ui::layout::{screen_rects, Shape};
use crate::ui::list_panel::ListView;
use crate::ui::text_field::TextField;

pub mod build;
pub mod overlays;
pub mod pending;
pub mod persist;
pub mod types;

#[cfg(test)]
mod tests;

use self::pending::PendingSettings;
pub use types::*;

#[derive(Default)]
pub struct SettingsComponent {
    /// The cursor and the window over `items`.
    pub list: ListView,
    /// Unsaved edits. Set by every change to the pending settings, cleared by a save.
    pub dirty: bool,
    /// Dialogs on top of the list. The last one takes keys and is the only one painted.
    pub overlays: Vec<Overlay>,
    /// The settings as the user has left them. `w` writes them.
    pending: Option<PendingSettings>,
    /// Built by `refresh` from the pending settings, read while painting.
    items: Vec<Item>,
    /// The selected row's detail, and its title, built by `refresh`.
    detail: Detail,
    detail_title: String,
    /// The row the cursor moves to at the next `refresh`: a group that was just renamed or
    /// added, or the group a cancelled menu was on.
    focus: Option<Row>,
}

impl SettingsComponent {
    /// Starts the pending copy from the saved settings the first time the screen needs it.
    fn ensure(&mut self, ctx: &AppContext) {
        if self.pending.is_none() {
            self.pending = Some(PendingSettings::from_core(&ctx.core));
            self.refresh(ctx);
        }
    }

    /// Rebuilds the rows and the detail from the pending settings. The cursor moves off a
    /// header if a change left it there.
    fn refresh(&mut self, ctx: &AppContext) {
        let Some(pending) = self.pending.as_ref() else {
            return;
        };
        self.items = build::items(pending, ctx);
        if let Some(row) = self.focus.take() {
            if let Some(index) = self.items.iter().position(|item| item.row == row) {
                self.list.cursor = index;
            }
        }
        if !self
            .items
            .get(self.list.cursor)
            .is_some_and(Item::selectable)
        {
            self.list.cursor = self.items.iter().position(Item::selectable).unwrap_or(0);
        }
        match self.items.get(self.list.cursor) {
            Some(item) => {
                self.detail = build::detail(&item.row, pending, ctx);
                self.detail_title = item.label.clone();
            }
            None => {
                self.detail = Detail::default();
                self.detail_title.clear();
            }
        }
    }

    /// The rows as the list paints them, headers included.
    pub fn items(&self) -> &[Item] {
        &self.items
    }

    /// The settings the screen edits, once it has been shown.
    pub fn pending(&self) -> Option<&PendingSettings> {
        self.pending.as_ref()
    }

    fn selected(&self) -> Option<&Item> {
        self.items.get(self.list.cursor)
    }

    /// Moves the cursor to the next selectable row in `down` or up. Headers are skipped.
    fn step(&mut self, down: bool) {
        let len = self.items.len();
        for _ in 0..len {
            self.list.step(down, len);
            if self.selected().is_some_and(Item::selectable) {
                break;
            }
        }
    }

    /// Enter and Space on the selected row: flip a flag, cycle a value, or open its dialog.
    fn activate(&mut self) {
        let Some(row) = self.selected().map(|item| item.row.clone()) else {
            return;
        };
        let Some(pending) = self.pending.as_mut() else {
            return;
        };
        let overlay = match row {
            Row::Header => None,
            Row::Flag(flag) => {
                pending.flags.toggle(flag);
                self.dirty = true;
                None
            }
            Row::Action => Some(Overlay::Action {
                cursor: build::action_index(pending.flags.action),
            }),
            Row::Theme => Some(Overlay::Theme {
                cursor: Theme::ALL
                    .iter()
                    .position(|t| *t == pending.theme)
                    .unwrap_or(0),
                original: pending.theme,
            }),
            Row::Icons => Some(Overlay::Icons {
                cursor: IconSet::ALL
                    .iter()
                    .position(|i| *i == pending.icons)
                    .unwrap_or(0),
                original: pending.icons,
            }),
            Row::GlobalIgnore => Some(Overlay::Ignore {
                scope: IgnoreScope::Global,
                cursor: 0,
                edit: None,
            }),
            Row::Group(group) => Some(Overlay::Group {
                group: group.clone(),
                cursor: 0,
                snapshot: Snapshot {
                    group,
                    config: pending.config.clone(),
                    changes: pending.group_changes.clone(),
                    dirty: self.dirty,
                },
            }),
        };
        if let Some(overlay) = overlay {
            self.overlays.push(overlay);
        }
    }

    fn toggle_flag(&mut self, flag: StowFlag) {
        if let Some(pending) = self.pending.as_mut() {
            pending.flags.toggle(flag);
            self.dirty = true;
        }
    }

    fn handle_list_key(&mut self, key: KeyEvent, ctx: &mut AppContext) -> Transition {
        if Input::is_up(&key) || key.code == KeyCode::Char('k') {
            self.step(false);
            return Transition::Handled;
        }
        if Input::is_down(&key) || key.code == KeyCode::Char('j') {
            self.step(true);
            return Transition::Handled;
        }
        match key.code {
            KeyCode::Enter | KeyCode::Char(' ') => self.activate(),
            KeyCode::Char('+') => self.overlays.push(Overlay::Name {
                target: NameTarget::New,
                field: TextField::default(),
            }),
            KeyCode::Char('x') => {
                if let Some(Row::Group(group)) = self.selected().map(|item| item.row.clone()) {
                    let refs = self
                        .pending
                        .as_ref()
                        .map_or(0, |pending| build::refs_into(ctx, pending, &group));
                    self.overlays.push(Overlay::Remove { group, refs });
                }
            }
            KeyCode::Char('w') => {
                persist::save_config(self, ctx);
            }
            KeyCode::Esc => {
                if !self.dirty {
                    return Transition::Pop;
                }
                self.overlays.push(Overlay::Leave);
            }
            KeyCode::Char('t') => {
                if let Some(pending) = self.pending.as_mut() {
                    pending.theme = pending.theme.next();
                }
                self.dirty = true;
            }
            KeyCode::Char('s') => {
                if let Some(pending) = self.pending.as_mut() {
                    pending.flags.action = pending.flags.action.next();
                }
                self.dirty = true;
            }
            _ => match flag_for_key(key.code) {
                Some(flag) => self.toggle_flag(flag),
                None => return Transition::None,
            },
        }
        Transition::Handled
    }

    /// `y` saves and leaves, `n` discards and leaves, Esc stays.
    fn handle_leave_key(&mut self, key: KeyEvent, ctx: &mut AppContext) -> Transition {
        match key.code {
            KeyCode::Char('y' | 'Y') => {
                self.overlays.pop();
                if persist::save_config(self, ctx) {
                    self.overlays.clear();
                    return Transition::Pop;
                }
                // Stay on Settings so the user can retry after a failed write.
                Transition::Handled
            }
            KeyCode::Char('n' | 'N') => {
                self.overlays.clear();
                self.dirty = false;
                Transition::Pop
            }
            KeyCode::Esc => {
                self.overlays.pop();
                Transition::Handled
            }
            _ => Transition::Handled,
        }
    }
}

impl Component for SettingsComponent {
    fn id(&self) -> &'static str {
        "Settings"
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }

    fn input_mode(&self) -> InputMode {
        match self.overlays.last() {
            None => InputMode::Normal,
            Some(Overlay::Name { .. } | Overlay::Ignore { edit: Some(_), .. }) => {
                InputMode::TextInput
            }
            Some(_) => InputMode::Modal,
        }
    }

    fn handle_key(&mut self, key: KeyEvent, ctx: &mut AppContext) -> Transition {
        self.ensure(ctx);
        let leaving = matches!(self.overlays.last(), Some(Overlay::Leave));
        let transition = if leaving {
            self.handle_leave_key(key, ctx)
        } else if !self.overlays.is_empty() {
            overlays::handle_key(self, key, ctx);
            Transition::Handled
        } else {
            self.handle_list_key(key, ctx)
        };
        self.refresh(ctx);
        transition
    }

    fn update(&mut self, ctx: &mut AppContext) -> Transition {
        self.ensure(ctx);
        Transition::None
    }

    fn hints(&self, _ctx: &AppContext) -> Vec<Hint> {
        match self.overlays.last() {
            Some(overlay) => overlays::hints(overlay),
            None => {
                let on_group = self
                    .selected()
                    .is_some_and(|item| matches!(item.row, Row::Group(_)));
                footer::settings_hints(on_group)
            }
        }
    }

    fn prepare(&mut self, area: Rect) {
        let rects = screen_rects(area, Shape::TwoPane);
        self.list.prepare(rects.list, self.items.len());
    }

    fn render(&mut self, frame: &mut Frame, area: Rect, ctx: &AppContext) {
        let Some(pending) = self.pending.as_ref() else {
            return;
        };
        let colors = ThemeColors::from_theme(pending.theme);
        let rects = screen_rects(area, Shape::TwoPane);
        ui::settings::render_list(
            frame,
            rects.list,
            &colors,
            self.dirty,
            &self.items,
            &self.list,
        );
        render_detail(
            frame,
            rects.detail,
            &colors,
            &self.detail_title,
            &self.detail,
        );
        if let Some(overlay) = self.overlays.last() {
            ui::settings::render_overlay(frame, area, &colors, overlay, pending, ctx);
        }
    }
}
