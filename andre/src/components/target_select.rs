//! The unstow group list: one row per group with its target, and the group's facts in the
//! detail. Enter opens the group's browse screen. With one group, unstow opens that browse
//! screen directly.

use std::borrow::Cow;
use std::path::PathBuf;

use crossterm::event::{KeyCode, KeyEvent, MouseEvent};
use ratatui::{
    layout::Rect,
    style::Style,
    text::{Line, Span},
    Frame,
};
use unicode_width::UnicodeWidthStr;

use crate::components::unstow::UnstowBrowseComponent;
use crate::components::{AppContext, Component, Transition};
use crate::input::{self, Input};
use crate::theme::ThemeColors;
use crate::ui::detail::{render_detail, Detail};
use crate::ui::display_path;
use crate::ui::elide;
use crate::ui::layout::{screen_rects, Shape};
use crate::ui::list_panel::{self, click_list, render_list_panel, Empty, ListView};
use crate::ui::selection::row_with_glyphs;
use andre_core::{discover_packages, Theme};

/// One group as the list shows it.
struct GroupRow {
    name: String,
    /// The group's source folder, or `None` when the group has none.
    source: Option<PathBuf>,
    /// The resolved target folder.
    target: PathBuf,
}

/// Picks a group, then opens its target directory in the browse screen.
pub struct TargetSelectComponent {
    /// The cursor and the window over the groups.
    pub list: ListView,
    /// The groups in name order. Built by `sync`.
    groups: Vec<GroupRow>,
    /// The cursor's detail. Built by `sync` when the cursor or the theme moves, since
    /// counting the packages reads the disk and the detail bakes in the theme's colours.
    detail: Detail,
    /// The cursor and theme the detail was built for.
    detail_key: Option<(usize, Theme)>,
}

impl TargetSelectComponent {
    pub fn unstow() -> Self {
        Self {
            list: ListView::default(),
            groups: Vec::new(),
            detail: Detail::default(),
            detail_key: None,
        }
    }

    /// Brings the groups up to date, and the detail when the cursor has moved.
    fn sync(&mut self, ctx: &AppContext) {
        let config = &ctx.core.config;
        self.groups = config
            .groups
            .keys()
            .map(|name| GroupRow {
                name: name.clone(),
                source: config.resolve_group_source(name, &ctx.config_dir, &ctx.home_dir),
                target: config
                    .resolve_group_target(name, &ctx.config_dir, &ctx.home_dir)
                    .unwrap_or_default(),
            })
            .collect();

        let key = (self.list.cursor, ctx.core.theme);
        if self.detail_key == Some(key) {
            return;
        }
        let colors = ThemeColors::from_theme(ctx.core.theme);
        self.detail = match self.groups.get(self.list.cursor) {
            Some(group) => group_detail(group, &ctx.home_dir, &colors),
            None => Detail::default(),
        };
        self.detail_key = Some(key);
    }
}

/// The facts of one group. Counting its packages reads its source folder.
fn group_detail(group: &GroupRow, home: &std::path::Path, colors: &ThemeColors) -> Detail {
    let source = match &group.source {
        Some(source) => display_path(source, home),
        None => "(none)".to_string(),
    };
    let packages = group
        .source
        .as_deref()
        .map_or(0, |source| discover_packages(source).len());
    Detail {
        headline: vec![Line::styled(
            "Browse links in this group's target and remove the ones you pick.",
            Style::default().fg(colors.text),
        )],
        facts: vec![
            (Cow::Borrowed("source"), source),
            (Cow::Borrowed("target"), display_path(&group.target, home)),
            (Cow::Borrowed("packages"), packages.to_string()),
        ],
        sections: Vec::new(),
    }
}

/// A group's name on the left and its target on the right, both in the row's width.
fn group_line(
    group: &GroupRow,
    selected: bool,
    colors: &ThemeColors,
    width: u16,
    home: &std::path::Path,
) -> Line<'static> {
    // The gutter and the margin before the border take two columns.
    let room = usize::from(width).saturating_sub(2);
    let target = display_path(&group.target, home);
    let target_width = target.width();
    let name = elide(&group.name, room.saturating_sub(target_width + 1));
    let pad = room.saturating_sub(name.width() + target_width);
    row_with_glyphs(
        Vec::new(),
        vec![
            Span::styled(name, Style::default().fg(colors.text)),
            Span::raw(" ".repeat(pad)),
            Span::styled(target, Style::default().fg(colors.text_dim)),
        ],
        selected,
        colors,
        width,
    )
}

