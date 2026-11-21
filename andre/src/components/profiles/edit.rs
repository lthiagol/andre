//! The profile editor. The left pane is the flat package list with a mark on each package; the
//! right pane is the description and the includes, with the resolved facts under them.
//! Edits go to a draft. `w` writes the draft to the profile file. Esc with unsaved edits
//! asks first.

use std::borrow::Cow;
use std::collections::BTreeSet;

use andre_core::profile::{known_packages, Origin, Profile, ProfileError, ResolvedProfile};
use andre_core::{Error, StowStatus};
use crossterm::event::{KeyCode, KeyEvent, MouseEvent};
use ratatui::{layout::Rect, Frame};

use super::edit_view;
use crate::components::{AppContext, Component, InputMode, Transition};
use crate::input::{self, Input};
use crate::theme::ThemeColors;
use crate::ui::dialog::{dialog_click, dialog_rect, leave_dialog, render_dialog};
use crate::ui::footer::{field_edit_hints, leave_dialog_hints, profile_edit_hints, Hint};
use crate::ui::icons::Icons;
use crate::ui::layout::{screen_rects, Shape};
use crate::ui::list_panel::{self, click_list, render_focused_list_panel, Empty, ListView};
use crate::ui::package_list::{mark_hit, package_line, Mark, PackageList, PackageRow, Toggle};
use crate::ui::text_field::TextField;

/// Which pane takes keys. The focused pane has the accent border.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Pane {
    #[default]
    Packages,
    Form,
}

#[derive(Default)]
pub struct ProfileEditComponent {
    /// The profile as the file holds it. A save makes the draft the saved profile.
    saved: Profile,
    /// The profile as the user has left it. `w` writes it.
    draft: Profile,
    /// Every package on disk, by name.
    known: BTreeSet<String>,
    /// The draft resolved against `known`. `None` when it does not resolve.
    resolved: Option<ResolvedProfile>,
    /// Why the draft does not resolve, shown in the form.
    resolve_error: Option<String>,
    /// One row per package on disk, then one per ref the packages folder lacks. Built by
    /// `refresh`.
    packages: PackageList,
    /// The cursor and the window over the package rows.
    list: ListView,
    /// The profiles the form offers as includes, in name order.
    includes: Vec<String>,
    pane: Pane,
    /// The form's cursor: 0 is the description, `n` is `includes[n - 1]`.
    form_cursor: usize,
    /// The description being typed. `Some` while the field has the keys.
    editing: Option<TextField>,
    /// The leave dialog is open.
    leaving: bool,
    /// The content area, as of the last `prepare`. The leave dialog is centred in it.
    content: Rect,
}

impl ProfileEditComponent {
    /// Opens the editor on profile `name`. `None` when no such profile is loaded.
    pub fn new(name: &str, ctx: &AppContext) -> Option<Self> {
        let mut saved = ctx.core.profiles.get(name)?.clone();
        saved.description = tidy(saved.description.take());
        let mut editor = Self {
            draft: saved.clone(),
            saved,
            ..Self::default()
        };
        editor.refresh(ctx);
        Some(editor)
    }

    /// The profile as the user has left it. What `w` would write.
    pub fn draft(&self) -> &Profile {
        &self.draft
    }

    /// Whether the draft differs from the file.
    pub fn is_dirty(&self) -> bool {
        self.draft != self.saved
    }

    /// Whether the packages pane has the keys (the form has them otherwise).
    pub fn on_packages(&self) -> bool {
        self.pane == Pane::Packages
    }

    /// The package rows in list order: packages on disk, then the missing refs.
    pub fn rows(&self) -> &[PackageRow] {
        self.packages.rows()
    }

    /// The packages the draft resolves to, in resolution order. Empty when it does not resolve.
    pub fn resolved_names(&self) -> Vec<String> {
        self.resolved
            .as_ref()
            .map(|resolved| resolved.entries.iter().map(|e| e.package.clone()).collect())
            .unwrap_or_default()
    }

