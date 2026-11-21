use std::path::PathBuf;

use crossterm::event::{KeyCode, KeyEvent};
use ratatui::{layout::Rect, Frame};

use crate::components::{AppContext, Component, Transition};
use crate::theme::ThemeColors;
use crate::ui::UnstowLink;

pub struct UnstowConfirmComponent {
    pub link: Option<UnstowLink>,
    pub unlink_error: Option<String>,
    pub unlink_success: bool,
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
        if self.unlink_success || self.unlink_error.is_some() {
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
            Transition::None
        } else if key.code == KeyCode::Char('n')
            || key.code == KeyCode::Char('N')
            || key.code == KeyCode::Esc
        {
            Transition::Pop
        } else {
            Transition::None
        }
    }

    fn render(&mut self, frame: &mut Frame, area: Rect, ctx: &AppContext) {
        let colors = ThemeColors::from_theme(ctx.core.theme);
        let title = format!(" {} > Unlink ", ctx.breadcrumb);
        crate::ui::render_unstow_confirm(
            frame,
            area,
            &colors,
            &title,
            self.link.as_ref(),
            self.unlink_error.as_deref(),
            self.unlink_success,
        );
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
