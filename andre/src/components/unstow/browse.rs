//! The unstow browse screen: the folders and links of one group's target. Space marks links,
//! Enter reviews the marked links (or the link under the cursor) for removal. Regular files
//! are not listed, since they are not stowed links.

use std::borrow::Cow;
use std::collections::BTreeMap;
use std::path::{Component as PathPart, Path, PathBuf};

use andre_core::{TargetEntry, Theme};
use crossterm::event::{KeyCode, KeyEvent};
use ratatui::{
    layout::Rect,
    style::Style,
    text::{Line, Span},
    Frame,
};
use unicode_width::{UnicodeWidthChar, UnicodeWidthStr};

use crate::components::browser::{all_entries, Browser};
use crate::components::{AppContext, Component, Transition};
use crate::input::Input;
use crate::theme::ThemeColors;
use crate::ui::detail::{render_detail, Detail, VALUE_WIDTH};
use crate::ui::display_path;
use crate::ui::footer::{unstow_browse_hints, Hint};
use crate::ui::icons::Icons;
use crate::ui::layout::{screen_rects, Shape};
use crate::ui::list_panel::{self, render_list_panel, Empty, ListView};
use crate::ui::selection::row_with_glyphs;

pub struct UnstowBrowseComponent {
    pub group_name: String,
    /// The folder being browsed and its listing. Its cursor is not used; `list` holds it.
    pub browser: Browser,
    /// The cursor and the window over the listed entries.
    pub list: ListView,
    /// The links marked for removal, in the order they were marked. Marks survive a
    /// move to another folder.
    marks: Vec<PathBuf>,
    /// Each listed symlink's raw target, by entry name. Read with the listing.
    targets: BTreeMap<String, String>,
    /// The cursor's detail. Built by `sync`.
    detail: Detail,
    /// The folder, cursor and theme the detail was built for. The detail bakes the theme's
    /// colours into its section lines, so a theme change rebuilds it.
    detail_key: Option<(PathBuf, usize, Theme)>,
}

impl UnstowBrowseComponent {
    pub fn with_path(group_name: String, browse_path: PathBuf) -> Self {
        Self {
            group_name,
            browser: Browser::new(browse_path, linked_entries),
            list: ListView::default(),
            marks: Vec::new(),
            targets: BTreeMap::new(),
            detail: Detail::default(),
            detail_key: None,
        }
    }

    /// The entry under the cursor.
    fn current(&self) -> Option<&TargetEntry> {
        self.browser.entries().get(self.list.cursor)
    }

    /// Re-lists the folder when it changed, and reads the targets of its symlinks with it.
    /// Reads the disk, so only update and keys call it.
    fn refresh(&mut self) {
        if !self.browser.refresh() {
            return;
        }
        self.targets = self
            .browser
            .entries()
            .iter()
            .filter(|entry| entry.is_symlink)
            .map(|entry| {
                let target = std::fs::read_link(self.browser.path.join(&entry.name))
                    .map(|p| p.display().to_string())
                    .unwrap_or_else(|_| "?".to_string());
                (entry.name.clone(), target)
            })
            .collect();
    }

    /// Brings the listing and the detail up to date.
    fn sync(&mut self, ctx: &AppContext) {
        self.refresh();
        let key = (self.browser.path.clone(), self.list.cursor, ctx.core.theme);
        if self.detail_key.as_ref() == Some(&key) {
            return;
        }
        self.detail = self.build_detail(ctx);
        self.detail_key = Some(key);
    }

