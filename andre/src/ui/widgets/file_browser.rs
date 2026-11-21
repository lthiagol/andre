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
use crate::ui::icons::Icons;
use crate::ui::selection;

pub struct FileBrowserWidget<'a> {
    pub colors: &'a ThemeColors,
    pub icons: &'a Icons,
    pub path: &'a std::path::Path,
    pub cursor: usize,
    pub title: &'a str,
    pub selected_files: Option<&'a [std::path::PathBuf]>,
    pub is_unstow: bool,
    pub entries: &'a [andre_core::TargetEntry],
    pub symlink_targets: &'a [(String, String)],
}

impl<'a> FileBrowserWidget<'a> {
    pub fn new(
        colors: &'a ThemeColors,
        icons: &'a Icons,
        path: &'a std::path::Path,
        cursor: usize,
        title: &'a str,
    ) -> Self {
        Self {
            colors,
            icons,
            path,
            cursor,
            title,
            selected_files: None,
            is_unstow: false,
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
        // Entries come from the caller's cache (`with_entries`); painting never reads the disk.
        let entries = self.entries;

        let legend_height: u16 = if area.height > 10 { 5 } else { 0 };

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

        let content_len = entries.len();
        let page_size = vert[1].height.saturating_sub(2).max(1) as usize;
        let (list_rect, scrollbar_rect) = if content_len > page_size && vert[1].width > 4 {
            let horiz = Layout::default()
                .direction(Direction::Horizontal)
                .constraints([Constraint::Min(1), Constraint::Length(1)])
                .split(vert[1]);
            (horiz[0], Some(horiz[1]))
        } else {
            (vert[1], None)
        };
        // The panel borders take one column on each side.
        let width = list_rect.width.saturating_sub(2);

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

                let mut glyphs: Vec<Span> = Vec::new();
                let mut spans: Vec<Span> = Vec::new();

                if self.is_unstow && entry.is_symlink {
                    let target_str = self
                        .symlink_targets
                        .iter()
                        .find(|(path, _)| *path == entry.name)
                        .map(|(_, target)| target.as_str())
                        .unwrap_or("?");
                    spans.push(Span::styled(
                        format!("{}  {}  →  {}", kind, entry.name, target_str),
                        Style::default().fg(entry_fg),
                    ));
                } else if let Some(selected) = self.selected_files {
                    let is_selected = selected.contains(&self.path.join(&entry.name));
                    if entry.is_selectable() {
                        let mark_fg = if is_selected {
                            self.colors.success
                        } else {
                            self.colors.muted
                        };
                        glyphs.push(Span::styled(
                            format!("{} ", self.icons.checkbox(is_selected)),
                            Style::default().fg(mark_fg),
                        ));
                    } else {
                        glyphs.push(Span::raw("  "));
                    }
                    let fg = if is_selected {
                        self.colors.success
                    } else {
                        entry_fg
                    };
                    spans.push(Span::styled(
                        format!("{} {}", kind, entry.name),
                        Style::default().fg(fg),
                    ));
                } else {
                    spans.push(Span::styled(
                        format!("{} {}", kind, entry.name),
                        Style::default().fg(entry_fg),
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

        let list_title = format!(
            " {} ({}) ",
            self.title,
            if entries.is_empty() {
                "empty".to_string()
            } else {
                format!("{} items", entries.len())
            }
        );

        let list =
            List::new(items).block(crate::ui::themed_panel(self.colors).title(list_title.as_str()));

        StatefulWidget::render(list, list_rect, buf, state);

        if let Some(scrollbar_rect) = scrollbar_rect {
            let scrollbar = Scrollbar::new(ScrollbarOrientation::VerticalRight)
                .begin_symbol(Some("↑"))
                .end_symbol(Some("↓"))
                .track_symbol(Some("│"))
                .thumb_style(Style::default().fg(self.colors.indicator))
                .track_style(Style::default().fg(self.colors.muted));
            let mut scrollbar_state = ScrollbarState::new(content_len)
                .position(self.cursor)
                .viewport_content_length(page_size);
            StatefulWidget::render(scrollbar, scrollbar_rect, buf, &mut scrollbar_state);
        }

        if legend_height > 0 {
            render_file_legend(buf, vert[2], self.colors);
        }
    }
}

fn render_file_legend(buf: &mut ratatui::buffer::Buffer, area: Rect, colors: &ThemeColors) {
    let items: Vec<ListItem> = vec![
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

    let legend = List::new(items).block(crate::ui::themed_panel(colors).title(" legend "));
    Widget::render(legend, area, buf);
}
