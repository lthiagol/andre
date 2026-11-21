mod topics;

pub use topics::Topic;

use crossterm::event::{KeyCode, KeyEvent, MouseEvent};
use ratatui::{
    layout::Rect,
    style::{Modifier, Style},
    text::{Line, Span},
    Frame,
};

use crate::components::{AppContext, Component, Transition};
use crate::input::Input;
use crate::theme::ThemeColors;
use crate::ui::detail::{detail_inner, detail_rows, fit_scroll, render_detail_scrolled, Detail};
use crate::ui::footer::{screen_hints, Hint};
use crate::ui::layout::{screen_rects, Shape};
use crate::ui::list_panel::{self, click_list, render_list_panel, ListView};
use crate::ui::selection;
use crate::ui::widgets::scrollbar::{border_track, render_scrollbar};

/// The help screen: the topics on the left, the one under the cursor on the right.
#[derive(Default)]
pub struct HelpComponent {
    /// The topic cursor and the window over the topics.
    pub list: ListView,
    /// The text of the topic under the cursor. Built by `update`, read while painting.
    detail: Detail,
    /// Rows the detail's text starts down. Clamped by `update`.
    scroll: u16,
    /// The detail pane, as of the last `prepare`.
    detail_rect: Rect,
}

impl HelpComponent {
    /// The topic under the cursor.
    fn topic(&self) -> Topic {
        Topic::ALL
            .get(self.list.cursor)
            .copied()
            .unwrap_or(Topic::Keys)
    }
}

impl Component for HelpComponent {
    fn id(&self) -> &'static str {
        "Help"
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }

    fn handle_key(&mut self, key: KeyEvent, _ctx: &mut AppContext) -> Transition {
        if Input::handle_list_navigation(&key, &mut self.list.cursor, Topic::ALL.len()) {
            self.scroll = 0;
            return Transition::Handled;
        }
        let page = detail_inner(self.detail_rect)
            .height
            .saturating_sub(1)
            .max(1);
        match key.code {
            KeyCode::PageUp => self.scroll = self.scroll.saturating_sub(page),
            KeyCode::PageDown => self.scroll = self.scroll.saturating_add(page),
            KeyCode::Esc => return Transition::Pop,
            _ => return Transition::None,
        }
        Transition::Handled
    }

    fn update(&mut self, ctx: &mut AppContext) -> Transition {
        self.detail = self.topic().detail(ctx);
        let colors = ThemeColors::from_theme(ctx.core.theme);
        self.scroll = fit_scroll(&self.detail, self.detail_rect, &colors, self.scroll);
        Transition::None
    }

    fn hints(&self, _ctx: &AppContext) -> Vec<Hint> {
        screen_hints(self.id())
    }

    fn handle_mouse(&mut self, ev: MouseEvent, _ctx: &mut AppContext) -> Transition {
        let before = self.list.cursor;
        match click_list(&mut self.list, ev, Topic::ALL.len()) {
            Some(_) => {
                if self.list.cursor != before {
                    self.scroll = 0;
                }
                Transition::Handled
            }
            None => Transition::None,
        }
    }

    fn prepare(&mut self, area: Rect) {
        let rects = screen_rects(area, Shape::TwoPane);
        self.list.prepare(rects.list, Topic::ALL.len());
        self.detail_rect = rects.detail;
    }

    fn render(&mut self, frame: &mut Frame, area: Rect, ctx: &AppContext) {
        let colors = ThemeColors::from_theme(ctx.core.theme);
        let rects = screen_rects(area, Shape::TwoPane);

        let width = list_panel::inner(rects.list).width;
        let rows: Vec<Line> = Topic::ALL
            .iter()
            .enumerate()
            .map(|(i, topic)| topic_row(*topic, i == self.list.cursor, &colors, width))
            .collect();
        render_list_panel(frame, rects.list, &colors, "help", rows, &self.list, None);
        render_detail_scrolled(
            frame,
            rects.detail,
            &colors,
            self.topic().label(),
            &self.detail,
            self.scroll,
        );
        let rows = detail_rows(&self.detail, rects.detail, &colors);
        let viewport = detail_inner(rects.detail).height;
        render_scrollbar(
            frame,
            border_track(rects.detail),
            &colors,
            rows,
            usize::from(viewport),
            usize::from(self.scroll),
        );
    }
}

/// One topic name. The selected one is bold; the shared painter draws its bar and fill.
fn topic_row(topic: Topic, selected: bool, colors: &ThemeColors, width: u16) -> Line<'static> {
    let style = if selected {
        Style::default()
            .fg(colors.text)
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(colors.text)
    };
    selection::row(
        vec![Span::styled(topic.label(), style)],
        selected,
        colors,
        width,
    )
}
