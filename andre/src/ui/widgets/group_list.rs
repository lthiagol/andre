use std::path::Path;

use ratatui::{
    layout::Rect,
    style::Style,
    text::{Line, Span},
    widgets::{List, ListItem, ListState, StatefulWidget},
};

use crate::theme::ThemeColors;
use andre_core::config::Config;
use andre_core::path::resolve_path;

pub struct GroupListWidget<'a> {
    pub config: &'a Config,
    pub colors: &'a ThemeColors,
    pub title: &'a str,
    pub cursor: usize,
    pub selected_groups: Option<&'a [String]>,
    pub target_label: &'a str,
    pub config_dir: &'a Path,
    pub home_dir: &'a Path,
}

impl<'a> GroupListWidget<'a> {
    pub fn new(
        config: &'a Config,
        colors: &'a ThemeColors,
        title: &'a str,
        cursor: usize,
        config_dir: &'a Path,
        home_dir: &'a Path,
    ) -> Self {
        Self {
            config,
            colors,
            title,
            cursor,
            selected_groups: None,
            target_label: "target",
            config_dir,
            home_dir,
        }
    }

    pub fn with_selected(mut self, selected: &'a [String]) -> Self {
        self.selected_groups = Some(selected);
        self
    }

    pub fn with_target_label(mut self, label: &'a str) -> Self {
        self.target_label = label;
        self
    }
}

impl<'a> StatefulWidget for GroupListWidget<'a> {
    type State = ListState;

    fn render(self, area: Rect, buf: &mut ratatui::buffer::Buffer, state: &mut Self::State) {
        let mut groups: Vec<_> = self.config.groups.keys().collect();
        groups.sort();

        state.select(Some(self.cursor.min(groups.len().saturating_sub(1))));

        let items: Vec<ListItem> = groups
            .iter()
            .enumerate()
            .map(|(i, group_name)| {
                let is_cursor = i == self.cursor;
                let mut spans = Vec::new();

                let marker = if is_cursor {
                    Span::styled("--> ", Style::default().fg(self.colors.highlight))
                } else {
                    Span::raw("    ")
                };
                spans.push(marker);

                if let Some(selected) = self.selected_groups {
                    let checkbox = if selected.contains(*group_name) {
                        "[x] "
                    } else {
                        "[ ] "
                    };
                    spans.push(Span::raw(checkbox));
                }

                spans.push(Span::raw(group_name.to_string()));

                if let Some(group) = self.config.groups.get(*group_name) {
                    let resolved = resolve_path(&group.target, self.config_dir, self.home_dir);
                    spans.push(Span::styled(
                        format!("  ({}: {})", self.target_label, resolved.display()),
                        Style::default().fg(self.colors.muted),
                    ));
                }

                ListItem::new(Line::from(spans)).style(Style::default().fg(self.colors.primary))
            })
            .collect();

        let list = List::new(items)
            .highlight_symbol(">> ")
            .highlight_style(Style::default().fg(self.colors.highlight))
            .block(crate::ui::themed_panel(self.colors).title(format!(" {} ", self.title)));

        StatefulWidget::render(list, area, buf, state);
    }
}