    /// Rebuilds everything the draft shows: the resolution, the package rows and the form.
    /// The cursor survives, clamped to the rows there are. Reads package status from disk for
    /// packages the cache has not seen.
    fn refresh(&mut self, ctx: &AppContext) {
        self.known = known_packages(&ctx.core.config, &ctx.config_dir, &ctx.home_dir);
        let resolved = ctx.core.profiles.resolve_draft(&self.draft, &self.known);
        self.resolve_error = resolved.as_ref().err().map(ToString::to_string);
        self.resolved = resolved.ok();

        let refs: BTreeSet<String> = self
            .known
            .iter()
            .chain(self.draft.packages.iter())
            .cloned()
            .collect();
        super::prefetch(ctx, refs.iter().map(String::as_str));
        self.packages = PackageList::new(self.package_rows(ctx));
        self.list.cursor = self
            .list
            .cursor
            .min(self.packages.rows().len().saturating_sub(1));

        self.includes = self.form_includes(ctx);
        self.form_cursor = self.form_cursor.min(self.includes.len());
    }

    /// One row per package on disk, in name order, then one per missing ref. A missing ref is
    /// marked as the draft holds it, so unmarking it keeps its row until `w` drops it.
    fn package_rows(&self, ctx: &AppContext) -> Vec<PackageRow> {
        let mut rows: Vec<PackageRow> = self
            .known
            .iter()
            .map(|name| self.known_row(ctx, name))
            .collect();
        for name in self.missing_refs() {
            let mark = if self.draft.packages.contains(&name) {
                Mark::On
            } else {
                Mark::Off
            };
            rows.push(PackageRow {
                name,
                status: Some(StowStatus::Missing),
                mark: Some(mark),
                meta: "missing".to_string(),
            });
        }
        rows
    }

    /// The refs the packages folder lacks: the ones the file held when the editor opened, then
    /// any the draft adds. Kept from the file, not the draft, so an unmarked ref stays listed.
    fn missing_refs(&self) -> Vec<String> {
        let mut refs: Vec<String> = Vec::new();
        for name in self.saved.packages.iter().chain(&self.draft.packages) {
            if !self.known.contains(name) && !refs.contains(name) {
                refs.push(name.clone());
            }
        }
        refs
    }

    /// A package on disk. Own packages are marked on; packages an include brings in are marked
    /// by that include and cannot be toggled; the rest are off.
    fn known_row(&self, ctx: &AppContext, name: &str) -> PackageRow {
        let mark = if self.draft.packages.iter().any(|p| p == name) {
            Mark::On
        } else {
            match self
                .resolved
                .as_ref()
                .and_then(|r| r.entries.iter().find(|e| e.package == name))
            {
                Some(entry) => match &entry.origin {
                    Origin::Included { include, .. } => Mark::Inherited(include.clone()),
                    Origin::Own => Mark::On,
                },
                None => Mark::Off,
            }
        };
        PackageRow {
            name: name.to_string(),
            status: ctx.status_cache.borrow().get(name),
            mark: Some(mark),
            meta: String::new(),
        }
    }

    /// Every other profile, plus any include the draft names (so it can be unchecked).
    fn form_includes(&self, ctx: &AppContext) -> Vec<String> {
        let mut names: BTreeSet<String> = ctx
            .core
            .profiles
            .iter()
            .map(|p| p.name.clone())
            .filter(|name| *name != self.draft.name)
            .collect();
        names.extend(self.draft.includes.iter().cloned());
        names.into_iter().collect()
    }

    /// Packages marked in the list, own marks and inherited ones both: the count the title
    /// shows.
    fn marked_count(&self) -> usize {
        self.packages
            .rows()
            .iter()
            .filter(|row| matches!(row.mark, Some(Mark::On | Mark::Inherited(_))))
            .count()
    }

    /// The packages the list marks, as the draft's `packages`. Refs that were already listed
    /// keep their place; new marks follow in list order.
    fn marked_refs(&self) -> Vec<String> {
        let marked = self.packages.marked();
        let mut next: Vec<String> = self
            .draft
            .packages
            .iter()
            .filter(|name| marked.contains(name))
            .cloned()
            .collect();
        for name in marked {
            if !next.contains(&name) {
                next.push(name);
            }
        }
        next
    }

