use std::path::PathBuf;

use crossterm::event::{KeyCode, KeyEvent};
use ratatui::{layout::Rect, text::Line, Frame};

use crate::components::{AppContext, Component, Transition};
use crate::theme::ThemeColors;
use crate::ui::confirm::{done_buttons, review_buttons};
use crate::ui::detail::{render_detail, Detail};
use crate::ui::dialog::render_action_bar;
use crate::ui::footer::{unstow_review_hints, Hint};
use crate::ui::layout::{screen_rects, Shape};
use crate::ui::list_panel::{self, render_list_panel, ListView};
use crate::ui::unstow_confirm::{link_detail, link_row, Outcome, UnstowLink};

pub struct UnstowConfirmComponent {
    pub link: Option<UnstowLink>,
    pub unlink_error: Option<String>,
    pub unlink_success: bool,
    /// The one row of the list. Its cursor is the review's, so the row is always selected.
    pub list: ListView,
    /// The link's detail. Built by `update`, which has the home folder.
    detail: Detail,
}

impl UnstowConfirmComponent {
    /// Reads the link target now, so the screen paints without touching the disk.
    pub fn with_path(path: PathBuf) -> Self {
        let target = std::fs::read_link(&path)
            .map(|p| p.display().to_string())
            .unwrap_or_else(|_| "(unresolvable)".to_string());
        Self {
            link: Some(UnstowLink { path, target }),
            unlink_error: None,
            unlink_success: false,
            list: ListView::default(),
            detail: Detail::default(),
        }
    }

    fn finished(&self) -> bool {
        self.unlink_success || self.unlink_error.is_some()
    }

    fn outcome(&self) -> Outcome<'_> {
        match (&self.unlink_error, self.unlink_success) {
            (Some(error), _) => Outcome::Failed(error),
            (None, true) => Outcome::Removed,
            (None, false) => Outcome::Pending,
        }
    }

    /// The link's detail for the current outcome.
    fn sync(&mut self, ctx: &AppContext) {
        let colors = ThemeColors::from_theme(ctx.core.theme);
        self.detail = match &self.link {
            Some(link) => link_detail(link, &ctx.home_dir, self.outcome(), &colors),
            None => Detail::default(),
        };
    }
}

impl Component for UnstowConfirmComponent {
    fn id(&self) -> &'static str {
        "UnstowConfirm"
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }

    fn handle_key(&mut self, key: KeyEvent, ctx: &mut AppContext) -> Transition {
        if self.finished() {
            match key.code {
                KeyCode::Enter | KeyCode::Esc => return Transition::PopAll,
                _ => return Transition::None,
            }
        }

        if key.code == KeyCode::Char('y') || key.code == KeyCode::Char('Y') {
            if let Some(ref link) = self.link {
                match andre_core::unlink_stowed_symlink(&link.path) {
                    Ok(true) => {
                        self.unlink_success = true;
                        ctx.status_cache.borrow_mut().invalidate_all();
                    }
                    Ok(false) => {
                        self.unlink_error = Some("Not a symlink, nothing to remove.".to_string());
                    }
                    Err(e) => {
                        self.unlink_error = Some(format!("Failed to unlink: {}", e));
                    }
                }
            }
            self.sync(ctx);
            Transition::Handled
        } else if key.code == KeyCode::Char('n')
            || key.code == KeyCode::Char('N')
            || key.code == KeyCode::Esc
        {
            Transition::Pop
        } else {
            Transition::None
        }
    }

    fn update(&mut self, ctx: &mut AppContext) -> Transition {
        self.sync(ctx);
        Transition::None
    }

    fn hints(&self, _ctx: &AppContext) -> Vec<Hint> {
        unstow_review_hints(self.finished())
    }

    fn prepare(&mut self, area: Rect) {
        let rects = screen_rects(area, Shape::Review);
        self.list.prepare(rects.list, 1);
    }

    fn render(&mut self, frame: &mut Frame, area: Rect, ctx: &AppContext) {
        let colors = ThemeColors::from_theme(ctx.core.theme);
        let rects = screen_rects(area, Shape::Review);

        let width = list_panel::inner(rects.list).width;
        let rows: Vec<Line> = self
            .link
            .iter()
            .map(|link| link_row(link, self.list.cursor == 0, &colors, width))
            .collect();
        render_list_panel(
            frame,
            rects.list,
            &colors,
            "remove 1 link",
            rows,
            &self.list,
            None,
        );

        let title = self
            .link
            .as_ref()
            .and_then(|link| link.path.file_name())
            .map_or_else(
                || "link".to_string(),
                |name| name.to_string_lossy().into_owned(),
            );
        render_detail(frame, rects.detail, &colors, &title, &self.detail);

        if let Some(actions) = rects.actions {
            let buttons = if self.finished() {
                done_buttons()
            } else {
                review_buttons("remove", 1)
            };
            render_action_bar(frame, actions, &colors, &buttons);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_with_path_reads_link_target_up_front() {
        let tmp = tempfile::TempDir::new().unwrap();
        let source = tmp.path().join("source.txt");
        std::fs::write(&source, "").unwrap();
        let link = tmp.path().join("link.txt");
        std::os::unix::fs::symlink(&source, &link).unwrap();

        let component = UnstowConfirmComponent::with_path(link.clone());
        let recorded = component.link.as_ref().expect("link recorded");
        assert_eq!(recorded.path, link);
        assert_eq!(recorded.target, source.display().to_string());
    }

    #[test]
    fn test_with_path_marks_missing_link_unresolvable() {
        let tmp = tempfile::TempDir::new().unwrap();
        let component = UnstowConfirmComponent::with_path(tmp.path().join("absent"));
        let recorded = component.link.as_ref().expect("link recorded");
        assert_eq!(recorded.target, "(unresolvable)");
    }

    /// Removing a stowed link changes the tree, so cached stow statuses must go.
    #[test]
    fn successful_unlink_invalidates_the_status_cache() {
        use andre_core::config::Config;
        use andre_core::StowStatus;
        use crossterm::event::KeyModifiers;

        let tmp = tempfile::TempDir::new().unwrap();
        let source = tmp.path().join("source.txt");
        std::fs::write(&source, "").unwrap();
        let link = tmp.path().join("link.txt");
        std::os::unix::fs::symlink(&source, &link).unwrap();

        let mut ctx = AppContext::for_tests(Config::default_empty(), tmp.path());
        ctx.status_cache.borrow_mut().insert(
            ("home".to_string(), "link.txt".to_string()),
            StowStatus::Stowed,
        );

        let mut component = UnstowConfirmComponent::with_path(link.clone());
        let yes = KeyEvent::new(KeyCode::Char('y'), KeyModifiers::NONE);
        component.handle_key(yes, &mut ctx);

        assert!(component.unlink_success);
        assert!(!link.is_symlink());
        assert!(ctx.status_cache.borrow().is_empty());
    }
}
