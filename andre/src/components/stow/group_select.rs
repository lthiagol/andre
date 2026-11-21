use crossterm::event::{KeyCode, KeyEvent};
use ratatui::{layout::Rect, widgets::ListState, Frame};

use crate::components::stow::PackageSelectComponent;
use crate::components::{AppContext, Component, Transition};
use crate::theme::ThemeColors;
use crate::ui::icons::Icons;
use crate::ui::widgets::group_list::GroupListWidget;
use crate::ui::widgets::scrollbar::{border_track, render_scrollbar};

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
        let groups: Vec<String> = ctx.core.config.groups.keys().cloned().collect();
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

        let mut state = ListState::default();
        let widget = GroupListWidget::new(
            &ctx.core.config,
            &colors,
            Icons::for_set(ctx.core.icons),
            "select groups",
            self.cursor,
            &ctx.config_dir,
            &ctx.home_dir,
        )
        .with_selected(&self.selected);
        frame.render_stateful_widget(widget, area, &mut state);
        render_scrollbar(
            frame,
            border_track(area),
            &colors,
            group_count,
            page_size,
            self.cursor,
        );
    }

    fn prepare(&mut self, area: Rect) {
        self.viewport_height = area.height.saturating_sub(2).max(1) as usize;
    }
}
