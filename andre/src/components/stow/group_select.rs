use crossterm::event::{KeyCode, KeyEvent};
use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    widgets::ListState,
    Frame,
};

use crate::components::stow::PackageSelectComponent;
use crate::components::{AppContext, Component, Transition};
use crate::theme::ThemeColors;
use crate::ui::widgets::group_list::GroupListWidget;
use crate::ui::widgets::scrollbar::render_scrollbar;

#[derive(Default)]
pub struct GroupSelectComponent {
    pub cursor: usize,
    pub selected: Vec<String>,
    viewport_height: usize,
}

impl Component for GroupSelectComponent {
    fn id(&self) -> &'static str {
        "GroupSelect"
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }

    fn handle_key(&mut self, key: KeyEvent, ctx: &mut AppContext) -> Transition {
        let mut groups: Vec<String> = ctx.core.config.groups.keys().cloned().collect();
        groups.sort();
        let len = groups.len();

        if len > 0
            && crate::input::Input::handle_list_scroll(
                &key,
                &mut self.cursor,
                len,
                self.viewport_height,
            )
        {
            return Transition::None;
        }

        if key.code == KeyCode::Char(' ') {
            if let Some(group) = groups.get(self.cursor) {
                if self.selected.contains(group) {
                    self.selected.retain(|g| g != group);
                } else {
                    self.selected.push(group.clone());
                }
                ctx.status_cache.borrow_mut().invalidate_all();
            }
        } else if key.code == KeyCode::Enter {
            if !self.selected.is_empty() {
                return Transition::Push(Box::new(PackageSelectComponent::new(
                    self.selected.clone(),
                )));
            }
        } else if key.code == KeyCode::Esc {
            return Transition::Pop;
        }

        Transition::None
    }

    fn render(&mut self, frame: &mut Frame, area: Rect, ctx: &AppContext) {
        let colors = ThemeColors::from_theme(ctx.core.theme);
        let group_count = ctx.core.config.groups.len();
        let page_size = self.viewport_height;

        if group_count > page_size && area.width > 4 {
            let chunks = Layout::default()
                .direction(Direction::Horizontal)
                .constraints([Constraint::Min(1), Constraint::Length(1)])
                .split(area);
            let mut state = ListState::default();
            let title = format!(" {} > Select Groups ", ctx.breadcrumb);
            let widget = GroupListWidget::new(
                &ctx.core.config,
                &colors,
                &title,
                self.cursor,
                &ctx.config_dir,
                &ctx.home_dir,
            )
            .with_selected(&self.selected);
            frame.render_stateful_widget(widget, chunks[0], &mut state);
            render_scrollbar(frame, chunks[1], &colors, group_count, self.cursor);
        } else {
            let mut state = ListState::default();
            let title = format!(" {} > Select Groups ", ctx.breadcrumb);
            let widget = GroupListWidget::new(
                &ctx.core.config,
                &colors,
                &title,
                self.cursor,
                &ctx.config_dir,
                &ctx.home_dir,
            )
            .with_selected(&self.selected);
            frame.render_stateful_widget(widget, area, &mut state);
        }
    }

    fn prepare(&mut self, area: Rect) {
        self.viewport_height = area.height.saturating_sub(2).max(1) as usize;
    }
}
