//! The packages browser: one flat list of the packages in the packages folder. Space marks a
//! package to run, and `*` marks them all or unmarks them all. Enter runs the marks, or else
//! the cursor's package, through the confirm screen. `p` opens the profile membership dialog
//! for the cursor's package, and `i` its ignore editor.

mod view;

use std::collections::hash_map::DefaultHasher;
use std::collections::BTreeSet;
use std::hash::{Hash, Hasher};

use andre_core::ignore::global_ignore_in;
use andre_core::profile::ProfileSet;
use andre_core::PackageEntry;
use crossterm::event::{KeyCode, KeyEvent, MouseButton, MouseEvent, MouseEventKind};
use ratatui::{
    layout::{Position, Rect},
    text::Line,
    Frame,
};

use crate::components::confirm::ConfirmComponent;
use crate::components::ignore::IgnoreEditComponent;
use crate::components::settings::SettingsComponent;
use crate::components::wizard::NewPackageWizard;
use crate::components::{AppContext, Component, InputMode, Transition};
use crate::input::{self, Input};
use crate::theme::ThemeColors;
use crate::ui::detail::{fit_scroll, render_detail_scrolled, wheel_over, wheel_scroll, Detail};
use crate::ui::dialog::{dialog_body_rect, dialog_click, dialog_row_click};
use crate::ui::footer::{membership_dialog_hints, packages_hints, Hint};
use crate::ui::icons::Icons;
use crate::ui::layout::{screen_rects, Shape};
use crate::ui::list_panel::{
    self, click_list, empty_hint_rect, render_list_panel, Empty, ListView,
};
use crate::ui::package_list::{mark_hit, package_line, Mark, PackageList, PackageRow};
use crate::ui::plural;

use view::{Blank, Fix};

/// The profile membership dialog open over the browser.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Membership {
    /// The package's name.
    pub package: String,
    /// Every profile, in name order.
    pub profiles: Vec<MemberRow>,
    pub cursor: usize,
}

/// One profile in the membership dialog.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MemberRow {
    pub name: String,
    /// Whether the profile lists the package when the dialog opened.
    pub was: bool,
    /// Whether it lists the package now. Save writes the profiles where this differs from `was`.
    pub now: bool,
}

impl Membership {
    fn open(package: String, profiles: &ProfileSet) -> Self {
        let profiles = profiles
            .iter()
            .map(|profile| {
                let listed = profile.packages.contains(&package);
                MemberRow {
                    name: profile.name.clone(),
                    was: listed,
                    now: listed,
                }
            })
            .collect();
        Self {
            package,
            profiles,
            cursor: 0,
        }
    }
}

#[derive(Default)]
pub struct PackagesComponent {
    /// The cursor and the window over the rows.
    pub list: ListView,
    /// One row per package, with its status and its mark. Built by `rebuild`.
    packages: PackageList,
    /// Each package's entry, in the same order as `packages`. Built by `rebuild`.
    entries: Vec<PackageEntry>,
    /// What the list says when it has no rows. `None` while it has some. Built by `rebuild`.
    blank: Option<Blank>,
    /// The cursor's facts, or the empty state's. Built by `rebuild`.
    detail: Detail,
    /// What `rebuild` was built from. `None` until the first update.
    detail_key: Option<u64>,
    popup: Option<Membership>,
    /// The content area, as of the last `prepare`. The membership dialog is centred in it.
    content: Rect,
    /// Rows the detail is scrolled down. It resets when the cursor moves.
    scroll: u16,
    /// The cursor row `scroll` belongs to.
    scroll_row: usize,
    /// The detail pane as the last `prepare` laid it out.
    detail_rect: Rect,
    /// The empty state's hint, as the last `prepare` laid it out. A click on it opens its fix.
    hint_rect: Option<Rect>,
}

