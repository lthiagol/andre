use std::path::PathBuf;

use crossterm::event::{KeyCode, KeyEvent};
use ratatui::{layout::Rect, widgets::ListState, Frame};

use crate::components::browser::{all_entries, Browser};
use crate::components::{AppContext, Component, Transition};
use crate::input::{step_cursor, Input};
use crate::theme::ThemeColors;
use crate::ui::icons::Icons;
use crate::ui::widgets::file_browser::FileBrowserWidget;

pub struct AdoptFileSelectComponent {
    pub group_name: String,
    pub browser: Browser,
    pub selected_files: Vec<PathBuf>,
}

impl AdoptFileSelectComponent {
    pub fn new(group_name: String) -> Self {
        Self::with_path(group_name, PathBuf::from("."))
    }

    pub fn with_path(group_name: String, browse_path: PathBuf) -> Self {
        Self {
            group_name,
            browser: Browser::new(browse_path, all_entries),
            selected_files: Vec::new(),
        }
    }

    /// Up/Down move the cursor, skipping entries that cannot be adopted (symlinks, artifacts).
    fn move_cursor(&mut self, key: &KeyEvent) -> bool {
        let len = self.browser.entries().len();
        let mut cursor = self.browser.cursor;
        if !Input::handle_list_navigation(key, &mut cursor, len) {
            return false;
        }

        // Keep stepping in the same direction past rows that cannot be adopted.
        let down = Input::is_down(key);
        for _ in 0..len {
            if self
                .browser
                .entries()
                .get(cursor)
                .is_some_and(andre_core::TargetEntry::is_selectable)
            {
                break;
            }
            cursor = step_cursor(cursor, len, down);
        }
        self.browser.cursor = cursor;
        true
    }
}

impl Component for AdoptFileSelectComponent {
    fn id(&self) -> &'static str {
        "AdoptFileSelect"
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }

    fn update(&mut self, _ctx: &mut AppContext) -> Transition {
        self.browser.refresh();
        Transition::None
    }

    fn handle_key(&mut self, key: KeyEvent, _ctx: &mut AppContext) -> Transition {
        if self.move_cursor(&key) {
            return Transition::None;
        }

        match key.code {
            KeyCode::Right => self.browser.enter_dir(),
            KeyCode::Enter | KeyCode::Tab => {
                if !self.selected_files.is_empty() {
                    return Transition::Push(Box::new(super::AdoptConfirmComponent {
                        group_name: self.group_name.clone(),
                        selected_files: self.selected_files.clone(),
                    }));
                }
            }
            KeyCode::Char(' ') => {
                if let Some(entry) = self.browser.current().filter(|e| e.is_selectable()) {
                    let path = self.browser.path.join(&entry.name);
                    if self.selected_files.contains(&path) {
                        self.selected_files.retain(|p| p != &path);
                    } else {
                        self.selected_files.push(path);
                    }
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
            "select files to adopt",
        )
        .with_selected(&self.selected_files)
        .with_entries(self.browser.entries(), &[]);
        frame.render_stateful_widget(widget, area, &mut state);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

    fn make_test_component() -> (AdoptFileSelectComponent, tempfile::TempDir) {
        let tmp = tempfile::TempDir::new().unwrap();
        // Create a nested directory structure
        std::fs::create_dir_all(tmp.path().join("subdir")).unwrap();
        std::fs::File::create(tmp.path().join("file1.txt")).unwrap();
        std::fs::File::create(tmp.path().join("subdir/file2.txt")).unwrap();

        let mut component =
            AdoptFileSelectComponent::with_path("test".to_string(), tmp.path().join("subdir"));
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
