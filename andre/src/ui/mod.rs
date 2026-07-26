mod adopt_browse;
mod adopt_confirm;
mod adopt_name;
mod adopt_preview;
mod banner;
mod block;
mod confirm;
mod execute;
mod footer;
pub mod help;
pub mod packages;
mod settings;
mod status;
mod status_bar;
mod toast;
mod unstow_browse;
mod unstow_confirm;
pub mod widgets;

pub use adopt_browse::render_adopt_browse;
pub use adopt_confirm::render_adopt_confirm;
pub use adopt_name::render_adopt_name;
pub use adopt_preview::render_adopt_preview;
pub use banner::render_banner;
pub use block::themed_panel;
pub use confirm::render_confirm;
pub use execute::{execute_list_len, render_execute, GAUGE_ROWS};
pub use footer::render_footer;
pub use help::render_help;
pub use packages::build_flat_items;
pub use packages::render_package_select;
pub use settings::render_settings;
pub use status::render_status;
pub use status_bar::{format_breadcrumb, render_status_bar};
pub use toast::render_toast;
pub use unstow_browse::render_unstow_browse;
pub use unstow_confirm::render_unstow_confirm;

use crate::app::App;
use crate::theme::ThemeColors;
use ratatui::{
    layout::{Alignment, Constraint, Direction, Layout, Margin},
    style::{Color, Style},
    text::{Line, Span, Text},
    widgets::{Block, Borders, Paragraph},
    Frame,
};

pub fn render(app: &mut App, frame: &mut Frame) {
    let (cols, rows) = app.ctx.terminal_size;
    if cols < 80 || rows < 24 {
        render_too_small_overlay(frame, cols, rows);
        return;
    }

    let colors = ThemeColors::from_theme(app.ctx.core.theme);

    let outer = crate::ui::themed_panel(&colors)
        .style(Style::default().fg(colors.border).bg(colors.background));
    frame.render_widget(&outer, frame.size());

    let inner = frame.size().inner(&Margin::new(1, 1));

    let v = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Max(7),
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Min(1),
            Constraint::Length(1),
            Constraint::Length(2),
        ])
        .split(inner);

    let sep = Block::default()
        .borders(Borders::TOP)
        .style(Style::default().fg(colors.border));
    for &i in &[1usize, 3, 5] {
        frame.render_widget(&sep, v[i]);
    }

    render_banner(frame, v[0], &colors);
    let stack_ids: Vec<&'static str> = app.component_stack.iter().map(|c| c.id()).collect();
    let screen_id = stack_ids.last().copied().unwrap_or("");
    render_status_bar(
        frame,
        v[2],
        &colors,
        app.ctx.core.verbosity as usize,
        app.ctx.core.dry_run,
        app.ctx.core.no_folding,
        app.ctx.core.adopt,
        app.ctx.core.dotfiles,
        app.ctx.core.effective_action().as_str(),
        app.ctx.core.theme.name(),
        app.ctx.debug,
        &stack_ids,
    );

    let h = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(15),
            Constraint::Percentage(70),
            Constraint::Percentage(15),
        ])
        .split(v[4]);

    if let Some(component) = app.component_stack.last_mut() {
        // Layout/viewport bookkeeping runs in `prepare`, not inside `render`.
        component.prepare(h[1]);
        component.render(frame, h[1], &app.ctx);
    }

    render_footer(frame, v[6], &colors, screen_id);

    if let Some(ref toast) = app.ctx.toast {
        // Overlay above chrome; Clear inside render_toast avoids ghosting.
        render_toast(frame, inner, &colors, toast);
    }
}

fn render_too_small_overlay(frame: &mut Frame, cols: u16, rows: u16) {
    let text = Text::from(vec![
        Line::from(Span::styled(
            "Terminal too small",
            Style::default().fg(Color::Rgb(255, 80, 80)),
        )),
        Line::from(Span::raw("")),
        Line::from(Span::styled(
            "Needs at least 80 columns x 24 rows",
            Style::default().fg(Color::Rgb(180, 180, 180)),
        )),
        Line::from(Span::styled(
            format!("Current size: {} columns x {} rows", cols, rows),
            Style::default().fg(Color::Rgb(120, 120, 120)),
        )),
    ]);

    let area = frame.size();
    let paragraph = Paragraph::new(text)
        .alignment(Alignment::Center)
        .block(Block::default().borders(Borders::ALL));
    let vertical = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Min(1),
            Constraint::Length(5),
            Constraint::Min(1),
        ])
        .split(area);
    let horizontal = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Min(1), Constraint::Max(40), Constraint::Min(1)])
        .split(vertical[1]);
    frame.render_widget(paragraph, horizontal[1]);
}

/// Truncate a string to `max_width` display columns, appending `…` if truncated.
pub fn elide(s: &str, max_width: usize) -> String {
    if max_width < 2 {
        return String::new();
    }
    let width = unicode_width::UnicodeWidthStr::width(s);
    if width <= max_width {
        s.to_string()
    } else {
        let mut truncated = String::new();
        let mut current_width = 0;
        let target = max_width.saturating_sub(1);
        for c in s.chars() {
            let w = unicode_width::UnicodeWidthChar::width(c).unwrap_or(0);
            if current_width + w > target {
                break;
            }
            truncated.push(c);
            current_width += w;
        }
        format!("{}…", truncated)
    }
}

#[cfg(test)]
mod tests {
    use super::elide;

    #[test]
    fn test_elide_no_truncation() {
        assert_eq!(elide("hello", 10), "hello");
    }

    #[test]
    fn test_elide_truncates() {
        assert_eq!(elide("hello world", 6), "hello…");
    }

    #[test]
    fn test_elide_max_width_too_small() {
        assert_eq!(elide("hello", 1), "");
    }

    #[test]
    fn test_elide_exact_fit() {
        assert_eq!(elide("hello", 5), "hello");
    }

    #[test]
    fn test_elide_wide_chars() {
        // "日本語" is 3 characters but 6 display columns wide
        assert_eq!(elide("日本語", 4), "日…");
    }
}
