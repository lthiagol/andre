use crossterm::event::{KeyCode, KeyEvent};
use ratatui::{layout::Rect, Frame};

use crate::components::{AppContext, Component, Transition};
use crate::theme::ThemeColors;

pub struct AdoptConfirmComponent {
    pub group_name: String,
    pub selected_files: Vec<std::path::PathBuf>,
}

impl Component for AdoptConfirmComponent {
    fn id(&self) -> &'static str {
        "AdoptConfirm"
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }

    fn handle_key(&mut self, key: KeyEvent, _ctx: &mut AppContext) -> Transition {
        if key.code == KeyCode::Char('y') || key.code == KeyCode::Char('Y') {
            Transition::Push(Box::new(super::AdoptNamePromptComponent::new(
                self.group_name.clone(),
                self.selected_files.clone(),
            )))
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
        let title = format!(" {} > Confirm Adoption ", ctx.breadcrumb);
        crate::ui::render_adopt_confirm(frame, area, &colors, &title, &self.selected_files);
    }
}
