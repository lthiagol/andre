//! The profiles screen. The list holds the profiles and the detail shows the selected one's
//! resolved packages. `+` makes a profile, `r` renames it and `x` deletes it, each in a
//! dialog. `e` opens the profile editor and `c` the status of the profile's packages. Enter
//! runs the current action on the whole profile through the review screen.

pub mod edit;
mod edit_view;
mod view;

use std::collections::hash_map::DefaultHasher;
use std::collections::HashMap;
use std::hash::{Hash, Hasher};

use andre_core::ignore::global_ignore_in;
use andre_core::profile::{known_packages, split_package_ref, Profile, ProfileSet};
use crossterm::event::{KeyCode, KeyEvent, MouseButton, MouseEvent, MouseEventKind};
use ratatui::{
    layout::{Position, Rect},
    text::Line,
    Frame,
};

use crate::components::confirm::ConfirmComponent;
use crate::components::status::StatusComponent;
use crate::components::{AppContext, Component, InputMode, Transition};
use crate::input::{self, Input};
use crate::theme::ThemeColors;
use crate::ui::detail::{render_detail, Detail};
use crate::ui::footer::{delete_dialog_hints, name_dialog_hints, profiles_hints, Hint};
use crate::ui::icons::Icons;
use crate::ui::layout::{screen_rects, Shape};
use crate::ui::list_panel::{self, click_list, render_list_panel, Empty, ListView};
use crate::ui::plural;
use crate::ui::text_field::TextField;

/// What the list shows with no profiles.
const EMPTY: Empty<'static> = Empty {
    message: "no profiles yet",
    hint: Some(("+", "new profile")),
};

/// The dialog open over the list, if any.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Popup {
    /// `+`: a new profile, named in the field.
    New(NameField),
    /// `r`: profile `from`, renamed to the name in the field.
    Rename { from: String, name: NameField },
    /// `x`: delete `name`. Profiles that include it lose the include; `included_by` names
    /// them, so the dialog can say so.
    Delete {
        name: String,
        included_by: Vec<String>,
    },
}

/// Whether a typed profile name can be used.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NameCheck {
    Empty,
    Invalid,
    Exists,
    Valid,
}

/// A name typed into a dialog, and whether it can be used. The check is made when the name
/// changes, so paint reads it and never touches the disk.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NameField {
    pub field: TextField,
    pub check: NameCheck,
}

impl NameField {
    fn new(field: TextField, profiles: &ProfileSet, keep: Option<&str>) -> Self {
        let check = view::name_check(&field.value, profiles, keep);
        Self { field, check }
    }

    /// Applies `key` to the name, then checks the name again.
    fn edit(&mut self, key: &KeyEvent, profiles: &ProfileSet, keep: Option<&str>) {
        self.field.handle_key(key);
        self.check = view::name_check(&self.field.value, profiles, keep);
    }
}

#[derive(Default)]
pub struct ProfilesComponent {
    /// The cursor and the window over the profiles.
    pub list: ListView,
    /// One row per profile. Built by `update`.
    rows: Vec<view::Row>,
    /// The selected profile's facts and packages. Built by `update`.
    detail: Detail,
    /// What `rows` and `detail` were built from. `None` until the first update.
    detail_key: Option<u64>,
    /// Rows the detail pane shows, as of the last `prepare`.
    detail_rows: usize,
    /// The `+ new profile` hint line of the empty list, as of the last `prepare`.
    empty_hint: Option<Rect>,
    /// Whether Enter can run the selected profile: it resolves. Built by `rebuild`.
    runnable: bool,
    popup: Option<Popup>,
    /// The profile to put the cursor on at the next rebuild (`--profile`).
    select: Option<String>,
}

