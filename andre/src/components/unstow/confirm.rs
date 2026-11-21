use std::path::{Path, PathBuf};

use crossterm::event::{KeyCode, KeyEvent, MouseEvent};
use ratatui::{layout::Rect, text::Line, Frame};

use crate::components::{AppContext, Component, Transition};
use crate::input::Input;
use crate::theme::ThemeColors;
use crate::ui::confirm::{done_buttons, review_buttons};
use crate::ui::detail::{render_detail, Detail};
use crate::ui::dialog::{button_click, render_action_bar, Button};
use crate::ui::footer::{unstow_review_hints, Hint};
use crate::ui::layout::{screen_rects, Shape};
use crate::ui::list_panel::{self, click_list, render_list_panel, ListView};
use crate::ui::plural;
use crate::ui::unstow_confirm::{link_detail, link_row, Outcome, UnstowLink};

use super::owner::{inside_packages, owning_package};

/// Reviews the links to remove. `y` removes them all; each row then says how its own
/// removal went.
pub struct UnstowConfirmComponent {
    pub links: Vec<UnstowLink>,
    /// What each unlink did, in row order. `None` until `y` runs them.
    outcomes: Option<Vec<Result<(), String>>>,
    /// The cursor and the window over the links.
    pub list: ListView,
    /// The cursor link's detail. Built by `sync`, which has the home folder.
    detail: Detail,
    /// The action bar, as the last `prepare` laid it out.
    actions: Option<Rect>,
}

impl UnstowConfirmComponent {
    /// Reviews the links, in the order given. Reads each link's target now, so the screen
    /// paints without touching the disk. `packages` is the packages folder, which names
    /// the package each link belongs to.
    pub fn with_paths(paths: Vec<PathBuf>, packages: Option<&Path>) -> Self {
        let links = paths
            .into_iter()
            .map(|path| {
                let target = std::fs::read_link(&path)
                    .map(|p| p.display().to_string())
                    .unwrap_or_else(|_| "(unresolvable)".to_string());
                let package = packages.and_then(|dir| owning_package(&path, &target, dir));
                let inside = packages
                    .and_then(|dir| inside_packages(&path, &target, dir))
                    .map(|inside| inside.display().to_string());
                UnstowLink {
                    path,
                    target,
                    package,
                    inside,
                }
            })
            .collect();
        Self {
            links,
            outcomes: None,
            list: ListView::default(),
            detail: Detail::default(),
            actions: None,
        }
    }

    /// The action bar's buttons: the review's, or the done ones once the unlinks ran.
    fn action_buttons(&self) -> Vec<Button> {
        if self.finished() {
            done_buttons()
        } else {
            review_buttons("remove", self.links.len())
        }
    }

    fn finished(&self) -> bool {
        self.outcomes.is_some()
    }

