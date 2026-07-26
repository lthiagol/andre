use std::path::Path;

use crate::theme::ThemeColors;
use crate::ui::widgets::file_browser::FileBrowserWidget;
use ratatui::{layout::Rect, widgets::ListState, Frame};

pub fn render_unstow_browse(
    frame: &mut Frame,
    area: Rect,
    colors: &ThemeColors,
    browse_path: &Path,
    browse_cursor: usize,
) {
    let mut state = ListState::default();
    let widget = FileBrowserWidget::new(colors, browse_path, browse_cursor, "Unstow Symlinks")
        .unstow_mode()
        .with_entries(&[], &[]);
    frame.render_stateful_widget(widget, area, &mut state);
}