impl Component for ProfilesComponent {
    fn id(&self) -> &'static str {
        "Profiles"
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }

    fn input_mode(&self) -> InputMode {
        match self.popup {
            Some(Popup::New(_)) | Some(Popup::Rename { .. }) => InputMode::TextInput,
            Some(Popup::Delete { .. }) => InputMode::Modal,
            None => InputMode::Normal,
        }
    }

    fn handle_key(&mut self, key: KeyEvent, ctx: &mut AppContext) -> Transition {
        if let Some(popup) = self.popup.take() {
            self.popup = self.next_popup(popup, key, ctx);
            return Transition::Handled;
        }
        if Input::handle_list_navigation(&key, &mut self.list.cursor, self.rows.len()) {
            return Transition::Handled;
        }
        match key.code {
            KeyCode::Enter => self.run(ctx),
            KeyCode::Esc => Transition::Pop,
            KeyCode::Char('+') => {
                let name = NameField::new(TextField::default(), &ctx.core.profiles, None);
                self.popup = Some(Popup::New(name));
                Transition::Handled
            }
            KeyCode::Char('r') => {
                if let Some(from) = self.selected_name() {
                    let name = NameField::new(text_field(&from), &ctx.core.profiles, Some(&from));
                    self.popup = Some(Popup::Rename { from, name });
                }
                Transition::Handled
            }
            KeyCode::Char('x') => {
                if let Some(name) = self.selected_name() {
                    let included_by = view::included_by(&ctx.core.profiles, &name);
                    self.popup = Some(Popup::Delete { name, included_by });
                }
                Transition::Handled
            }
            KeyCode::Char('e') => match self.selected_name() {
                Some(name) => match edit::ProfileEditComponent::new(&name, ctx) {
                    Some(editor) => Transition::Push(Box::new(editor)),
                    None => Transition::Handled,
                },
                None => Transition::Handled,
            },
            KeyCode::Char('c') => self.check(ctx),
            _ => Transition::None,
        }
    }

    fn update(&mut self, ctx: &mut AppContext) -> Transition {
        let key = self.fingerprint(ctx);
        if self.detail_key != Some(key) {
            self.rebuild(ctx);
            // Reading the statuses changes the cache, so the key is taken again afterwards.
            self.detail_key = Some(self.fingerprint(ctx));
        }
        Transition::None
    }

    fn hints(&self, ctx: &AppContext) -> Vec<Hint> {
        match &self.popup {
            Some(Popup::New(_)) => name_dialog_hints("create"),
            Some(Popup::Rename { .. }) => name_dialog_hints("rename"),
            Some(Popup::Delete { .. }) => delete_dialog_hints(),
            None => profiles_hints(
                self.runnable.then(|| ctx.core.flags.action.as_str()),
                !self.rows.is_empty(),
            ),
        }
    }

    fn handle_mouse(&mut self, ev: MouseEvent, ctx: &mut AppContext) -> Transition {
        // The empty list's hint line is a button for `+`.
        let on_hint = self
            .empty_hint
            .is_some_and(|hint| hint.contains(Position::new(ev.column, ev.row)));
        if on_hint && ev.kind == MouseEventKind::Down(MouseButton::Left) {
            return self.handle_key(input::key(KeyCode::Char('+')), ctx);
        }
        let len = self.rows.len();
        match click_list(&mut self.list, ev, len) {
            Some(click) if click.activate => self.handle_key(input::key(KeyCode::Enter), ctx),
            Some(_) => Transition::Handled,
            None => Transition::None,
        }
    }

    fn prepare(&mut self, area: Rect) {
        let rects = screen_rects(area, Shape::TwoPane);
        self.list.prepare(rects.list, self.rows.len());
        self.empty_hint = if self.rows.is_empty() {
            list_panel::empty_hint_rect(list_panel::inner(rects.list), &EMPTY)
        } else {
            None
        };
        self.detail_rows = usize::from(rects.detail.height.saturating_sub(2));
    }

    fn render(&mut self, frame: &mut Frame, area: Rect, ctx: &AppContext) {
        let colors = ThemeColors::from_theme(ctx.core.theme);
        let icons = Icons::for_set(ctx.core.icons);
        let rects = screen_rects(area, Shape::TwoPane);

        let width = list_panel::inner(rects.list).width;
        let rows: Vec<Line> = self
            .rows
            .iter()
            .enumerate()
            .map(|(i, row)| {
                view::list_row(
                    row,
                    i == self.list.cursor,
                    icons.menu_profiles,
                    &colors,
                    width,
                )
            })
            .collect();
        let title = format!("profiles {}", self.rows.len());
        render_list_panel(
            frame,
            rects.list,
            &colors,
            &title,
            rows,
            &self.list,
            Some(EMPTY),
        );

        let title = self
            .selected_name()
            .unwrap_or_else(|| "profiles".to_string());
        let detail = view::fit(&self.detail, self.detail_rows, &colors);
        render_detail(frame, rects.detail, &colors, &title, &detail);

        if let Some(popup) = &self.popup {
            view::render_popup(frame, area, &colors, popup);
        }
    }
}

