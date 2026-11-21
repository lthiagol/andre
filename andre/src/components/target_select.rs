use std::path::PathBuf;

use crossterm::event::{KeyCode, KeyEvent};
use ratatui::{layout::Rect, widgets::ListState, Frame};

use crate::components::adopt::AdoptFileSelectComponent;
use crate::components::unstow::UnstowBrowseComponent;
use crate::components::{AppContext, Component, Transition};
use crate::input::Input;
use crate::theme::ThemeColors;
use crate::ui::widgets::group_list::GroupListWidget;
use andre_core::path::resolve_path;

/// What differs between the adopt and unstow target pickers.
struct TargetFlow {
    id: &'static str,
    title: &'static str,
    target_label: Option<&'static str>,
    /// Opens the browse screen for the chosen group at its resolved target.
    open: fn(String, PathBuf) -> Box<dyn Component>,
}

static ADOPT: TargetFlow = TargetFlow {
    id: "AdoptTargetSelect",
    title: "Select Target for Adoption",
    target_label: Some("Adopt from"),
    open: |group, path| Box::new(AdoptFileSelectComponent::with_path(group, path)),
};

static UNSTOW: TargetFlow = TargetFlow {
    id: "UnstowTargetSelect",
    title: "Select Target for Unstow",
    target_label: None,
    open: |group, path| Box::new(UnstowBrowseComponent::with_path(group, path)),
};

/// Picks a group, then opens its target directory in the flow's browse screen.
pub struct TargetSelectComponent {
    pub cursor: usize,
    flow: &'static TargetFlow,
}

impl TargetSelectComponent {
    pub fn adopt() -> Self {
        Self {
            cursor: 0,
            flow: &ADOPT,
        }
    }

    pub fn unstow() -> Self {
        Self {
            cursor: 0,
            flow: &UNSTOW,
        }
    }
}

impl Component for TargetSelectComponent {
    fn id(&self) -> &'static str {
        self.flow.id
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

        if len > 0 && Input::handle_list_navigation(&key, &mut self.cursor, len) {
            return Transition::None;
        }

        match key.code {
            KeyCode::Enter => {
                let group_name = groups.get(self.cursor).cloned().unwrap_or_default();
                let browse_path = ctx
                    .core
                    .config
                    .groups
                    .get(&group_name)
                    .map(|g| resolve_path(&g.target, &ctx.config_dir, &ctx.home_dir))
                    .unwrap_or_else(|| PathBuf::from("."));
                Transition::Push((self.flow.open)(group_name, browse_path))
            }
            KeyCode::Esc => Transition::Pop,
            _ => Transition::None,
        }
    }

    fn render(&mut self, frame: &mut Frame, area: Rect, ctx: &AppContext) {
        let colors = ThemeColors::from_theme(ctx.core.theme);
        let mut state = ListState::default();
        let mut widget = GroupListWidget::new(
            &ctx.core.config,
            &colors,
            self.flow.title,
            self.cursor,
            &ctx.config_dir,
            &ctx.home_dir,
        );
        if let Some(label) = self.flow.target_label {
            widget = widget.with_target_label(label);
        }
        frame.render_stateful_widget(widget, area, &mut state);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_flows_report_their_ids() {
        assert_eq!(TargetSelectComponent::adopt().id(), "AdoptTargetSelect");
        assert_eq!(TargetSelectComponent::unstow().id(), "UnstowTargetSelect");
    }
}
