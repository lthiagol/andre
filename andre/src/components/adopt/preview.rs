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
                ctx.log(LogLevel::Exec, || {
                    format!(
                        "Adoption plan ready for {} / {}",
                        self.group_name, self.package_name
                    )
                });
            }
        }
        Transition::None
    }

    fn handle_key(&mut self, key: KeyEvent, ctx: &mut AppContext) -> Transition {
        if key.code == KeyCode::Char('y') || key.code == KeyCode::Char('Y') {
            if let Some(ref plan) = self.cached_plan {
                let result = andre_core::execute_adoption(
                    plan,
                    &ctx.core.flags,
                    &ctx.core.config.effective_ignores(&self.group_name),
                    Some(ctx.home_dir.as_path()),
                );
                // Adoption can add a package and move links, so cached statuses are stale.
                ctx.status_cache.borrow_mut().invalidate_all();
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
        // The title differs between the preview and the result. The compact banner
        // already shows the breadcrumb, so the panel does not repeat it.
        let title = if self.execution_result.is_some() {
            " adoption result "
        } else {
            " adoption plan preview "
        };
        crate::ui::render_adopt_preview(
            frame,
            area,
            &colors,
            title,
            self.cached_plan.as_ref(),
            self.execution_result.as_deref(),
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use andre_core::config::Config;
    use andre_core::StowStatus;
    use crossterm::event::KeyModifiers;

    /// Adopting a file moves it into the package and leaves a link, so cached stow statuses must go.
    #[test]
    fn successful_adoption_invalidates_the_status_cache() {
        let tmp = tempfile::TempDir::new().unwrap();
        let config_path = tmp.path().join("andre.yml");
        std::fs::write(
            &config_path,
            "global: {}\ngroups:\n  home:\n    source: dotfiles/home\n    target: \"~\"\n",
        )
        .unwrap();
        std::fs::create_dir_all(tmp.path().join("dotfiles/home")).unwrap();
        std::fs::write(tmp.path().join(".bashrc"), "alias l='ls -la'").unwrap();

        let config = Config::load(&config_path).unwrap();
        let mut ctx = AppContext::for_tests(config, tmp.path());
        ctx.status_cache
            .borrow_mut()
            .insert(("home".to_string(), "bash".to_string()), StowStatus::Stowed);

        let mut preview = AdoptPreviewComponent::new(
            "home".to_string(),
            "bash".to_string(),
            vec![tmp.path().join(".bashrc")],
        );
        preview.update(&mut ctx);
        let yes = KeyEvent::new(KeyCode::Char('y'), KeyModifiers::NONE);
        preview.handle_key(yes, &mut ctx);

        assert_eq!(
            preview.execution_result.as_deref(),
            Some("Adoption completed successfully.")
        );
        assert!(ctx.status_cache.borrow().is_empty());
    }
}
