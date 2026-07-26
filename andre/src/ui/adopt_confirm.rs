use ratatui::{
    layout::{Alignment, Rect},
    style::Style,
    text::{Line, Span, Text},
    widgets::Paragraph,
    Frame,
};

use crate::theme::ThemeColors;

pub fn render_adopt_confirm(
    frame: &mut Frame,
    area: Rect,
    colors: &ThemeColors,
    selected_files: &[std::path::PathBuf],
) {
    let mut lines: Vec<Line> = Vec::new();

    lines.push(Line::from(Span::styled(
        "Adopt and create package for:",
        Style::default().fg(colors.primary),
    )));
    lines.push(Line::from(""));

    for file in selected_files {
        let name = file
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| file.display().to_string());
        lines.push(Line::from(Span::styled(
            format!("  {}", name),
            Style::default().fg(colors.secondary),
        )));
    }

    lines.push(Line::from(""));
    lines.push(Line::from(Span::styled(
        "Press y to proceed, n to go back",
        Style::default().fg(colors.muted),
    )));

    let paragraph = Paragraph::new(Text::from(lines))
        .block(crate::ui::themed_panel(colors).title("Confirm Adoption"))
        .alignment(Alignment::Left);

    frame.render_widget(paragraph, area);
}
