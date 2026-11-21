use ratatui::{
    layout::Rect,
    style::Style,
    text::{Line, Span},
    widgets::{List, ListItem, ListState},
    Frame,
};

use crate::theme::ThemeColors;
use crate::ui::widgets::scrollbar::{border_track, render_scrollbar};

fn k(label: &str, colors: &ThemeColors) -> Span<'static> {
    Span::styled(label.to_string(), Style::default().fg(colors.key))
}

fn h(label: &str, colors: &ThemeColors) -> Span<'static> {
    Span::styled(label.to_string(), Style::default().fg(colors.value))
}

pub fn help_item_count() -> usize {
    items(&crate::theme::ThemeColors::from_theme(
        andre_core::Theme::Default,
    ))
    .len()
}

fn items(colors: &ThemeColors) -> Vec<ListItem<'static>> {
    vec![
        ListItem::new(Line::from(vec![h("Keyboard Shortcuts", colors)])),
        ListItem::new(Line::from("")),
        ListItem::new(Line::from(vec![h("Navigation", colors)])),
        ListItem::new(Line::from(vec![
            Span::raw("  "),
            k("\u{2191}\u{2193} / kj", colors),
            Span::raw("   Navigate between items"),
        ])),
        ListItem::new(Line::from(vec![
            Span::raw("  "),
            k("Enter", colors),
            Span::raw("      Confirm / Select"),
        ])),
        ListItem::new(Line::from(vec![
            Span::raw("  "),
            k("Space", colors),
            Span::raw("      Toggle selection"),
        ])),
        ListItem::new(Line::from(vec![
            Span::raw("  "),
            k("Esc", colors),
            Span::raw("        Back / Cancel"),
        ])),
        ListItem::new(Line::from("")),
        ListItem::new(Line::from(vec![h("Flags (global shortcuts)", colors)])),
        ListItem::new(Line::from(vec![
            Span::raw("    "),
            k("v", colors),
            Span::raw("  Cycle verbosity (0-5)"),
        ])),
        ListItem::new(Line::from(vec![
            Span::raw("    "),
            k("d", colors),
            Span::raw("  Toggle dry-run"),
        ])),
        ListItem::new(Line::from(vec![
            Span::raw("    "),
            k("n", colors),
            Span::raw("  Toggle no-folding"),
        ])),
        ListItem::new(Line::from(vec![
            Span::raw("    "),
            k("a", colors),
            Span::raw("  Toggle adopt"),
        ])),
        ListItem::new(Line::from(vec![
            Span::raw("    "),
            k("o", colors),
            Span::raw("  Toggle dotfiles"),
        ])),
        ListItem::new(Line::from(vec![
            Span::raw("    "),
            k("t", colors),
            Span::raw("  Cycle theme"),
        ])),
        ListItem::new(Line::from("")),
        ListItem::new(Line::from(vec![h("Workflow", colors)])),
        ListItem::new(Line::from(vec![
            Span::raw("  1.  "),
            k("Select Groups", colors),
            Span::raw("    \u{2192}  choose which groups to stow"),
        ])),
        ListItem::new(Line::from(vec![
            Span::raw("  2.  "),
            k("Select Packages", colors),
            Span::raw("  \u{2192}  review and toggle packages"),
        ])),
        ListItem::new(Line::from(vec![
            Span::raw("  3.  "),
            k("Confirm", colors),
            Span::raw("          \u{2192}  review stow commands"),
        ])),
        ListItem::new(Line::from(vec![
            Span::raw("  4.  "),
            k("Execute", colors),
            Span::raw("          \u{2192}  run stow and see results"),
        ])),
        ListItem::new(Line::from("")),
        ListItem::new(Line::from(vec![Span::styled(
            "Press Esc to return",
            Style::default().fg(colors.text_dim),
        )])),
    ]
}

pub fn render_help(
    frame: &mut Frame,
    area: Rect,
    colors: &ThemeColors,
    title: &str,
    scroll_index: usize,
    viewport_height: usize,
) {
    let total = items(colors).len();
    let list = List::new(items(colors)).block(crate::ui::themed_panel(colors).title(title));

    let scroll_at = scroll_index.min(total.saturating_sub(1));
    let mut state = ListState::default();
    state.select(Some(scroll_at));
    frame.render_stateful_widget(list, area, &mut state);
    render_scrollbar(
        frame,
        border_track(area),
        colors,
        total,
        viewport_height,
        scroll_at,
    );
}