impl Component for PackagesComponent {
    fn id(&self) -> &'static str {
        "Packages"
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }

    fn input_mode(&self) -> InputMode {
        match self.popup {
            Some(_) => InputMode::Modal,
            None => InputMode::Normal,
        }
    }

    fn handle_key(&mut self, key: KeyEvent, ctx: &mut AppContext) -> Transition {
        // Keys can arrive before the next update, so the rows are brought up to date first.
        self.sync(ctx);
        if let Some(mut membership) = self.popup.take() {
            if membership_key(&mut membership, key, ctx) {
                self.popup = Some(membership);
            }
            return Transition::Handled;
        }
        if Input::handle_list_navigation(&key, &mut self.list.cursor, self.packages.rows().len()) {
            return Transition::Handled;
        }
        match key.code {
            KeyCode::Char(' ') => {
                self.packages.toggle_mark(self.list.cursor);
            }
            KeyCode::Char('*') => {
                self.packages.toggle_all();
            }
            KeyCode::Char('p') => {
                if let Some(entry) = self.selected_entry() {
                    self.popup = Some(Membership::open(entry.name.clone(), &ctx.core.profiles));
                }
            }
            KeyCode::Char('i') => return self.open_ignore(ctx),
            // The empty state's hint names this key; it opens the same screen as a click.
            KeyCode::Char(c) if self.blank_key(c) => return self.open_fix(),
            KeyCode::Enter => return self.run(ctx),
            KeyCode::Esc => return Transition::Pop,
            _ => return Transition::None,
        }
        Transition::Handled
    }

    fn update(&mut self, ctx: &mut AppContext) -> Transition {
        self.sync(ctx);
        Transition::None
    }

    fn hints(&self, ctx: &AppContext) -> Vec<Hint> {
        match self.popup {
            Some(_) => membership_dialog_hints(),
            None => {
                let count = self.selection().len();
                let run =
                    (count > 0).then(|| format!("{} {count}", ctx.core.flags.action.as_str()));
                packages_hints(run, self.selected_entry().is_some())
            }
        }
    }

    /// The membership dialog takes the mouse: its buttons and rows work, and the browser
    /// behind it does not.
    fn mouse_blocked(&self) -> bool {
        false
    }

    fn handle_mouse(&mut self, ev: MouseEvent, ctx: &mut AppContext) -> Transition {
        self.sync(ctx);
        if self.popup.is_some() {
            return self.popup_click(ev, ctx);
        }
        if let Some(down) = wheel_over(ev, self.detail_rect) {
            let colors = ThemeColors::from_theme(ctx.core.theme);
            self.scroll = wheel_scroll(&self.detail, self.detail_rect, &colors, self.scroll, down);
            return Transition::Handled;
        }
        let on_hint = self
            .hint_rect
            .is_some_and(|hint| hint.contains(Position::new(ev.column, ev.row)));
        if on_hint && ev.kind == MouseEventKind::Down(MouseButton::Left) {
            return self.open_fix();
        }
        let len = self.packages.rows().len();
        let Some(click) = click_list(&mut self.list, ev, len) else {
            return Transition::None;
        };
        // A click on the checkbox marks the row; a click on the row the cursor already
        // rests on marks it too, as Space does.
        let marks = mark_hit(&self.packages.rows()[click.row], click.col, true);
        if marks || click.activate {
            return self.handle_key(input::key(KeyCode::Char(' ')), ctx);
        }
        Transition::Handled
    }

    fn prepare(&mut self, area: Rect) {
        let rects = screen_rects(area, Shape::TwoPane);
        self.content = area;
        self.detail_rect = rects.detail;
        self.list.prepare(rects.list, self.packages.rows().len());
        self.hint_rect = self
            .blank
            .as_ref()
            .and_then(|blank| empty_hint_rect(list_panel::inner(rects.list), &empty_state(blank)));
    }

    fn render(&mut self, frame: &mut Frame, area: Rect, ctx: &AppContext) {
        let colors = ThemeColors::from_theme(ctx.core.theme);
        let icons = Icons::for_set(ctx.core.icons);
        let rects = screen_rects(area, Shape::TwoPane);

        let width = list_panel::inner(rects.list).width;
        let lines: Vec<Line> = self
            .packages
            .rows()
            .iter()
            .enumerate()
            .map(|(i, row)| package_line(row, i == self.list.cursor, true, icons, &colors, width))
            .collect();
        let title = view::packages_title(self.packages.marked().len(), self.packages.rows().len());
        let empty = self.blank.as_ref().map(empty_state);
        render_list_panel(frame, rects.list, &colors, &title, lines, &self.list, empty);

        render_detail_scrolled(
            frame,
            rects.detail,
            &colors,
            &self.detail_title(),
            &self.detail,
            self.scroll,
        );

        if let Some(membership) = &self.popup {
            view::render_membership(frame, area, &colors, icons, membership);
        }
    }
}