impl ProfilesComponent {
    /// The screen with the cursor on `name`, opened by `--profile`. An unknown name
    /// leaves the cursor on the first profile.
    pub fn opened_at(name: &str) -> Self {
        Self {
            select: Some(name.to_string()),
            ..Self::default()
        }
    }

    /// The dialog open over the list, if any.
    pub fn popup(&self) -> Option<&Popup> {
        self.popup.as_ref()
    }

    fn selected_name(&self) -> Option<String> {
        self.rows.get(self.list.cursor).map(|row| row.name.clone())
    }

    /// Hash of everything the rows and the detail depend on. A change in any of these
    /// means they are rebuilt on the next update.
    fn fingerprint(&self, ctx: &AppContext) -> u64 {
        let mut hasher = DefaultHasher::new();
        self.list.cursor.hash(&mut hasher);
        ctx.status_cache.borrow().generation().hash(&mut hasher);
        ctx.core.theme.name().hash(&mut hasher);
        for (name, group) in &ctx.core.config.groups {
            name.hash(&mut hasher);
            group.source.hash(&mut hasher);
            group.target.hash(&mut hasher);
        }
        for profile in ctx.core.profiles.iter() {
            profile.name.hash(&mut hasher);
            profile.description.hash(&mut hasher);
            profile.includes.hash(&mut hasher);
            profile.packages.hash(&mut hasher);
        }
        ctx.core.profiles.issues().len().hash(&mut hasher);
        hasher.finish()
    }

    /// Rebuilds the rows and the selected profile's detail. Reads package status from disk
    /// for the selected profile's packages that the cache has not seen yet.
    fn rebuild(&mut self, ctx: &AppContext) {
        let known = known_packages(&ctx.core.config, &ctx.config_dir, &ctx.home_dir);
        self.rows = view::rows(&ctx.core.profiles, &known);
        if let Some(name) = self.select.take() {
            if let Some(index) = self.rows.iter().position(|row| row.name == name) {
                self.list.cursor = index;
            }
        }
        self.list.cursor = self.list.cursor.min(self.rows.len().saturating_sub(1));

        self.detail = match self.selected_name() {
            Some(name) => {
                let resolved = ctx.core.profiles.resolve(&name, &known);
                self.runnable = resolved.is_ok();
                if let Ok(resolved) = &resolved {
                    prefetch(
                        ctx,
                        resolved
                            .entries
                            .iter()
                            .filter(|e| !e.missing)
                            .map(|e| e.package.as_str()),
                    );
                }
                match ctx.core.profiles.get(&name) {
                    Some(profile) => view::profile_detail(ctx, profile, &resolved),
                    None => Detail::default(),
                }
            }
            None => {
                self.runnable = false;
                view::empty_detail(ctx)
            }
        };
    }

    /// The profile under the cursor, resolved and run through the review screen. Refused
    /// with a toast when it does not resolve or names no existing package.
    fn run(&self, ctx: &mut AppContext) -> Transition {
        let Some(name) = self.selected_name() else {
            return Transition::Handled;
        };
        let known = known_packages(&ctx.core.config, &ctx.config_dir, &ctx.home_dir);
        let resolved = match ctx.core.profiles.resolve(&name, &known) {
            Ok(resolved) => resolved,
            Err(err) => {
                ctx.show_error_toast(err.to_string());
                return Transition::Handled;
            }
        };

        let mut selected: HashMap<String, Vec<String>> = HashMap::new();
        for entry in resolved.entries.iter().filter(|e| !e.missing) {
            if let Some((group, package)) = split_package_ref(&entry.package) {
                selected
                    .entry(group.to_string())
                    .or_default()
                    .push(package.to_string());
            }
        }
        if selected.is_empty() {
            ctx.show_error_toast(format!("profile {name} has no packages to run"));
            return Transition::Handled;
        }
        Transition::Push(Box::new(ConfirmComponent::new(selected, ctx)))
    }

    /// Opens the status of the selected profile's packages. Refused with a toast when the
    /// profile does not resolve.
    fn check(&self, ctx: &mut AppContext) -> Transition {
        let Some(name) = self.selected_name() else {
            return Transition::Handled;
        };
        let known = known_packages(&ctx.core.config, &ctx.config_dir, &ctx.home_dir);
        match ctx.core.profiles.resolve(&name, &known) {
            Ok(_) => Transition::Push(Box::new(StatusComponent::for_profile(&name))),
            Err(err) => {
                ctx.show_error_toast(err.to_string());
                Transition::Handled
            }
        }
    }