    fn build_detail(&self, ctx: &AppContext) -> Detail {
        let colors = ThemeColors::from_theme(ctx.core.theme);
        let home = &ctx.home_dir;
        let Some(entry) = self.current() else {
            return Detail::default();
        };
        let path = self.browser.path.join(&entry.name);
        if !entry.is_symlink {
            // The links inside this folder, not the ones in the listing above it.
            let links = linked_entries(&path)
                .iter()
                .filter(|e| e.is_symlink)
                .count();
            return Detail {
                headline: Vec::new(),
                facts: vec![
                    (Cow::Borrowed("folder"), display_path(&path, home)),
                    (Cow::Borrowed("links"), links.to_string()),
                ],
                sections: vec![Line::styled(
                    "Open it with \u{2192} to see the links inside.",
                    Style::default().fg(colors.text_dim),
                )],
            };
        }

        let target = self.targets.get(&entry.name).map(String::as_str);
        let resolved = target.map(|target| resolve_link(&path, target));
        let mut facts = vec![(Cow::Borrowed("link"), display_path(&path, home))];
        let points_to = match &resolved {
            Some(at) => display_path(at, home),
            None => "(unresolvable)".to_string(),
        };
        // The label sits on the first row; the rest of the target wraps under the value.
        for (i, row) in wrap_value(&points_to, VALUE_WIDTH).into_iter().enumerate() {
            let label = if i == 0 { "points to" } else { "" };
            facts.push((Cow::Borrowed(label), row));
        }
        if let Some(package) = resolved.as_deref().and_then(|at| self.owner(ctx, at)) {
            facts.push((Cow::Borrowed("package"), package));
        }
        Detail {
            headline: Vec::new(),
            facts,
            sections: vec![Line::styled(
                "Unlinking leaves the package file in place; only the link goes.",
                Style::default().fg(colors.text_dim),
            )],
        }
    }

    /// The `group/package` a resolved link points into, when it points into this group's
    /// source folder.
    fn owner(&self, ctx: &AppContext, resolved: &Path) -> Option<String> {
        let source = ctx.core.config.resolve_group_source(
            &self.group_name,
            &ctx.config_dir,
            &ctx.home_dir,
        )?;
        let inside = resolved.strip_prefix(normalize(&source)).ok()?;
        let package = inside.components().next()?;
        Some(format!(
            "{}/{}",
            self.group_name,
            package.as_os_str().to_string_lossy()
        ))
    }

    fn reset_cursor(&mut self) {
        self.list.cursor = 0;
        self.list.offset = 0;
    }

    /// Right: opens the folder under the cursor. Links do nothing.
    fn open_dir(&mut self) {
        let Some(name) = self.current().filter(|e| e.is_dir).map(|e| e.name.clone()) else {
            return;
        };
        self.browser.path.push(name);
        self.reset_cursor();
        self.refresh();
    }

    /// Left: back to the parent folder.
    fn go_up(&mut self) {
        self.browser.go_up();
        self.reset_cursor();
        self.refresh();
    }

    /// Space: marks or unmarks the link under the cursor. A folder cannot be marked, so
    /// it is refused with the reason.
    fn toggle_mark(&mut self) -> Result<(), String> {
        let Some(entry) = self.current() else {
            return Ok(());
        };
        if !entry.is_symlink {
            return Err(format!(
                "{} is not a link; only links can be removed",
                entry.name
            ));
        }
        let path = self.browser.path.join(&entry.name);
        match self.marks.iter().position(|marked| marked == &path) {
            Some(at) => {
                self.marks.remove(at);
            }
            None => self.marks.push(path),
        }
        Ok(())
    }

    /// Enter: reviews the marked links, or the link under the cursor when none is marked.
    fn review(&self) -> Transition {
        let paths = if self.marks.is_empty() {
            match self.current().filter(|e| e.is_symlink) {
                Some(entry) => vec![self.browser.path.join(&entry.name)],
                None => return Transition::None,
            }
        } else {
            self.marks.clone()
        };
        Transition::Push(Box::new(super::UnstowConfirmComponent::with_paths(paths)))
    }

    fn list_title(&self, home: &Path) -> String {
        let path = display_path(&self.browser.path, home);
        match self.marks.len() {
            0 => path,
            marked => format!("{path}  \u{b7} {marked} marked"),
        }
    }

