use crossterm::event::{KeyCode, KeyEvent, MouseEvent};
use ratatui::{layout::Rect, text::Line, widgets::Paragraph, Frame};

use crate::components::{AppContext, Component, LogLevel, Transition};
use crate::execute::{ExecutionResult, ExecutionState, ExecutionUpdate};
use crate::input::Input;
use crate::theme::ThemeColors;
use crate::ui::confirm::done_buttons;
use crate::ui::detail::{detail_inner, fit_scroll, render_detail_scrolled, Detail};
use crate::ui::dialog::render_action_bar;
use crate::ui::execute::{
    progress_line, result_detail, result_row, running_detail, running_row, spinner, summary_line,
};
use crate::ui::footer::{execute_hints, Hint};
use crate::ui::layout::{screen_rects, Shape};
use crate::ui::list_panel::{self, click_list, render_list_panel, Empty, ListView};

pub struct ExecuteComponent {
    rx: Option<tokio::sync::mpsc::Receiver<ExecutionUpdate>>,
    pub state: ExecutionState,
    pub results: Vec<ExecutionResult>,
    pub total: usize,
    pub current: usize,
    pub spinner_frame: usize,
    /// The action the run applies, named in the progress chip.
    action: &'static str,
    /// The packages the run applies, in order. Empty when the run was not planned.
    planned: Vec<(String, String)>,
    /// How many of `results` came before the run, the skips. They precede the planned
    /// packages, so the one in flight is `planned[results - initial]`.
    initial: usize,
    /// The result rows, then the package in flight while the run goes on.
    pub list: ListView,
    /// The cursor follows the newest result until the user moves it.
    follow: bool,
    /// The detail of the cursor's row. Built by `update`.
    detail: Detail,
    /// Rows the detail is scrolled down. It resets when the cursor moves.
    scroll: u16,
    /// The detail pane as the last `prepare` laid it out. `update` clamps the scroll to it.
    detail_rect: Rect,
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
            initial: initial_results.len(),
            results: initial_results,
            total,
            current: 0,
            spinner_frame: 0,
            action: "run",
            planned: Vec::new(),
            list: ListView::default(),
            follow: true,
            detail: Detail::default(),
            scroll: 0,
            detail_rect: Rect::default(),
        }
    }

    /// Names the run's action and lists the packages it will apply, in order.
    pub fn for_run(mut self, action: &'static str, planned: Vec<(String, String)>) -> Self {
        self.action = action;
        self.planned = planned;
        self
    }

    /// The package in flight, as `group/package`. `None` once the run has finished.
    fn in_flight_label(&self) -> Option<String> {
        if self.state != ExecutionState::Running {
            return None;
        }
        let next = self.results.len().saturating_sub(self.initial);
        Some(match self.planned.get(next) {
            Some((group, package)) => format!("{group}/{package}"),
            None => "running\u{2026}".to_string(),
        })
    }

    /// The rows the list shows: the results, then the package in flight.
    fn rows_len(&self) -> usize {
        self.results.len() + usize::from(self.in_flight_label().is_some())
    }

    fn rows(&self, colors: &ThemeColors, width: u16) -> Vec<Line<'static>> {
        let cursor = self.list.cursor;
        let mut rows: Vec<Line> = self
            .results
            .iter()
            .enumerate()
            .map(|(i, result)| result_row(result, i == cursor, colors, width))
            .collect();
        if let Some(label) = self.in_flight_label() {
            let selected = self.results.len() == cursor;
            rows.push(running_row(
                spinner(self.spinner_frame),
                &label,
                selected,
                colors,
                width,
            ));
        }
        rows
    }

    fn chip_line(&self, colors: &ThemeColors, width: u16) -> Line<'static> {
        match self.state {
            ExecutionState::Completed => summary_line(&self.results, colors),
            ExecutionState::Running => progress_line(
                self.action,
                self.results.len(),
                self.total,
                spinner(self.spinner_frame),
                width,
                colors,
            ),
        }
    }

    fn detail_title(&self) -> String {
        match self.results.get(self.list.cursor) {
            Some(result) => format!("{}/{}", result.group, result.package),
            None => self.in_flight_label().unwrap_or_default(),
        }
    }

    fn detail_for(&self, colors: &ThemeColors) -> Detail {
        match self.results.get(self.list.cursor) {
            Some(result) => result_detail(result, colors),
            None => match self.in_flight_label() {
                Some(label) => running_detail(spinner(self.spinner_frame), label, colors),
                None => Detail::default(),
            },
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
        if (key.code == KeyCode::Enter || key.code == KeyCode::Esc)
            && self.state == ExecutionState::Completed
        {
            return Transition::PopAll;
        }
        let len = self.rows_len();
        if Input::handle_list_navigation(&key, &mut self.list.cursor, len) {
            self.follow = false;
            self.scroll = 0;
            return Transition::Handled;
        }
        let page = detail_inner(self.detail_rect)
            .height
            .saturating_sub(1)
            .max(1);
        match key.code {
            KeyCode::PageUp => self.scroll = self.scroll.saturating_sub(page),
            KeyCode::PageDown => self.scroll = self.scroll.saturating_add(page),
            _ => return Transition::None,
        }
        Transition::Handled
    }

    fn update(&mut self, ctx: &mut AppContext) -> Transition {
        let before = self.list.cursor;
        if let Some(ref mut rx) = self.rx {
            while let Ok(update) = rx.try_recv() {
                match update {
                    // The total (commands + skipped) is fixed at construction.
                    ExecutionUpdate::Progress { current } => {
                        self.current = current;
                        ctx.log(LogLevel::Exec, || {
                            format!("Progress: {}/{}", current, self.total)
                        });
                    }
                    ExecutionUpdate::Result(result) => {
                        ctx.log(LogLevel::Exec, || {
                            format!(
                                "Result: {} {} success={}",
                                result.group, result.package, result.success
                            )
                        });
                        self.results.push(result);
                    }
                    ExecutionUpdate::Complete => {
                        self.rx = None;
                        self.state = ExecutionState::Completed;
                        // The links changed on disk, so every cached stow status is stale.
                        ctx.status_cache.borrow_mut().invalidate_all();
                        ctx.log(LogLevel::Exec, || "Execution complete".into());
                        break;
                    }
                }
            }
        }
        if self.follow && !self.results.is_empty() {
            self.list.cursor = self.results.len() - 1;
        }
        self.spinner_frame = self.spinner_frame.wrapping_add(1);
        if self.list.cursor != before {
            self.scroll = 0;
        }
        let colors = ThemeColors::from_theme(ctx.core.theme);
        self.detail = self.detail_for(&colors);
        self.scroll = fit_scroll(&self.detail, self.detail_rect, &colors, self.scroll);
        Transition::None
    }

    fn hints(&self, _ctx: &AppContext) -> Vec<Hint> {
        execute_hints(self.state == ExecutionState::Completed)
    }

    fn handle_mouse(&mut self, ev: MouseEvent, _ctx: &mut AppContext) -> Transition {
        let len = self.rows_len();
        match click_list(&mut self.list, ev, len) {
            Some(_) => Transition::Handled,
            None => Transition::None,
        }
    }

    fn prepare(&mut self, area: Rect) {
        let rects = screen_rects(area, Shape::ChipReview);
        self.list.prepare(rects.list, self.rows_len());
        self.detail_rect = rects.detail;
    }

    fn render(&mut self, frame: &mut Frame, area: Rect, ctx: &AppContext) {
        let colors = ThemeColors::from_theme(ctx.core.theme);
        let rects = screen_rects(area, Shape::ChipReview);
        if let Some(chip) = rects.chip {
            let line = self.chip_line(&colors, chip.width);
            frame.render_widget(Paragraph::new(line), chip);
        }

        let width = list_panel::inner(rects.list).width;
        let rows = self.rows(&colors, width);
        let empty = Empty {
            message: "nothing to run",
            hint: None,
        };
        render_list_panel(
            frame,
            rects.list,
            &colors,
            "results",
            rows,
            &self.list,
            Some(empty),
        );
        render_detail_scrolled(
            frame,
            rects.detail,
            &colors,
            &self.detail_title(),
            &self.detail,
            self.scroll,
        );
        if let (Some(actions), ExecutionState::Completed) = (rects.actions, self.state) {
            render_action_bar(frame, actions, &colors, &done_buttons());
        }
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
            core: CoreState::from_config(Config::default_empty()),
            config_dir: std::path::PathBuf::from("."),
            config_path: std::path::PathBuf::from("."),
            home_dir: std::path::PathBuf::from("."),
            search_dir: None,
            status_cache: std::cell::RefCell::new(crate::status_cache::StatusCache::new()),
            terminal_size: (80, 24),
            debug: false,
            log_file: None,
            event_log: std::cell::RefCell::new(Vec::new()),
            toast: None,
        }
    }

    fn result(package: &str, success: bool) -> ExecutionResult {
        ExecutionResult {
            group: "home".into(),
            package: package.into(),
            success,
            error_message: (!success).then(|| "boom".to_string()),
        }
    }

    /// Paint never changes the list: `prepare` owns the cursor clamp and the window.
    #[test]
    fn prepare_clamps_the_cursor_and_render_leaves_it_alone() {
        let (_tx, rx) = tokio::sync::mpsc::channel(1);
        let mut comp = ExecuteComponent::new(rx, 0, Vec::new());
        comp.list.cursor = 99;

        let mut terminal = Terminal::new(TestBackend::new(80, 24)).unwrap();
        let ctx = make_ctx();
        comp.prepare(Rect::new(0, 0, 80, 24));
        assert_eq!(comp.list.cursor, 0, "prepare clamps to the rows");

        comp.list.cursor = 3;
        terminal
            .draw(|f| comp.render(f, Rect::new(0, 0, 80, 24), &ctx))
            .unwrap();
        assert_eq!(comp.list.cursor, 3, "render must not move the cursor");
    }

    /// A finished execution changes links on disk, so cached stow statuses must go.
    #[test]
    fn completion_invalidates_the_status_cache() {
        use andre_core::StowStatus;

        let (tx, rx) = tokio::sync::mpsc::channel(4);
        let mut comp = ExecuteComponent::new(rx, 1, Vec::new());
        let mut ctx = make_ctx();
        ctx.status_cache
            .borrow_mut()
            .insert(("home".to_string(), "bash".to_string()), StowStatus::Stowed);

        comp.update(&mut ctx);
        assert!(
            !ctx.status_cache.borrow().is_empty(),
            "no completion yet, so the cache stays"
        );

        tx.try_send(ExecutionUpdate::Complete).unwrap();
        comp.update(&mut ctx);
        assert!(ctx.status_cache.borrow().is_empty());
    }

    #[test]
    fn cursor_follows_the_newest_result_until_the_user_moves_it() {
        let (tx, rx) = tokio::sync::mpsc::channel(8);
        let mut comp = ExecuteComponent::new(rx, 3, Vec::new());
        let mut ctx = make_ctx();

        tx.try_send(ExecutionUpdate::Result(result("a", true)))
            .unwrap();
        tx.try_send(ExecutionUpdate::Result(result("b", true)))
            .unwrap();
        comp.update(&mut ctx);
        assert_eq!(comp.list.cursor, 1, "follows the newest result");

        comp.handle_key(
            KeyEvent::new(KeyCode::Up, crossterm::event::KeyModifiers::NONE),
            &mut ctx,
        );
        tx.try_send(ExecutionUpdate::Result(result("c", true)))
            .unwrap();
        comp.update(&mut ctx);
        assert_eq!(
            comp.list.cursor, 0,
            "a moved cursor stays where the user put it"
        );
    }

    #[test]
    fn the_package_in_flight_is_the_next_planned_one_after_the_skips() {
        let (_tx, rx) = tokio::sync::mpsc::channel(1);
        let skipped = vec![ExecutionResult {
            group: "home".into(),
            package: "gone".into(),
            success: false,
            error_message: Some("Skipped: source directory not found".into()),
        }];
        // The skip is not planned: only the packages the engine runs are.
        let comp = ExecuteComponent::new(rx, 3, skipped).for_run(
            "stow",
            vec![("home".into(), "a".into()), ("home".into(), "b".into())],
        );
        assert_eq!(comp.in_flight_label().as_deref(), Some("home/a"));
    }

    fn key(code: KeyCode) -> KeyEvent {
        KeyEvent::new(code, crossterm::event::KeyModifiers::NONE)
    }

    /// A failure whose message is forty lines: more than a 24-row screen shows.
    fn long_failure() -> ExecutionResult {
        let message = (0..40)
            .map(|i| format!("  * item{i}"))
            .collect::<Vec<_>>()
            .join("\n");
        ExecutionResult {
            group: "home".into(),
            package: "shell".into(),
            success: false,
            error_message: Some(message),
        }
    }

    /// Page keys move the failure's detail a page at a time, and stop at its last row.
    #[test]
    fn page_keys_scroll_a_long_failure_and_stop_at_its_last_row() {
        let (_tx, rx) = tokio::sync::mpsc::channel(1);
        let mut comp = ExecuteComponent::new(rx, 1, vec![long_failure()]);
        comp.state = ExecutionState::Completed;
        let mut ctx = make_ctx();
        let area = Rect::new(0, 0, 80, 24);
        let height = detail_inner(screen_rects(area, Shape::ChipReview).detail).height;
        let page = height - 1;
        // The headline, a blank row, then forty message rows.
        let last = 42 - height;

        comp.prepare(area);
        comp.update(&mut ctx);
        for _ in 0..20 {
            comp.handle_key(key(KeyCode::PageDown), &mut ctx);
            comp.update(&mut ctx);
        }
        assert_eq!(comp.scroll, last, "the last row is at the bottom");

        comp.handle_key(key(KeyCode::PageUp), &mut ctx);
        comp.update(&mut ctx);
        assert_eq!(comp.scroll, last - page, "one page back");
    }

    /// Another row starts its detail at the top, and so does a list move.
    #[test]
    fn moving_to_another_row_starts_its_detail_at_the_top() {
        let (_tx, rx) = tokio::sync::mpsc::channel(1);
        let mut comp = ExecuteComponent::new(rx, 2, vec![long_failure(), result("b", true)]);
        comp.state = ExecutionState::Completed;
        comp.follow = false;
        let mut ctx = make_ctx();
        comp.prepare(Rect::new(0, 0, 80, 24));
        comp.update(&mut ctx);
        comp.handle_key(key(KeyCode::PageDown), &mut ctx);
        comp.update(&mut ctx);
        assert!(comp.scroll > 0, "the failure scrolls");

        comp.handle_key(key(KeyCode::Down), &mut ctx);
        comp.update(&mut ctx);
        assert_eq!(comp.scroll, 0, "the next row starts at the top");
    }
}
