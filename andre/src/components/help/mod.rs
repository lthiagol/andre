use crossterm::event::{KeyCode, KeyEvent};
use ratatui::{layout::Rect, Frame};

use crate::components::{AppContext, Component, Transition};
use crate::theme::ThemeColors;

pub struct HelpComponent {
    pub scroll_index: usize,
    viewport_height: usize,
}

impl Default for HelpComponent {
    fn default() -> Self {
        Self {
            scroll_index: 0,
            viewport_height: 10,
        }
    }
}

impl Component for HelpComponent {
    fn id(&self) -> &'static str {
        "Help"
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }

    fn handle_key(&mut self, key: KeyEvent, _ctx: &mut AppContext) -> Transition {
        let max_index = crate::ui::help::help_item_count().saturating_sub(1);
        if key.code == KeyCode::Up || key.code == KeyCode::Char('k') {
            self.scroll_index = self.scroll_index.saturating_sub(1);
            Transition::None
        } else if key.code == KeyCode::Down || key.code == KeyCode::Char('j') {
            self.scroll_index = (self.scroll_index + 1).min(max_index);
            Transition::None
        } else if key.code == KeyCode::Enter || key.code == KeyCode::Esc {
            Transition::Pop
        } else {
            Transition::None
        }
    }

    fn render(&mut self, frame: &mut Frame, area: Rect, ctx: &AppContext) {
        let colors = ThemeColors::from_theme(ctx.core.theme);
        let title = format!(" {} > Help ", ctx.breadcrumb);
        crate::ui::render_help(
            frame,
            area,
            &colors,
            &title,
            self.scroll_index,
            self.viewport_height,
        );
    }

    fn prepare(&mut self, area: Rect) {
        self.viewport_height = area.height.saturating_sub(2).max(1) as usize;
    }
}
