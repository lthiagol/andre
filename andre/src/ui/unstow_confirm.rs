use std::path::PathBuf;

use ratatui::{
    layout::{Alignment, Rect},
    style::Style,
    text::{Line, Span, Text},
    widgets::Paragraph,
    Frame,
};

use crate::theme::ThemeColors;

/// The symlink an unstow confirm asks about, with the path it points at. The
/// component reads the target once, so paint never touches the disk.
pub struct UnstowLink {
    pub path: PathBuf,
    /// What the link points at, or `(unresolvable)`.
    pub target: String,
}

pub fn render_unstow_confirm(
    frame: &mut Frame,
    area: Rect,
    colors: &ThemeColors,
    title: &str,
    link: Option<&UnstowLink>,
    unlink_error: Option<&str>,
    unlink_success: bool,
) {
    let mut lines: Vec<Line> = Vec::new();

    if let Some(error) = unlink_error {
        lines.push(Line::from(Span::styled(
            error,
            Style::default().fg(colors.err),
        )));
        lines.push(Line::from(""));
        lines.push(Line::from(Span::styled(
            "Press Enter to continue",
            Style::default().fg(colors.text_dim),
        )));
        let paragraph = Paragraph::new(Text::from(lines))
            .block(crate::ui::themed_panel(colors).title(title))
            .alignment(Alignment::Left);
        frame.render_widget(paragraph, area);
        return;
    }

    if unlink_success {
        lines.push(Line::from(Span::styled(
            "Symlink removed successfully.",
            Style::default().fg(colors.ok),
        )));
        lines.push(Line::from(""));
        lines.push(Line::from(Span::styled(
            "Press Enter to continue",
            Style::default().fg(colors.text_dim),
        )));
        let paragraph = Paragraph::new(Text::from(lines))
            .block(crate::ui::themed_panel(colors).title(title))
            .alignment(Alignment::Left);
        frame.render_widget(paragraph, area);
        return;
    }

    lines.push(Line::from(Span::styled(
        "Remove symlink?",
        Style::default().fg(colors.accent),
    )));
    lines.push(Line::from(""));

    if let Some(link) = link {
        lines.push(Line::from(Span::styled(
            format!("  path: {}", link.path.display()),
            Style::default().fg(colors.text),
        )));
        lines.push(Line::from(Span::styled(
            format!("  →    {}", link.target),
            Style::default().fg(colors.value),
        )));
    }

    lines.push(Line::from(""));
    lines.push(Line::from(Span::styled(
        "This will only remove the link. The source file is safe.",
        Style::default().fg(colors.text_dim),
    )));
    lines.push(Line::from(""));
    lines.push(Line::from(Span::styled(
        "Press y to remove, n to cancel",
        Style::default().fg(colors.text_dim),
    )));

    let paragraph = Paragraph::new(Text::from(lines))
        .block(crate::ui::themed_panel(colors).title(title))
        .alignment(Alignment::Left);

    frame.render_widget(paragraph, area);
}
