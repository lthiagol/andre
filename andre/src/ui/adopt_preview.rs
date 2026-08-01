use ratatui::{
    layout::{Alignment, Rect},
    style::Style,
    text::{Line, Span, Text},
    widgets::Paragraph,
    Frame,
};

use crate::theme::ThemeColors;
use andre_core::AdoptionPlan;

pub fn render_adopt_preview(
    frame: &mut Frame,
    area: Rect,
    colors: &ThemeColors,
    title: &str,
    plan: Option<&AdoptionPlan>,
    result: Option<&str>,
) {
    if let Some(result) = result {
        let (fg, msg) = if result.starts_with("Adoption completed") {
            (colors.success, result)
        } else {
            (colors.error, result)
        };
        let paragraph = Paragraph::new(Text::from(vec![
            Line::from(Span::styled(msg, Style::default().fg(fg))),
            Line::from(""),
            Line::from(Span::styled(
                "Press Esc to continue",
                Style::default().fg(colors.muted),
            )),
        ]))
        .block(crate::ui::themed_panel(colors).title(title))
        .alignment(Alignment::Left);
        frame.render_widget(paragraph, area);
        return;
    }

    let plan = match plan {
        Some(p) => p,
        None => {
            let msg = Paragraph::new("Building plan...").style(Style::default().fg(colors.warning));
            frame.render_widget(msg, area);
            return;
        }
    };

    let mut lines: Vec<Line> = Vec::new();

    let sep_width = area.width.saturating_sub(2).max(10) as usize;
    let sep = "\u{2550}".repeat(sep_width);
    lines.push(Line::from(Span::styled(
        "Adoption Plan",
        Style::default().fg(colors.primary),
    )));
    lines.push(Line::from(Span::styled(
        sep.clone(),
        Style::default().fg(colors.muted),
    )));

    lines.push(Line::from(Span::styled(
        format!("Target:   {}", plan.target_path.display()),
        Style::default().fg(colors.secondary),
    )));
    lines.push(Line::from(Span::styled(
        format!("Source:   {}", plan.source_path.display()),
        Style::default().fg(colors.secondary),
    )));
    lines.push(Line::from(Span::styled(
        format!("Package:  {}", plan.package_name),
        Style::default().fg(colors.highlight),
    )));
    lines.push(Line::from(""));

    for (step_num, step) in (1..).zip(plan.steps.iter()) {
        let step_header = match step.action {
            andre_core::AdoptionAction::CreateDir => "Create directories",
            andre_core::AdoptionAction::MoveFile => "Move files into package",
            andre_core::AdoptionAction::StowPackage => "Stow the package",
        };

        if step_num == 1
            || matches!(
                step.action,
                andre_core::AdoptionAction::MoveFile | andre_core::AdoptionAction::StowPackage
            )
        {
            if step_num > 1 {
                lines.push(Line::from(""));
            }
            lines.push(Line::from(Span::styled(
                format!("Step {} \u{2014} {}:", step_num, step_header),
                Style::default().fg(colors.primary),
            )));
        }

        lines.push(Line::from(Span::styled(
            format!("  {}", step.description),
            Style::default().fg(colors.secondary),
        )));
    }

    lines.push(Line::from(""));
    lines.push(Line::from(Span::styled(
        sep,
        Style::default().fg(colors.muted),
    )));
    lines.push(Line::from(Span::styled(
        "Press y to execute, n to cancel",
        Style::default().fg(colors.muted),
    )));

    let paragraph = Paragraph::new(Text::from(lines))
        .block(crate::ui::themed_panel(colors).title(title))
        .alignment(Alignment::Left);

    frame.render_widget(paragraph, area);
}
