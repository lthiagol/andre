use std::path::PathBuf;

use crossterm::event::{KeyCode, KeyEvent};
use ratatui::{layout::Rect, widgets::ListState, Frame};

use crate::components::{AppContext, Component, Transition};
use crate::theme::ThemeColors;
use crate::ui::widgets::file_browser::FileBrowserWidget;

pub struct UnstowBrowseComponent {
    pub group_name: String,
    pub browse_path: PathBuf,
    pub cursor: usize,
    pub selected_path: Option<PathBuf>,
    cached_entries: Option<Vec<andre_core::TargetEntry>>,
    cached_path: PathBuf,
    cached_symlinks: Option<Vec<(String, String)>>,
}

impl UnstowBrowseComponent {
    pub fn new(group_name: String) -> Self {
        Self {
            group_name,
            browse_path: PathBuf::from("."),
            cursor: 0,
            selected_path: None,
            cached_entries: None,
            cached_path: PathBuf::new(),
            cached_symlinks: None,
        }
    }

    pub fn with_path(group_name: String, browse_path: PathBuf) -> Self {
        Self {
            group_name,
            browse_path,
            cursor: 0,
            selected_path: None,
            cached_entries: None,
            cached_path: PathBuf::new(),
            cached_symlinks: None,
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
        if self.cached_path != self.browse_path || self.cached_entries.is_none() {
            let entries =
                andre_core::adopt::list_target_entries(&self.browse_path).unwrap_or_default();
            self.cached_symlinks = Some(
                entries
                    .iter()
                    .filter(|e| e.is_symlink)
                    .map(|e| {
                        let link = self.browse_path.join(&e.name);
                        let target = std::fs::read_link(&link)
                            .map(|p| p.display().to_string())
                            .unwrap_or_else(|_| "?".to_string());
                        (e.name.clone(), target)
                    })
                    .collect(),
            );
            self.cached_entries = Some(entries);
            self.cached_path = self.browse_path.clone();
        }
        Transition::None
    }

    fn handle_key(&mut self, key: KeyEvent, _ctx: &mut AppContext) -> Transition {
        let len = self.cached_entries.as_ref().map_or(0, |e| e.len());
        let mut cursor = self.cursor;

        if crate::input::Input::handle_list_navigation(&key, &mut cursor, len) {
            self.cursor = cursor;
            return Transition::None;
        }

        if key.code == KeyCode::Right {
            if let Some(entries) = &self.cached_entries {
                if let Some(entry) = entries.get(cursor) {
                    if entry.is_dir {
                        self.browse_path.push(&entry.name);
                        cursor = 0;
                    }
                }
            }
        } else if key.code == KeyCode::Enter {
            if let Some(entries) = &self.cached_entries {
                if let Some(entry) = entries.get(cursor) {
                    if entry.is_symlink {
                        let path = self.browse_path.join(&entry.name);
                        return Transition::Push(Box::new(
                            super::UnstowConfirmComponent::with_path(path),
                        ));
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
        let widget =
            FileBrowserWidget::new(&colors, &self.browse_path, self.cursor, "Unstow Symlinks")
                .unstow_mode()
                .with_entries(
                    self.cached_entries.as_deref().unwrap_or_default(),
                    self.cached_symlinks.as_deref().unwrap_or_default(),
                );
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