    /// Writes the draft to its file. Returns whether it did.
    fn save(&mut self, ctx: &mut AppContext) -> bool {
        match ctx.core.profiles.save(self.draft.clone()) {
            Ok(()) => {
                ctx.show_toast(format!("saved profile {}", self.draft.name));
                self.saved = self.draft.clone();
                // A ref the save dropped leaves the list with it.
                self.refresh(ctx);
                true
            }
            Err(err) => {
                ctx.show_error_toast(err.to_string());
                false
            }
        }
    }

    fn handle_description_key(&mut self, key: KeyEvent, ctx: &AppContext) {
        let Some(field) = self.editing.as_mut() else {
            return;
        };
        if matches!(key.code, KeyCode::Enter | KeyCode::Esc) {
            let text = field.value.clone();
            self.editing = None;
            self.draft.description = tidy(Some(text));
            self.refresh(ctx);
        } else {
            field.handle_key(&key);
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

    fn handle_packages_key(&mut self, key: KeyEvent, ctx: &mut AppContext) -> Transition {
        if Input::handle_list_navigation(&key, &mut self.list.cursor, self.packages.rows().len()) {
            return Transition::Handled;
        }
        match key.code {
            KeyCode::Char(' ') => {
                let toggle = self.packages.toggle_mark(self.list.cursor);
                self.apply_toggle(toggle, ctx);
            }
            KeyCode::Char('*') => {
                let toggle = self.packages.toggle_all();
                self.apply_toggle(toggle, ctx);
            }
            _ => return Transition::None,
        }
        Transition::Handled
    }

    /// Acts on what a mark change did. A change is written to the draft and the rows rebuilt.
    /// A row an include marks cannot change here, and the toast says which include.
    fn apply_toggle(&mut self, toggle: Toggle, ctx: &mut AppContext) {
        match toggle {
            Toggle::Changed => {
                self.draft.packages = self.marked_refs();
                self.refresh(ctx);
            }
            Toggle::Inherited { via } => {
                ctx.show_toast(format!("comes from {via} — edit {via} or drop the include"));
            }
            Toggle::Unmarkable => {}
        }
    }

    fn handle_form_key(&mut self, key: KeyEvent, ctx: &mut AppContext) -> Transition {
        if Input::handle_list_navigation(&key, &mut self.form_cursor, self.includes.len() + 1) {
            return Transition::Handled;
        }
        match key.code {
            KeyCode::Enter if self.form_cursor == 0 => {
                self.editing = Some(text_field(self.draft.description.as_deref().unwrap_or("")));
            }
            KeyCode::Char(' ') if self.form_cursor > 0 => self.toggle_include(ctx),
            _ => return Transition::None,
        }
        Transition::Handled
    }

    /// Adds or drops the include under the form cursor. Adding one that would make a cycle is
    /// refused, and the toast says which cycle.
    fn toggle_include(&mut self, ctx: &mut AppContext) {
        let name = self.includes[self.form_cursor - 1].clone();
        if let Some(pos) = self.draft.includes.iter().position(|inc| *inc == name) {
            self.draft.includes.remove(pos);
        } else {
            let mut next = self.draft.clone();
            next.includes.push(name.clone());
            // Only a cycle through the new include is refused. A problem that was already there
            // (an unknown include, say) does not stop the user from adding others.
            if let Err(err @ Error::Profile(ProfileError::Cycle(_))) =
                ctx.core.profiles.resolve_draft(&next, &self.known)
            {
                ctx.show_error_toast(format!("can't include {name}: {err}"));
                return;
            }
            self.draft = next;
        }
        self.refresh(ctx);
    }
}

impl Component for ProfileEditComponent {
    fn id(&self) -> &'static str {
        "ProfileEdit"
    }

    fn crumbs(&self, _ctx: &AppContext) -> Vec<Cow<'static, str>> {
        vec![Cow::Owned(self.draft.name.clone())]
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
        if self.leaving {
            return self.handle_leave_key(key, ctx);
        }
        if self.editing.is_some() {
            self.handle_description_key(key, ctx);
            return Transition::Handled;
        }
        match key.code {
            KeyCode::Esc if self.is_dirty() => {
                self.leaving = true;
                Transition::Handled
            }
            KeyCode::Esc => Transition::Pop,
            KeyCode::Tab => {
                self.pane = match self.pane {
                    Pane::Packages => Pane::Form,
                    Pane::Form => Pane::Packages,
                };
                Transition::Handled
            }
            KeyCode::Char('w') => {
                if self.is_dirty() {
                    self.save(ctx);
                }
                Transition::Handled
            }
            _ => match self.pane {
                Pane::Packages => self.handle_packages_key(key, ctx),
                Pane::Form => self.handle_form_key(key, ctx),
            },
        }
    }

