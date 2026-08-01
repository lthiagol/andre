use std::path::PathBuf;

use crossterm::event::{KeyCode, KeyEvent};
use ratatui::{layout::Rect, Frame};

use crate::components::{AppContext, Component, Transition};
use crate::theme::ThemeColors;

pub struct UnstowConfirmComponent {
    pub selected_path: Option<PathBuf>,
    pub unlink_error: Option<String>,
    pub unlink_success: bool,
}

impl UnstowConfirmComponent {
    pub fn with_path(path: PathBuf) -> Self {
        Self {
            selected_path: Some(path),
            unlink_error: None,
            unlink_success: false,
        }
    }
}

impl Component for UnstowConfirmComponent {
    fn id(&self) -> &'static str {
        "UnstowConfirm"
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }

    fn handle_key(&mut self, key: KeyEvent, _ctx: &mut AppContext) -> Transition {
        if self.unlink_success || self.unlink_error.is_some() {
            match key.code {
                KeyCode::Enter | KeyCode::Esc => return Transition::PopAll,
                _ => return Transition::None,
            }
        }

        if key.code == KeyCode::Char('y') || key.code == KeyCode::Char('Y') {
            if let Some(ref path) = self.selected_path {
                match andre_core::unlink_stowed_symlink(path) {
                    Ok(true) => {
                        self.unlink_success = true;
                    }
                    Ok(false) => {
                        self.unlink_error = Some("Not a symlink, nothing to remove.".to_string());
                    }
                    Err(e) => {
                        self.unlink_error = Some(format!("Failed to unlink: {}", e));
                    }
                }
            }
            Transition::None
        } else if key.code == KeyCode::Char('n')
            || key.code == KeyCode::Char('N')
            || key.code == KeyCode::Esc
        {
            Transition::Pop
        } else {
            Transition::None
        }
    }

    fn render(&mut self, frame: &mut Frame, area: Rect, ctx: &AppContext) {
        let colors = ThemeColors::from_theme(ctx.core.theme);
        let title = format!(" {} > Unlink ", ctx.breadcrumb);
        crate::ui::render_unstow_confirm(
            frame,
            area,
            &colors,
            &title,
            self.selected_path.as_ref(),
            self.unlink_error.as_deref(),
            self.unlink_success,
        );
    }
}
