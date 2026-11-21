use std::path::PathBuf;

use crossterm::event::{KeyCode, KeyEvent};
use ratatui::{layout::Rect, widgets::ListState, Frame};

use crate::components::browser::{all_entries, Browser};
use crate::components::{AppContext, Component, Transition};
use crate::theme::ThemeColors;
use crate::ui::icons::Icons;
use crate::ui::widgets::file_browser::FileBrowserWidget;

pub struct UnstowBrowseComponent {
    pub group_name: String,
    pub browser: Browser,
    cached_symlinks: Vec<(String, String)>,
}

impl UnstowBrowseComponent {
    pub fn new(group_name: String) -> Self {
        Self::with_path(group_name, PathBuf::from("."))
    }

    pub fn with_path(group_name: String, browse_path: PathBuf) -> Self {
        Self {
            group_name,
            browser: Browser::new(browse_path, all_entries),
            cached_symlinks: Vec::new(),
        }
    }
}

impl Component for UnstowBrowseComponent {
    fn id(&self) -> &'static str {
        "UnstowBrowse"
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }

    fn update(&mut self, _ctx: &mut AppContext) -> Transition {
        if self.browser.refresh() {
            self.cached_symlinks = self
                .browser
                .entries()
                .iter()
                .filter(|e| e.is_symlink)
                .map(|e| {
                    let link = self.browser.path.join(&e.name);
                    let target = std::fs::read_link(&link)
                        .map(|p| p.display().to_string())
                        .unwrap_or_else(|_| "?".to_string());
                    (e.name.clone(), target)
                })
                .collect();
        }
        Transition::None
    }

    fn handle_key(&mut self, key: KeyEvent, _ctx: &mut AppContext) -> Transition {
        if self.browser.move_cursor(&key) {
            return Transition::None;
        }

        match key.code {
            KeyCode::Right => self.browser.enter_dir(),
            KeyCode::Enter => {
                if let Some(entry) = self.browser.current().filter(|e| e.is_symlink) {
                    let path = self.browser.path.join(&entry.name);
                    return Transition::Push(Box::new(super::UnstowConfirmComponent::with_path(
                        path,
                    )));
                }
            }
            KeyCode::Left | KeyCode::Backspace => self.browser.go_up(),
            KeyCode::Esc => return Transition::Pop,
            _ => {}
        }
        Transition::None
    }

    fn render(&mut self, frame: &mut Frame, area: Rect, ctx: &AppContext) {
        let colors = ThemeColors::from_theme(ctx.core.theme);
        let mut state = ListState::default();
        let widget = FileBrowserWidget::new(
            &colors,
            Icons::for_set(ctx.core.icons),
            &self.browser.path,
            self.browser.cursor,
            "unstow symlinks",
        )
        .unstow_mode()
        .with_entries(self.browser.entries(), &self.cached_symlinks);
        frame.render_stateful_widget(widget, area, &mut state);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

    fn make_test_component() -> (UnstowBrowseComponent, tempfile::TempDir) {
        let tmp = tempfile::TempDir::new().unwrap();
        std::fs::create_dir_all(tmp.path().join("subdir")).unwrap();
        std::fs::File::create(tmp.path().join("file1.txt")).unwrap();
        std::fs::File::create(tmp.path().join("subdir/file2.txt")).unwrap();

        let mut component =
            UnstowBrowseComponent::with_path("test".to_string(), tmp.path().join("subdir"));
        component.browser.cursor = 1;
        (component, tmp)
    }

    fn left_key() -> KeyEvent {
        KeyEvent::new(KeyCode::Left, KeyModifiers::NONE)
    }

    #[test]
    fn test_cursor_resets_on_navigate_up() {
        let (mut component, _tmp) = make_test_component();
        let mut ctx = crate::components::AppContext {
            core: andre_core::AppState::from_config(andre_core::config::Config::default_empty()),
            config_dir: std::path::PathBuf::from("."),
            config_path: std::path::PathBuf::from("."),
            home_dir: std::path::PathBuf::from("."),
            status_cache: std::cell::RefCell::new(crate::status_cache::StatusCache::new()),
            terminal_size: (80, 24),
            debug: false,
            log_file: None,
            event_log: std::cell::RefCell::new(Vec::new()),
            toast: None,
            breadcrumb: String::from("Main"),
            breadcrumb_cache: Vec::new(),
        };

        component.update(&mut ctx);
        assert_eq!(component.browser.cursor, 1);

        let result = component.handle_key(left_key(), &mut ctx);
        assert!(matches!(result, Transition::None));
        assert_eq!(component.browser.cursor, 0);
        assert_eq!(component.browser.path, _tmp.path());
    }
}
