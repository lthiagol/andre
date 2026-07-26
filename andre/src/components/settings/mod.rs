use crate::components::{AppContext, Component, InputMode, Transition};
use crossterm::event::{KeyCode, KeyEvent};
use ratatui::{layout::Rect, Frame};
pub mod dialogs;
pub mod pending;
pub mod persist;
pub mod picker;
pub mod rows;
pub mod types;
use self::pending::PendingSettings;
use self::rows::*;
use crate::theme::ThemeColors;
pub use types::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ConfigMessage {
    #[default]
    Hidden,
    ConfirmDiscard,
    Saved,
}

#[derive(Default)]
pub struct SettingsComponent {
    pub cursor: usize,
    pub picker: Option<PickerState>,
    pub config_dirty: bool,
    pub config_message: ConfigMessage,
    pub dialog: Option<GroupDialog>,
    pending: Option<PendingSettings>,
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
        match (&self.picker, &self.dialog) {
            (Some(_), _) => InputMode::Modal,
            (
                _,
                Some(
                    GroupDialog::AddName { .. }
                    | GroupDialog::EditName { .. }
                    | GroupDialog::IgnoreAdd { .. },
                ),
            ) => InputMode::TextInput,
            _ => InputMode::Normal,
        }
    }

    fn handle_key(&mut self, key: KeyEvent, ctx: &mut AppContext) -> Transition {
        if self.pending.is_none() {
            self.pending = Some(PendingSettings::from_core(&ctx.core));
        }
        if self.dialog.is_some() {
            return dialogs::handle_dialog_key(self, key, ctx);
        }

        // Picker popup
        if picker::handle_picker_key(self, &key) {
            return Transition::Handled;
        }

        // Navigation
        let show_saved = self.config_message != ConfigMessage::Hidden;
        let total = row_count(show_saved);
        if crate::input::Input::is_up(&key) || key.code == KeyCode::Char('k') {
            for _ in 0..total {
                self.cursor = (self.cursor + total - 1) % total;
                if is_interactive_row(self.cursor, show_saved) {
                    break;
                }
            }
            return Transition::None;
        }
        if crate::input::Input::is_down(&key) || key.code == KeyCode::Char('j') {
            for _ in 0..total {
                self.cursor = (self.cursor + 1) % total;
                if is_interactive_row(self.cursor, show_saved) {
                    break;
                }
            }
            return Transition::None;
        }

        let cursor = self.cursor;

        match key.code {
            KeyCode::Enter => {
                if cursor == ROW_VERBOSITY {
                    let p = self.pending.as_mut().unwrap();
                    p.verbosity = (p.verbosity + 1) % 6;
                    self.config_dirty = true;
                } else if cursor == ROW_DRY_RUN {
                    let p = self.pending.as_mut().unwrap();
                    p.dry_run = !p.dry_run;
                    self.config_dirty = true;
                } else if cursor == ROW_NO_FOLDING {
                    let p = self.pending.as_mut().unwrap();
                    p.no_folding = !p.no_folding;
                    self.config_dirty = true;
                } else if cursor == ROW_ADOPT {
                    let p = self.pending.as_mut().unwrap();
                    p.adopt = !p.adopt;
                    self.config_dirty = true;
                } else if cursor == ROW_DOTFILES {
                    let p = self.pending.as_mut().unwrap();
                    p.dotfiles = !p.dotfiles;
                    self.config_dirty = true;
                } else if cursor == ROW_ACTION {
                    let val = self.pending.as_ref().unwrap().action.as_str().to_string();
                    picker::open_picker(self, "Action", PickerField::Action, &val);
                } else if cursor == ROW_THEME {
                    let val = self.pending.as_ref().unwrap().theme.name().to_string();
                    picker::open_picker(self, "Theme", PickerField::Theme, &val);
                } else if cursor == ROW_IGNORE {
                    self.dialog = Some(GroupDialog::IgnoreManage {
                        group: None,
                        cursor: 0,
                    });
                } else if cursor == ROW_GROUP_LINE {
                    self.dialog = Some(GroupDialog::Actions { cursor: 0 });
                } else if cursor == ROW_SAVE_BUTTON {
                    persist::save_config(self, ctx);
                }
            }
            KeyCode::Char(' ') if (ROW_DRY_RUN..=ROW_DOTFILES).contains(&cursor) => {
                let p = self.pending.as_mut().unwrap();
                match cursor {
                    ROW_DRY_RUN => p.dry_run = !p.dry_run,
                    ROW_NO_FOLDING => p.no_folding = !p.no_folding,
                    ROW_ADOPT => p.adopt = !p.adopt,
                    ROW_DOTFILES => p.dotfiles = !p.dotfiles,
                    _ => {}
                }
                self.config_dirty = true;
            }
            KeyCode::Esc => {
                if self.config_dirty {
                    if self.config_message == ConfigMessage::ConfirmDiscard {
                        self.pending = None;
                        self.config_dirty = false;
                        self.config_message = ConfigMessage::Hidden;
                        return Transition::Pop;
                    }
                    self.config_message = ConfigMessage::ConfirmDiscard;
                    return Transition::None;
                }
                self.pending = None;
                return Transition::Pop;
            }
            KeyCode::Char('y') | KeyCode::Char('Y')
                if self.config_message == ConfigMessage::ConfirmDiscard && self.config_dirty =>
            {
                if persist::save_config(self, ctx) {
                    return Transition::Pop;
                }
                // Stay on Settings so the user can retry after a failed write.
                return Transition::Handled;
            }
            KeyCode::Char('n') | KeyCode::Char('N') => {
                if self.config_message == ConfigMessage::Saved {
                    self.config_message = ConfigMessage::Hidden;
                    return Transition::Handled;
                }
                if self.config_message == ConfigMessage::ConfirmDiscard && self.config_dirty {
                    self.pending = None;
                    self.config_dirty = false;
                    self.config_message = ConfigMessage::Hidden;
                    return Transition::Pop;
                } else if let Some(ref mut pending) = self.pending {
                    pending.no_folding = !pending.no_folding;
                    self.config_dirty = true;
                    return Transition::Handled;
                }
            }
            KeyCode::Char('t') => {
                if let Some(ref mut pending) = self.pending {
                    pending.theme = pending.theme.next();
                    self.config_dirty = true;
                }
                return Transition::Handled;
            }
            KeyCode::Char('v') => {
                if let Some(ref mut pending) = self.pending {
                    pending.verbosity = (pending.verbosity + 1) % 6;
                    self.config_dirty = true;
                }
                return Transition::Handled;
            }
            KeyCode::Char('d') => {
                if let Some(ref mut pending) = self.pending {
                    pending.dry_run = !pending.dry_run;
                    self.config_dirty = true;
                }
                return Transition::Handled;
            }
            KeyCode::Char('a') => {
                if let Some(ref mut pending) = self.pending {
                    pending.adopt = !pending.adopt;
                    self.config_dirty = true;
                }
                return Transition::Handled;
            }
            KeyCode::Char('o') => {
                if let Some(ref mut pending) = self.pending {
                    pending.dotfiles = !pending.dotfiles;
                    self.config_dirty = true;
                }
                return Transition::Handled;
            }
            _ => {}
        }

        Transition::None
    }

    fn render(&mut self, frame: &mut Frame, area: Rect, ctx: &AppContext) {
        if self.pending.is_none() {
            self.pending = Some(PendingSettings::from_core(&ctx.core));
        }
        let pending = self.pending.as_ref().unwrap();
        let colors = ThemeColors::from_theme(pending.theme);
        let filename = ctx
            .config_path
            .file_name()
            .and_then(|s| s.to_str())
            .unwrap_or("andre.yml");
        let total = row_count(self.config_message != ConfigMessage::Hidden);
        let cursor = self.cursor.min(total.saturating_sub(1));
        let config_message = self.config_message;
        crate::ui::render_settings(
            frame,
            area,
            &colors,
            &pending.config,
            &ctx.config_path,
            &ctx.config_dir,
            pending.verbosity as usize,
            pending.dry_run,
            pending.no_folding,
            pending.adopt,
            pending.dotfiles,
            pending.action.as_str(),
            pending.theme.name(),
            cursor,
            config_message,
            self.config_dirty,
            filename,
            self.picker.as_ref(),
            self.dialog.as_ref(),
        );
    }
}
