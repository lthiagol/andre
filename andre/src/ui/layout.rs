//! Screen rectangles. Every screen splits its content area through [`screen_rects`], so
//! the pane widths and the rows taken by chips and action bars match across screens.

use ratatui::layout::Rect;

/// Columns between the list pane and the detail pane.
pub const PANE_GAP: u16 = 1;

const LIST_MIN: u16 = 34;
const LIST_MAX: u16 = 48;

/// The rows a screen takes above and below its panes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Shape {
    /// List | detail.
    TwoPane,
    /// A chip row above list | detail.
    ChipTwoPane,
    /// A step bar, a blank row, then list | detail.
    Wizard,
    /// List | detail above an action bar.
    Review,
    /// A chip row, list | detail, then an action bar.
    ChipReview,
    /// One panel across the full width, above an action bar.
    Wide,
}

/// The parts of one screen. `detail` is empty for [`Shape::Wide`].
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct ScreenRects {
    /// The chip row, or the step bar for [`Shape::Wizard`].
    pub chip: Option<Rect>,
    pub list: Rect,
    pub detail: Rect,
    pub actions: Option<Rect>,
}

/// Width of the list pane for a content area `content_width` columns wide:
/// two fifths of it, kept between 34 and 48 columns.
pub fn list_width(content_width: u16) -> u16 {
    let two_fifths = u32::from(content_width) * 2 / 5;
    two_fifths.clamp(u32::from(LIST_MIN), u32::from(LIST_MAX)) as u16
}

/// Splits `area` for `shape`. Rows above the panes come first, then the panes, then the
/// action bar on the last row.
pub fn screen_rects(area: Rect, shape: Shape) -> ScreenRects {
    let head = match shape {
        Shape::TwoPane | Shape::Review | Shape::Wide => 0,
        Shape::ChipTwoPane | Shape::ChipReview => 1,
        Shape::Wizard => 2,
    };
    let foot = match shape {
        Shape::Review | Shape::ChipReview | Shape::Wide => 1,
        Shape::TwoPane | Shape::ChipTwoPane | Shape::Wizard => 0,
    };

    let top = area.y;
    let panes_y = area.y.saturating_add(head);
    let panes_height = area.height.saturating_sub(head + foot);
    let chip = (head > 0).then_some(Rect { height: 1, ..area });
    let actions = (foot > 0).then(|| Rect {
        y: area.bottom().saturating_sub(1),
        height: 1,
        ..area
    });

    if shape == Shape::Wide {
        return ScreenRects {
            chip,
            list: Rect {
                y: panes_y,
                height: panes_height,
                ..area
            },
            detail: Rect::new(area.x, top, 0, 0),
            actions,
        };
    }

    let list_w = list_width(area.width).min(area.width);
    let detail_x = area.x.saturating_add(list_w).saturating_add(PANE_GAP);
    let detail_w = area.right().saturating_sub(detail_x);
    ScreenRects {
        chip,
        list: Rect::new(area.x, panes_y, list_w, panes_height),
        detail: Rect::new(detail_x, panes_y, detail_w, panes_height),
        actions,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_list_width_is_two_fifths_clamped_to_34_and_48() {
        assert_eq!(list_width(76), 34);
        assert_eq!(list_width(120), 48);
        assert_eq!(list_width(100), 40);
        assert_eq!(list_width(10), 34);
        assert_eq!(list_width(400), 48);
    }

    #[test]
    fn test_two_pane_at_80_columns_is_34_then_41() {
        let area = Rect::new(2, 5, 76, 16);
        let rects = screen_rects(area, Shape::TwoPane);
        assert_eq!(rects.chip, None);
        assert_eq!(rects.actions, None);
        assert_eq!(rects.list, Rect::new(2, 5, 34, 16));
        assert_eq!(rects.detail, Rect::new(37, 5, 41, 16));
    }

    #[test]
    fn test_two_pane_at_120_columns_is_48_then_71() {
        let rects = screen_rects(Rect::new(0, 0, 120, 16), Shape::TwoPane);
        assert_eq!(rects.list.width, 48);
        assert_eq!(rects.detail.width, 71);
        assert_eq!(rects.detail.x, 49);
    }

    #[test]
    fn test_chip_two_pane_gives_the_top_row_to_the_chip() {
        let rects = screen_rects(Rect::new(0, 0, 76, 16), Shape::ChipTwoPane);
        assert_eq!(rects.chip, Some(Rect::new(0, 0, 76, 1)));
        assert_eq!(rects.list.y, 1);
        assert_eq!(rects.list.height, 15);
        assert_eq!(rects.detail.height, 15);
    }

    #[test]
    fn test_wizard_has_a_step_row_and_a_blank_row_above_the_panes() {
        let rects = screen_rects(Rect::new(0, 0, 76, 16), Shape::Wizard);
        assert_eq!(rects.chip, Some(Rect::new(0, 0, 76, 1)));
        assert_eq!(rects.list.y, 2);
        assert_eq!(rects.list.height, 14);
    }

    #[test]
    fn test_review_keeps_the_action_bar_on_the_last_row() {
        let rects = screen_rects(Rect::new(0, 0, 76, 16), Shape::Review);
        assert_eq!(rects.list.height, 15);
        assert_eq!(rects.actions, Some(Rect::new(0, 15, 76, 1)));
    }

    #[test]
    fn test_chip_review_is_chip_panes_and_bar() {
        let rects = screen_rects(Rect::new(0, 0, 76, 16), Shape::ChipReview);
        assert_eq!(rects.chip.map(|r| r.height), Some(1));
        assert_eq!(rects.list.height, 14);
        assert_eq!(rects.actions.map(|r| r.y), Some(15));
    }

    #[test]
    fn test_wide_gives_the_list_rect_the_full_width_and_no_detail() {
        let rects = screen_rects(Rect::new(0, 0, 76, 16), Shape::Wide);
        assert_eq!(rects.list, Rect::new(0, 0, 76, 15));
        assert_eq!(rects.detail.width, 0);
        assert_eq!(rects.actions, Some(Rect::new(0, 15, 76, 1)));
    }

    #[test]
    fn test_panes_never_underflow_in_a_tiny_area() {
        let rects = screen_rects(Rect::new(0, 0, 3, 1), Shape::ChipReview);
        assert_eq!(rects.list.height, 0);
        assert_eq!(rects.detail.width, 0);
    }
}
