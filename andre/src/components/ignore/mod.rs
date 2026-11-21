//! The ignore editor: the two ignore files of one package, `.gitignore` and
//! `.stow-local-ignore`, edited as lists of lines. `Tab` switches file. `+` adds a line,
//! Enter or `e` edits the cursor's line and `x` deletes it. `w` writes the files that
//! changed. The detail says what the cursor's line matches in the package. Esc with unsaved
//! edits asks first.

mod preview;
mod view;

use std::borrow::Cow;
use std::path::PathBuf;

use andre_core::ignore::{read_lines, save_lines, IgnoreRules, RuleSource};
use andre_core::{IgnoreFile, Theme};
use crossterm::event::{KeyCode, KeyEvent};
use ratatui::{layout::Rect, Frame};

use crate::components::{AppContext, Component, InputMode, Transition};
use crate::input::Input;
use crate::theme::ThemeColors;
use crate::ui::detail::{render_detail, Detail};
use crate::ui::dialog::{dialog_rect, leave_dialog, render_dialog};
use crate::ui::footer::{ignore_edit_hints, ignore_hints, leave_dialog_hints, Hint};
use crate::ui::layout::{screen_rects, Shape};
use crate::ui::list_panel::{self, render_list_panel, Empty, ListView};
use crate::ui::text_field::TextField;

/// The package's ignore files, in tab order.
const FILES: [IgnoreFile; 2] = [IgnoreFile::GitIgnore, IgnoreFile::StowLocal];

/// One ignore file as the disk holds it and as the user has left it. `None` is no file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IgnoreDraft {
    pub kind: IgnoreFile,
    saved: Option<Vec<String>>,
    lines: Option<Vec<String>>,
}

impl IgnoreDraft {
    /// The lines as the user has left them. Empty when there is no file.
    pub fn lines(&self) -> &[String] {
        self.lines.as_deref().unwrap_or(&[])
    }

    /// Whether the user has a file here, saved or not. `false` until a line is added.
    pub fn present(&self) -> bool {
        self.lines.is_some()
    }

    /// Whether the lines differ from the file.
    pub fn is_dirty(&self) -> bool {
        self.lines != self.saved
    }
}

/// The line being typed. `row` is its index. `new` means the line is not in the file yet.
/// `creates_file` means the file did not exist and starting the line gave it its starter
/// lines, so cancelling the line must take them back.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Editing {
    row: usize,
    new: bool,
    creates_file: bool,
    field: TextField,
}

pub struct IgnoreEditComponent {
    /// `group/package`, for the leave dialog.
    package: String,
    /// The package folder, where the ignore files live.
    dir: PathBuf,
    files: Vec<IgnoreDraft>,
    /// Index into `files` of the file on the tab.
    active: usize,
    /// Every file in the package, for the detail's matches. Read once, when the editor opens.
    package_files: Vec<String>,
    list: ListView,
    /// The line being typed. `Some` while the list has the keys as a text field.
    editing: Option<Editing>,
    /// The leave dialog is open.
    leaving: bool,
    /// The detail's title and content for the cursor's row. Rebuilt after every key, and when
    /// the theme changes (the colours are in the detail's lines).
    detail_title: String,
    detail: Detail,
    detail_theme: Theme,
}

impl IgnoreEditComponent {
    /// Opens the editor on the ignore files in `dir`, the folder of package `package`
    /// (`group/package`), drawn in `theme`. Fails when an existing file cannot be read.
    pub fn open(package: String, dir: PathBuf, theme: Theme) -> andre_core::Result<Self> {
        let mut files = Vec::new();
        for kind in FILES {
            let saved = read_lines(&kind.path_in(&dir))?;
            files.push(IgnoreDraft {
                kind,
                lines: saved.clone(),
                saved,
            });
        }
        let mut editor = Self {
            package,
            package_files: preview::package_files(&dir),
            dir,
            files,
            active: 0,
            list: ListView::default(),
            editing: None,
            leaving: false,
            detail_title: String::new(),
            detail: Detail::default(),
            detail_theme: theme,
        };
        editor.refresh_detail(theme);
        Ok(editor)
    }

