mod banner;
mod block;
pub mod chips;
pub mod confirm;
pub mod detail;
pub mod dialog;
pub mod execute;
pub mod footer;
pub mod icons;
pub mod layout;
pub mod list_panel;
pub mod selection;
pub mod settings;
mod status_bar;
pub mod status_style;
pub mod step_bar;
pub mod text_field;
mod toast;
pub mod tree;
pub mod unstow_confirm;
pub mod widgets;

pub use banner::{render_banner, render_compact_banner};
pub use block::{focus_panel, themed_panel};
pub use footer::render_footer;
pub use status_bar::{breadcrumb_label, render_status_bar};
pub use toast::render_toast;

use std::borrow::Cow;

use crate::app::App;
use crate::components::MainMenuComponent;
use crate::theme::ThemeColors;

use ratatui::{
    layout::{Constraint, Direction, Layout, Margin, Rect},
    style::Style,
    text::{Line, Span, Text},
    widgets::{Block, Borders, Paragraph},
    Frame,
};
use unicode_width::UnicodeWidthStr;

/// Rows of the full banner on the main menu. Every other screen gets a one-row title.
const BIG_BANNER_ROWS: u16 = 7;
const COMPACT_BANNER_ROWS: u16 = 1;
/// Rows that are neither banner nor content: the three separators inside the frame, the
/// separator above the footer and the footer itself.
const CHROME_ROWS: u16 = 5;
/// Widest a screen's content grows. Wider frames centre the content.
const MAX_CONTENT_WIDTH: u16 = 120;
/// The smallest terminal the layout works in.
const MIN_COLS: u16 = 80;
const MIN_ROWS: u16 = 24;
/// The too-small box: its outer size.
const OVERLAY_WIDTH: u16 = 40;
const OVERLAY_HEIGHT: u16 = 6;

/// The noun for a count: `plural(1, "item", "items")` is `item`; 0 and 2+ are `items`.
pub fn plural(n: usize, one: &'static str, many: &'static str) -> &'static str {
    if n == 1 {
        one
    } else {
        many
    }
}

pub fn render(app: &mut App, frame: &mut Frame) {
    let (cols, rows) = app.ctx.terminal_size;
    let colors = ThemeColors::from_theme(app.ctx.core.theme);
    if cols < MIN_COLS || rows < MIN_ROWS {
        render_too_small_overlay(frame, cols, rows, &colors);
        return;
    }

    let outer =
        crate::ui::themed_panel(&colors).style(Style::default().fg(colors.border).bg(colors.bg));
    frame.render_widget(&outer, frame.area());

    let inner = frame.area().inner(Margin::new(1, 1));

    let screen_id = app.component_stack.last().map_or("", |c| c.id());
    let crumbs: Vec<Cow<'static, str>> = app
        .component_stack
        .iter()
        .flat_map(|component| component.crumbs(&app.ctx))
        .collect();
    let hints = app
        .component_stack
        .last()
        .map(|c| c.hints(&app.ctx))
        .unwrap_or_default();
    let big_banner = screen_id == "MainMenu" && main_menu_fits_big_banner(app, inner.height);
    let banner_rows = if big_banner {
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

    if big_banner {
        render_banner(frame, v[0], &colors);
    } else {
        render_compact_banner(frame, v[0].inner(Margin::new(1, 0)), &colors, &crumbs);
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
        &hints,
        app.ctx.core.theme.name(),
    );

    if let Some(ref toast) = app.ctx.toast {
        // Overlay above chrome; Clear inside render_toast avoids ghosting.
        render_toast(frame, inner, &colors, toast);
    }
}

/// The main menu keeps the big banner only while every item fits under it, with its two
/// border rows. Otherwise it gets the one-row title (D8). Reads the menu's item count;
/// the menu is not asked to paint.
fn main_menu_fits_big_banner(app: &App, inner_rows: u16) -> bool {
    let items = app
        .component_stack
        .last()
        .and_then(|c| c.as_any().downcast_ref::<MainMenuComponent>())
        .map_or(0, MainMenuComponent::item_count);
    let content_rows = inner_rows.saturating_sub(BIG_BANNER_ROWS + CHROME_ROWS);
    items + 2 <= usize::from(content_rows)
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

/// The too-small box: the title, then the size the layout needs and the size the terminal
/// has. A dimension that passes is drawn in `ok`, one that fails in `err`.
fn render_too_small_overlay(frame: &mut Frame, cols: u16, rows: u16, colors: &ThemeColors) {
    let label = Style::default().fg(colors.text_dim);
    let verdict = |ok: bool| Style::default().fg(if ok { colors.ok } else { colors.err });
    let title = "terminal too small";
    let inner_width = usize::from(OVERLAY_WIDTH - 2);
    let indent = " ".repeat(inner_width.saturating_sub(title.width()) / 2);
    let text = Text::from(vec![
        Line::from(Span::styled(
            format!("{indent}{title}"),
            Style::default().fg(colors.text),
        )),
        Line::raw(""),
        Line::from(vec![
            Span::styled("     needs   ", label),
            Span::styled(
                format!("{MIN_COLS} \u{d7} {MIN_ROWS}"),
                Style::default().fg(colors.text),
            ),
        ]),
        Line::from(vec![
            Span::styled("     now     ", label),
            Span::styled(cols.to_string(), verdict(cols >= MIN_COLS)),
            Span::styled(" \u{d7} ", label),
            Span::styled(rows.to_string(), verdict(rows >= MIN_ROWS)),
        ]),
    ]);

    let area = frame.area();
    // Paint the surface, so the text colours sit on the theme's background
    // and not on whatever the terminal shows.
    frame.render_widget(Block::default().style(Style::default().bg(colors.bg)), area);
    let paragraph = Paragraph::new(text).block(themed_panel(colors));
    let vertical = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Min(1),
            Constraint::Length(OVERLAY_HEIGHT),
            Constraint::Min(1),
        ])
        .split(area);
    let horizontal = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Min(1),
            Constraint::Max(OVERLAY_WIDTH),
            Constraint::Min(1),
        ])
        .split(vertical[1]);
    frame.render_widget(paragraph, horizontal[1]);
}

/// `path` with the home directory shown as `~`.
pub fn display_path(path: &std::path::Path, home: &std::path::Path) -> String {
    match path.strip_prefix(home) {
        Ok(rest) if rest.as_os_str().is_empty() => "~".to_string(),
        Ok(rest) => format!("~/{}", rest.display()),
        Err(_) => path.display().to_string(),
    }
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
    use super::{display_path, elide, plural};
    use std::path::Path;

    #[test]
    fn test_display_path_uses_tilde_only_under_home() {
        let home = Path::new("/home/me");
        assert_eq!(display_path(Path::new("/home/me"), home), "~");
        assert_eq!(
            display_path(Path::new("/home/me/.config"), home),
            "~/.config"
        );
        assert_eq!(display_path(Path::new("/etc/andre"), home), "/etc/andre");
    }

    #[test]
    fn test_plural_uses_singular_only_for_one() {
        assert_eq!(plural(0, "item", "items"), "items");
        assert_eq!(plural(1, "item", "items"), "item");
        assert_eq!(plural(2, "item", "items"), "items");
    }

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
