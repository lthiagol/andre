use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::Style,
    text::{Line, Span},
    widgets::{
        List, ListItem, ListState, Paragraph, Scrollbar, ScrollbarOrientation, ScrollbarState,
        StatefulWidget, Widget,
    },
};

use crate::theme::ThemeColors;

pub struct FileBrowserWidget<'a> {
    pub colors: &'a ThemeColors,
    pub path: &'a std::path::Path,
    pub cursor: usize,
    pub title: &'a str,
    pub selected_files: Option<&'a [std::path::PathBuf]>,
    pub is_unstow: bool,
    pub dirs_only: bool,
    pub entries: &'a [andre_core::TargetEntry],
    pub symlink_targets: &'a [(String, String)],
}

impl<'a> FileBrowserWidget<'a> {
    pub fn new(
        colors: &'a ThemeColors,
        path: &'a std::path::Path,
        cursor: usize,
        title: &'a str,
    ) -> Self {
        Self {
            colors,
            path,
            cursor,
            title,
            selected_files: None,
            is_unstow: false,
            dirs_only: false,
            entries: &[],
            symlink_targets: &[],
        }
    }

    pub fn with_selected(mut self, selected: &'a [std::path::PathBuf]) -> Self {
        self.selected_files = Some(selected);
        self
    }

    pub fn unstow_mode(mut self) -> Self {
        self.is_unstow = true;
        self
    }

    pub fn dirs_only(mut self) -> Self {
        self.dirs_only = true;
        self
    }

    pub fn with_entries(
        mut self,
        entries: &'a [andre_core::TargetEntry],
        symlink_targets: &'a [(String, String)],
    ) -> Self {
        self.entries = entries;
        self.symlink_targets = symlink_targets;
        self
    }
}

impl<'a> StatefulWidget for FileBrowserWidget<'a> {
    type State = ListState;

    fn render(self, area: Rect, buf: &mut ratatui::buffer::Buffer, state: &mut Self::State) {
        let entries: &[andre_core::TargetEntry] = if self.dirs_only && self.entries.is_empty() {
            // Legacy fallback for callers that haven't been migrated to cache entries.
            // New code should always pass entries via with_entries().
            &andre_core::adopt::list_target_dirs(self.path).unwrap_or_default()
        } else {
            self.entries
        };

        let legend_height: u16 = if area.height > 10 { 6 } else { 0 };

        let vert = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(1),
                Constraint::Min(0),
                Constraint::Length(legend_height),
            ])
            .split(area);

        let breadcrumb = Paragraph::new(format!("Path: {}", self.path.display()))
            .style(Style::default().fg(self.colors.muted));
        Widget::render(breadcrumb, vert[0], buf);

        state.select(Some(self.cursor.min(entries.len().saturating_sub(1))));

        let items: Vec<ListItem> = entries
            .iter()
            .enumerate()
            .map(|(i, entry)| {
                let is_cursor = i == self.cursor;
                let kind = entry.kind_icon();

                let entry_fg = if entry.is_dir {
                    self.colors.highlight
                } else if entry.is_symlink {
                    self.colors.success
                } else if entry.is_artifact {
                    self.colors.muted
                } else {
                    self.colors.primary
                };

                let mut parts: Vec<Span> = Vec::new();

                let marker = if is_cursor {
                    Span::styled("--> ", Style::default().fg(self.colors.highlight))
                } else {
                    Span::raw("    ")
                };
                parts.push(marker);

                if self.is_unstow && entry.is_symlink {
                    let target_str = self
                        .symlink_targets
                        .iter()
                        .find(|(path, _)| *path == entry.name)
                        .map(|(_, target)| target.as_str())
                        .unwrap_or("?");
                    parts.push(Span::styled(
                        format!("{}  {}  →  {}", kind, entry.name, target_str),
                        Style::default().fg(entry_fg),
                    ));
                } else if let Some(selected) = self.selected_files {
                    let is_selected = selected.contains(&self.path.join(&entry.name));
                    let show_check = entry.is_selectable();
                    let mark = if show_check && is_selected {
                        "[x] "
                    } else if show_check {
                        "[ ] "
                    } else {
                        "    "
                    };
                    let fg = if is_selected {
                        self.colors.success
                    } else {
                        entry_fg
                    };
                    parts.push(Span::styled(
                        format!("{}{} {}", mark, kind, entry.name),
                        Style::default().fg(fg),
                    ));
                } else {
                    parts.push(Span::styled(
                        format!("{} {}", kind, entry.name),
                        Style::default().fg(entry_fg),
                    ));
                }

                ListItem::new(Line::from(parts))
            })
            .collect();

        let list_title = format!(
            " {} ({}) ",
            self.title,
            if entries.is_empty() {
                "empty".to_string()
            } else {
                format!("{} items", entries.len())
            }
        );

        let list = List::new(items)
            .highlight_symbol(">> ")
            .highlight_style(Style::default().fg(self.colors.highlight))
            .block(crate::ui::themed_panel(self.colors).title(list_title.as_str()));

        let content_len = entries.len();
        let page_size = vert[1].height.saturating_sub(2).max(1) as usize;

        if content_len > page_size && vert[1].width > 4 {
            let horiz = Layout::default()
                .direction(Direction::Horizontal)
                .constraints([Constraint::Min(1), Constraint::Length(1)])
                .split(vert[1]);
            StatefulWidget::render(list, horiz[0], buf, state);

            let scrollbar = Scrollbar::new(ScrollbarOrientation::VerticalRight)
                .begin_symbol(Some("↑"))
                .end_symbol(Some("↓"))
                .track_symbol(Some("│"))
                .thumb_style(Style::default().fg(self.colors.indicator))
                .track_style(Style::default().fg(self.colors.muted));
            let mut scrollbar_state = ScrollbarState::new(content_len)
                .position(self.cursor)
                .viewport_content_length(page_size);
            StatefulWidget::render(scrollbar, horiz[1], buf, &mut scrollbar_state);
        } else {
            StatefulWidget::render(list, vert[1], buf, state);
        }

        if legend_height > 0 {
            render_file_legend(buf, vert[2], self.colors);
        }
    }
}

fn render_file_legend(buf: &mut ratatui::buffer::Buffer, area: Rect, colors: &ThemeColors) {
    let items: Vec<ListItem> = vec![
        ListItem::new(Line::from("Legend:")).style(Style::default().fg(colors.primary)),
        ListItem::new(Line::from(vec![
            Span::styled("  /  ", Style::default().fg(colors.secondary)),
            Span::styled("Directory", Style::default().fg(colors.muted)),
        ])),
        ListItem::new(Line::from(vec![
            Span::styled("  ~  ", Style::default().fg(colors.success)),
            Span::styled("Symlink", Style::default().fg(colors.muted)),
        ])),
        ListItem::new(Line::from(vec![
            Span::styled("  \u{2022}  ", Style::default().fg(colors.secondary)),
            Span::styled("File", Style::default().fg(colors.muted)),
        ])),
    ];

    let legend = List::new(items).block(crate::ui::themed_panel(colors));
    Widget::render(legend, area, buf);
}