    /// The files in tab order.
    pub fn files(&self) -> &[IgnoreDraft] {
        &self.files
    }

    /// Index of the file on the tab.
    pub fn active(&self) -> usize {
        self.active
    }

    /// Whether any file differs from what the disk holds.
    pub fn is_dirty(&self) -> bool {
        self.files.iter().any(IgnoreDraft::is_dirty)
    }

    /// The cursor's row. Its index is in the list, so it can be a line, the line being typed
    /// or the add row.
    pub fn cursor(&self) -> usize {
        self.list.cursor
    }

    /// The text of the line being typed, if one is.
    pub fn editing_text(&self) -> Option<&str> {
        self.editing.as_ref().map(|edit| edit.field.value.as_str())
    }

    fn current(&self) -> &IgnoreDraft {
        &self.files[self.active]
    }

    fn current_mut(&mut self) -> &mut IgnoreDraft {
        &mut self.files[self.active]
    }

    /// Whether the line being typed is a new one, below the file's lines.
    fn typing_new(&self) -> bool {
        self.editing.as_ref().is_some_and(|edit| edit.new)
    }

    /// Rows the list shows: the file's lines, the line being typed if it is new, then the add
    /// row. A file that does not exist has no rows.
    fn row_count(&self) -> usize {
        match self.current().lines {
            Some(_) => self.current().lines().len() + usize::from(self.typing_new()) + 1,
            None => 0,
        }
    }

    /// Whether the cursor is on one of the file's lines (not the add row, not a new line).
    fn on_line(&self) -> bool {
        self.current().lines.is_some() && self.list.cursor < self.current().lines().len()
    }

    fn clamp_cursor(&mut self) {
        self.list.cursor = self.list.cursor.min(self.row_count().saturating_sub(1));
    }

    /// Starts a new line at the end of the file. A file that does not exist starts from its
    /// starter lines, so the user sees what it will hold.
    fn start_new(&mut self) {
        let draft = self.current_mut();
        let creates_file = draft.lines.is_none();
        if creates_file {
            let starter = draft.kind.template();
            draft.lines = Some(starter.lines().map(str::to_string).collect());
        }
        let row = self.current().lines().len();
        self.editing = Some(Editing {
            row,
            new: true,
            creates_file,
            field: TextField::default(),
        });
        self.list.cursor = row;
    }

    /// Starts editing the cursor's line.
    fn start_edit(&mut self) {
        let row = self.list.cursor;
        let text = self.current().lines()[row].clone();
        self.editing = Some(Editing {
            row,
            new: false,
            creates_file: false,
            field: text_field(&text),
        });
    }

    /// Drops the line being typed. When that line was the first one of a file that did not
    /// exist, the file goes back to not existing, so a cancelled `+` leaves nothing to save.
    fn cancel_edit(&mut self) {
        let Some(edit) = self.editing.take() else {
            return;
        };
        if edit.creates_file {
            self.current_mut().lines = None;
            self.clamp_cursor();
        }
    }

    /// Removes the cursor's line.
    fn delete_line(&mut self) {
        if !self.on_line() {
            return;
        }
        let row = self.list.cursor;
        if let Some(lines) = self.current_mut().lines.as_mut() {
            lines.remove(row);
        }
        self.clamp_cursor();
    }

    /// Stores the line being typed. A blank line removes an existing line and adds nothing.
    /// A stow line that does not compile keeps the field open with the reason shown.
    fn commit_edit(&mut self) {
        let Some(mut edit) = self.editing.take() else {
            return;
        };
        let text = edit.field.value.trim().to_string();
        if !text.is_empty() && !self.is_valid_line(&text) {
            edit.field.error = Some("not a valid pattern".to_string());
            self.editing = Some(edit);
            return;
        }
        if edit.creates_file && text.is_empty() {
            self.current_mut().lines = None;
            self.clamp_cursor();
            return;
        }
        let lines = self.current_mut().lines.get_or_insert_with(Vec::new);
        match (edit.new, text.is_empty()) {
            (true, true) => {}
            (true, false) => lines.push(text),
            (false, true) => {
                lines.remove(edit.row);
            }
            (false, false) => lines[edit.row] = text,
        }
        self.clamp_cursor();
    }