    /// One row: a checkbox on links, the kind glyph, then the name.
    fn row(
        &self,
        entry: &TargetEntry,
        selected: bool,
        colors: &ThemeColors,
        icons: &Icons,
        width: u16,
    ) -> Line<'static> {
        let marked = entry.is_symlink && self.marks.contains(&self.browser.path.join(&entry.name));
        let mark = if entry.is_symlink {
            let fg = if marked {
                colors.accent
            } else {
                colors.text_dim
            };
            Span::styled(icons.checkbox(marked), Style::default().fg(fg))
        } else {
            Span::raw(" ")
        };
        let kind = Span::styled(entry.kind_icon(), Style::default().fg(colors.text_dim));
        row_with_glyphs(
            vec![mark, Span::raw(" "), kind],
            vec![Span::styled(
                format!(" {}", entry.name),
                Style::default().fg(colors.text),
            )],
            selected,
            colors,
            width,
        )
    }

    /// The key-driven part of `handle_key`, between two syncs.
    fn act(&mut self, key: KeyEvent, ctx: &mut AppContext) -> Transition {
        if Input::handle_list_navigation(&key, &mut self.list.cursor, self.browser.entries().len())
        {
            return Transition::None;
        }
        match key.code {
            KeyCode::Char(' ') => {
                if let Err(message) = self.toggle_mark() {
                    ctx.show_error_toast(message);
                }
            }
            KeyCode::Right => self.open_dir(),
            KeyCode::Left | KeyCode::Backspace => self.go_up(),
            KeyCode::Enter => return self.review(),
            KeyCode::Esc => return Transition::Pop,
            _ => return Transition::None,
        }
        Transition::Handled
    }
}

/// The entries the screen lists: folders and links. Regular files and artifacts cannot be
/// unstowed, so they are left out.
fn linked_entries(path: &Path) -> Vec<TargetEntry> {
    all_entries(path)
        .into_iter()
        .filter(|entry| !entry.is_artifact && (entry.is_dir || entry.is_symlink))
        .collect()
}

/// Where a link points, as an absolute path. A relative target is taken from the link's
/// folder. Nothing is read from the disk.
fn resolve_link(link: &Path, target: &str) -> PathBuf {
    let target = Path::new(target);
    if target.is_absolute() {
        return normalize(target);
    }
    match link.parent() {
        Some(folder) => normalize(&folder.join(target)),
        None => normalize(target),
    }
}

/// `text` split into rows of at most `width` columns. A row breaks after a `/` where it can,
/// so a path wraps between its folders; a piece wider than a row breaks inside itself.
fn wrap_value(text: &str, width: usize) -> Vec<String> {
    let mut rows = Vec::new();
    let mut row = String::new();
    for piece in text.split_inclusive('/') {
        if row.width() + piece.width() <= width {
            row.push_str(piece);
            continue;
        }
        if !row.is_empty() {
            rows.push(std::mem::take(&mut row));
        }
        for ch in piece.chars() {
            if !row.is_empty() && row.width() + ch.width().unwrap_or(0) > width {
                rows.push(std::mem::take(&mut row));
            }
            row.push(ch);
        }
    }
    rows.push(row);
    rows
}

