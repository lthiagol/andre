use crossterm::event::{KeyCode, KeyEvent};
use ratatui::{layout::Rect, Frame};

use crate::components::{AppContext, Component, LogLevel, Transition};
use crate::theme::ThemeColors;

pub struct AdoptPreviewComponent {
    pub group_name: String,
    pub package_name: String,
    pub selected_files: Vec<std::path::PathBuf>,
    cached_plan: Option<andre_core::AdoptionPlan>,
    pub execution_result: Option<String>,
}

impl AdoptPreviewComponent {
    pub fn new(
        group_name: String,
        package_name: String,
        selected_files: Vec<std::path::PathBuf>,
    ) -> Self {
        Self {
            group_name,
            package_name,
            selected_files,
            cached_plan: None,
            execution_result: None,
        }
    }
}

impl Component for AdoptPreviewComponent {
    fn id(&self) -> &'static str {
        "AdoptPreview"
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }

    fn update(&mut self, ctx: &mut AppContext) -> Transition {
        if self.cached_plan.is_none() {
            self.cached_plan = ctx.core.config.groups.get(&self.group_name).and_then(|g| {
                let target =
                    andre_core::path::resolve_path(&g.target, &ctx.config_dir, &ctx.home_dir);
                let source =
                    andre_core::path::resolve_path(&g.source, &ctx.config_dir, &ctx.home_dir);
                andre_core::plan_adoption(
                    &target,
                    &source,
                    &self.package_name,
                    &self.selected_files,
                )
                .ok()
            });
            if self.cached_plan.is_some() {
                ctx.log(
                    LogLevel::Exec,
                    format!(
                        "Adoption plan ready for {} / {}",
                        self.group_name, self.package_name
                    ),
                );
            }
        }
        Transition::None
    }

    fn handle_key(&mut self, key: KeyEvent, ctx: &mut AppContext) -> Transition {
        if key.code == KeyCode::Char('y') || key.code == KeyCode::Char('Y') {
            if let Some(ref plan) = self.cached_plan {
                let result = andre_core::execute_adoption(
                    plan,
                    ctx.core.verbosity,
                    ctx.core.dry_run,
                    ctx.core.no_folding,
                    ctx.core.adopt,
                    ctx.core.dotfiles,
                    &ctx.core.config.effective_ignores(&self.group_name),
                );
                self.execution_result = Some(match &result {
                    Ok(()) => "Adoption completed successfully.".to_string(),
                    Err(e) => format!("Adoption failed: {}", e),
                });
            } else {
                self.execution_result = Some("No plan available.".to_string());
            }
            Transition::Handled
        } else if key.code == KeyCode::Char('n')
            || key.code == KeyCode::Char('N')
            || key.code == KeyCode::Esc
        {
            if self.execution_result.is_some() {
                Transition::PopAll
            } else {
                Transition::Pop
            }
        } else {
            Transition::Handled
        }
    }

    fn render(&mut self, frame: &mut Frame, area: Rect, ctx: &AppContext) {
        let colors = ThemeColors::from_theme(ctx.core.theme);
        crate::ui::render_adopt_preview(
            frame,
            area,
            &colors,
            self.cached_plan.as_ref(),
            self.execution_result.as_deref(),
        );
    }
}