    /// Whether `text` may be a line of the current file. Only `.stow-local-ignore` lines are
    /// checked: a `.gitignore` line is a glob and any text is one.
    fn is_valid_line(&self, text: &str) -> bool {
        match self.current().kind {
            IgnoreFile::StowLocal => {
                IgnoreRules::from_text(text, RuleSource::Local, "this line").is_ok()
            }
            IgnoreFile::GitIgnore => true,
        }
    }

    /// Writes every file that differs from the disk. The first failure is shown and the
    /// files after it are left as they were. Returns whether every write succeeded.
    fn save(&mut self, ctx: &mut AppContext) -> bool {
        let mut written = Vec::new();
        for draft in self.files.iter_mut().filter(|draft| draft.is_dirty()) {
            let Some(lines) = draft.lines.as_deref() else {
                continue;
            };
            let path = draft.kind.path_in(&self.dir);
            if let Err(err) = save_lines(&path, draft.kind, lines) {
                ctx.show_error_toast(err.to_string());
                return false;
            }
            draft.saved = draft.lines.clone();
            written.push(draft.kind.file_name());
        }
        if !written.is_empty() {
            // The package's status depends on its ignore files, so the cache is read again.
            ctx.status_cache.borrow_mut().invalidate_all();
            ctx.show_toast(format!("saved {}", written.join(", ")));
        }
        true
    }

    fn handle_list_key(&mut self, key: KeyEvent, ctx: &mut AppContext) -> Transition {
        let len = self.row_count();
        if Input::handle_list_navigation(&key, &mut self.list.cursor, len) {
            return Transition::Handled;
        }
        match key.code {
            KeyCode::Tab => {
                self.active = (self.active + 1) % self.files.len();
                self.list.cursor = 0;
            }
            KeyCode::Char('+') => self.start_new(),
            KeyCode::Enter if self.on_line() => self.start_edit(),
            KeyCode::Enter => self.start_new(),
            KeyCode::Char('e') if self.on_line() => self.start_edit(),
            KeyCode::Char('x') => self.delete_line(),
            KeyCode::Char('w') => {
                if self.is_dirty() {
                    self.save(ctx);
                }
            }
            KeyCode::Esc if self.is_dirty() => self.leaving = true,
            KeyCode::Esc => return Transition::Pop,
            _ => return Transition::None,
        }
        Transition::Handled
    }

    fn handle_edit_key(&mut self, key: KeyEvent) {
        if key.code == KeyCode::Enter {
            self.commit_edit();
        } else if key.code == KeyCode::Esc {
            self.cancel_edit();
        } else if let Some(edit) = self.editing.as_mut() {
            edit.field.handle_key(&key);
        }
    }

    fn handle_leave_key(&mut self, key: KeyEvent, ctx: &mut AppContext) -> Transition {
        match key.code {
            KeyCode::Char('y' | 'Y') => {
                self.leaving = false;
                if self.save(ctx) {
                    return Transition::Pop;
                }
            }
            KeyCode::Char('n' | 'N') => return Transition::Pop,
            KeyCode::Esc => self.leaving = false,
            _ => {}
        }
        Transition::Handled
    }

    /// Rebuilds the detail for the cursor's row: the line's meaning, what it matches and
    /// the files it matches. Nothing here reads the disk.
    fn refresh_detail(&mut self, theme: Theme) {
        let colors = ThemeColors::from_theme(theme);
        let kind = self.current().kind;
        let text = match &self.editing {
            Some(edit) => Some(edit.field.value.trim().to_string()),
            None if self.on_line() => {
                Some(self.current().lines()[self.list.cursor].trim().to_string())
            }
            None => None,
        };
        let (title, detail) = match text {
            Some(text) => (
                view::line_title(&text),
                view::line_detail(&colors, kind, &text, self.matches(kind, &text)),
            ),
            None => (
                kind.file_name().to_string(),
                view::file_detail(&colors, self.current()),
            ),
        };
        self.detail_title = title;
        self.detail = detail;
        self.detail_theme = theme;
    }

