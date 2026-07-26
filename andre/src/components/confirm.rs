use std::collections::HashMap;

use crossterm::event::KeyEvent;
use ratatui::{layout::Rect, Frame};

use crate::components::execute::ExecuteComponent;
use crate::components::{AppContext, Component, Transition};
use crate::execute::{build_execution_results, AsyncExecutor};
use crate::input::Input;
use crate::theme::ThemeColors;
use andre_core::{engine_for, LinkOp};

pub struct ConfirmComponent {
    pub selected_packages: HashMap<String, Vec<String>>,
    pub action: String,
    scroll_index: usize,
    viewport_height: usize,
    content_len: usize,
}

impl ConfirmComponent {
    pub fn new(selected_packages: HashMap<String, Vec<String>>, action: String) -> Self {
        let content_len: usize = selected_packages.values().map(|v| v.len()).sum();
        Self {
            selected_packages,
            action,
            scroll_index: 0,
            viewport_height: 10,
            content_len,
        }
    }

    fn total_lines(&self) -> usize {
        // Header (title) + action line + settings line + prompt + footer gap
        const HEADER_LINES: usize = 4;
        self.content_len + HEADER_LINES
    }
}

impl Component for ConfirmComponent {
    fn id(&self) -> &'static str {
        "Confirm"
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }

    fn handle_key(&mut self, key: KeyEvent, ctx: &mut AppContext) -> Transition {
        if key.code == crossterm::event::KeyCode::Char('y')
            || key.code == crossterm::event::KeyCode::Char('Y')
        {
            let (commands, skipped) = build_execution_results(
                &self.selected_packages,
                &ctx.core.config,
                &ctx.config_dir,
                &ctx.home_dir,
            );
            // Select the link engine from config (native default, stow opt-in).
            let engine = engine_for(&ctx.core.config.effective_engine());
            let ops: Vec<LinkOp> = commands
                .into_iter()
                .map(|(group, package, source, target, ignores)| LinkOp {
                    group,
                    package,
                    source,
                    target,
                    ignores,
                    action: ctx.core.action,
                    dry_run: ctx.core.dry_run,
                    no_folding: ctx.core.no_folding,
                    adopt: ctx.core.adopt,
                    dotfiles: ctx.core.dotfiles,
                    verbosity: ctx.core.verbosity,
                })
                .collect();
            let total = ops.len() + skipped.len();
            let (executor, rx) = AsyncExecutor::spawn(ops, engine);
            // Carry executor on Replace so App owns the AbortHandle for
            // Execute's lifetime. ExecuteComponent receives rx only.
            return Transition::Replace(
                Box::new(ExecuteComponent::new(rx, total, skipped)),
                Some(executor),
            );
        } else if key.code == crossterm::event::KeyCode::Char('n')
            || key.code == crossterm::event::KeyCode::Char('N')
            || key.code == crossterm::event::KeyCode::Esc
        {
            return Transition::Pop;
        } else {
            let lines = self.total_lines();
            Input::handle_list_scroll(&key, &mut self.scroll_index, lines, self.viewport_height);
        }
        Transition::None
    }

    fn render(&mut self, frame: &mut Frame, area: Rect, ctx: &AppContext) {
        let colors = ThemeColors::from_theme(ctx.core.theme);
        crate::ui::render_confirm(
            frame,
            area,
            &colors,
            &self.selected_packages,
            &self.action,
            &ctx.core.config,
            &ctx.config_dir,
            &ctx.home_dir,
            ctx.core.verbosity as usize,
            ctx.core.dry_run,
            ctx.core.no_folding,
            ctx.core.adopt,
            ctx.core.dotfiles,
            self.scroll_index,
            self.viewport_height,
        );
    }

    fn prepare(&mut self, area: Rect) {
        // borders (2). Owned here so paint is a pure draw.
        self.viewport_height = area.height.saturating_sub(2).max(1) as usize;
    }
}
