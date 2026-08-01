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

    // Flags only — centered. Breadcrumb moved into each screen's panel title.
    let max = area.width as usize;
    let truncated = elide(&flags_plain, max);
    let text = Text::from(Line::from(Span::styled(
        truncated,
        Style::default().fg(colors.secondary),
    )));
    let paragraph = Paragraph::new(text).alignment(Alignment::Center);
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
