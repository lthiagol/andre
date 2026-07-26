use crossterm::event::KeyEvent;
use ratatui::{layout::Rect, Frame};

use crate::components::{AppContext, Component, LogLevel, Transition};
use crate::execute::{ExecutionResult, ExecutionState, ExecutionUpdate};
use crate::input::Input;
use crate::ui::{execute_list_len, GAUGE_ROWS};

pub struct ExecuteComponent {
    rx: Option<tokio::sync::mpsc::Receiver<ExecutionUpdate>>,
    pub state: ExecutionState,
    pub results: Vec<ExecutionResult>,
    pub total: usize,
    pub current: usize,
    pub spinner_frame: usize,
    pub scroll_index: usize,
    viewport_height: usize,
}

impl ExecuteComponent {
    pub fn new(
        rx: tokio::sync::mpsc::Receiver<ExecutionUpdate>,
        total: usize,
        initial_results: Vec<ExecutionResult>,
    ) -> Self {
        Self {
            rx: Some(rx),
            state: ExecutionState::Running,
            results: initial_results,
            total,
            current: 0,
            spinner_frame: 0,
            scroll_index: 0,
            viewport_height: 10,
        }
    }
}

impl Component for ExecuteComponent {
    fn id(&self) -> &'static str {
        "Execute"
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }

    fn handle_key(&mut self, key: KeyEvent, _ctx: &mut AppContext) -> Transition {
        if (key.code == crossterm::event::KeyCode::Enter
            || key.code == crossterm::event::KeyCode::Esc)
            && self.state == ExecutionState::Completed
        {
            return Transition::PopAll;
        }
        let len = execute_list_len(self.state, &self.results);
        Input::handle_list_scroll(&key, &mut self.scroll_index, len, self.viewport_height);
        Transition::None
    }

    fn update(&mut self, ctx: &mut AppContext) -> Transition {
        if let Some(ref mut rx) = self.rx {
            while let Ok(update) = rx.try_recv() {
                match update {
                    // Progress.total is commands-only; keep the full total
                    // (commands + skipped) established at construction.
                    ExecutionUpdate::Progress { current, .. } => {
                        self.current = current;
                        ctx.log(
                            LogLevel::Exec,
                            format!("Progress: {}/{}", current, self.total),
                        );
                    }
                    ExecutionUpdate::Result(result) => {
                        ctx.log(
                            LogLevel::Exec,
                            format!(
                                "Result: {} {} success={}",
                                result.group, result.package, result.success
                            ),
                        );
                        self.results.push(result);
                    }
                    ExecutionUpdate::Complete => {
                        self.rx = None;
                        self.state = ExecutionState::Completed;
                        ctx.log(LogLevel::Exec, "Execution complete".into());
                        break;
                    }
                }
            }
        }
        if self.state == ExecutionState::Completed {
            // Stay on results screen until user presses Enter/Esc
        }
        self.spinner_frame = self.spinner_frame.wrapping_add(1);
        Transition::None
    }

    fn render(&mut self, frame: &mut Frame, area: Rect, ctx: &AppContext) {
        let colors = crate::theme::ThemeColors::from_theme(ctx.core.theme);
        crate::ui::render_execute(
            frame,
            area,
            &colors,
            self.state,
            &self.results,
            self.spinner_frame,
            self.total,
            Some(self.scroll_index),
        );
    }

    fn prepare(&mut self, area: Rect) {
        // borders (2) + gauge row. Owned here so paint is a pure draw.
        self.viewport_height = area.height.saturating_sub(2 + GAUGE_ROWS).max(1) as usize;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use andre_core::config::Config;
    use andre_core::AppState as CoreState;
    use ratatui::backend::TestBackend;
    use ratatui::Terminal;

    fn make_ctx() -> AppContext {
        AppContext {
            core: CoreState::from_config(Config::default_empty(), false),
            config_dir: std::path::PathBuf::from("."),
            config_path: std::path::PathBuf::from("."),
            home_dir: std::path::PathBuf::from("."),
            status_cache: std::cell::RefCell::new(crate::status_cache::StatusCache::new()),
            terminal_size: (80, 24),
            debug: false,
            log_file: None,
            event_log: std::cell::RefCell::new(Vec::new()),
            toast: None,
        }
    }

    /// M05 AC-01 regression: viewport bookkeeping is owned by `prepare`, and
    /// `render` must NOT mutate `viewport_height` (paint draws prepared state).
    /// Uses a sentinel so any reassignment in render is caught regardless of
    /// the value it would write (L1: green tests do not imply correct behavior).
    #[test]
    fn prepare_owns_viewport_height_and_render_does_not_mutate_it() {
        let (_tx, rx) = tokio::sync::mpsc::channel(1);
        let mut comp = ExecuteComponent::new(rx, 0, Vec::new());

        // Sentinel: prove render never reassigns viewport_height.
        comp.viewport_height = 999;
        let mut terminal = Terminal::new(TestBackend::new(80, 24)).unwrap();
        let ctx = make_ctx();
        terminal
            .draw(|f| comp.render(f, Rect::new(0, 0, 80, 24), &ctx))
            .unwrap();
        assert_eq!(
            comp.viewport_height, 999,
            "render must not mutate viewport_height — paint is a pure draw of prepared state"
        );

        // prepare owns the bookkeeping: borders(2) + GAUGE_ROWS(1) => 24 - 3 = 21.
        comp.prepare(Rect::new(0, 0, 80, 24));
        assert_eq!(
            comp.viewport_height, 21,
            "prepare must set viewport_height = area.height - 2 - GAUGE_ROWS"
        );
    }
}
