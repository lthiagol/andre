use crossterm::event::KeyEvent;
use ratatui::{layout::Rect, Frame};

use crate::components::{AppContext, Component, Transition};
use crate::input::Input;
use crate::theme::ThemeColors;

#[derive(Default)]
pub struct StatusComponent {
    invalidated: bool,
    scroll_index: usize,
    viewport_height: usize,
    content_len: usize,
}

impl Component for StatusComponent {
    fn id(&self) -> &'static str {
        "Status"
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }

    fn update(&mut self, ctx: &mut AppContext) -> Transition {
        if !self.invalidated {
            ctx.status_cache.borrow_mut().invalidate_all();
            // Prefetch all groups' status
            for (group_name, group) in &ctx.core.config.groups {
                let source =
                    andre_core::path::resolve_path(&group.source, &ctx.config_dir, &ctx.home_dir);
                let target =
                    andre_core::path::resolve_path(&group.target, &ctx.config_dir, &ctx.home_dir);
                if source.exists() && target.exists() {
                    let packages = andre_core::discover_packages(&source);
                    let mut cache = ctx.status_cache.borrow_mut();
                    cache.prefetch_group(&source, &target, group_name, &packages);
                }
            }
            self.invalidated = true;
        }
        Transition::None
    }

    fn handle_key(&mut self, key: KeyEvent, _ctx: &mut AppContext) -> Transition {
        if key.code == crossterm::event::KeyCode::Enter
            || key.code == crossterm::event::KeyCode::Esc
        {
            return Transition::Pop;
        }
        let len = self.content_len.max(self.viewport_height);
        Input::handle_list_scroll(&key, &mut self.scroll_index, len, self.viewport_height);
        Transition::None
    }

    fn render(&mut self, frame: &mut Frame, area: Rect, ctx: &AppContext) {
        let colors = ThemeColors::from_theme(ctx.core.theme);
        // content_len is discovered while building the status list for paint;
        // viewport bookkeeping lives in prepare.
        self.content_len = crate::ui::render_status(
            frame,
            area,
            &colors,
            &ctx.breadcrumb,
            &ctx.core.config,
            &ctx.config_dir,
            &ctx.home_dir,
            &ctx.status_cache,
            self.scroll_index,
            self.viewport_height,
        );
    }

    fn prepare(&mut self, area: Rect) {
        self.viewport_height = area.height.saturating_sub(2).max(1) as usize;
    }
}