    fn hints(&self, _ctx: &AppContext) -> Vec<Hint> {
        if self.leaving {
            leave_dialog_hints()
        } else if self.editing.is_some() {
            field_edit_hints()
        } else {
            let space_toggles = self.pane == Pane::Packages || self.form_cursor > 0;
            profile_edit_hints(space_toggles, self.pane == Pane::Packages)
        }
    }

    /// A text field being edited takes the mouse; the leave dialog's buttons do not.
    fn mouse_blocked(&self) -> bool {
        self.editing.is_some()
    }

    fn handle_mouse(&mut self, ev: MouseEvent, ctx: &mut AppContext) -> Transition {
        if self.leaving {
            let colors = ThemeColors::from_theme(ctx.core.theme);
            let dialog = leave_dialog(&format!("profile `{}`", self.draft.name), &colors);
            return match dialog_click(self.content, &dialog, ev) {
                Some(key) => self.handle_key(key, ctx),
                None => Transition::Handled,
            };
        }
        if self.editing.is_some() {
            return Transition::None;
        }
        let len = self.packages.rows().len();
        let Some(click) = click_list(&mut self.list, ev, len) else {
            return Transition::None;
        };
        self.pane = Pane::Packages;
        // A click on the checkbox marks the row; a click on the row the cursor already rests on
        // marks it too, as Space does.
        let marks = mark_hit(&self.packages.rows()[click.row], click.col, true);
        if marks || click.activate {
            return self.handle_key(input::key(KeyCode::Char(' ')), ctx);
        }
        Transition::Handled
    }

    fn prepare(&mut self, area: Rect) {
        self.content = area;
        let rects = screen_rects(area, Shape::TwoPane);
        self.list.prepare(rects.list, self.packages.rows().len());
    }

    fn render(&mut self, frame: &mut Frame, area: Rect, ctx: &AppContext) {
        let colors = ThemeColors::from_theme(ctx.core.theme);
        let icons = Icons::for_set(ctx.core.icons);
        let rects = screen_rects(area, Shape::TwoPane);
        let packages_focused = self.pane == Pane::Packages;

        let width = list_panel::inner(rects.list).width;
        let lines = self
            .packages
            .rows()
            .iter()
            .enumerate()
            .map(|(i, row)| {
                let selected = packages_focused && i == self.list.cursor;
                package_line(row, selected, true, icons, &colors, width)
            })
            .collect();
        let title = edit_view::packages_title(self.marked_count(), self.packages.rows().len());
        let empty = Empty {
            message: "no packages on disk",
            hint: None,
        };
        render_focused_list_panel(
            frame,
            rects.list,
            &colors,
            packages_focused,
            &title,
            lines,
            &self.list,
            Some(empty),
        );

        let form = edit_view::FormView {
            draft: &self.draft,
            saved: &self.saved,
            resolved: self.resolved.as_ref(),
            resolve_error: self.resolve_error.as_deref(),
            includes: &self.includes,
            cursor: self.form_cursor,
            focused: self.pane == Pane::Form,
            editing: self.editing.as_ref(),
            profiles: &ctx.core.profiles,
        };
        edit_view::render_form(frame, rects.detail, &colors, icons, &form);

        if self.leaving {
            let dialog = leave_dialog(&format!("profile `{}`", self.draft.name), &colors);
            render_dialog(frame, dialog_rect(area, &dialog), &colors, &dialog);
        }
    }
}

/// A description with its ends trimmed. Blank is no description.
fn tidy(description: Option<String>) -> Option<String> {
    description
        .map(|text| text.trim().to_string())
        .filter(|text| !text.is_empty())
}

/// A name field holding `value` with the cursor at its end.
fn text_field(value: &str) -> TextField {
    TextField {
        value: value.to_string(),
        cursor: value.chars().count(),
        error: None,
    }
}