impl PackagesComponent {
    /// A click on a button of the membership dialog sends the button's key. A click on a
    /// profile's row moves the cursor there; a second click on the cursor's row toggles it,
    /// as Space does. The wheel moves the cursor a row. Other clicks change nothing.
    fn popup_click(&mut self, ev: MouseEvent, ctx: &mut AppContext) -> Transition {
        let colors = ThemeColors::from_theme(ctx.core.theme);
        let icons = Icons::for_set(ctx.core.icons);
        let Some(membership) = self.popup.as_ref() else {
            return Transition::None;
        };
        let shown = view::visible_profiles(
            self.content.height,
            membership.profiles.len(),
            membership.cursor,
        );
        let dialog = view::membership_dialog(membership, shown.clone(), &colors, icons);
        let cursor = membership.cursor;
        let button = dialog_click(self.content, &dialog, ev);
        // Body lines are the profiles shown, in order, then the hint line.
        let row = dialog_row_click(self.content, &dialog, ev)
            .filter(|&line| line < shown.len())
            .map(|line| shown.start + line);
        let wheel = wheel_over(ev, dialog_body_rect(self.content, &dialog));

        if let Some(key) = button {
            return self.handle_key(key, ctx);
        }
        if let Some(row) = row {
            if row == cursor {
                return self.handle_key(input::key(KeyCode::Char(' ')), ctx);
            }
            if let Some(membership) = self.popup.as_mut() {
                membership.cursor = row;
            }
            return Transition::Handled;
        }
        if let Some(down) = wheel {
            let code = if down { KeyCode::Down } else { KeyCode::Up };
            return self.handle_key(input::key(code), ctx);
        }
        Transition::Handled
    }

    /// The profile membership dialog open over the browser, if any.
    pub fn popup(&self) -> Option<&Membership> {
        self.popup.as_ref()
    }

    /// The names of the packages the user marked, in list order.
    pub fn marked(&self) -> Vec<String> {
        self.packages.marked()
    }

    /// The rows as drawn: one per package, in name order.
    pub fn rows(&self) -> &[PackageRow] {
        self.packages.rows()
    }

    /// The entry of the package under the cursor. `None` when the list is empty.
    fn selected_entry(&self) -> Option<&PackageEntry> {
        self.entries.get(self.list.cursor)
    }

    /// The packages Enter runs: the marked ones, else the cursor's package.
    fn selection(&self) -> Vec<String> {
        let marked = self.packages.marked();
        if !marked.is_empty() {
            return marked;
        }
        self.selected_entry()
            .map(|entry| vec![entry.name.clone()])
            .unwrap_or_default()
    }

    /// Whether `c` is the key the empty state's hint names. Only an empty list has one.
    fn blank_key(&self, c: char) -> bool {
        self.blank
            .as_ref()
            .is_some_and(|blank| blank.fix.hint().0 == c.to_string())
    }

    /// Opens the screen that fixes the empty list: settings when no packages folder is set,
    /// the new-package wizard when the folder holds no packages.
    fn open_fix(&self) -> Transition {
        match self.blank.as_ref().map(|blank| blank.fix) {
            Some(Fix::Settings) => Transition::Push(Box::new(SettingsComponent::default())),
            Some(Fix::NewPackage) => Transition::Push(Box::new(NewPackageWizard::default())),
            None => Transition::Handled,
        }
    }

    /// Opens the ignore editor on the cursor's package.
    fn open_ignore(&self, ctx: &mut AppContext) -> Transition {
        let Some(entry) = self.selected_entry() else {
            return Transition::Handled;
        };
        match IgnoreEditComponent::open(entry.name.clone(), entry.dir.clone(), ctx.core.theme) {
            Ok(editor) => Transition::Push(Box::new(editor)),
            Err(err) => {
                ctx.show_error_toast(err.to_string());
                Transition::Handled
            }
        }
    }

    /// Runs the selection through the confirm screen.
    fn run(&self, ctx: &mut AppContext) -> Transition {
        let names = self.selection();
        if names.is_empty() {
            return Transition::Handled;
        }
        Transition::Push(Box::new(ConfirmComponent::new(names, ctx)))
    }

    /// The detail title: the cursor's package name, or `packages` when there are none.
    fn detail_title(&self) -> String {
        match self.selected_entry() {
            Some(entry) => entry.name.clone(),
            None => "packages".to_string(),
        }
    }

