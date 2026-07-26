use crossterm::event::{KeyCode, KeyEvent};
use ratatui::{layout::Rect, Frame};

use crate::components::{AppContext, Component, InputMode, Transition};
use crate::theme::ThemeColors;

pub struct AdoptNamePromptComponent {
    pub group_name: String,
    pub selected_files: Vec<std::path::PathBuf>,
    pub name: String,
    cached_existing: Vec<String>,
}

impl AdoptNamePromptComponent {
    pub fn new(group_name: String, selected_files: Vec<std::path::PathBuf>) -> Self {
        Self {
            group_name,
            selected_files,
            name: String::new(),
            cached_existing: Vec::new(),
        }
    }
}

impl Component for AdoptNamePromptComponent {
    fn id(&self) -> &'static str {
        "AdoptNamePrompt"
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }

    fn input_mode(&self) -> InputMode {
        InputMode::TextInput
    }

    fn update(&mut self, ctx: &mut AppContext) -> Transition {
        if self.cached_existing.is_empty() {
            self.cached_existing = ctx
                .core
                .config
                .groups
                .values()
                .flat_map(|g| {
                    let resolved =
                        andre_core::path::resolve_path(&g.source, &ctx.config_dir, &ctx.home_dir);
                    andre_core::discover_packages(&resolved)
                })
                .collect();
        }
        Transition::None
    }

    fn handle_key(&mut self, key: KeyEvent, _ctx: &mut AppContext) -> Transition {
        match key.code {
            KeyCode::Enter if !self.name.is_empty() => {
                return Transition::Push(Box::new(super::AdoptPreviewComponent::new(
                    self.group_name.clone(),
                    self.name.clone(),
                    self.selected_files.clone(),
                )));
            }
            KeyCode::Char(c) => {
                self.name.push(c);
            }
            KeyCode::Backspace => {
                self.name.pop();
            }
            KeyCode::Esc => return Transition::Pop,
            _ => {}
        }
        Transition::Handled
    }

    fn render(&mut self, frame: &mut Frame, area: Rect, ctx: &AppContext) {
        let colors = ThemeColors::from_theme(ctx.core.theme);
        crate::ui::render_adopt_name(frame, area, &colors, &self.name, &self.cached_existing);
    }
}
