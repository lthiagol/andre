use std::path::{Path, PathBuf};

use andre_core::TargetEntry;
use crossterm::event::KeyEvent;

use crate::input::{step_cursor, Input};

/// Lists one directory for a [`Browser`].
pub type ListFn = fn(&Path) -> Vec<TargetEntry>;

/// Every entry in a directory (the unstow and adopt browse screens).
pub fn all_entries(path: &Path) -> Vec<TargetEntry> {
    andre_core::adopt::list_target_entries(path).unwrap_or_default()
}

/// Only the directories in a directory (the settings directory picker).
pub fn dir_entries(path: &Path) -> Vec<TargetEntry> {
    andre_core::adopt::list_target_dirs(path).unwrap_or_default()
}

/// A directory listing with a cursor, shared by the browse screens and the
/// settings directory picker.
///
/// The listing is cached per path: [`Browser::refresh`] re-reads the directory
/// only when `path` changed since the last read, so callers do the I/O in
/// `update()` or `handle_key()` and `entries()` stays a plain read for paint.
#[derive(Debug, Clone)]
pub struct Browser {
    pub path: PathBuf,
    pub cursor: usize,
    entries: Vec<TargetEntry>,
    listed_path: Option<PathBuf>,
    list: ListFn,
}

impl Browser {
    /// A browser that has not listed its directory yet; the first `refresh` does.
    pub fn new(path: PathBuf, list: ListFn) -> Self {
        Self {
            path,
            cursor: 0,
            entries: Vec::new(),
            listed_path: None,
            list,
        }
    }

    /// A browser with its directory already listed, for dialogs that render without a tick.
    pub fn open(path: PathBuf, list: ListFn) -> Self {
        let mut browser = Self::new(path, list);
        browser.refresh();
        browser
    }

    /// Re-lists the directory if `path` changed since the last listing.
    /// Returns true when it re-listed.
    pub fn refresh(&mut self) -> bool {
        if self.listed_path.as_deref() == Some(self.path.as_path()) {
            return false;
        }
        self.entries = (self.list)(&self.path);
        self.listed_path = Some(self.path.clone());
        true
    }

    /// The last listing. Stale until the next [`Browser::refresh`].
    pub fn entries(&self) -> &[TargetEntry] {
        &self.entries
    }

    /// Up/Down (and `j`/`k`) move the cursor with wrap-around.
    /// Returns true when the key was a cursor key.
    pub fn move_cursor(&mut self, key: &KeyEvent) -> bool {
        Input::handle_list_navigation(key, &mut self.cursor, self.entries.len())
    }

    /// Moves the cursor one row down or up, wrapping at both ends.
    pub fn step(&mut self, down: bool) {
        self.cursor = step_cursor(self.cursor, self.entries.len(), down);
    }

    /// The entry under the cursor.
    pub fn current(&self) -> Option<&TargetEntry> {
        self.entries.get(self.cursor)
    }

    /// Right: descend into the entry under the cursor when it is a directory.
    pub fn enter_dir(&mut self) {
        let Some(entry) = self.entries.get(self.cursor).filter(|e| e.is_dir) else {
            return;
        };
        self.path.push(&entry.name);
        self.cursor = 0;
    }

    /// Left / Backspace: move to the parent directory, if there is one.
    pub fn go_up(&mut self) {
        if self.path.parent().is_some() {
            self.path.pop();
            self.cursor = 0;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crossterm::event::{KeyCode, KeyModifiers};

    fn sample_dir() -> (tempfile::TempDir, PathBuf) {
        let tmp = tempfile::TempDir::new().unwrap();
        std::fs::create_dir_all(tmp.path().join("subdir")).unwrap();
        std::fs::File::create(tmp.path().join("file1.txt")).unwrap();
        let root = tmp.path().to_path_buf();
        (tmp, root)
    }

    fn key(code: KeyCode) -> KeyEvent {
        KeyEvent::new(code, KeyModifiers::NONE)
    }

    #[test]
    fn test_refresh_lists_only_when_path_changes() {
        let (_tmp, root) = sample_dir();
        let mut browser = Browser::new(root.clone(), all_entries);

        assert!(browser.refresh());
        assert_eq!(browser.entries().len(), 2);
        assert!(!browser.refresh());

        browser.path = root.join("subdir");
        assert!(browser.refresh());
        assert!(browser.entries().is_empty());
    }

    #[test]
    fn test_dir_entries_lists_directories_only() {
        let (_tmp, root) = sample_dir();
        let mut browser = Browser::new(root, dir_entries);
        browser.refresh();

        let names: Vec<&str> = browser.entries().iter().map(|e| e.name.as_str()).collect();
        assert_eq!(names, vec!["subdir"]);
    }

    #[test]
    fn test_step_wraps_at_both_ends() {
        let (_tmp, root) = sample_dir();
        let mut browser = Browser::new(root, all_entries);
        browser.refresh();

        browser.step(false);
        assert_eq!(browser.cursor, 1);
        browser.step(true);
        assert_eq!(browser.cursor, 0);
    }

    #[test]
    fn test_move_cursor_ignores_non_cursor_keys_and_empty_listing() {
        let (_tmp, root) = sample_dir();
        let mut browser = Browser::new(root, all_entries);
        assert!(!browser.move_cursor(&key(KeyCode::Down)));

        browser.refresh();
        assert!(!browser.move_cursor(&key(KeyCode::Enter)));
        assert!(browser.move_cursor(&key(KeyCode::Down)));
        assert_eq!(browser.cursor, 1);
    }

    #[test]
    fn test_enter_dir_descends_and_resets_cursor() {
        let (_tmp, root) = sample_dir();
        let mut browser = Browser::new(root.clone(), all_entries);
        browser.refresh();
        browser.cursor = 1;
        browser.enter_dir();
        assert_eq!(browser.path, root.join("subdir"));
        assert_eq!(browser.cursor, 0);
    }

    #[test]
    fn test_enter_dir_on_file_is_noop() {
        let (_tmp, root) = sample_dir();
        let mut browser = Browser::new(root.clone(), all_entries);
        browser.refresh();
        browser.cursor = browser
            .entries()
            .iter()
            .position(|e| !e.is_dir)
            .expect("fixture has a file");
        browser.enter_dir();
        assert_eq!(browser.path, root);
    }
}