    /// Rebuilds the rows and the detail if anything they depend on has changed.
    fn sync(&mut self, ctx: &AppContext) {
        let key = self.fingerprint(ctx);
        if self.detail_key != Some(key) {
            self.rebuild(ctx);
            // Reading the statuses changes the cache, so the key is taken again afterwards.
            self.detail_key = Some(self.fingerprint(ctx));
        }
        // A detail starts at its top when the cursor moves onto its row.
        if self.list.cursor != self.scroll_row {
            self.scroll = 0;
            self.scroll_row = self.list.cursor;
        }
        let colors = ThemeColors::from_theme(ctx.core.theme);
        self.scroll = fit_scroll(&self.detail, self.detail_rect, &colors, self.scroll);
    }

    /// Hash of everything the rows and the detail depend on. A change in any of these
    /// means they are rebuilt on the next update.
    fn fingerprint(&self, ctx: &AppContext) -> u64 {
        let mut hasher = DefaultHasher::new();
        self.list.cursor.hash(&mut hasher);
        self.packages.marked().hash(&mut hasher);
        ctx.status_cache.borrow().generation().hash(&mut hasher);
        ctx.core.config.packages.hash(&mut hasher);
        ctx.core.config.target.hash(&mut hasher);
        for profile in ctx.core.profiles.iter() {
            profile.name.hash(&mut hasher);
            profile.packages.hash(&mut hasher);
        }
        hasher.finish()
    }

    /// Rereads the packages, keeping the marks, then builds the rows and the detail.
    /// Statuses are read through the cache, which fills in what it has not seen.
    fn rebuild(&mut self, ctx: &AppContext) {
        let marked: BTreeSet<String> = self.packages.marked().into_iter().collect();
        let global = global_ignore_in(&ctx.home_dir);
        self.entries = ctx.package_entries();
        // A package whose target folder is missing has no status: it reads as unknown, as the
        // review and the run treat it.
        let mut cache = ctx.status_cache.borrow_mut();
        for entry in self.entries.iter().filter(|entry| entry.target.is_dir()) {
            cache.prefetch(entry, Some(&global));
        }
        drop(cache);
        let rows = self
            .entries
            .iter()
            .map(|entry| PackageRow {
                name: entry.name.clone(),
                status: ctx.status_cache.borrow().get(&entry.name),
                mark: Some(match marked.contains(&entry.name) {
                    true => Mark::On,
                    false => Mark::Off,
                }),
                meta: String::new(),
            })
            .collect();
        self.packages = PackageList::new(rows);
        self.blank = self.packages.rows().is_empty().then(|| view::blank(ctx));
        self.list.cursor = self
            .list
            .cursor
            .min(self.packages.rows().len().saturating_sub(1));
        self.detail = self.build_detail(ctx);
    }

    /// The detail of the cursor's package, or the empty state's when there is none.
    fn build_detail(&self, ctx: &AppContext) -> Detail {
        match (
            self.selected_entry(),
            self.packages.rows().get(self.list.cursor),
        ) {
            (Some(entry), Some(row)) => view::package_detail(ctx, entry, row.status),
            _ => view::empty_detail(ctx),
        }
    }
}

/// The empty state drawn for `blank`: its message and the hint that names its fix.
fn empty_state(blank: &Blank) -> Empty<'_> {
    Empty {
        message: &blank.message,
        hint: Some(blank.fix.hint()),
    }
}

/// Applies a key to the membership dialog. Returns whether the dialog stays open. Enter
/// writes the profiles whose membership changed.
fn membership_key(membership: &mut Membership, key: KeyEvent, ctx: &mut AppContext) -> bool {
    if Input::handle_list_navigation(&key, &mut membership.cursor, membership.profiles.len()) {
        return true;
    }
    match key.code {
        KeyCode::Char(' ') => {
            if let Some(row) = membership.profiles.get_mut(membership.cursor) {
                row.now = !row.now;
            }
            true
        }
        KeyCode::Enter => {
            save_membership(membership, ctx);
            false
        }
        KeyCode::Esc => false,
        _ => true,
    }
}

