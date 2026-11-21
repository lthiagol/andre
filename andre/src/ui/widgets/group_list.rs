use std::path::Path;

use ratatui::{
    layout::Rect,
    style::{Modifier, Style},
    text::Span,
    widgets::{List, ListItem, ListState, StatefulWidget},
};

use crate::theme::ThemeColors;
use crate::ui::icons::Icons;
use crate::ui::selection;
use andre_core::config::Config;
use andre_core::path::resolve_path;

pub struct GroupListWidget<'a> {
    pub config: &'a Config,
    pub colors: &'a ThemeColors,
    pub icons: &'a Icons,
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
        icons: &'a Icons,
        title: &'a str,
        cursor: usize,
        config_dir: &'a Path,
        home_dir: &'a Path,
    ) -> Self {
        Self {
            config,
            colors,
            icons,
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
        let groups: Vec<_> = self.config.groups.keys().collect();

        state.select(Some(self.cursor.min(groups.len().saturating_sub(1))));

        // The panel borders take one column on each side.
        let width = area.width.saturating_sub(2);

        let items: Vec<ListItem> = groups
            .iter()
            .enumerate()
            .map(|(i, group_name)| {
                let is_cursor = i == self.cursor;
                let mut glyphs = Vec::new();
                let mut spans = Vec::new();

                if let Some(selected) = self.selected_groups {
                    let checked = selected.contains(*group_name);
                    let fg = if checked {
                        self.colors.ok
                    } else {
                        self.colors.text_dim
                    };
                    glyphs.push(Span::styled(
                        format!("{} ", self.icons.checkbox(checked)),
                        Style::default().fg(fg),
                    ));
                }

                let name_style = if is_cursor {
                    Style::default()
                        .fg(self.colors.text)
                        .add_modifier(Modifier::BOLD)
                } else {
                    Style::default().fg(self.colors.text)
                };
                spans.push(Span::styled(group_name.to_string(), name_style));

                if let Some(group) = self.config.groups.get(*group_name) {
                    let resolved = resolve_path(&group.target, self.config_dir, self.home_dir);
                    spans.push(Span::styled(
                        format!("  ({}: {})", self.target_label, resolved.display()),
                        Style::default().fg(self.colors.text_dim),
                    ));
                }

                ListItem::new(selection::row_with_glyphs(
                    glyphs,
                    spans,
                    is_cursor,
                    self.colors,
                    width,
                ))
            })
            .collect();

        let list = List::new(items)
            .block(crate::ui::themed_panel(self.colors).title(format!(" {} ", self.title)));

        StatefulWidget::render(list, area, buf, state);
    }
}
