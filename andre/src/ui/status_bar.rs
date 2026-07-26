use ratatui::{
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::Style,
    text::{Line, Span, Text},
    widgets::Paragraph,
    Frame,
};

use crate::theme::ThemeColors;
use crate::ui::elide;

/// Short display label for a component stack id in the breadcrumb.
pub fn breadcrumb_label(id: &str) -> &str {
    match id {
        "MainMenu" => "Main",
        "GroupSelect" => "Groups",
        "PackageSelect" => "Packages",
        "Settings" => "Config",
        "Status" => "Status",
        "Help" => "Help",
        "Confirm" => "Confirm",
        "Execute" => "Execute",
        "AdoptTargetSelect" => "Adopt",
        "AdoptFileSelect" => "Files",
        "AdoptConfirm" => "AdoptConfirm",
        "AdoptNamePrompt" => "Name",
        "AdoptPreview" => "Preview",
        "UnstowTargetSelect" => "Unstow",
        "UnstowBrowse" => "Browse",
        "UnstowConfirm" => "UnstowConfirm",
        other => other,
    }
}

/// Join stack ids into a breadcrumb string (`Main > Config`).
pub fn format_breadcrumb(stack_ids: &[&str]) -> String {
    stack_ids
        .iter()
        .map(|id| breadcrumb_label(id))
        .collect::<Vec<_>>()
        .join(" > ")
}

#[allow(clippy::too_many_arguments)]
pub fn render_status_bar(
    frame: &mut Frame,
    area: Rect,
    colors: &ThemeColors,
    verbosity: usize,
    dry_run: bool,
    no_folding: bool,
    adopt: bool,
    dotfiles: bool,
    action: &str,
    theme_name: &str,
    debug: bool,
    stack_ids: &[&str],
) {
    if area.width == 0 || area.height == 0 {
        return;
    }

    let crumb = format_breadcrumb(stack_ids);
    let action_str = action.to_uppercase();

    let flag_style =
        |on: bool| -> Style { Style::default().fg(if on { colors.success } else { colors.error }) };

    let val_str = |on: bool| -> &'static str {
        if on {
            "ON "
        } else {
            "OFF"
        }
    };

    let debug_str = if debug { "  [DEBUG]" } else { "" };
    let flags_plain = format!(
        "Verbosity:{}  Dry-Run:{}  No-Folding:{}  Adopt:{}  Dotfiles:{}  Action:{}  Theme:{}{}",
        verbosity,
        val_str(dry_run),
        val_str(no_folding),
        val_str(adopt),
        val_str(dotfiles),
        action_str,
        theme_name,
        debug_str,
    );

    // Breadcrumb left, toggles right. On very narrow widths collapse to one elided line.
    if !crumb.is_empty() && area.width >= 16 {
        let crumb_cols = unicode_width::UnicodeWidthStr::width(crumb.as_str());
        // Cap breadcrumb so flags keep at least half the row when possible.
        let max_crumb = (area.width as usize / 2).max(8).min(area.width as usize);
        let left_w = (crumb_cols + 1).clamp(1, max_crumb) as u16;
        let chunks = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Length(left_w), Constraint::Min(1)])
            .split(area);

        let crumb_para = Paragraph::new(Text::from(Line::from(Span::styled(
            elide(&crumb, chunks[0].width as usize),
            Style::default().fg(colors.highlight),
        ))))
        .alignment(Alignment::Left);
        frame.render_widget(crumb_para, chunks[0]);

        render_flags(
            frame,
            chunks[1],
            colors,
            verbosity,
            dry_run,
            no_folding,
            adopt,
            dotfiles,
            &action_str,
            theme_name,
            debug,
            &flags_plain,
            flag_style,
            val_str,
        );
        return;
    }

    let max = area.width as usize;
    let combined = if crumb.is_empty() {
        flags_plain
    } else {
        format!("{}  {}", crumb, flags_plain)
    };
    let truncated = elide(&combined, max);
    let paragraph = Paragraph::new(Text::from(Line::from(Span::styled(
        truncated,
        Style::default().fg(colors.secondary),
    ))))
    .alignment(Alignment::Left);
    frame.render_widget(paragraph, area);
}

#[allow(clippy::too_many_arguments)]
fn render_flags(
    frame: &mut Frame,
    area: Rect,
    colors: &ThemeColors,
    verbosity: usize,
    dry_run: bool,
    no_folding: bool,
    adopt: bool,
    dotfiles: bool,
    action_str: &str,
    theme_name: &str,
    debug: bool,
    flags_plain: &str,
    flag_style: impl Fn(bool) -> Style,
    val_str: impl Fn(bool) -> &'static str,
) {
    if area.width == 0 {
        return;
    }
    let max = area.width as usize;
    if unicode_width::UnicodeWidthStr::width(flags_plain) > max {
        let truncated = elide(flags_plain, max);
        let paragraph = Paragraph::new(Text::from(Line::from(Span::styled(
            truncated,
            Style::default().fg(colors.secondary),
        ))))
        .alignment(Alignment::Right);
        frame.render_widget(paragraph, area);
        return;
    }

    let text = Text::from(vec![Line::from(vec![
        Span::raw("Verbosity:"),
        Span::styled(verbosity.to_string(), Style::default().fg(colors.primary)),
        Span::raw("  Dry-Run:"),
        Span::styled(val_str(dry_run), flag_style(dry_run)),
        Span::raw("  No-Folding:"),
        Span::styled(val_str(no_folding), flag_style(no_folding)),
        Span::raw("  Adopt:"),
        Span::styled(val_str(adopt), flag_style(adopt)),
        Span::raw("  Dotfiles:"),
        Span::styled(val_str(dotfiles), flag_style(dotfiles)),
        Span::raw("  Action:"),
        Span::styled(action_str.to_string(), Style::default().fg(colors.primary)),
        Span::raw("  Theme:"),
        Span::styled(
            theme_name.to_string(),
            Style::default().fg(colors.highlight),
        ),
        if debug {
            Span::styled("  [DEBUG]", Style::default().fg(colors.warning))
        } else {
            Span::raw("")
        },
    ])]);

    let paragraph = Paragraph::new(text)
        .alignment(Alignment::Right)
        .style(Style::default().fg(colors.secondary));

    frame.render_widget(paragraph, area);
}

#[cfg(test)]
mod tests {
    use super::{breadcrumb_label, format_breadcrumb};

    #[test]
    fn test_format_breadcrumb_single() {
        assert_eq!(format_breadcrumb(&["MainMenu"]), "Main");
    }

    #[test]
    fn test_format_breadcrumb_stack() {
        assert_eq!(
            format_breadcrumb(&["MainMenu", "Settings"]),
            "Main > Config"
        );
    }

    #[test]
    fn test_breadcrumb_label_unknown_passthrough() {
        assert_eq!(breadcrumb_label("CustomScreen"), "CustomScreen");
    }
}
