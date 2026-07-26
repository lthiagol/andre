use std::path::PathBuf;

use crossterm::event::{KeyCode, KeyEvent};
use ratatui::{layout::Rect, widgets::ListState, Frame};

use crate::components::{AppContext, Component, Transition};
use crate::theme::ThemeColors;
use crate::ui::widgets::group_list::GroupListWidget;
use andre_core::path::resolve_path;

#[derive(Default)]
pub struct UnstowTargetSelectComponent {
    pub cursor: usize,
}

impl Component for UnstowTargetSelectComponent {
    fn id(&self) -> &'static str {
        "UnstowTargetSelect"
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }

    fn handle_key(&mut self, key: KeyEvent, ctx: &mut AppContext) -> Transition {
        let groups: Vec<String> = ctx.core.config.groups.keys().cloned().collect();
        let len = groups.len();

        if len > 0 && crate::input::Input::handle_list_navigation(&key, &mut self.cursor, len) {
            return Transition::None;
        }

        if key.code == KeyCode::Enter {
            let group_name = groups.get(self.cursor).cloned().unwrap_or_default();
            let browse_path = ctx
                .core
                .config
                .groups
                .get(&group_name)
                .map(|g| resolve_path(&g.target, &ctx.config_dir, &ctx.home_dir))
                .unwrap_or_else(|| PathBuf::from("."));
            Transition::Push(Box::new(super::UnstowBrowseComponent::with_path(
                group_name,
                browse_path,
            )))
        } else if key.code == KeyCode::Esc {
            Transition::Pop
        } else {
            Transition::None
        }
    }

    fn render(&mut self, frame: &mut Frame, area: Rect, ctx: &AppContext) {
        let colors = ThemeColors::from_theme(ctx.core.theme);
        let mut state = ListState::default();
        let widget = GroupListWidget::new(
            &ctx.core.config,
            &colors,
            "Select Target for Unstow",
            self.cursor,
            &ctx.config_dir,
            &ctx.home_dir,
        );
        frame.render_stateful_widget(widget, area, &mut state);
    }
}