/// `path` with `.` dropped and each `..` taken from the part before it. Nothing is read
/// from the disk, so a link through a symlinked folder keeps its written path.
fn normalize(path: &Path) -> PathBuf {
    let mut out = PathBuf::new();
    for part in path.components() {
        match part {
            PathPart::ParentDir => {
                out.pop();
            }
            PathPart::CurDir => {}
            other => out.push(other.as_os_str()),
        }
    }
    out
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

    fn update(&mut self, ctx: &mut AppContext) -> Transition {
        self.sync(ctx);
        Transition::None
    }

    fn handle_key(&mut self, key: KeyEvent, ctx: &mut AppContext) -> Transition {
        // Keys can arrive before the next update, so the listing is brought up to date first.
        self.sync(ctx);
        let transition = self.act(key, ctx);
        self.sync(ctx);
        transition
    }

    fn hints(&self, _ctx: &AppContext) -> Vec<Hint> {
        unstow_browse_hints(self.marks.len())
    }

    fn prepare(&mut self, area: Rect) {
        let rects = screen_rects(area, Shape::TwoPane);
        self.list.prepare(rects.list, self.browser.entries().len());
    }

    fn render(&mut self, frame: &mut Frame, area: Rect, ctx: &AppContext) {
        let colors = ThemeColors::from_theme(ctx.core.theme);
        let icons = Icons::for_set(ctx.core.icons);
        let rects = screen_rects(area, Shape::TwoPane);

        let width = list_panel::inner(rects.list).width;
        let rows: Vec<Line> = self
            .browser
            .entries()
            .iter()
            .enumerate()
            .map(|(i, entry)| self.row(entry, i == self.list.cursor, &colors, icons, width))
            .collect();
        let empty = Empty {
            message: "no links or folders here",
            hint: None,
        };
        render_list_panel(
            frame,
            rects.list,
            &colors,
            &self.list_title(&ctx.home_dir),
            rows,
            &self.list,
            Some(empty),
        );

        let title = self
            .current()
            .map_or_else(|| "links".to_string(), |entry| entry.name.clone());
        render_detail(frame, rects.detail, &colors, &title, &self.detail);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use andre_core::config::Config;
    use crossterm::event::{KeyCode, KeyModifiers};

    /// A home folder with a `home` group whose package `shell` holds `.bashrc`, a folder
    /// `.config`, a regular file `notes.txt` and a link `~/.bashrc` that points into the
    /// package by a relative path.
    fn fixture() -> (tempfile::TempDir, AppContext) {
        let tmp = tempfile::TempDir::new().unwrap();
        let home = tmp.path();
        std::fs::write(
            home.join("andre.yml"),
            "global: {}\ngroups:\n  home:\n    source: dotfiles/home\n    target: \"~\"\n",
        )
        .unwrap();
        let package = home.join("dotfiles/home/shell");
        std::fs::create_dir_all(&package).unwrap();
        std::fs::write(package.join(".bashrc"), "alias l=ls").unwrap();
        std::fs::create_dir_all(home.join(".config")).unwrap();
        std::fs::write(home.join("notes.txt"), "").unwrap();
        std::os::unix::fs::symlink("dotfiles/home/shell/.bashrc", home.join(".bashrc")).unwrap();
        let config = Config::load(&home.join("andre.yml")).unwrap();
        let ctx = AppContext::for_tests(config, home);
        (tmp, ctx)
    }

    fn browse(home: &Path) -> UnstowBrowseComponent {
        UnstowBrowseComponent::with_path("home".to_string(), home.to_path_buf())
    }

    fn key(code: KeyCode) -> KeyEvent {
        KeyEvent::new(code, KeyModifiers::NONE)
    }

    /// The cursor on the entry called `name`, after a sync.
    fn select(component: &mut UnstowBrowseComponent, ctx: &AppContext, name: &str) {
        component.sync(ctx);
        component.list.cursor = component
            .browser
            .entries()
            .iter()
            .position(|e| e.name == name)
            .expect("entry listed");
        component.sync(ctx);
    }

    #[test]
    fn test_listing_shows_folders_and_links_but_not_regular_files() {
        let (_tmp, ctx) = fixture();
        let mut component = browse(&ctx.home_dir);
        component.sync(&ctx);
        let names: Vec<&str> = component
            .browser
            .entries()
            .iter()
            .map(|e| e.name.as_str())
            .collect();
        assert!(names.contains(&".config"));
        assert!(names.contains(&".bashrc"));
        assert!(
            !names.contains(&"notes.txt"),
            "regular files are not listed"
        );
    }

    #[test]
    fn test_left_goes_up_and_resets_the_cursor() {
        let (_tmp, mut ctx) = fixture();
        let mut component = browse(&ctx.home_dir.join(".config"));
        component.list.cursor = 1;
        let transition = component.handle_key(key(KeyCode::Left), &mut ctx);
        assert!(matches!(transition, Transition::Handled));
        assert_eq!(component.browser.path, ctx.home_dir);
        assert_eq!(component.list.cursor, 0);
    }

    #[test]
    fn test_space_marks_a_link_and_a_second_space_unmarks_it() {
        let (_tmp, ctx) = fixture();
        let mut component = browse(&ctx.home_dir);
        select(&mut component, &ctx, ".bashrc");
        component.toggle_mark().unwrap();
        assert_eq!(component.marks, vec![ctx.home_dir.join(".bashrc")]);
        component.toggle_mark().unwrap();
        assert!(component.marks.is_empty());
    }

    #[test]
    fn test_space_on_a_folder_is_refused_with_the_reason() {
        let (_tmp, ctx) = fixture();
        let mut component = browse(&ctx.home_dir);
        select(&mut component, &ctx, ".config");
        let message = component.toggle_mark().unwrap_err();
        assert_eq!(message, ".config is not a link; only links can be removed");
        assert!(component.marks.is_empty());
    }

    #[test]
    fn test_enter_reviews_the_marks_then_the_cursor_link_then_nothing() {
        let (_tmp, ctx) = fixture();
        let mut component = browse(&ctx.home_dir);
        select(&mut component, &ctx, ".config");
        assert!(matches!(component.review(), Transition::None));

        select(&mut component, &ctx, ".bashrc");
        assert!(matches!(component.review(), Transition::Push(_)));

        component.toggle_mark().unwrap();
        assert!(matches!(component.review(), Transition::Push(_)));
        assert_eq!(component.hints(&ctx).len(), 5);
    }

    #[test]
    fn test_detail_names_the_package_a_relative_link_points_into() {
        let (_tmp, ctx) = fixture();
        let mut component = browse(&ctx.home_dir);
        select(&mut component, &ctx, ".bashrc");
        let facts: Vec<(&str, &str)> = component
            .detail
            .facts
            .iter()
            .map(|(label, value)| (label.as_ref(), value.as_str()))
            .collect();
        assert_eq!(
            facts,
            [
                ("link", "~/.bashrc"),
                ("points to", "~/dotfiles/home/shell/"),
                ("", ".bashrc"),
                ("package", "home/shell"),
            ]
        );
    }

    #[test]
    fn test_wrap_value_breaks_after_a_slash_then_inside_a_long_piece() {
        assert_eq!(
            wrap_value("~/dotfiles/home/shell/.bashrc", 27),
            ["~/dotfiles/home/shell/", ".bashrc"]
        );
        assert_eq!(wrap_value("~/.bashrc", 27), ["~/.bashrc"]);
        assert_eq!(wrap_value("abcdefghij", 4), ["abcd", "efgh", "ij"]);
        assert_eq!(wrap_value("", 4), [""]);
    }

    #[test]
    fn test_folder_detail_counts_the_links_inside_it_not_the_listing() {
        let (_tmp, ctx) = fixture();
        std::os::unix::fs::symlink(
            "dotfiles/home/shell/.bashrc",
            ctx.home_dir.join(".config/init.lua"),
        )
        .unwrap();
        std::os::unix::fs::symlink("dotfiles/home/shell/.bashrc", ctx.home_dir.join(".profile"))
            .unwrap();
        let mut component = browse(&ctx.home_dir);
        select(&mut component, &ctx, ".config");
        let links = component
            .detail
            .facts
            .iter()
            .find(|(label, _)| label.as_ref() == "links")
            .map(|(_, value)| value.as_str());
        assert_eq!(links, Some("1"));
    }

    #[test]
    fn test_theme_change_rebuilds_the_detail_colours() {
        let (_tmp, mut ctx) = fixture();
        let mut component = browse(&ctx.home_dir);
        select(&mut component, &ctx, ".bashrc");
        let before = ThemeColors::from_theme(ctx.core.theme).text_dim;

        ctx.core.theme = ctx.core.theme.next();
        component.sync(&ctx);
        let after = ThemeColors::from_theme(ctx.core.theme).text_dim;
        assert_ne!(
            before, after,
            "the two themes must differ to prove anything"
        );
        assert_eq!(component.detail.sections[0].style.fg, Some(after));
    }

    #[test]
    fn test_normalize_takes_dot_dot_from_the_text() {
        assert_eq!(
            normalize(Path::new("/a/b/../c/./d")),
            PathBuf::from("/a/c/d")
        );
    }

    #[test]
    fn test_resolve_link_takes_a_relative_target_from_the_links_folder() {
        assert_eq!(
            resolve_link(Path::new("/home/u/.bashrc"), "dotfiles/home/shell/.bashrc"),
            PathBuf::from("/home/u/dotfiles/home/shell/.bashrc")
        );
        assert_eq!(
            resolve_link(Path::new("/home/u/.config/x"), "../../src/y"),
            PathBuf::from("/home/src/y")
        );
    }
}
