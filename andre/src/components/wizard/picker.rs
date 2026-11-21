//! The live-file picker of step 2: one folder listing with marks, opened at the target folder.
//! Symlinks and artifacts are listed but cannot be marked, since a move cannot take them.
//! A marked folder takes its contents with it, so no mark sits inside another.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use andre_core::TargetEntry;

use crate::components::browser::{all_entries, Browser};
use crate::ui::list_panel::ListView;

pub struct FilePicker {
    /// The folder being browsed. Its cursor is kept at zero; `list` holds the real one.
    pub browser: Browser,
    /// The cursor and the window over the entries.
    pub list: ListView,
    /// Absolute paths of the marked entries, in the order they were marked.
    pub marks: Vec<PathBuf>,
    /// Where each symlink of the listing points, by entry name. Read with the listing.
    links: BTreeMap<String, PathBuf>,
    /// The size of the file under the cursor, with its path. Read when the cursor lands on it.
    size: Option<(PathBuf, u64)>,
    /// The folder the picker opened at. Moves are shown relative to it.
    root: PathBuf,
}

/// Why an entry cannot be moved into a package: symlinks and artifacts are listed only.
pub fn refusal(entry: &TargetEntry) -> String {
    if entry.is_symlink {
        "symlinks cannot be moved in".to_string()
    } else {
        format!("{} cannot be moved in", entry.name)
    }
}

impl FilePicker {
    pub fn new(root: PathBuf) -> Self {
        Self {
            browser: Browser::new(root.clone(), all_entries),
            list: ListView::default(),
            marks: Vec::new(),
            links: BTreeMap::new(),
            size: None,
            root,
        }
    }

    /// Re-lists the folder when it changed, and reads the targets of its symlinks with it.
    /// Reads the disk, so only update and keys call it.
    pub fn refresh(&mut self) {
        if !self.browser.refresh() {
            return;
        }
        self.links = self
            .browser
            .entries()
            .iter()
            .filter(|entry| entry.is_symlink)
            .filter_map(|entry| {
                let target = std::fs::read_link(self.browser.path.join(&entry.name)).ok()?;
                Some((entry.name.clone(), target))
            })
            .collect();
    }

    /// Stats the file under the cursor, once per cursor move. Reads the disk, so only update
    /// and keys call it.
    pub fn read_current(&mut self) {
        let Some(entry) = self.current() else {
            return;
        };
        if entry.is_dir || !entry.is_selectable() || self.current_size().is_some() {
            return;
        }
        let path = self.browser.path.join(&entry.name);
        self.size = std::fs::metadata(&path)
            .ok()
            .map(|meta| (path.clone(), meta.len()));
    }

    pub fn entries(&self) -> &[TargetEntry] {
        self.browser.entries()
    }

    /// The entry under the cursor.
    pub fn current(&self) -> Option<&TargetEntry> {
        self.browser.entries().get(self.list.cursor)
    }

    /// The size in bytes of the file under the cursor, once [`FilePicker::read_current`] has
    /// read it for that path.
    pub fn current_size(&self) -> Option<u64> {
        let entry = self.current()?;
        let path = self.browser.path.join(&entry.name);
        self.size
            .as_ref()
            .filter(|(read, _)| read == &path)
            .map(|(_, len)| *len)
    }

    /// Where a symlink of the listing points.
    pub fn link_target(&self, entry: &TargetEntry) -> Option<&PathBuf> {
        self.links.get(&entry.name)
    }

    pub fn is_marked(&self, entry: &TargetEntry) -> bool {
        self.marks.contains(&self.browser.path.join(&entry.name))
    }

    /// The marked folder that holds the folder being browsed, if there is one.
    pub fn covering_mark(&self) -> Option<&PathBuf> {
        self.marks
            .iter()
            .find(|mark| self.browser.path.starts_with(mark))
    }

    /// Whether the folder being browsed sits inside a marked folder, so its entries are
    /// marked with it and cannot be marked themselves.
    pub fn is_covered(&self) -> bool {
        self.covering_mark().is_some()
    }

    /// The folder inside the target, as `.config/nvim`. Empty at the target itself.
    pub fn relative_dir(&self) -> String {
        self.browser
            .path
            .strip_prefix(&self.root)
            .map(|rel| rel.display().to_string())
            .unwrap_or_default()
    }

    /// Right: opens the folder under the cursor. Files and symlinks do nothing.
    pub fn open_dir(&mut self) {
        if !self.current().is_some_and(|entry| entry.is_dir) {
            return;
        }
        self.browser.cursor = self.list.cursor;
        self.browser.enter_dir();
        self.reset_cursor();
        self.refresh();
    }

    /// Left: back to the parent folder, never above the target.
    pub fn go_up(&mut self) {
        if self.browser.path == self.root {
            return;
        }
        self.browser.go_up();
        self.reset_cursor();
        self.refresh();
    }

    /// Space: marks or unmarks the entry under the cursor. An entry that cannot be moved
    /// in, or that sits in a marked folder, is refused with the reason. Marking a folder
    /// drops the marks already made inside it.
    pub fn toggle_mark(&mut self) -> Result<(), String> {
        let Some(entry) = self.current() else {
            return Ok(());
        };
        if !entry.is_selectable() {
            return Err(refusal(entry));
        }
        if let Some(mark) = self.covering_mark() {
            return Err(format!("{} is already marked", file_name(mark)));
        }
        let is_dir = entry.is_dir;
        let path = self.browser.path.join(&entry.name);
        match self.marks.iter().position(|marked| marked == &path) {
            Some(at) => {
                self.marks.remove(at);
            }
            None => {
                if is_dir {
                    self.marks.retain(|marked| !marked.starts_with(&path));
                }
                self.marks.push(path);
            }
        }
        Ok(())
    }