/// The screen unstow opens. With exactly one group it is that group's browse screen;
/// otherwise the group list.
pub fn unstow_screen(ctx: &AppContext) -> Box<dyn Component> {
    let config = &ctx.core.config;
    let mut names = config.groups.keys();
    if let (Some(only), None) = (names.next(), names.next()) {
        if let Some(target) = config.resolve_group_target(only, &ctx.config_dir, &ctx.home_dir) {
            return Box::new(UnstowBrowseComponent::with_path(only.clone(), target));
        }
    }
    Box::new(TargetSelectComponent::unstow())
}

impl Component for TargetSelectComponent {
    fn id(&self) -> &'static str {
        "UnstowTargetSelect"
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }

    fn update(&mut self, ctx: &mut AppContext) -> Transition {
        self.sync(ctx);
        Transition::None
    }

    fn handle_key(&mut self, key: KeyEvent, ctx: &mut AppContext) -> Transition {
        // Keys can arrive before the next update, so the groups are brought up to date first.
        self.sync(ctx);
        if Input::handle_list_navigation(&key, &mut self.list.cursor, self.groups.len()) {
            self.sync(ctx);
            return Transition::None;
        }

        match key.code {
            KeyCode::Enter => match self.groups.get(self.list.cursor) {
                Some(group) => Transition::Push(Box::new(UnstowBrowseComponent::with_path(
                    group.name.clone(),
                    group.target.clone(),
                ))),
                None => Transition::None,
            },
            KeyCode::Esc => Transition::Pop,
            _ => Transition::None,
        }
    }

    fn handle_mouse(&mut self, ev: MouseEvent, ctx: &mut AppContext) -> Transition {
        self.sync(ctx);
        let len = self.groups.len();
        match click_list(&mut self.list, ev, len) {
            Some(click) if click.activate => self.handle_key(input::key(KeyCode::Enter), ctx),
            Some(_) => Transition::Handled,
            None => Transition::None,
        }
    }

    fn prepare(&mut self, area: Rect) {
        let rects = screen_rects(area, Shape::TwoPane);
        self.list.prepare(rects.list, self.groups.len());
    }

    fn render(&mut self, frame: &mut Frame, area: Rect, ctx: &AppContext) {
        let colors = ThemeColors::from_theme(ctx.core.theme);
        let rects = screen_rects(area, Shape::TwoPane);

        let width = list_panel::inner(rects.list).width;
        let rows: Vec<Line> = self
            .groups
            .iter()
            .enumerate()
            .map(|(i, group)| {
                group_line(group, i == self.list.cursor, &colors, width, &ctx.home_dir)
            })
            .collect();
        let title = format!("groups {}", self.groups.len());
        let empty = Empty {
            message: "no groups in andre.yml",
            hint: None,
        };
        render_list_panel(
            frame,
            rects.list,
            &colors,
            &title,
            rows,
            &self.list,
            Some(empty),
        );

        let name = self
            .groups
            .get(self.list.cursor)
            .map_or_else(|| "groups".to_string(), |group| group.name.clone());
        render_detail(frame, rects.detail, &colors, &name, &self.detail);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use andre_core::config::Config;

    /// A home folder with one group, `home`, whose source is `dotfiles/home`.
    fn fixture() -> (tempfile::TempDir, AppContext) {
        let tmp = tempfile::TempDir::new().unwrap();
        let home = tmp.path();
        std::fs::write(
            home.join("andre.yml"),
            "global: {}\ngroups:\n  home:\n    source: dotfiles/home\n    target: \"~\"\n",
        )
        .unwrap();
        let config = Config::load(&home.join("andre.yml")).unwrap();
        let ctx = AppContext::for_tests(config, home);
        (tmp, ctx)
    }

    #[test]
    fn test_unstow_list_reports_its_id() {
        assert_eq!(TargetSelectComponent::unstow().id(), "UnstowTargetSelect");
    }

    #[test]
    fn test_theme_change_rebuilds_the_detail_colours() {
        let (_tmp, mut ctx) = fixture();
        let mut component = TargetSelectComponent::unstow();
        component.sync(&ctx);
        let before = ThemeColors::from_theme(ctx.core.theme).text;

        ctx.core.theme = ctx.core.theme.next();
        component.sync(&ctx);
        let after = ThemeColors::from_theme(ctx.core.theme).text;
        assert_ne!(
            before, after,
            "the two themes must differ to prove anything"
        );
        assert_eq!(component.detail.headline[0].style.fg, Some(after));
    }
}
