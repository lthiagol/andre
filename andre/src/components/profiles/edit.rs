//! The profile editor. The left pane is the package tree with a mark on each package; the
//! right pane is the description and the includes, with the resolved facts under them.
//! Edits go to a draft. `w` writes the draft to the profile file. Esc with unsaved edits
//! asks first.

use std::borrow::Cow;
use std::collections::BTreeSet;

use andre_core::profile::{
    known_packages, split_package_ref, Origin, Profile, ProfileError, ResolvedProfile,
};
use andre_core::Error;
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
use crate::ui::text_field::TextField;
use crate::ui::tree::{click_key, tree_line, Mark, Toggle, Tree, TreeGroup, TreePackage, TreeRow};

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
    /// Every package on disk, as `group/package`.
    known: BTreeSet<String>,
    /// The draft resolved against `known`. `None` when it does not resolve.
    resolved: Option<ResolvedProfile>,
    /// Why the draft does not resolve, shown in the form.
    resolve_error: Option<String>,
    /// Packages marked, and packages in all, as of the last refresh.
    marked: usize,
    total: usize,
    tree: Tree,
    /// The tree's visible rows, as of the last refresh.
    rows: Vec<TreeRow>,
    packages: ListView,
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

    /// The packages the draft resolves to, in resolution order. Empty when it does not resolve.
    pub fn resolved_names(&self) -> Vec<String> {
        self.resolved
            .as_ref()
            .map(|resolved| resolved.entries.iter().map(|e| e.package.clone()).collect())
            .unwrap_or_default()
    }

    /// Rebuilds everything the draft shows: the resolution, the tree and the form. Folds and
    /// the cursor survive. Reads package status from disk for packages the cache has not seen.
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
        let groups = self.tree_groups(ctx, &refs);
        self.marked = groups.iter().map(count_marked).sum();
        self.total = groups.iter().map(|g| g.packages.len()).sum();
        self.tree.replace_groups(groups);
        self.rows = self.tree.rows();

        self.includes = self.form_includes(ctx);
        self.form_cursor = self.form_cursor.min(self.includes.len());
    }

    /// The tree's groups: every configured group, then any other group a ref names, each
    /// with its packages in name order.
    fn tree_groups(&self, ctx: &AppContext, refs: &BTreeSet<String>) -> Vec<TreeGroup> {
        let mut names: BTreeSet<String> = ctx.core.config.groups.keys().cloned().collect();
        names.extend(
            refs.iter()
                .filter_map(|r| split_package_ref(r))
                .map(|(g, _)| g.to_string()),
        );
        names
            .into_iter()
            .map(|group| {
                let packages = refs
                    .iter()
                    .filter_map(|reference| {
                        let (g, package) = split_package_ref(reference)?;
                        (g == group).then(|| self.tree_package(ctx, reference, g, package))
                    })
                    .collect();
                TreeGroup {
                    name: group,
                    packages,
                }
            })
            .collect()
    }

    /// One package row. Own packages are marked on; packages an include brings in are marked
    /// by that include and cannot be toggled; the rest are off.
    fn tree_package(
        &self,
        ctx: &AppContext,
        reference: &str,
        group: &str,
        package: &str,
    ) -> TreePackage {
        let on_disk = self.known.contains(reference);
        let mark = if self.draft.packages.iter().any(|p| p == reference) {
            Mark::On
        } else {
            match self
                .resolved
                .as_ref()
                .and_then(|r| r.entries.iter().find(|e| e.package == reference))
            {
                Some(entry) => match &entry.origin {
                    Origin::Included { include, .. } => Mark::Inherited(include.clone()),
                    Origin::Own => Mark::On,
                },
                None => Mark::Off,
            }
        };
        TreePackage {
            name: package.to_string(),
            status: on_disk
                .then(|| ctx.status_cache.borrow().get(group, package))
                .flatten(),
            mark: Some(mark),
            meta: if on_disk {
                String::new()
            } else {
                "missing".to_string()
            },
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

    /// The packages the tree marks, as the draft's `packages`. Refs that were already listed
    /// keep their place; new marks follow in tree order.
    fn marked_refs(&self) -> Vec<String> {
        let marked: Vec<String> = self
            .tree
            .marked()
            .into_iter()
            .map(|(group, package)| format!("{group}/{package}"))
            .collect();
        let mut next: Vec<String> = self
            .draft
            .packages
            .iter()
            .filter(|reference| marked.contains(reference))
            .cloned()
            .collect();
        for reference in marked {
            if !next.contains(&reference) {
                next.push(reference);
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
        if Input::handle_list_navigation(&key, &mut self.packages.cursor, self.rows.len()) {
            return Transition::Handled;
        }
        match key.code {
            KeyCode::Char(' ') => match self.tree.toggle_mark(self.packages.cursor) {
                Toggle::Changed => {
                    self.draft.packages = self.marked_refs();
                    self.refresh(ctx);
                }
                Toggle::Inherited { via } => {
                    ctx.show_toast(format!("comes from {via} — edit {via} or drop the include"));
                }
                Toggle::Unmarkable => {}
            },
            KeyCode::Left => {
                // Folding hides the package under the cursor, so the cursor moves to the
                // group's header first, and stays on it.
                let header = self.group_header(self.packages.cursor);
                self.tree.fold(header, true);
                self.rows = self.tree.rows();
                self.packages.cursor = header;
            }
            KeyCode::Right => {
                self.tree.fold(self.packages.cursor, false);
                self.rows = self.tree.rows();
            }
            _ => return Transition::None,
        }
        Transition::Handled
    }

    /// The row of the group header that `row` sits under, or `row` when it is a header.
    fn group_header(&self, row: usize) -> usize {
        self.rows
            .iter()
            .take(row + 1)
            .rposition(|r| matches!(r, TreeRow::Group { .. }))
            .unwrap_or(row)
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
            profile_edit_hints(space_toggles)
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
        let len = self.rows.len();
        let Some(click) = click_list(&mut self.packages, ev, len) else {
            return Transition::None;
        };
        self.pane = Pane::Packages;
        match click_key(&self.rows[click.row], click, true, Some(KeyCode::Char(' '))) {
            Some(code) => self.handle_key(input::key(code), ctx),
            None => Transition::Handled,
        }
    }

    fn prepare(&mut self, area: Rect) {
        self.content = area;
        let rects = screen_rects(area, Shape::TwoPane);
        self.packages.prepare(rects.list, self.rows.len());
    }

    fn render(&mut self, frame: &mut Frame, area: Rect, ctx: &AppContext) {
        let colors = ThemeColors::from_theme(ctx.core.theme);
        let icons = Icons::for_set(ctx.core.icons);
        let rects = screen_rects(area, Shape::TwoPane);
        let packages_focused = self.pane == Pane::Packages;

        let width = list_panel::inner(rects.list).width;
        let lines = self
            .rows
            .iter()
            .enumerate()
            .map(|(i, row)| {
                let selected = packages_focused && i == self.packages.cursor;
                tree_line(row, selected, icons, &colors, width)
            })
            .collect();
        let title = edit_view::packages_title(self.marked, self.total);
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
            &self.packages,
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

/// Marked packages in a group: own marks and inherited ones both count as marked.
fn count_marked(group: &TreeGroup) -> usize {
    group
        .packages
        .iter()
        .filter(|p| matches!(p.mark, Some(Mark::On | Mark::Inherited(_))))
        .count()
}

/// A name field holding `value` with the cursor at its end.
fn text_field(value: &str) -> TextField {
    TextField {
        value: value.to_string(),
        cursor: value.chars().count(),
        error: None,
    }
}
