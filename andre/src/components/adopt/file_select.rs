use std::path::PathBuf;

use crossterm::event::{KeyCode, KeyEvent};
use ratatui::{layout::Rect, widgets::ListState, Frame};

use crate::components::{AppContext, Component, Transition};
use crate::theme::ThemeColors;
use crate::ui::widgets::file_browser::FileBrowserWidget;

pub struct AdoptFileSelectComponent {
    pub group_name: String,
    pub browse_path: PathBuf,
    pub cursor: usize,
    pub selected_files: Vec<PathBuf>,
    cached_entries: Option<Vec<andre_core::TargetEntry>>,
    cached_path: PathBuf,
}

impl AdoptFileSelectComponent {
    pub fn new(group_name: String) -> Self {
        Self {
            group_name,
            browse_path: PathBuf::from("."),
            cursor: 0,
            selected_files: Vec::new(),
            cached_entries: None,
            cached_path: PathBuf::new(),
        }
    }

    pub fn with_path(group_name: String, browse_path: PathBuf) -> Self {
        Self {
            group_name,
            browse_path,
            cursor: 0,
            selected_files: Vec::new(),
            cached_entries: None,
            cached_path: PathBuf::new(),
        }
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
        if self.cached_path != self.browse_path || self.cached_entries.is_none() {
            self.cached_entries =
                Some(andre_core::adopt::list_target_entries(&self.browse_path).unwrap_or_default());
            self.cached_path = self.browse_path.clone();
        }
        Transition::None
    }

    fn handle_key(&mut self, key: KeyEvent, _ctx: &mut AppContext) -> Transition {
        let entries = self.cached_entries.as_deref().unwrap_or_default();
        let len = entries.len();
        let mut cursor = self.cursor;

        if crate::input::Input::handle_list_navigation(&key, &mut cursor, len) {
            // Skip non-selectable entries (symlinks, artifacts)
            for _ in 0..len {
                if let Some(entry) = entries.get(cursor) {
                    if entry.is_selectable() {
                        break;
                    }
                }
                if crate::input::Input::is_up(&key) || key.code == KeyCode::Char('k') {
                    cursor = (cursor + len - 1) % len;
                } else {
                    cursor = (cursor + 1) % len;
                }
            }
            self.cursor = cursor;
            return Transition::None;
        }

        if key.code == KeyCode::Right {
            if let Some(entry) = entries.get(cursor) {
                if entry.is_dir {
                    self.browse_path.push(&entry.name);
                    cursor = 0;
                }
            }
        } else if key.code == KeyCode::Enter || key.code == KeyCode::Tab {
            if !self.selected_files.is_empty() {
                return Transition::Push(Box::new(super::AdoptConfirmComponent {
                    group_name: self.group_name.clone(),
                    selected_files: self.selected_files.clone(),
                }));
            }
        } else if key.code == KeyCode::Char(' ') {
            if let Some(entry) = entries.get(cursor) {
                if entry.is_selectable() {
                    let path = self.browse_path.join(&entry.name);
                    if self.selected_files.contains(&path) {
                        self.selected_files.retain(|p| p != &path);
                    } else {
                        self.selected_files.push(path);
                    }
                }
            }
        } else if key.code == KeyCode::Left || key.code == KeyCode::Backspace {
            if self.browse_path.parent().is_some() {
                self.browse_path.pop();
                cursor = 0;
            }
        } else if key.code == KeyCode::Esc {
            return Transition::Pop;
        }

        self.cursor = cursor;
        Transition::None
    }

    fn render(&mut self, frame: &mut Frame, area: Rect, ctx: &AppContext) {
        let colors = ThemeColors::from_theme(ctx.core.theme);
        let mut state = ListState::default();
        let widget = FileBrowserWidget::new(
            &colors,
            &self.browse_path,
            self.cursor,
            "Select Files to Adopt",
        )
        .with_selected(&self.selected_files)
        .with_entries(self.cached_entries.as_deref().unwrap_or_default(), &[]);
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
        component.cursor = 1;
        (component, tmp)
    }

    fn left_key() -> KeyEvent {
        KeyEvent::new(KeyCode::Left, KeyModifiers::NONE)
    }

    #[test]
    fn test_cursor_resets_on_navigate_up() {
        let (mut component, _tmp) = make_test_component();
        let mut ctx = crate::components::AppContext {
            core: andre_core::AppState::from_config(
                andre_core::config::Config::default_empty(),
                false,
            ),
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
        assert_eq!(component.cursor, 1);

        let result = component.handle_key(left_key(), &mut ctx);
        assert!(matches!(result, Transition::None));
        assert_eq!(component.cursor, 0);
        assert_eq!(component.browse_path, _tmp.path());
    }
}
