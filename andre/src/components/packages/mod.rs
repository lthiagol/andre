//! The packages browser. Groups are rows with their packages folded under them. Space marks
//! packages to run, and Enter runs the marks, or else the cursor's package or group, through
//! the confirm screen. `p` opens the profile membership dialog for the cursor's package, and
//! `i` its ignore editor.

mod view;

use std::collections::hash_map::DefaultHasher;
use std::collections::{BTreeMap, BTreeSet};
use std::hash::{Hash, Hasher};

use andre_core::ignore::global_ignore_in;
use andre_core::profile::ProfileSet;
use andre_core::{discover_packages, PackageEntry};
use crossterm::event::{KeyCode, KeyEvent, MouseEvent};
use ratatui::{layout::Rect, text::Line, Frame};

use crate::components::confirm::ConfirmComponent;
use crate::components::ignore::IgnoreEditComponent;
use crate::components::{grouped_name, AppContext, Component, InputMode, Transition};
use crate::input::{self, Input};
use crate::theme::ThemeColors;
use crate::ui::detail::{fit_scroll, render_detail_scrolled, wheel_over, wheel_scroll, Detail};
use crate::ui::dialog::{dialog_body_rect, dialog_click, dialog_row_click};
use crate::ui::footer::{membership_dialog_hints, packages_hints, Hint};
use crate::ui::icons::Icons;
use crate::ui::layout::{screen_rects, Shape};
use crate::ui::list_panel::{self, click_list, render_list_panel, Empty, ListView};
use crate::ui::plural;
use crate::ui::tree::{click_key, tree_line, Mark, Toggle, Tree, TreeGroup, TreePackage, TreeRow};