    /// The package files a line of `kind` matches, or why the line does not compile.
    fn matches(&self, kind: IgnoreFile, text: &str) -> Result<Vec<String>, String> {
        if text.is_empty() || text.starts_with('#') {
            return Ok(Vec::new());
        }
        match kind {
            IgnoreFile::GitIgnore => Ok(preview::gitignore_matches(text, &self.package_files)),
            IgnoreFile::StowLocal => preview::stow_matches(text, &self.package_files),
        }
    }
}

impl Component for IgnoreEditComponent {
    fn id(&self) -> &'static str {
        "IgnoreEdit"
    }

    fn crumbs(&self, _ctx: &AppContext) -> Vec<Cow<'static, str>> {
        vec![Cow::Owned(self.package.clone()), Cow::Borrowed("ignore")]
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }

    fn input_mode(&self) -> InputMode {
        if self.leaving {
            InputMode::Modal
        } else if self.editing.is_some() {
            InputMode::TextInput
        } else {
            InputMode::Normal
        }
    }

    fn handle_key(&mut self, key: KeyEvent, ctx: &mut AppContext) -> Transition {
        let transition = if self.leaving {
            self.handle_leave_key(key, ctx)
        } else if self.editing.is_some() {
            self.handle_edit_key(key);
            Transition::Handled
        } else {
            self.handle_list_key(key, ctx)
        };
        self.refresh_detail(ctx.core.theme);
        transition
    }

    fn update(&mut self, ctx: &mut AppContext) -> Transition {
        if ctx.core.theme != self.detail_theme {
            self.refresh_detail(ctx.core.theme);
        }
        Transition::None
    }

    fn hints(&self, _ctx: &AppContext) -> Vec<Hint> {
        if self.leaving {
            leave_dialog_hints()
        } else if self.editing.is_some() {
            ignore_edit_hints()
        } else {
            ignore_hints(self.on_line())
        }
    }

    fn prepare(&mut self, area: Rect) {
        let rects = screen_rects(area, Shape::ChipTwoPane);
        let len = self.row_count();
        self.list.prepare(rects.list, len);
    }

    fn render(&mut self, frame: &mut Frame, area: Rect, ctx: &AppContext) {
        let colors = ThemeColors::from_theme(ctx.core.theme);
        let rects = screen_rects(area, Shape::ChipTwoPane);

        if let Some(chip) = rects.chip {
            view::render_chips(frame, chip, &colors, &self.files, self.active);
        }

        let width = list_panel::inner(rects.list).width;
        let rows = view::list_rows(self, &colors, width);
        let draft = self.current();
        let message = format!("no {} yet", draft.kind.file_name());
        let empty = draft.lines.is_none().then_some(Empty {
            message: &message,
            hint: Some(("+", "create it")),
        });
        render_list_panel(
            frame,
            rects.list,
            &colors,
            &view::list_title(draft),
            rows,
            &self.list,
            empty,
        );

        render_detail(
            frame,
            rects.detail,
            &colors,
            &self.detail_title,
            &self.detail,
        );

        if self.leaving {
            let subject = format!("the ignore files of {}", self.package);
            let dialog = leave_dialog(&subject, &colors);
            render_dialog(frame, dialog_rect(area, &dialog), &colors, &dialog);
        }
    }
}

/// A text field holding `value` with the cursor at its end.
fn text_field(value: &str) -> TextField {
    TextField {
        value: value.to_string(),
        cursor: value.chars().count(),
        error: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn text_field_puts_the_cursor_after_the_last_character() {
        let field = text_field("é*.env");
        assert_eq!(field.cursor, 6);
        assert!(field.error.is_none());
    }
}
