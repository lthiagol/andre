use ratatui::{
    layout::{Alignment, Rect},
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
) {
    if area.width == 0 || area.height == 0 {
        return;
    }

    let action_str = action.to_uppercase();

    let on_off = |on: bool| -> &'static str {
        if on {
            "ON "
        } else {
            "OFF"
        }
    };
    let toggle_style =
        |on: bool| -> Style { Style::default().fg(if on { colors.success } else { colors.error }) };

    let debug_str = if debug { "  [DEBUG]" } else { "" };
    let flags_plain = format!(
        "Verbosity:{}  Dry-Run:{}  No-Folding:{}  Adopt:{}  Dotfiles:{}  Action:{}  Theme:{}{}",
        verbosity,
        on_off(dry_run),
        on_off(no_folding),
        on_off(adopt),
        on_off(dotfiles),
        action_str,
        theme_name,
        debug_str,
    );

    // Flags only — centered. Breadcrumb moved into each screen's panel title.
    // When the content fits, render with per-span colors so toggle state is
    // glanceable (ON = success/green, OFF = error/red). When it overflows,
    // fall back to a plain elided line — losing colors is preferable to
    // wrapping or truncation mid-token.
    let max = area.width as usize;
    let paragraph = if unicode_width::UnicodeWidthStr::width(flags_plain.as_str()) > max {
        let truncated = elide(&flags_plain, max);
        Paragraph::new(Text::from(Line::from(Span::styled(
            truncated,
            Style::default().fg(colors.secondary),
        ))))
        .alignment(Alignment::Center)
    } else {
        Paragraph::new(Text::from(Line::from(vec![
            Span::raw("Verbosity:"),
            Span::styled(verbosity.to_string(), Style::default().fg(colors.primary)),
            Span::raw("  Dry-Run:"),
            Span::styled(on_off(dry_run), toggle_style(dry_run)),
            Span::raw("  No-Folding:"),
            Span::styled(on_off(no_folding), toggle_style(no_folding)),
            Span::raw("  Adopt:"),
            Span::styled(on_off(adopt), toggle_style(adopt)),
            Span::raw("  Dotfiles:"),
            Span::styled(on_off(dotfiles), toggle_style(dotfiles)),
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
        ])))
        .alignment(Alignment::Center)
    };
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
