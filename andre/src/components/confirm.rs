use std::collections::HashMap;

use crossterm::event::KeyEvent;
use ratatui::{layout::Rect, Frame};

use crate::components::execute::ExecuteComponent;
use crate::components::{AppContext, Component, Transition};
use crate::execute::{build_execution_results, spawn_execution};
use crate::input::Input;
use crate::theme::ThemeColors;
use crate::ui::{elide, ConfirmRow};
use andre_core::{build_stow_args, engine_for, EngineKind, StowAction, StowFlags};

pub struct ConfirmComponent {
    pub selected_packages: HashMap<String, Vec<String>>,
    /// The action this review runs. `None` follows the global action.
    action: Option<StowAction>,
    /// Review list, built by `new` and rebuilt by `update` when the global flags
    /// change, so that paint only draws it.
    rows: Vec<ConfirmRow>,
    /// The flags `rows` was built from.
    rows_flags: StowFlags,
    scroll_index: usize,
    viewport_height: usize,
    content_len: usize,
}

/// The review list: the action, then each group with the command each of its
/// packages would run. Resolves group paths on disk, so it runs when the screen
/// is built and again when a global flag changes, never on paint.
fn build_rows(
    selected_packages: &HashMap<String, Vec<String>>,
    ctx: &AppContext,
    flags: &StowFlags,
) -> Vec<ConfirmRow> {
    let config = &ctx.core.config;
    let mut rows = vec![
        ConfirmRow::Action(format!("Action: {}", flags.action.as_str().to_uppercase())),
        ConfirmRow::Blank,
    ];

    for (group_name, packages) in selected_packages {
        rows.push(ConfirmRow::Group(format!("Group: {}", group_name)));

        let source = config.get_group_source(group_name, &ctx.config_dir, &ctx.home_dir);
        let target = config.get_group_target(group_name, &ctx.config_dir, &ctx.home_dir);

        for pkg in packages {
            // Engine-aware preview: native (default) does not shell out, so do not
            // show a `stow ...` command that would mislead users. Only the stow
            // engine renders its actual command line.
            let cmd_str = if config.effective_engine() == EngineKind::Native {
                match (&source, &target) {
                    (Some(_), Some(_)) => format!("native: {}  (relative symlink)", pkg),
                    _ => format!("native: {} (path unresolved)", pkg),
                }
            } else if let (Some(src), Some(tgt)) = (&source, &target) {
                let ignores = config.effective_ignores(group_name);
                let cmd = build_stow_args(src, tgt, pkg, flags, &ignores);
                let args: Vec<String> = cmd
                    .get_args()
                    .map(|a| a.to_string_lossy().to_string())
                    .collect();
                let prog = cmd.get_program().to_string_lossy();
                format!("{} {}", prog, args.join(" "))
            } else {
                format!("stow {} (path unresolved)", pkg)
            };

            rows.push(ConfirmRow::Command(format!("  {}", elide(&cmd_str, 80))));
        }

        rows.push(ConfirmRow::Blank);
    }
    rows
}

/// The global flags, with the action replaced when the review has its own.
fn flags_for(ctx: &AppContext, action: Option<StowAction>) -> StowFlags {
    StowFlags {
        action: action.unwrap_or(ctx.core.flags.action),
        ..ctx.core.flags
    }
}

impl ConfirmComponent {
    pub fn new(selected_packages: HashMap<String, Vec<String>>, ctx: &AppContext) -> Self {
        Self::build(selected_packages, ctx, None)
    }

    /// A review that always stows, whatever the global action is: the bootstrap item.
    pub fn stow_only(selected_packages: HashMap<String, Vec<String>>, ctx: &AppContext) -> Self {
        Self::build(selected_packages, ctx, Some(StowAction::Stow))
    }

    fn build(
        selected_packages: HashMap<String, Vec<String>>,
        ctx: &AppContext,
        action: Option<StowAction>,
    ) -> Self {
        let content_len: usize = selected_packages.values().map(Vec::len).sum();
        let flags = flags_for(ctx, action);
        let rows = build_rows(&selected_packages, ctx, &flags);
        Self {
            selected_packages,
            action,
            rows,
            rows_flags: flags,
            scroll_index: 0,
            viewport_height: 10,
            content_len,
        }
    }

    fn flags(&self, ctx: &AppContext) -> StowFlags {
        flags_for(ctx, self.action)
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
            let (ops, skipped) = build_execution_results(
                &self.selected_packages,
                &ctx.core.config,
                &ctx.config_dir,
                &ctx.home_dir,
                &self.flags(ctx),
            );
            // Select the link engine from config (native default, stow opt-in).
            let engine = engine_for(ctx.core.config.effective_engine());
            let total = ops.len() + skipped.len();
            let rx = spawn_execution(ops, engine);
            return Transition::Replace(Box::new(ExecuteComponent::new(rx, total, skipped)));
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

    fn update(&mut self, ctx: &mut AppContext) -> Transition {
        // The global flag keys stay live on this screen, so rebuild the preview
        // before the next paint when they change.
        let flags = self.flags(ctx);
        if flags != self.rows_flags {
            self.rows = build_rows(&self.selected_packages, ctx, &flags);
            self.rows_flags = flags;
        }
        Transition::None
    }

    fn render(&mut self, frame: &mut Frame, area: Rect, ctx: &AppContext) {
        let colors = ThemeColors::from_theme(ctx.core.theme);
        let title = " review commands ";
        crate::ui::render_confirm(
            frame,
            area,
            &colors,
            title,
            &self.rows,
            self.scroll_index,
            self.viewport_height,
        );
    }

    fn prepare(&mut self, area: Rect) {
        // borders (2). Owned here so paint is a pure draw.
        self.viewport_height = area.height.saturating_sub(2).max(1) as usize;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use andre_core::config::Config;

    /// A group `dots` whose source and target both exist, with the package `alpha`.
    fn fixture() -> (tempfile::TempDir, AppContext) {
        let tmp = tempfile::TempDir::new().unwrap();
        std::fs::create_dir_all(tmp.path().join("src/alpha")).unwrap();
        std::fs::create_dir_all(tmp.path().join("tgt")).unwrap();
        let mut config = Config::default_empty();
        config.add_group(
            "dots".into(),
            tmp.path().join("src").display().to_string(),
            tmp.path().join("tgt").display().to_string(),
        );
        let ctx = AppContext::for_tests(config, tmp.path());
        (tmp, ctx)
    }

    #[test]
    fn test_new_builds_rows_for_selected_packages() {
        let (_tmp, ctx) = fixture();
        let selected = HashMap::from([("dots".to_string(), vec!["alpha".to_string()])]);
        let component = ConfirmComponent::new(selected, &ctx);

        assert!(component
            .rows
            .contains(&ConfirmRow::Group("Group: dots".into())));
        assert!(component.rows.contains(&ConfirmRow::Command(
            "  native: alpha  (relative symlink)".into()
        )));
    }

    #[test]
    fn test_new_marks_unresolved_group_paths() {
        let (_tmp, mut ctx) = fixture();
        ctx.core.config.remove_group("dots");
        let selected = HashMap::from([("dots".to_string(), vec!["alpha".to_string()])]);
        let component = ConfirmComponent::new(selected, &ctx);

        assert!(component.rows.contains(&ConfirmRow::Command(
            "  native: alpha (path unresolved)".into()
        )));
    }
}
