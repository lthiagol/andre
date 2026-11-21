//! The settings screen: a list of stow options, appearance, the packages folder and the global
//! ignore list, with the selected row's detail beside it. Edits go to a pending copy. `w`
//! writes it to `andre.yml`. Esc with unsaved edits asks first. Dialogs for pickers, the
//! packages folder and the ignore list sit on top of the list (see `overlays`).

use andre_core::{IconSet, StowAction, StowFlag, Theme};
use crossterm::event::{KeyCode, KeyEvent, MouseEvent, MouseEventKind};
use ratatui::{layout::Rect, Frame};

use crate::components::{AppContext, Component, InputMode, Transition};
use crate::input::{self, flag_for_key, Input};
use crate::theme::ThemeColors;
use crate::ui;
use crate::ui::detail::{fit_scroll, render_detail_scrolled, wheel_over, wheel_scroll, Detail};
use crate::ui::dialog::{dialog_body_rect, dialog_click, dialog_row_click};
use crate::ui::footer::{self, Hint};
use crate::ui::layout::{screen_rects, Shape};
use crate::ui::list_panel::{click_list, ListView};

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
    /// The row the cursor moves to at the next `refresh`, set when the screen opens on a row
    /// (see `on_packages`).
    focus: Option<Row>,
    /// The content area, as of the last `prepare`. Dialogs are centred in it.
    content: Rect,
    /// The detail pane as the last `prepare` laid it out.
    detail_rect: Rect,
    /// Rows the detail is scrolled down. It resets when the cursor moves.
    scroll: u16,
    /// The cursor row `scroll` belongs to.
    scroll_row: usize,
}

/// The cursor and option count of a picker overlay, or `None` for a dialog that is not a
/// picker.
fn picker_state(overlay: &Overlay) -> Option<(usize, usize)> {
    match overlay {
        Overlay::Action { cursor } => Some((*cursor, StowAction::ALL.len())),
        Overlay::Theme { cursor, .. } => Some((*cursor, Theme::ALL.len())),
        Overlay::Icons { cursor, .. } => Some((*cursor, IconSet::ALL.len())),
        _ => None,
    }
}

impl SettingsComponent {
    /// A settings screen whose cursor starts on the packages folder, as `--onboard` opens it.
    pub fn on_packages() -> Self {
        Self {
            focus: Some(Row::Packages),
            ..Self::default()
        }
    }

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
        // A detail starts at its top when the cursor moves onto its row.
        if self.list.cursor != self.scroll_row {
            self.scroll = 0;
            self.scroll_row = self.list.cursor;
        }
        let colors = ThemeColors::from_theme(ctx.core.theme);
        self.scroll = fit_scroll(&self.detail, self.detail_rect, &colors, self.scroll);
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
    fn activate(&mut self, ctx: &AppContext) {
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
            Row::Packages => Some(Overlay::Browse {
                browser: overlays::browser_at(
                    pending.config.packages_dir(&ctx.config_dir, &ctx.home_dir),
                    &ctx.home_dir,
                ),
            }),
            Row::GlobalIgnore => Some(Overlay::Ignore {
                cursor: 0,
                edit: None,
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
            KeyCode::Enter | KeyCode::Char(' ') => self.activate(ctx),
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

    /// Mouse input while a dialog is open. A button sends its key. On a picker a click on an
    /// option moves the cursor there, and a click on the cursor's option keeps it, as Enter
    /// does. The wheel moves a picker's cursor a row. Other clicks change nothing.
    fn overlay_mouse(&mut self, ev: MouseEvent, ctx: &mut AppContext) -> Transition {
        let colors = ThemeColors::from_theme(ctx.core.theme);
        let (Some(pending), Some(overlay)) = (self.pending.as_ref(), self.overlays.last()) else {
            return Transition::Handled;
        };
        let dialog = ui::settings::overlay_dialog(overlay, pending, ctx, &colors);
        let picker = picker_state(overlay);
        let button = dialog_click(self.content, &dialog, ev);
        let option = dialog_row_click(self.content, &dialog, ev);
        let wheel = wheel_over(ev, dialog_body_rect(self.content, &dialog));

        if let Some(key) = button {
            return self.handle_key(key, ctx);
        }
        if let Some((cursor, len)) = picker {
            if let Some(index) = option.filter(|&index| index < len) {
                if index == cursor {
                    return self.handle_key(input::key(KeyCode::Enter), ctx);
                }
                // Each step goes through the arrow keys, so the theme and icon pickers preview.
                let code = if index > cursor {
                    KeyCode::Down
                } else {
                    KeyCode::Up
                };
                for _ in 0..index.abs_diff(cursor) {
                    self.handle_key(input::key(code), ctx);
                }
                return Transition::Handled;
            }
            if let Some(down) = wheel {
                let code = if down { KeyCode::Down } else { KeyCode::Up };
                return self.handle_key(input::key(code), ctx);
            }
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
            Some(Overlay::Ignore { edit: Some(_), .. }) => InputMode::TextInput,
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

    /// Rebuilds the rows each tick, as `build` documents, so a change made outside a key (the
    /// cursor, a value in the core) shows at the next paint.
    fn update(&mut self, ctx: &mut AppContext) -> Transition {
        self.ensure(ctx);
        self.refresh(ctx);
        Transition::None
    }

    fn hints(&self, _ctx: &AppContext) -> Vec<Hint> {
        match self.overlays.last() {
            Some(overlay) => overlays::hints(overlay),
            None => footer::settings_hints(),
        }
    }

    /// Dialogs take the mouse: their buttons and picker rows work, and the list behind them
    /// does not (see `overlay_mouse`).
    fn mouse_blocked(&self) -> bool {
        false
    }

    fn handle_mouse(&mut self, ev: MouseEvent, ctx: &mut AppContext) -> Transition {
        self.ensure(ctx);
        if !self.overlays.is_empty() {
            return self.overlay_mouse(ev, ctx);
        }
        if let Some(down) = wheel_over(ev, self.detail_rect) {
            let colors = ThemeColors::from_theme(ctx.core.theme);
            self.scroll = wheel_scroll(&self.detail, self.detail_rect, &colors, self.scroll, down);
            return Transition::Handled;
        }
        // A wheel notch moves to the next selectable row, as ↑↓ does, so it never rests on a
        // header. It works over a header row too.
        if matches!(
            ev.kind,
            MouseEventKind::ScrollUp | MouseEventKind::ScrollDown
        ) {
            if !self.list.contains(ev.column, ev.row) {
                return Transition::None;
            }
            self.step(ev.kind == MouseEventKind::ScrollDown);
            self.refresh(ctx);
            return Transition::Handled;
        }
        // A click on a header does nothing: the cursor never rests on one.
        if let Some(row) = self.list.row_at(ev.column, ev.row) {
            if self.items.get(row).is_none_or(|item| !item.selectable()) {
                return Transition::None;
            }
        }
        let len = self.items.len();
        match click_list(&mut self.list, ev, len) {
            Some(click) if click.activate => self.handle_key(input::key(KeyCode::Enter), ctx),
            Some(_) => {
                self.refresh(ctx);
                Transition::Handled
            }
            None => Transition::None,
        }
    }

    fn prepare(&mut self, area: Rect) {
        let rects = screen_rects(area, Shape::TwoPane);
        self.content = area;
        self.detail_rect = rects.detail;
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
        render_detail_scrolled(
            frame,
            rects.detail,
            &colors,
            &self.detail_title,
            &self.detail,
            self.scroll,
        );
        if let Some(overlay) = self.overlays.last() {
            ui::settings::render_overlay(frame, area, &colors, overlay, pending, ctx);
        }
    }
}