/// The profile membership dialog open over the browser.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Membership {
    /// `group/package`.
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
    /// Groups, folds and marks. Built by `rebuild`.
    tree: Tree,
    /// The tree's visible rows, as of the last change to the tree.
    rows: Vec<TreeRow>,
    /// Every package in each configured group, in name order. Enter on a group row runs these.
    listing: BTreeMap<String, Vec<String>>,
    /// The cursor row's facts. Built by `rebuild`.
    detail: Detail,
    /// What `rows` and `detail` were built from. `None` until the first update.
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
        if Input::handle_list_navigation(&key, &mut self.list.cursor, self.rows.len()) {
            return Transition::Handled;
        }
        match key.code {
            KeyCode::Char(' ') => {
                if let Toggle::Changed = self.tree.toggle_mark(self.list.cursor) {
                    self.rows = self.tree.rows();
                }
            }
            KeyCode::Left => {
                // On a package, Left moves to its group's header. On a header, it folds the group.
                let header = self.group_header(self.list.cursor);
                if header != self.list.cursor {
                    self.list.cursor = header;
                } else {
                    self.tree.fold(header, true);
                    self.rows = self.tree.rows();
                }
            }
            KeyCode::Right => {
                self.tree.fold(self.list.cursor, false);
                self.rows = self.tree.rows();
            }
            KeyCode::Char('p') => {
                if let Some((group, package)) = self.selected_package() {
                    let package = format!("{group}/{package}");
                    self.popup = Some(Membership::open(package, &ctx.core.profiles));
                }
            }
            KeyCode::Char('i') => return self.open_ignore(ctx),
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
                let count = self.selection_pairs().len();
                let run =
                    (count > 0).then(|| format!("{} {count}", ctx.core.flags.action.as_str()));
                packages_hints(
                    run,
                    !self.rows.is_empty(),
                    self.selected_package().is_some(),
                )
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
        let len = self.rows.len();
        let Some(click) = click_list(&mut self.list, ev, len) else {
            return Transition::None;
        };
        match click_key(&self.rows[click.row], click, true, Some(KeyCode::Char(' '))) {
            Some(code) => self.handle_key(input::key(code), ctx),
            None => Transition::Handled,
        }
    }

    fn prepare(&mut self, area: Rect) {
        let rects = screen_rects(area, Shape::TwoPane);
        self.content = area;
        self.detail_rect = rects.detail;
        self.list.prepare(rects.list, self.rows.len());
    }

    fn render(&mut self, frame: &mut Frame, area: Rect, ctx: &AppContext) {
        let colors = ThemeColors::from_theme(ctx.core.theme);
        let icons = Icons::for_set(ctx.core.icons);
        let rects = screen_rects(area, Shape::TwoPane);

        let width = list_panel::inner(rects.list).width;
        let lines: Vec<Line> = self
            .rows
            .iter()
            .enumerate()
            .map(|(i, row)| tree_line(row, i == self.list.cursor, icons, &colors, width))
            .collect();
        let total: usize = self.listing.values().map(Vec::len).sum();
        let title = view::packages_title(self.tree.marked().len(), total);
        let empty = Empty {
            message: "no groups in andre.yml",
            hint: None,
        };
        render_list_panel(
            frame,
            rects.list,
            &colors,
            &title,
            lines,
            &self.list,
            Some(empty),
        );

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

    /// The `(group, package)` pairs marked by the user, in config order.
    pub fn marked(&self) -> Vec<(String, String)> {
        self.tree.marked()
    }

    /// The rows as drawn: each group, then its packages unless it is folded.
    pub fn rows(&self) -> &[TreeRow] {
        &self.rows
    }

    /// The package under the cursor, as `(group, package)`. `None` on a group row.
    fn selected_package(&self) -> Option<(String, String)> {
        match self.rows.get(self.list.cursor) {
            Some(TreeRow::Package { group, name, .. }) => Some((group.clone(), name.clone())),
            _ => None,
        }
    }

    /// The row of the group header that `row` sits under, or `row` when it is a header.
    fn group_header(&self, row: usize) -> usize {
        self.rows
            .iter()
            .take(row + 1)
            .rposition(|r| matches!(r, TreeRow::Group { .. }))
            .unwrap_or(row)
    }

    /// The packages Enter runs, as `(group, package)`: the marked ones, else the cursor's
    /// package, else every package of the cursor's group.
    fn selection_pairs(&self) -> Vec<(String, String)> {
        let marked = self.tree.marked();
        if !marked.is_empty() {
            return marked;
        }
        match self.rows.get(self.list.cursor) {
            Some(TreeRow::Package { group, name, .. }) => vec![(group.clone(), name.clone())],
            Some(TreeRow::Group { name, .. }) => self
                .listing
                .get(name)
                .into_iter()
                .flatten()
                .map(|package| (name.clone(), package.clone()))
                .collect(),
            None => Vec::new(),
        }
    }

    /// Opens the ignore editor on the cursor's package. A package without a folder says so.
    fn open_ignore(&self, ctx: &mut AppContext) -> Transition {
        let Some((group, package)) = self.selected_package() else {
            return Transition::Handled;
        };
        let source = ctx
            .core
            .config
            .get_group_source(&group, &ctx.config_dir, &ctx.home_dir);
        let Some(dir) = source
            .map(|source| source.join(&package))
            .filter(|dir| dir.is_dir())
        else {
            ctx.show_error_toast(format!("{group}/{package} has no folder"));
            return Transition::Handled;
        };
        match IgnoreEditComponent::open(format!("{group}/{package}"), dir, ctx.core.theme) {
            Ok(editor) => Transition::Push(Box::new(editor)),
            Err(err) => {
                ctx.show_error_toast(err.to_string());
                Transition::Handled
            }
        }
    }

    /// Runs the selection through the confirm screen. A group with no packages says so.
    fn run(&self, ctx: &mut AppContext) -> Transition {
        let pairs = self.selection_pairs();
        if pairs.is_empty() {
            if let Some(TreeRow::Group { name, .. }) = self.rows.get(self.list.cursor) {
                ctx.show_error_toast(format!("{name} has no packages"));
            }
            return Transition::Handled;
        }
        let names = pairs
            .into_iter()
            .map(|(group, package)| grouped_name(&group, &package))
            .collect();
        Transition::Push(Box::new(ConfirmComponent::new(names, ctx)))
    }

    /// The detail title: `group/package` for a package, the group's name for a group.
    fn detail_title(&self) -> String {
        match self.rows.get(self.list.cursor) {
            Some(TreeRow::Package { group, name, .. }) => format!("{group}/{name}"),
            Some(TreeRow::Group { name, .. }) => name.clone(),
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
        self.tree.marked().hash(&mut hasher);
        ctx.status_cache.borrow().generation().hash(&mut hasher);
        for (name, group) in &ctx.core.config.groups {
            name.hash(&mut hasher);
            group.source.hash(&mut hasher);
            group.target.hash(&mut hasher);
            group.ignore.hash(&mut hasher);
        }
        for profile in ctx.core.profiles.iter() {
            profile.name.hash(&mut hasher);
            profile.packages.hash(&mut hasher);
        }
        hasher.finish()
    }

    /// Rescans the groups, keeping the marks and folds, then builds the rows and the detail.
    /// Statuses are read through the cache, which fills in what it has not seen.
    fn rebuild(&mut self, ctx: &AppContext) {
        let marked: BTreeSet<(String, String)> = self.tree.marked().into_iter().collect();
        let global = global_ignore_in(&ctx.home_dir);
        let entries = ctx.package_entries();
        let mut groups = Vec::new();
        let mut listing = BTreeMap::new();
        for name in ctx.core.config.groups.keys() {
            let source = ctx
                .core
                .config
                .get_group_source(name, &ctx.config_dir, &ctx.home_dir);
            let target = ctx
                .core
                .config
                .get_group_target(name, &ctx.config_dir, &ctx.home_dir);
            let names = source.as_deref().map(discover_packages).unwrap_or_default();
            if let (Some(_), Some(_)) = (&source, &target) {
                let group_entries: Vec<PackageEntry> = entries
                    .iter()
                    .filter(|entry| entry.name.starts_with(&format!("{name}/")))
                    .cloned()
                    .collect();
                ctx.status_cache
                    .borrow_mut()
                    .prefetch_all(&group_entries, Some(&global));
            }
            let packages = names
                .iter()
                .map(|package| TreePackage {
                    name: package.clone(),
                    status: ctx.status_cache.borrow().get(&grouped_name(name, package)),
                    mark: Some(if marked.contains(&(name.clone(), package.clone())) {
                        Mark::On
                    } else {
                        Mark::Off
                    }),
                    meta: String::new(),
                })
                .collect();
            groups.push(TreeGroup {
                name: name.clone(),
                packages,
            });
            listing.insert(name.clone(), names);
        }

        self.tree.replace_groups(groups);
        self.listing = listing;
        self.rows = self.tree.rows();
        self.list.cursor = self.list.cursor.min(self.rows.len().saturating_sub(1));
        self.detail = self.build_detail(ctx);
    }

    /// The detail of the cursor's row: the package's facts, or the group's summary.
    fn build_detail(&self, ctx: &AppContext) -> Detail {
        match self.rows.get(self.list.cursor) {
            Some(TreeRow::Package {
                group,
                name,
                status,
                ..
            }) => view::package_detail(ctx, group, name, *status),
            Some(TreeRow::Group { name, .. }) => {
                let statuses: Vec<_> = self
                    .listing
                    .get(name)
                    .into_iter()
                    .flatten()
                    .map(|package| ctx.status_cache.borrow().get(&grouped_name(name, package)))
                    .collect();
                let marked = self.tree.marked().iter().filter(|(g, _)| g == name).count();
                view::group_detail(ctx, name, &statuses, marked)
            }
            None => view::empty_detail(ctx),
        }
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

    /// One group `dots` with packages `alpha` and `beta`, each with one file.
    fn fixture() -> (tempfile::TempDir, AppContext) {
        let tmp = tempfile::TempDir::new().unwrap();
        std::fs::create_dir_all(tmp.path().join("src/alpha")).unwrap();
        std::fs::create_dir_all(tmp.path().join("src/beta")).unwrap();
        std::fs::create_dir_all(tmp.path().join("tgt")).unwrap();
        std::fs::write(tmp.path().join("src/alpha/f"), "").unwrap();
        std::fs::write(tmp.path().join("src/beta/g"), "").unwrap();
        let mut config = Config::default_empty();
        config.add_group(
            "dots".into(),
            tmp.path().join("src").display().to_string(),
            tmp.path().join("tgt").display().to_string(),
        );
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
        assert_eq!(component.list.cursor, 1, "the cursor is on alpha");
        let colors = ThemeColors::from_theme(ctx.core.theme);
        assert!(
            detail_rows(&component.detail, pane, &colors)
                > usize::from(WHEEL_ROWS) + usize::from(detail_inner(pane).height),
            "alpha's detail overflows the pane"
        );
        assert_eq!(component.scroll, 0, "alpha's detail starts at its top");
    }
}
