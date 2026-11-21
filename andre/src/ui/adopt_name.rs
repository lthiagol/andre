use ratatui::{
    layout::{Alignment, Rect},
    style::Style,
    text::{Line, Span, Text},
    widgets::Paragraph,
    Frame,
};

use crate::theme::ThemeColors;

pub fn render_adopt_name(
    frame: &mut Frame,
    area: Rect,
    colors: &ThemeColors,
    title: &str,
    package_name: &str,
    existing_packages: &[String],
) {
    let mut lines: Vec<Line> = vec![
        Line::from(Span::styled(
            "Enter a name for the new package:",
            Style::default().fg(colors.accent),
        )),
        Line::from(""),
        Line::from(vec![
            Span::raw("> "),
            Span::styled(package_name.to_string(), Style::default().fg(colors.value)),
            Span::styled("_", Style::default().fg(colors.key)),
        ]),
        Line::from(""),
        Line::from(Span::styled(
            "Enter: confirm | Esc: cancel | Backspace: delete",
            Style::default().fg(colors.text_dim),
        )),
    ];

    if !existing_packages.is_empty() {
        lines.push(Line::from(""));
        lines.push(Line::from(Span::styled(
            "Existing packages (use as name):",
            Style::default().fg(colors.text_dim),
        )));
        for pkg in existing_packages {
            if package_name.is_empty()
                || pkg.to_lowercase().starts_with(&package_name.to_lowercase())
            {
                lines.push(Line::from(Span::styled(
                    format!("  {}", pkg),
                    Style::default().fg(colors.text),
                )));
            }
        }
    }

    let paragraph = Paragraph::new(Text::from(lines))
        .block(crate::ui::themed_panel(colors).title(title))
        .alignment(Alignment::Left);

    frame.render_widget(paragraph, area);
}