    /// Applies a key to the open dialog. Returns the dialog to keep open, or `None` when the
    /// key closed it.
    fn next_popup(&mut self, popup: Popup, key: KeyEvent, ctx: &mut AppContext) -> Option<Popup> {
        match popup {
            Popup::New(mut name) => match key.code {
                KeyCode::Esc => None,
                KeyCode::Enter if name.check == NameCheck::Valid => {
                    self.create(&name.field.value, ctx);
                    None
                }
                KeyCode::Enter => Some(Popup::New(name)),
                _ => {
                    name.edit(&key, &ctx.core.profiles, None);
                    Some(Popup::New(name))
                }
            },
            Popup::Rename { from, mut name } => match key.code {
                KeyCode::Esc => None,
                KeyCode::Enter if name.field.value == from => None,
                KeyCode::Enter if name.check == NameCheck::Valid => {
                    self.rename(&from, &name.field.value, ctx);
                    None
                }
                KeyCode::Enter => Some(Popup::Rename { from, name }),
                _ => {
                    name.edit(&key, &ctx.core.profiles, Some(&from));
                    Some(Popup::Rename { from, name })
                }
            },
            Popup::Delete { name, included_by } => match key.code {
                KeyCode::Char('y' | 'Y') => {
                    self.delete(&name, ctx);
                    None
                }
                KeyCode::Char('n' | 'N') | KeyCode::Esc => None,
                _ => Some(Popup::Delete { name, included_by }),
            },
        }
    }

    fn create(&mut self, name: &str, ctx: &mut AppContext) {
        let profile = Profile {
            name: name.to_string(),
            ..Profile::default()
        };
        match ctx.core.profiles.create(profile) {
            Ok(()) => {
                self.select(name, ctx);
                ctx.show_toast(format!("created profile {name}"));
            }
            Err(err) => ctx.show_error_toast(err.to_string()),
        }
    }

    fn rename(&mut self, from: &str, to: &str, ctx: &mut AppContext) {
        match ctx.core.profiles.rename(from, to) {
            Ok(changed) => {
                self.select(to, ctx);
                let updated = match changed.len() {
                    0 => String::new(),
                    n => format!("; updated {n} {}", plural(n, "profile", "profiles")),
                };
                ctx.show_toast(format!("renamed {from} to {to}{updated}"));
            }
            Err(err) => ctx.show_error_toast(err.to_string()),
        }
    }

    fn delete(&mut self, name: &str, ctx: &mut AppContext) {
        match ctx.core.profiles.delete_unlinking(name) {
            Ok(changed) => {
                let unlinked = match changed.len() {
                    0 => String::new(),
                    n => format!("; removed it from {n} {}", plural(n, "profile", "profiles")),
                };
                ctx.show_toast(format!("deleted profile {name}{unlinked}"));
            }
            Err(err) => ctx.show_error_toast(err.to_string()),
        }
    }

    /// Moves the cursor onto `name`. The rows follow the profile set, which is sorted by name.
    fn select(&mut self, name: &str, ctx: &AppContext) {
        if let Some(index) = ctx.core.profiles.iter().position(|p| p.name == name) {
            self.list.cursor = index;
        }
    }
}

/// Reads the status of the `group/package` refs in `refs` that the cache has not seen yet.
/// Packages whose group has no source or target on disk stay unknown.
fn prefetch<'a>(ctx: &AppContext, refs: impl IntoIterator<Item = &'a str>) {
    let global = global_ignore_in(&ctx.home_dir);
    let mut cache = ctx.status_cache.borrow_mut();
    for reference in refs {
        let Some((group, package)) = split_package_ref(reference) else {
            continue;
        };
        if cache.get(group, package).is_some() {
            continue;
        }
        let source = ctx
            .core
            .config
            .get_group_source(group, &ctx.config_dir, &ctx.home_dir);
        let target = ctx
            .core
            .config
            .get_group_target(group, &ctx.config_dir, &ctx.home_dir);
        if let (Some(source), Some(target)) = (source, target) {
            cache.prefetch_package(&source, &target, group, package, Some(&global));
        }
    }
}

/// A name field holding `value` with the cursor at its end.
fn text_field(value: &str) -> TextField {
    TextField {
        value: value.to_string(),
        cursor: value.chars().count(),
        error: None,
    }
}
