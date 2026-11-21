mod adopt_confirm;
mod adopt_name;
mod adopt_preview;
mod banner;
mod block;
mod confirm;
mod execute;
mod footer;
pub mod help;
pub mod icons;
pub mod packages;
pub mod selection;
mod settings;
mod status;
mod status_bar;
pub mod status_style;
mod toast;
mod unstow_confirm;
pub mod widgets;

pub use adopt_confirm::render_adopt_confirm;
pub use adopt_name::render_adopt_name;
pub use adopt_preview::render_adopt_preview;
pub use banner::{render_banner, render_compact_banner};
pub use block::themed_panel;
pub use confirm::{render_confirm, ConfirmRow};
pub use execute::{execute_list_len, render_execute, GAUGE_ROWS};
pub use footer::render_footer;
pub use help::render_help;
pub use packages::render_package_select;
pub use settings::render_settings;
pub use status::{render_status, status_layout, StatusRow};
pub use status_bar::{breadcrumb_label, format_breadcrumb, render_status_bar};
pub use toast::render_toast;
pub use unstow_confirm::{render_unstow_confirm, UnstowLink};

use crate::app::App;
use crate::theme::ThemeColors;
use ratatui::{
    layout::{Alignment, Constraint, Direction, Layout, Margin, Rect},
    style::{Color, Style},
    text::{Line, Span, Text},
    widgets::{Block, Borders, Paragraph},
    Frame,
};

/// Rows of the full banner on the main menu. Every other screen gets a one-row title.
const BIG_BANNER_ROWS: u16 = 7;
const COMPACT_BANNER_ROWS: u16 = 1;
/// Widest a screen's content grows. Wider frames centre the content.
const MAX_CONTENT_WIDTH: u16 = 120;

pub fn render(app: &mut App, frame: &mut Frame) {
    let (cols, rows) = app.ctx.terminal_size;
    if cols < 80 || rows < 24 {
        render_too_small_overlay(frame, cols, rows);
        return;
    }

    let colors = ThemeColors::from_theme(app.ctx.core.theme);

    let outer = crate::ui::themed_panel(&colors)
        .style(Style::default().fg(colors.border).bg(colors.background));
    frame.render_widget(&outer, frame.area());

    let inner = frame.area().inner(Margin::new(1, 1));

    let stack_ids: Vec<&'static str> = app.component_stack.iter().map(|c| c.id()).collect();
    let screen_id = stack_ids.last().copied().unwrap_or("");
    let banner_rows = if screen_id == "MainMenu" {
        BIG_BANNER_ROWS
    } else {
        COMPACT_BANNER_ROWS
    };

    let v = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Max(banner_rows),
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Min(1),
            Constraint::Length(1),
            Constraint::Length(1),
        ])
        .split(inner);

    let sep = Block::default()
        .borders(Borders::TOP)
        .style(Style::default().fg(colors.border));
    for &i in &[1usize, 3, 5] {
        frame.render_widget(&sep, v[i]);
    }

    // Refresh the breadcrumb only when the stack actually changes — the
    // rendered chrome stays correct on every frame and `format_breadcrumb`
    // doesn't allocate a fresh Vec + String on the ~10 fps redraw path.
    if app.ctx.breadcrumb_cache != stack_ids {
        app.ctx.breadcrumb = format_breadcrumb(&stack_ids);
        app.ctx.breadcrumb_cache.clear();
        app.ctx.breadcrumb_cache.extend(stack_ids.iter().copied());
    }
    if screen_id == "MainMenu" {
        render_banner(frame, v[0], &colors);
    } else {
        render_compact_banner(
            frame,
            v[0].inner(Margin::new(1, 0)),
            &colors,
            &app.ctx.breadcrumb,
        );
    }
    // The bars keep one column of air on each side, like the content below.
    render_status_bar(
        frame,
        v[2].inner(Margin::new(1, 0)),
        &colors,
        &app.ctx.core.flags,
        app.ctx.core.icons,
        app.ctx.debug,
    );

    let content = centred(v[4].inner(Margin::new(1, 0)), MAX_CONTENT_WIDTH);
    if let Some(component) = app.component_stack.last_mut() {
        // Layout/viewport bookkeeping runs in `prepare`, not inside `render`.
        component.prepare(content);
        component.render(frame, content, &app.ctx);
    }

    render_footer(
        frame,
        v[6].inner(Margin::new(1, 0)),
        &colors,
        screen_id,
        app.ctx.core.theme.name(),
    );

    if let Some(ref toast) = app.ctx.toast {
        // Overlay above chrome; Clear inside render_toast avoids ghosting.
        render_toast(frame, inner, &colors, toast);
    }
}

/// `area` narrowed to `max_width` columns, centred horizontally.
fn centred(area: Rect, max_width: u16) -> Rect {
    let width = area.width.min(max_width);
    Rect {
        x: area.x + (area.width - width) / 2,
        width,
        ..area
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

    let area = frame.area();
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