/// Writes every profile whose membership changed, adding the package at the end of its list
/// or removing it. Each file is written on its own. The first failure is shown and the
/// profiles after it are left as they were.
fn save_membership(membership: &Membership, ctx: &mut AppContext) {
    let package = &membership.package;
    let mut changed = 0;
    for row in membership.profiles.iter().filter(|row| row.now != row.was) {
        let Some(mut profile) = ctx.core.profiles.get(&row.name).cloned() else {
            continue;
        };
        if row.now {
            profile.packages.push(package.clone());
        } else {
            profile.packages.retain(|listed| listed != package);
        }
        if let Err(err) = ctx.core.profiles.save(profile) {
            ctx.show_error_toast(err.to_string());
            return;
        }
        changed += 1;
    }
    if changed > 0 {
        ctx.show_toast(format!(
            "updated {package} in {changed} {}",
            plural(changed, "profile", "profiles")
        ));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ui::detail::{detail_inner, detail_rows, WHEEL_ROWS};
    use andre_core::config::Config;
    use crossterm::event::{KeyCode, KeyModifiers, MouseEventKind};
    use ratatui::text::Line;

    /// A wheel notch at column `x`, row `y`.
    fn notch(kind: MouseEventKind, x: u16, y: u16) -> MouseEvent {
        MouseEvent {
            kind,
            column: x,
            row: y,
            modifiers: KeyModifiers::NONE,
        }
    }

    /// A detail of forty rows, more than any pane at 80x24 shows.
    fn long_detail() -> Detail {
        Detail {
            sections: (0..40).map(|i| Line::raw(format!("row{i}"))).collect(),
            ..Detail::default()
        }
    }

    #[test]
    fn test_wheel_over_the_detail_scrolls_it_three_rows() {
        let mut ctx = AppContext::for_tests(Config::default_empty(), std::path::Path::new("/"));
        let mut component = PackagesComponent::default();
        component.sync(&ctx);
        component.detail = long_detail();
        let area = Rect::new(0, 0, 80, 24);
        component.prepare(area);
        let detail = screen_rects(area, Shape::TwoPane).detail;
        let (x, y) = (detail.x + 1, detail.y + 1);
        component.handle_mouse(notch(MouseEventKind::ScrollDown, x, y), &mut ctx);
        assert_eq!(component.scroll, WHEEL_ROWS, "one notch is three rows");
        component.handle_mouse(notch(MouseEventKind::ScrollUp, x, y), &mut ctx);
        assert_eq!(component.scroll, 0, "and a notch up takes them back");
    }

    /// A flat config whose packages folder holds `alpha` and `beta`, each with one file.
    fn fixture() -> (tempfile::TempDir, AppContext) {
        let tmp = tempfile::TempDir::new().unwrap();
        std::fs::create_dir_all(tmp.path().join("src/alpha")).unwrap();
        std::fs::create_dir_all(tmp.path().join("src/beta")).unwrap();
        std::fs::write(tmp.path().join("src/alpha/f"), "").unwrap();
        std::fs::write(tmp.path().join("src/beta/g"), "").unwrap();
        let mut config = Config::default_empty();
        config.set_packages(Some(tmp.path().join("src").display().to_string()));
        let ctx = AppContext::for_tests(config, tmp.path());
        (tmp, ctx)
    }

    /// The cursor moves onto a row whose detail overflows the pane, so a scroll left over from
    /// the last row would show: only the reset clears it.
    #[test]
    fn test_moving_the_cursor_starts_the_detail_at_the_top() {
        let (_tmp, mut ctx) = fixture();
        let mut component = PackagesComponent::default();
        component.sync(&ctx);
        let area = Rect::new(0, 0, 80, 4);
        component.prepare(area);
        let pane = screen_rects(area, Shape::TwoPane).detail;
        component.detail = long_detail();
        component.handle_mouse(
            notch(MouseEventKind::ScrollDown, pane.x + 1, pane.y + 1),
            &mut ctx,
        );
        assert_eq!(component.scroll, WHEEL_ROWS, "the first detail scrolls");

        component.handle_key(KeyEvent::new(KeyCode::Down, KeyModifiers::NONE), &mut ctx);
        component.update(&mut ctx);
        assert_eq!(component.list.cursor, 1, "the cursor is on beta");
        let colors = ThemeColors::from_theme(ctx.core.theme);
        assert!(
            detail_rows(&component.detail, pane, &colors)
                > usize::from(WHEEL_ROWS) + usize::from(detail_inner(pane).height),
            "beta's detail overflows the pane"
        );
        assert_eq!(component.scroll, 0, "beta's detail starts at its top");
    }

    #[test]
    fn test_blank_is_set_only_while_the_list_is_empty() {
        let (_tmp, ctx) = fixture();
        let mut component = PackagesComponent::default();
        component.sync(&ctx);
        assert!(
            component.blank.is_none(),
            "two packages leave nothing to say"
        );

        let mut config = Config::default_empty();
        config.set_packages(None);
        let ctx = AppContext::for_tests(config, std::path::Path::new("/"));
        component.sync(&ctx);
        assert_eq!(component.blank.as_ref().map(|b| b.fix), Some(Fix::Settings));
    }
}