    fn reset_cursor(&mut self) {
        self.list.cursor = 0;
        self.list.offset = 0;
    }
}

/// The last component of a path, as text.
fn file_name(path: &Path) -> String {
    path.file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    /// `root/` with a folder `nvim/` holding `init.lua`, and a file `notes.txt`.
    fn fixture() -> (FilePicker, TempDir) {
        let dir = TempDir::new().unwrap();
        let root = dir.path().to_path_buf();
        std::fs::create_dir_all(root.join("nvim")).unwrap();
        std::fs::write(root.join("nvim/init.lua"), "").unwrap();
        std::fs::write(root.join("notes.txt"), "").unwrap();
        let mut picker = FilePicker::new(root);
        picker.refresh();
        (picker, dir)
    }

    fn cursor_on(picker: &mut FilePicker, name: &str) {
        picker.list.cursor = picker
            .entries()
            .iter()
            .position(|entry| entry.name == name)
            .unwrap();
    }

    #[test]
    fn test_open_dir_and_go_up_round_trip_and_reset_the_cursor() {
        let (mut picker, _dir) = fixture();
        let root = picker.browser.path.clone();
        cursor_on(&mut picker, "nvim");

        picker.open_dir();
        assert_eq!(picker.browser.path, root.join("nvim"));
        assert_eq!(picker.list.cursor, 0);
        assert_eq!(picker.relative_dir(), "nvim");

        picker.go_up();
        assert_eq!(picker.browser.path, root);
        assert_eq!(picker.relative_dir(), "");
    }

    #[test]
    fn test_open_dir_on_a_file_stays_put() {
        let (mut picker, _dir) = fixture();
        let root = picker.browser.path.clone();
        cursor_on(&mut picker, "notes.txt");

        picker.open_dir();
        assert_eq!(picker.browser.path, root);
    }

    #[test]
    fn test_go_up_stops_at_the_target() {
        let (mut picker, _dir) = fixture();
        let root = picker.browser.path.clone();

        picker.go_up();
        assert_eq!(picker.browser.path, root);
    }

    #[test]
    fn test_toggle_mark_marks_then_unmarks_a_file() {
        let (mut picker, _dir) = fixture();
        cursor_on(&mut picker, "notes.txt");
        let entry = picker.current().unwrap().clone();

        assert_eq!(picker.toggle_mark(), Ok(()));
        assert!(picker.is_marked(&entry));
        assert_eq!(picker.toggle_mark(), Ok(()));
        assert!(!picker.is_marked(&entry));
        assert!(picker.marks.is_empty());
    }

    #[test]
    fn test_a_marked_folder_cannot_have_marked_contents() {
        let (mut picker, _dir) = fixture();
        let root = picker.browser.path.clone();
        cursor_on(&mut picker, "nvim");
        assert_eq!(picker.toggle_mark(), Ok(()));
        picker.open_dir();
        cursor_on(&mut picker, "init.lua");

        assert_eq!(
            picker.toggle_mark(),
            Err("nvim is already marked".to_string())
        );
        assert!(picker.is_covered());
        assert_eq!(picker.marks, vec![root.join("nvim")]);
    }

    #[test]
    fn test_marking_a_folder_drops_the_marks_already_inside_it() {
        let (mut picker, _dir) = fixture();
        let root = picker.browser.path.clone();
        cursor_on(&mut picker, "nvim");
        picker.open_dir();
        cursor_on(&mut picker, "init.lua");
        assert_eq!(picker.toggle_mark(), Ok(()));
        assert_eq!(picker.marks, vec![root.join("nvim/init.lua")]);

        picker.go_up();
        cursor_on(&mut picker, "nvim");
        assert_eq!(picker.toggle_mark(), Ok(()));

        assert_eq!(picker.marks, vec![root.join("nvim")]);
        assert!(!picker.is_covered());
    }

    #[test]
    fn test_current_size_is_read_for_the_file_under_the_cursor() {
        let (mut picker, _dir) = fixture();
        let root = picker.browser.path.clone();
        std::fs::write(root.join("notes.txt"), vec![0u8; 2150]).unwrap();
        cursor_on(&mut picker, "notes.txt");
        assert_eq!(picker.current_size(), None);

        picker.read_current();
        assert_eq!(picker.current_size(), Some(2150));

        cursor_on(&mut picker, "nvim");
        assert_eq!(picker.current_size(), None);
    }

    #[cfg(unix)]
    #[test]
    fn test_symlink_is_listed_but_cannot_be_marked() {
        let dir = TempDir::new().unwrap();
        std::fs::write(dir.path().join("notes.txt"), "").unwrap();
        std::os::unix::fs::symlink(dir.path().join("notes.txt"), dir.path().join("spell")).unwrap();
        let mut picker = FilePicker::new(dir.path().to_path_buf());
        picker.refresh();
        cursor_on(&mut picker, "spell");

        assert_eq!(
            picker.toggle_mark(),
            Err("symlinks cannot be moved in".to_string())
        );
        assert!(picker.marks.is_empty());
    }

    #[cfg(unix)]
    #[test]
    fn test_symlink_target_is_read_with_the_listing() {
        let dir = TempDir::new().unwrap();
        let target = dir.path().join("notes.txt");
        std::fs::write(&target, "").unwrap();
        std::os::unix::fs::symlink(&target, dir.path().join("spell")).unwrap();
        let mut picker = FilePicker::new(dir.path().to_path_buf());
        picker.refresh();
        cursor_on(&mut picker, "spell");
        let spell = picker.current().unwrap().clone();
        cursor_on(&mut picker, "notes.txt");
        let notes = picker.current().unwrap().clone();

        assert_eq!(picker.link_target(&spell), Some(&target));
        assert_eq!(picker.link_target(&notes), None);
    }
}