    /// Where the link at `index` stands.
    fn outcome(&self, index: usize) -> Outcome<'_> {
        match self
            .outcomes
            .as_ref()
            .and_then(|results| results.get(index))
        {
            None => Outcome::Pending,
            Some(Ok(())) => Outcome::Removed,
            Some(Err(message)) => Outcome::Failed(message),
        }
    }

    /// Unlinks every link and keeps each result.
    fn remove_all(&mut self, ctx: &mut AppContext) {
        let results: Vec<Result<(), String>> = self
            .links
            .iter()
            .map(|link| match andre_core::unlink_stowed_symlink(&link.path) {
                Ok(true) => Ok(()),
                Ok(false) => Err("Not a symlink, nothing to remove.".to_string()),
                Err(e) => Err(format!("Failed to unlink: {}", e)),
            })
            .collect();
        // Removing a stowed link changes the tree, so cached stow statuses must go.
        if results.iter().any(Result::is_ok) {
            ctx.status_cache.borrow_mut().invalidate_all();
        }
        self.outcomes = Some(results);
    }

    /// The cursor link's detail for its outcome.
    fn sync(&mut self, ctx: &AppContext) {
        let colors = ThemeColors::from_theme(ctx.core.theme);
        let cursor = self.list.cursor;
        self.detail = match self.links.get(cursor) {
            Some(link) => link_detail(link, &ctx.home_dir, self.outcome(cursor), &colors),
            None => Detail::default(),
        };
    }

    /// `remove 2 links` while waiting; `removed 1, failed 1` once `y` has run.
    fn list_title(&self) -> String {
        let total = self.links.len();
        match &self.outcomes {
            None => format!("remove {total} {}", plural(total, "link", "links")),
            Some(results) => {
                let failed = results.iter().filter(|result| result.is_err()).count();
                let removed = total - failed;
                if failed == 0 {
                    format!("removed {removed}")
                } else {
                    format!("removed {removed}, failed {failed}")
                }
            }
        }
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
        self.sync(ctx);
        if Input::handle_list_navigation(&key, &mut self.list.cursor, self.links.len()) {
            self.sync(ctx);
            return Transition::None;
        }

        if self.finished() {
            return match key.code {
                KeyCode::Enter | KeyCode::Esc => Transition::PopAll,
                _ => Transition::None,
            };
        }

        match key.code {
            KeyCode::Char('y') | KeyCode::Char('Y') => {
                self.remove_all(ctx);
                self.sync(ctx);
                Transition::Handled
            }
            KeyCode::Char('n') | KeyCode::Char('N') | KeyCode::Esc => Transition::Pop,
            _ => Transition::None,
        }
    }

    fn update(&mut self, ctx: &mut AppContext) -> Transition {
        self.sync(ctx);
        Transition::None
    }

    fn hints(&self, _ctx: &AppContext) -> Vec<Hint> {
        unstow_review_hints(self.finished())
    }

    fn handle_mouse(&mut self, ev: MouseEvent, ctx: &mut AppContext) -> Transition {
        if let Some(actions) = self.actions {
            if let Some(key) = button_click(actions, &self.action_buttons(), ev) {
                return self.handle_key(key, ctx);
            }
        }
        let len = self.links.len();
        match click_list(&mut self.list, ev, len) {
            Some(_) => Transition::Handled,
            None => Transition::None,
        }
    }

    fn prepare(&mut self, area: Rect) {
        let rects = screen_rects(area, Shape::Review);
        self.list.prepare(rects.list, self.links.len());
        self.actions = rects.actions;
    }

    fn render(&mut self, frame: &mut Frame, area: Rect, ctx: &AppContext) {
        let colors = ThemeColors::from_theme(ctx.core.theme);
        let rects = screen_rects(area, Shape::Review);

        let width = list_panel::inner(rects.list).width;
        let rows: Vec<Line> = self
            .links
            .iter()
            .enumerate()
            .map(|(i, link)| link_row(link, i == self.list.cursor, &colors, width))
            .collect();
        render_list_panel(
            frame,
            rects.list,
            &colors,
            &self.list_title(),
            rows,
            &self.list,
            None,
        );

        let title = self
            .links
            .get(self.list.cursor)
            .and_then(|link| link.path.file_name())
            .map_or_else(
                || "link".to_string(),
                |name| name.to_string_lossy().into_owned(),
            );
        render_detail(frame, rects.detail, &colors, &title, &self.detail);

        if let Some(actions) = rects.actions {
            render_action_bar(frame, actions, &colors, &self.action_buttons());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use andre_core::config::Config;
    use andre_core::StowStatus;
    use crossterm::event::KeyModifiers;

    #[test]
    fn test_with_paths_reads_link_target_up_front() {
        let tmp = tempfile::TempDir::new().unwrap();
        let source = tmp.path().join("source.txt");
        std::fs::write(&source, "").unwrap();
        let link = tmp.path().join("link.txt");
        std::os::unix::fs::symlink(&source, &link).unwrap();

        let component = UnstowConfirmComponent::with_paths(vec![link.clone()], None);
        assert_eq!(component.links.len(), 1);
        assert_eq!(component.links[0].path, link);
        assert_eq!(component.links[0].target, source.display().to_string());
    }

    #[test]
    fn test_with_paths_marks_missing_link_unresolvable() {
        let tmp = tempfile::TempDir::new().unwrap();
        let component = UnstowConfirmComponent::with_paths(vec![tmp.path().join("absent")], None);
        assert_eq!(component.links[0].target, "(unresolvable)");
    }

    /// Each link names the package its target sits in; a link outside the packages folder
    /// names none.
    #[test]
    fn test_with_paths_names_the_package_each_link_points_into() {
        let tmp = tempfile::TempDir::new().unwrap();
        let packages = tmp.path().join("dotfiles");
        let source = packages.join("shell/.bashrc");
        std::fs::create_dir_all(source.parent().unwrap()).unwrap();
        std::fs::write(&source, "").unwrap();
        let inside = tmp.path().join(".bashrc");
        std::os::unix::fs::symlink("dotfiles/shell/.bashrc", &inside).unwrap();
        let outside = tmp.path().join(".profile");
        std::os::unix::fs::symlink("/etc/profile", &outside).unwrap();

        let component = UnstowConfirmComponent::with_paths(vec![inside, outside], Some(&packages));
        assert_eq!(component.links[0].package.as_deref(), Some("shell"));
        assert_eq!(component.links[0].inside.as_deref(), Some("shell/.bashrc"));
        assert_eq!(component.links[1].package, None);
        assert_eq!(component.links[1].inside, None);
    }

    /// Removing a stowed link changes the tree, so cached stow statuses must go.
    #[test]
    fn successful_unlink_invalidates_the_status_cache() {
        let tmp = tempfile::TempDir::new().unwrap();
        let source = tmp.path().join("source.txt");
        std::fs::write(&source, "").unwrap();
        let link = tmp.path().join("link.txt");
        std::os::unix::fs::symlink(&source, &link).unwrap();

        let mut ctx = AppContext::for_tests(Config::default_empty(), tmp.path());
        ctx.status_cache
            .borrow_mut()
            .insert("link.txt".to_string(), StowStatus::Stowed);

        let mut component = UnstowConfirmComponent::with_paths(vec![link.clone()], None);
        let yes = KeyEvent::new(KeyCode::Char('y'), KeyModifiers::NONE);
        component.handle_key(yes, &mut ctx);

        assert_eq!(component.outcomes, Some(vec![Ok(())]));
        assert!(!link.is_symlink());
        assert!(ctx.status_cache.borrow().is_empty());
    }

    /// `y` removes every link in the review, and a link that is not a symlink fails alone.
    #[test]
    fn test_y_removes_each_link_and_reports_the_one_that_failed() {
        let tmp = tempfile::TempDir::new().unwrap();
        let source = tmp.path().join("source.txt");
        std::fs::write(&source, "").unwrap();
        let first = tmp.path().join("first.txt");
        std::os::unix::fs::symlink(&source, &first).unwrap();
        let second = tmp.path().join("second.txt");
        std::os::unix::fs::symlink(&source, &second).unwrap();
        let plain = tmp.path().join("plain.txt");
        std::fs::write(&plain, "").unwrap();

        let mut ctx = AppContext::for_tests(Config::default_empty(), tmp.path());
        let mut component = UnstowConfirmComponent::with_paths(
            vec![first.clone(), plain.clone(), second.clone()],
            None,
        );
        assert_eq!(component.list_title(), "remove 3 links");

        let yes = KeyEvent::new(KeyCode::Char('y'), KeyModifiers::NONE);
        component.handle_key(yes, &mut ctx);

        assert!(!first.is_symlink());
        assert!(!second.is_symlink());
        assert!(plain.exists(), "a regular file is never removed");
        assert_eq!(component.list_title(), "removed 2, failed 1");
        assert!(matches!(component.outcome(1), Outcome::Failed(_)));
        assert!(matches!(component.outcome(0), Outcome::Removed));
    }

    #[test]
    fn test_arrows_move_the_cursor_through_the_links() {
        let tmp = tempfile::TempDir::new().unwrap();
        let mut ctx = AppContext::for_tests(Config::default_empty(), tmp.path());
        let mut component = UnstowConfirmComponent::with_paths(
            vec![tmp.path().join("a"), tmp.path().join("b")],
            None,
        );
        let down = KeyEvent::new(KeyCode::Down, KeyModifiers::NONE);
        component.handle_key(down, &mut ctx);
        assert_eq!(component.list.cursor, 1);
    }
}
