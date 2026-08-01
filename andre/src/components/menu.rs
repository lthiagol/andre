use crossterm::event::{KeyCode, KeyEvent};
use ratatui::{
    layout::Rect,
    style::Style,
    widgets::{List, ListItem, ListState},
    Frame,
};

use crate::components::adopt::AdoptTargetSelectComponent;
use crate::components::help::HelpComponent;
use crate::components::settings::SettingsComponent;
use crate::components::status::StatusComponent;
use crate::components::stow::GroupSelectComponent;
use crate::components::unstow::UnstowTargetSelectComponent;
use crate::components::{AppContext, Component, Transition};
use crate::input;
use crate::theme::ThemeColors;

#[derive(Default)]
pub struct MainMenuComponent {
    pub index: usize,
}

impl Component for MainMenuComponent {
    fn id(&self) -> &'static str {
        "MainMenu"
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }

    fn handle_key(&mut self, key: KeyEvent, _ctx: &mut AppContext) -> Transition {
        let items_len = 7;

        if input::Input::is_up(&key) {
            self.index = (self.index + items_len - 1) % items_len;
        } else if input::Input::is_down(&key) {
            self.index = (self.index + 1) % items_len;
        } else if input::Input::is_quit_key(&key) {
            // q and Ctrl+C: checked before the Char arm so the digit
            // dispatcher never swallows Ctrl+C / Ctrl+1 / etc.
            return Transition::Quit;
        } else if key.code == KeyCode::Enter {
            return self.activate(self.index);
        } else if let KeyCode::Char(c) = key.code {
            // Unmodified digit keys 1..=7 jump to and activate the Nth menu
            // item, matching the labels rendered in `render`. '0' and '8'..='9'
            // are out of range and ignored so they cannot crash input.
            if key.modifiers.is_empty() {
                if let Some(d) = c.to_digit(10) {
                    if (1..=items_len as u32).contains(&d) {
                        let idx = (d - 1) as usize;
                        self.index = idx;
                        return self.activate(idx);
                    }
                }
            }
        }

        Transition::None
    }

    fn render(&mut self, frame: &mut Frame, area: Rect, ctx: &AppContext) {
        let colors = ThemeColors::from_theme(ctx.core.theme);
        let action = ctx.core.effective_action().to_uppercase();

        let items = vec![
            ListItem::new(format!("1. {} selected packages", action)),
            ListItem::new("2. Guided adoption (import existing files)"),
            ListItem::new("3. Interactive unstow (remove symlinks)"),
            ListItem::new("4. Verify status (check all symlinks)"),
            ListItem::new("5. Config & Settings"),
            ListItem::new("6. Help"),
            ListItem::new("7. Quit"),
        ];

        let list = List::new(items)
            .highlight_symbol(">> ")
            .highlight_style(Style::default().fg(colors.highlight))
            .style(Style::default().fg(colors.primary))
            .block(crate::ui::themed_panel(&colors).title(" Main Menu "));

        let mut state = ListState::default();
        state.select(Some(self.index));

        frame.render_stateful_widget(list, area, &mut state);
    }
}

impl MainMenuComponent {
    /// Activate the Nth menu item. Extracted so digit-key dispatch and
    /// Enter share the same transition table.
    fn activate(&self, index: usize) -> Transition {
        match index {
            0 => Transition::Push(Box::new(GroupSelectComponent::default())),
            1 => Transition::Push(Box::new(AdoptTargetSelectComponent::default())),
            2 => Transition::Push(Box::new(UnstowTargetSelectComponent::default())),
            3 => Transition::Push(Box::new(StatusComponent::default())),
            4 => Transition::Push(Box::new(SettingsComponent::default())),
            5 => Transition::Push(Box::new(HelpComponent::default())),
            6 => Transition::Quit,
            _ => Transition::None,
        }
    }
}
