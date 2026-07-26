use std::path::Path;

use crate::theme::ThemeColors;
use crate::ui::widgets::file_browser::FileBrowserWidget;
use ratatui::{layout::Rect, widgets::ListState, Frame};

pub fn render_adopt_browse(
    frame: &mut Frame,
    area: Rect,
    colors: &ThemeColors,
    browse_path: &Path,
    browse_cursor: usize,
    selected_files: &[std::path::PathBuf],
) {
    let mut state = ListState::default();
    let widget =
        FileBrowserWidget::new(colors, browse_path, browse_cursor, "Select Files to Adopt")
            .with_selected(selected_files)
            .with_entries(&[], &[]);
    frame.render_stateful_widget(widget, area, &mut state);
}
