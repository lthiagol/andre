//! The stow status screen. Every configured group is a row with its packages folded under it,
//! each package with its status glyph. `f` filters the rows to one profile, `r` reads every
//! status again, and the detail shows the cursor's status with the fix for a conflict or a
//! missing package.

mod view;

use std::collections::hash_map::DefaultHasher;
use std::collections::BTreeMap;
use std::hash::{Hash, Hasher};

use andre_core::ignore::global_ignore_in;
use andre_core::profile::{known_packages, Origin};
use andre_core::{discover_packages, IconSet, StowStatus};
use crossterm::event::{KeyCode, KeyEvent, MouseButton, MouseEvent, MouseEventKind};
use ratatui::{
    layout::{Position, Rect},
    text::Line,
    Frame,
};

use crate::components::{AppContext, Component, InputMode, Transition};
use crate::input::{self, step_cursor, Input};
use crate::theme::ThemeColors;
use crate::ui::detail::{fit_scroll, render_detail_scrolled, wheel_over, wheel_scroll, Detail};
use crate::ui::footer::{status_filter_hints, status_hints, Hint};
use crate::ui::icons::Icons;
use crate::ui::layout::{screen_rects, Shape};
use crate::ui::list_panel::{self, click_list, render_list_panel, Empty, ListView};
use crate::ui::tree::{
    click_key, tree_line_without_checkboxes, Tree, TreeGroup, TreePackage, TreeRow,
};

/// The profile dropdown open over the screen.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FilterMenu {
    /// `None` is every package. The rest are the profiles, in name order.
    pub options: Vec<Option<String>>,
    pub cursor: usize,
}

#[derive(Default)]
pub struct StatusComponent {
    /// The cursor and the window over the rows.
    list: ListView,
    /// Rows the list shows at once, as of the last `prepare`. Page keys move by half of it.
    viewport_height: usize,
    /// Groups and their folds. Built by `rebuild`; the tree holds no marks.
    tree: Tree,
    /// The tree's visible rows, with folded groups summarised. Built by `rebuild` and by folds.
    rows: Vec<TreeRow>,
    /// The statuses of each group's shown packages, in row order. Built by `rebuild`.
    statuses: BTreeMap<String, Vec<Option<StowStatus>>>,
    /// The counts for the chip row. Built by `rebuild`.
    summary: view::Summary,
    /// The cursor's facts. Built by `rebuild`.
    detail: Detail,
    /// What `rows` and `detail` were built from. `None` until the first update.
    detail_key: Option<u64>,
    /// Show only the packages this profile resolves to. `None` lists every package.
    profile: Option<String>,
    filter: Option<FilterMenu>,
    /// The chip row, and the content area, as the last `prepare` laid them out. A click on
    /// the profile chip opens the dropdown, which is drawn under the chip row.
    chip_row: Option<Rect>,
    content: Rect,
    /// The detail pane as the last `prepare` laid it out.
    detail_rect: Rect,
    /// Rows the detail is scrolled down. It resets when the cursor moves.
    scroll: u16,
    /// The cursor row `scroll` belongs to.
    scroll_row: usize,
}

impl StatusComponent {
    /// The status of the packages in `name`, as the profiles screen's `c` opens it.
    pub fn for_profile(name: &str) -> Self {
        Self {
            profile: Some(name.to_string()),
            ..Self::default()
        }
    }

    /// The profile the rows are filtered to, if any.
    pub fn profile(&self) -> Option<&str> {
        self.profile.as_deref()
    }
}

impl Component for StatusComponent {
    fn id(&self) -> &'static str {
        "Status"
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }

    fn input_mode(&self) -> InputMode {
        match self.filter {
            Some(_) => InputMode::Modal,
            None => InputMode::Normal,
        }
    }

    fn update(&mut self, ctx: &mut AppContext) -> Transition {
        self.sync(ctx);
        Transition::None
    }

    fn handle_key(&mut self, key: KeyEvent, ctx: &mut AppContext) -> Transition {
        // Keys can arrive before the next update, so the rows are brought up to date first.
        self.sync(ctx);
        if let Some(menu) = self.filter.take() {
            self.filter_key(menu, key, ctx);
            return Transition::Handled;
        }
        if Input::handle_list_scroll(
            &key,
            &mut self.list.cursor,
            self.rows.len(),
            self.viewport_height,
        ) {
            return Transition::Handled;
        }
        let icons = Icons::for_set(ctx.core.icons);
        match key.code {
            KeyCode::Left => {
                // On a package, Left moves to its group's header. On a header, it folds the group.
                let header = self.group_header(self.list.cursor);
                if header != self.list.cursor {
                    self.list.cursor = header;
                } else {
                    self.tree.fold(header, true);
                    self.rows = self.flatten(icons);
                }
            }
            KeyCode::Right => {
                self.tree.fold(self.list.cursor, false);
                self.rows = self.flatten(icons);
            }
            KeyCode::Char('f') => self.open_filter(ctx),
            KeyCode::Char('r') => {
                ctx.status_cache.borrow_mut().invalidate_all();
                self.sync(ctx);
            }
            KeyCode::Enter | KeyCode::Esc => return Transition::Pop,
            _ => return Transition::None,
        }
        Transition::Handled
    }

    fn hints(&self, _ctx: &AppContext) -> Vec<Hint> {
        match self.filter {
            Some(_) => status_filter_hints(),
            None => status_hints(),
        }
    }

    /// The dropdown takes the mouse while it is open (see `filter_click`). The list and the
    /// profile chip take it otherwise, so none of them is blocked here.
    fn mouse_blocked(&self) -> bool {
        false
    }

    fn handle_mouse(&mut self, ev: MouseEvent, ctx: &mut AppContext) -> Transition {
        self.sync(ctx);
        if self.filter.is_some() {
            return self.filter_click(ev, ctx);
        }
        let icons = Icons::for_set(ctx.core.icons);
        let chip = self
            .chip_row
            .and_then(|row| view::profile_chip_rect(row, self.profile.as_deref(), icons));
        if chip.is_some_and(|rect| rect.contains(Position::new(ev.column, ev.row))) {
            if ev.kind == MouseEventKind::Down(MouseButton::Left) {
                self.open_filter(ctx);
            }
            return Transition::Handled;
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
        match click_key(&self.rows[click.row], click, false, None) {
            Some(code) => self.handle_key(input::key(code), ctx),
            None => Transition::Handled,
        }
    }

    fn prepare(&mut self, area: Rect) {
        let rects = screen_rects(area, Shape::ChipTwoPane);
        self.content = area;
        self.chip_row = rects.chip;
        self.detail_rect = rects.detail;
        self.viewport_height = list_panel::inner(rects.list).height.into();
        self.list.prepare(rects.list, self.rows.len());
    }

    fn render(&mut self, frame: &mut Frame, area: Rect, ctx: &AppContext) {
        let colors = ThemeColors::from_theme(ctx.core.theme);
        let icons = Icons::for_set(ctx.core.icons);
        let rects = screen_rects(area, Shape::ChipTwoPane);

        if let Some(chip) = rects.chip {
            view::render_chip_row(
                frame,
                chip,
                &colors,
                icons,
                self.profile.as_deref(),
                &self.summary,
            );
        }

        let width = list_panel::inner(rects.list).width;
        let lines: Vec<Line> = self
            .rows
            .iter()
            .enumerate()
            .map(|(i, row)| {
                tree_line_without_checkboxes(row, i == self.list.cursor, icons, &colors, width)
            })
            .collect();
        let title = view::list_title(self.profile.as_deref(), self.summary.total);
        let empty = Empty {
            message: view::empty_message(self.profile.as_deref()),
            hint: view::empty_hint(self.profile.as_deref()),
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

        if let (Some(menu), Some(chip)) = (&self.filter, rects.chip) {
            view::render_filter(frame, area, chip, &colors, menu);
        }
    }
}

impl StatusComponent {
    /// Brings the rows and the detail up to date. A first visit reads every status afresh;
    /// after that the statuses are kept until `r` or the next visit.
    fn sync(&mut self, ctx: &AppContext) {
        if self.detail_key.is_none() {
            ctx.status_cache.borrow_mut().invalidate_all();
        }
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
        self.profile.hash(&mut hasher);
        ctx.status_cache.borrow().generation().hash(&mut hasher);
        ctx.core.theme.name().hash(&mut hasher);
        matches!(ctx.core.icons, IconSet::Nerd).hash(&mut hasher);
        for (name, group) in &ctx.core.config.groups {
            name.hash(&mut hasher);
            group.source.hash(&mut hasher);
            group.target.hash(&mut hasher);
        }
        for profile in ctx.core.profiles.iter() {
            profile.name.hash(&mut hasher);
            profile.includes.hash(&mut hasher);
            profile.packages.hash(&mut hasher);
        }
        hasher.finish()
    }

    /// Reads the statuses through the cache and builds the rows, the counts and the detail.
    /// A group with no package to show is left out. A package the profile reaches through an
    /// include says which one.
    fn rebuild(&mut self, ctx: &AppContext) {
        let only = self.profile.as_deref().map(|name| profile_refs(ctx, name));
        let global = global_ignore_in(&ctx.home_dir);
        let mut groups = Vec::new();
        let mut statuses: BTreeMap<String, Vec<Option<StowStatus>>> = BTreeMap::new();
        for name in ctx.core.config.groups.keys() {
            let source = ctx
                .core
                .config
                .get_group_source(name, &ctx.config_dir, &ctx.home_dir);
            let target = ctx
                .core
                .config
                .get_group_target(name, &ctx.config_dir, &ctx.home_dir);
            let discovered = source.as_deref().map(discover_packages).unwrap_or_default();
            if let (Some(source), Some(target)) = (&source, &target) {
                ctx.status_cache.borrow_mut().prefetch_group(
                    source,
                    target,
                    name,
                    &discovered,
                    Some(&global),
                );
            }

            let mut shown = Vec::new();
            for package in &discovered {
                let via = match &only {
                    None => None,
                    Some(refs) => match refs.get(&format!("{name}/{package}")) {
                        Some(via) => via.clone(),
                        None => continue,
                    },
                };
                // A group whose source or target is missing has no status to read. Its packages
                // show `·`, as on the packages browser.
                let status = match (&source, &target) {
                    (Some(_), Some(_)) => ctx.status_cache.borrow().get(name, package),
                    _ => None,
                };
                shown.push(TreePackage {
                    name: package.clone(),
                    status,
                    mark: None,
                    meta: via.map(|via| format!("via {via}")).unwrap_or_default(),
                });
            }
            if shown.is_empty() {
                continue;
            }
            statuses.insert(name.clone(), shown.iter().map(|p| p.status).collect());
            groups.push(TreeGroup {
                name: name.clone(),
                packages: shown,
            });
        }

        let all: Vec<Option<StowStatus>> = statuses
            .values()
            .flat_map(|group| group.iter().copied())
            .collect();
        self.summary = view::summarize(&all);
        self.tree.replace_groups(groups);
        self.statuses = statuses;
        self.rows = self.flatten(Icons::for_set(ctx.core.icons));
        self.list.cursor = self.list.cursor.min(self.rows.len().saturating_sub(1));
        self.detail = self.build_detail(ctx);
    }

    /// The visible rows: the tree's, with each folded group's row showing its worst status.
    fn flatten(&self, icons: &Icons) -> Vec<TreeRow> {
        self.tree
            .rows()
            .into_iter()
            .map(|row| match row {
                TreeRow::Group {
                    name,
                    folded: true,
                    mark,
                    meta,
                } => {
                    let meta = self
                        .statuses
                        .get(&name)
                        .map_or(meta, |statuses| view::folded_meta(statuses, icons));
                    TreeRow::Group {
                        name,
                        folded: true,
                        mark,
                        meta,
                    }
                }
                row => row,
            })
            .collect()
    }

    /// The detail of the cursor's row: the package's status, or the group's summary.
    fn build_detail(&self, ctx: &AppContext) -> Detail {
        match self.rows.get(self.list.cursor) {
            Some(TreeRow::Package {
                group,
                name,
                status,
                ..
            }) => view::package_detail(ctx, group, name, *status),
            Some(TreeRow::Group { name, .. }) => {
                let statuses = self.statuses.get(name).map_or(&[][..], Vec::as_slice);
                view::group_detail(ctx, name, statuses)
            }
            None => view::empty_detail(ctx, self.profile.as_deref()),
        }
    }

    /// The detail title: `group/package` for a package, the group's name for a group.
    fn detail_title(&self) -> String {
        match self.rows.get(self.list.cursor) {
            Some(TreeRow::Package { group, name, .. }) => format!("{group}/{name}"),
            Some(TreeRow::Group { name, .. }) => name.clone(),
            None => "status".to_string(),
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

    /// Opens the profile dropdown with the current choice under the cursor.
    fn open_filter(&mut self, ctx: &AppContext) {
        let options: Vec<Option<String>> = std::iter::once(None)
            .chain(
                ctx.core
                    .profiles
                    .iter()
                    .map(|profile| Some(profile.name.clone())),
            )
            .collect();
        let cursor = options
            .iter()
            .position(|option| *option == self.profile)
            .unwrap_or(0);
        self.filter = Some(FilterMenu { options, cursor });
    }

    /// The dropdown's mouse rules, as a list's: the wheel over it moves the cursor a row, and
    /// a click on an option that is not under the cursor moves the cursor there. A click on
    /// the cursor's option picks it, as Enter does. Clicks and wheel notches elsewhere do
    /// nothing: the dropdown stays open until Enter or Esc.
    fn filter_click(&mut self, ev: MouseEvent, ctx: &mut AppContext) -> Transition {
        let Some(row) = self.chip_row else {
            return Transition::Handled;
        };
        let content = self.content;
        let Some(menu) = self.filter.as_mut() else {
            return Transition::Handled;
        };
        let rect = view::filter_rect(content, row, menu);
        let count = menu.options.len();
        if let Some(down) = wheel_over(ev, rect) {
            menu.cursor = step_cursor(menu.cursor, count, down);
            return Transition::Handled;
        }
        let Some(index) = view::option_at(rect, count, ev) else {
            return Transition::Handled;
        };
        if index != menu.cursor {
            menu.cursor = index;
            return Transition::Handled;
        }
        self.handle_key(input::key(KeyCode::Enter), ctx)
    }

    /// Applies a key to the profile dropdown. Enter picks the cursor's choice and Esc keeps
    /// the current one; both close it. Other keys move its cursor and keep it open.
    fn filter_key(&mut self, mut menu: FilterMenu, key: KeyEvent, ctx: &AppContext) {
        match key.code {
            KeyCode::Enter => {
                self.profile = menu.options[menu.cursor].clone();
                self.list.cursor = 0;
                self.sync(ctx);
            }
            KeyCode::Esc => {}
            _ => {
                Input::handle_list_navigation(&key, &mut menu.cursor, menu.options.len());
                self.filter = Some(menu);
            }
        }
    }
}

/// The `group/package` refs profile `name` runs, each with the include that brings it in, or
/// `None` for the profile's own. A profile that does not resolve runs none.
fn profile_refs(ctx: &AppContext, name: &str) -> BTreeMap<String, Option<String>> {
    let known = known_packages(&ctx.core.config, &ctx.config_dir, &ctx.home_dir);
    match ctx.core.profiles.resolve(name, &known) {
        Ok(resolved) => resolved
            .entries
            .iter()
            .filter(|entry| !entry.missing)
            .map(|entry| (entry.package.clone(), via(&entry.origin)))
            .collect(),
        Err(_) => BTreeMap::new(),
    }
}

/// The include a package was reached through, or `None` when the profile lists it itself.
fn via(origin: &Origin) -> Option<String> {
    match origin {
        Origin::Own => None,
        Origin::Included { include, .. } => Some(include.clone()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ui::detail::{detail_inner, detail_rows, WHEEL_ROWS};
    use andre_core::config::Config;
    use crossterm::event::{KeyCode, KeyModifiers};
    use ratatui::text::Line;

    /// One group `dots` with packages `alpha` (file `f`) and `beta` (file `g`). Nothing links
    /// them, so both are unstowed.
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

    fn key(code: KeyCode) -> KeyEvent {
        KeyEvent::new(code, KeyModifiers::NONE)
    }

    /// The package names in the rows, in order.
    fn package_names(component: &StatusComponent) -> Vec<&str> {
        component
            .rows
            .iter()
            .filter_map(|row| match row {
                TreeRow::Package { name, .. } => Some(name.as_str()),
                _ => None,
            })
            .collect()
    }

    /// The `meta` of each package row, in order.
    fn package_metas(component: &StatusComponent) -> Vec<&str> {
        component
            .rows
            .iter()
            .filter_map(|row| match row {
                TreeRow::Package { meta, .. } => Some(meta.as_str()),
                _ => None,
            })
            .collect()
    }

    /// `fixture` with a profile `one` that runs `dots/alpha` and includes `two`, which runs
    /// `dots/beta` and also names a package that is not on disk.
    fn profile_fixture() -> (tempfile::TempDir, AppContext) {
        let (tmp, mut ctx) = fixture();
        let mut profiles = andre_core::profile::ProfileSet::new(tmp.path().join("profiles"));
        let profile =
            |name: &str, includes: &[&str], packages: &[&str]| andre_core::profile::Profile {
                name: name.into(),
                includes: includes.iter().map(|s| s.to_string()).collect(),
                packages: packages.iter().map(|s| s.to_string()).collect(),
                ..Default::default()
            };
        profiles
            .create(profile("two", &[], &["dots/beta", "dots/ghost"]))
            .unwrap();
        profiles
            .create(profile("one", &["two"], &["dots/alpha"]))
            .unwrap();
        ctx.core.profiles = profiles;
        (tmp, ctx)
    }

    /// A component synced to `ctx`, as its first update leaves it.
    fn synced(ctx: &AppContext, profile: Option<&str>) -> StatusComponent {
        let mut component = StatusComponent {
            profile: profile.map(str::to_string),
            ..StatusComponent::default()
        };
        component.sync(ctx);
        component
    }

    #[test]
    fn test_rebuild_lists_each_package_under_its_group() {
        let (_tmp, ctx) = fixture();
        let component = synced(&ctx, None);

        assert_eq!(
            component.rows[0],
            TreeRow::Group {
                name: "dots".into(),
                folded: false,
                mark: None,
                meta: String::new(),
            }
        );
        assert_eq!(package_names(&component), vec!["alpha", "beta"]);
        assert_eq!(component.summary.total, 2);
    }

    #[test]
    fn test_a_group_with_no_packages_is_left_out_and_the_list_is_empty() {
        let tmp = tempfile::TempDir::new().unwrap();
        let ctx = AppContext::for_tests(Config::default_empty(), tmp.path());
        let component = synced(&ctx, None);
        assert!(component.rows.is_empty());
        assert_eq!(component.summary, view::Summary::default());
    }

    #[test]
    fn test_a_read_status_is_kept_until_the_next_visit_or_a_rescan() {
        let (_tmp, ctx) = fixture();
        let mut component = synced(&ctx, None);
        ctx.status_cache
            .borrow_mut()
            .insert(("dots".into(), "alpha".into()), StowStatus::Conflict);
        component.sync(&ctx);
        assert_eq!(
            component.summary.counts,
            vec![(StowStatus::Unstowed, 1), (StowStatus::Conflict, 1)],
            "the cached conflict shows"
        );

        // A new visit reads the disk again: nothing is linked, so alpha is unstowed.
        let visit = synced(&ctx, None);
        assert_eq!(visit.summary.counts, vec![(StowStatus::Unstowed, 2)]);
    }

    #[test]
    fn test_r_reads_the_disk_again() {
        let (tmp, mut ctx) = fixture();
        let mut component = synced(&ctx, None);
        // `alpha` really is stowed now: its file is linked from the target.
        std::os::unix::fs::symlink(tmp.path().join("src/alpha/f"), tmp.path().join("tgt/f"))
            .unwrap();

        component.sync(&ctx);
        assert_eq!(
            component.summary.counts,
            vec![(StowStatus::Unstowed, 2)],
            "the change waits for a rescan"
        );

        component.handle_key(key(KeyCode::Char('r')), &mut ctx);
        assert_eq!(
            component.summary.counts,
            vec![(StowStatus::Stowed, 1), (StowStatus::Unstowed, 1)]
        );
    }

    #[test]
    fn test_a_profile_lists_its_resolved_packages_and_names_the_include_it_came_by() {
        let (_tmp, ctx) = profile_fixture();
        let component = synced(&ctx, Some("one"));

        assert_eq!(
            package_names(&component),
            vec!["alpha", "beta"],
            "in name order"
        );
        assert_eq!(package_metas(&component), vec!["", "via two"]);
        assert_eq!(component.profile(), Some("one"));
    }

    #[test]
    fn test_a_profile_that_does_not_resolve_has_no_rows() {
        let (_tmp, ctx) = profile_fixture();
        let component = synced(&ctx, Some("missing"));
        assert!(component.rows.is_empty());
        assert_eq!(view::empty_message(Some("missing")), "No packages on disk.");
    }

    #[test]
    fn test_for_profile_filters_the_rows_it_builds_on_visit() {
        let (_tmp, mut ctx) = profile_fixture();
        let mut component = StatusComponent::for_profile("two");
        component.update(&mut ctx);
        assert_eq!(package_names(&component), vec!["beta"]);
    }

    #[test]
    fn test_f_opens_the_dropdown_with_every_package_first_and_enter_picks() {
        let (_tmp, mut ctx) = profile_fixture();
        let mut component = synced(&ctx, Some("one"));

        component.handle_key(key(KeyCode::Char('f')), &mut ctx);
        let menu = component.filter.clone().expect("dropdown open");
        assert_eq!(
            menu.options,
            vec![None, Some("one".into()), Some("two".into())]
        );
        assert_eq!(menu.cursor, 1, "starts on the current profile");
        assert_eq!(component.input_mode(), InputMode::Modal);

        component.handle_key(key(KeyCode::Up), &mut ctx);
        component.handle_key(key(KeyCode::Enter), &mut ctx);
        assert!(component.filter.is_none());
        assert_eq!(component.profile(), None, "every package");
        assert_eq!(package_names(&component), vec!["alpha", "beta"]);
        assert_eq!(component.summary.total, 2);
    }

    #[test]
    fn test_esc_closes_the_dropdown_and_keeps_the_profile() {
        let (_tmp, mut ctx) = profile_fixture();
        let mut component = synced(&ctx, Some("one"));

        component.handle_key(key(KeyCode::Char('f')), &mut ctx);
        component.handle_key(key(KeyCode::Down), &mut ctx);
        component.handle_key(key(KeyCode::Esc), &mut ctx);
        assert!(component.filter.is_none());
        assert_eq!(component.profile(), Some("one"));
        assert_eq!(component.input_mode(), InputMode::Normal);
    }

    #[test]
    fn test_left_moves_to_the_header_then_folds_the_group() {
        let (_tmp, mut ctx) = fixture();
        let mut component = synced(&ctx, None);
        component.list.cursor = 2;

        component.handle_key(key(KeyCode::Left), &mut ctx);
        assert_eq!(
            component.list.cursor, 0,
            "on a package, Left goes to the header"
        );
        assert_eq!(component.rows.len(), 3);

        component.handle_key(key(KeyCode::Left), &mut ctx);
        assert_eq!(
            component.rows.len(),
            1,
            "on the header, Left folds the group"
        );
        let icons = Icons::for_set(ctx.core.icons);
        assert_eq!(
            component.rows[0],
            TreeRow::Group {
                name: "dots".into(),
                folded: true,
                mark: None,
                meta: format!("{} 2", icons.status(StowStatus::Unstowed)),
            },
            "a folded group shows its worst status and how many have it"
        );
    }

    #[test]
    fn test_a_conflict_and_a_missing_package_get_a_fix_line() {
        let (_tmp, ctx) = fixture();
        let mut component = synced(&ctx, None);
        ctx.status_cache
            .borrow_mut()
            .insert(("dots".into(), "alpha".into()), StowStatus::Conflict);

        component.list.cursor = 1;
        component.sync(&ctx);
        assert_eq!(component.detail_title(), "dots/alpha");
        let sections = texts(&component.detail.sections);
        assert!(
            sections.last().unwrap().starts_with("Fix:"),
            "conflict has a fix line: {sections:?}"
        );

        component.list.cursor = 2;
        component.sync(&ctx);
        let sections = texts(&component.detail.sections);
        assert!(
            sections.iter().all(|line| !line.starts_with("Fix:")),
            "beta is unstowed, no fix: {sections:?}"
        );
    }

    #[test]
    fn test_an_unread_package_says_so_in_its_headline() {
        let (_tmp, ctx) = fixture();
        let detail = view::package_detail(&ctx, "dots", "alpha", None);
        let headline: String = detail.headline[0]
            .spans
            .iter()
            .map(|span| span.content.as_ref())
            .collect();
        assert!(headline.contains("status not read yet"), "{headline}");
    }

    /// The text of each line.
    fn texts(lines: &[Line]) -> Vec<String> {
        lines
            .iter()
            .map(|line| {
                line.spans
                    .iter()
                    .map(|span| span.content.as_ref())
                    .collect()
            })
            .collect()
    }

    #[test]
    fn test_a_package_whose_target_is_missing_is_unread_and_gets_the_target_fix() {
        let (tmp, ctx) = fixture();
        std::fs::remove_dir(tmp.path().join("tgt")).unwrap();
        let mut component = synced(&ctx, None);
        assert_eq!(package_names(&component), vec!["alpha", "beta"]);
        assert_eq!(
            component.summary.unknown, 2,
            "nothing is read without a target"
        );

        component.list.cursor = 1;
        component.sync(&ctx);
        assert_eq!(component.detail_title(), "dots/alpha");
        assert!(
            texts(&component.detail.headline)[0].contains("status not read yet"),
            "{:?}",
            texts(&component.detail.headline)
        );
        assert!(component
            .detail
            .facts
            .iter()
            .any(|(label, value)| label == "target" && value == "not found"));
        assert_eq!(
            texts(&component.detail.sections),
            vec!["Fix: create the group's target folder, or fix its path in andre.yml."],
        );
    }

    #[test]
    fn test_a_conflict_detail_lists_the_file_in_the_way_before_the_unlinked_ones() {
        let (tmp, ctx) = fixture();
        // A real file sits where alpha's `f` must link. `a_first` is unlinked and sorts first by name.
        std::fs::write(tmp.path().join("tgt/f"), "real").unwrap();
        std::fs::write(tmp.path().join("src/alpha/a_first"), "").unwrap();
        let mut component = synced(&ctx, None);
        component.list.cursor = 1;
        component.sync(&ctx);

        let icons = Icons::for_set(ctx.core.icons);
        let sections = texts(&component.detail.sections);
        assert_eq!(
            sections.len(),
            4,
            "two files, a blank line and the fix: {sections:?}"
        );
        assert!(
            sections[0].starts_with(icons.status(StowStatus::Conflict))
                && sections[0].ends_with("real file"),
            "{:?}",
            sections[0]
        );
        assert!(sections[1].ends_with("not linked"), "{:?}", sections[1]);
        assert_eq!(sections[2], "");
        assert_eq!(
            sections[3],
            "Fix: move the real file out of the way, then stow again."
        );
    }

    #[test]
    fn test_scroll_stops_at_the_last_row_when_rows_fit() {
        let (_tmp, mut ctx) = fixture();
        let mut component = synced(&ctx, None);
        // 80x24: the content area is 16 rows, so every row of the fixture fits.
        component.prepare(Rect::new(0, 0, 80, 16));
        let last = component.rows.len() - 1;

        for _ in 0..10 {
            component.handle_key(key(KeyCode::Down), &mut ctx);
        }
        assert_eq!(component.list.cursor, last);

        component.handle_key(key(KeyCode::Up), &mut ctx);
        assert_eq!(component.list.cursor, last - 1);
    }

    #[test]
    fn test_wheel_over_the_detail_scrolls_it_three_rows() {
        let (_tmp, mut ctx) = fixture();
        let mut component = synced(&ctx, None);
        component.detail = long_detail();
        let area = Rect::new(0, 0, 80, 24);
        component.prepare(area);
        let detail = screen_rects(area, Shape::ChipTwoPane).detail;
        let (x, y) = (detail.x + 1, detail.y + 1);
        component.handle_mouse(notch(MouseEventKind::ScrollDown, x, y), &mut ctx);
        assert_eq!(component.scroll, WHEEL_ROWS, "one notch is three rows");
        component.handle_mouse(notch(MouseEventKind::ScrollUp, x, y), &mut ctx);
        assert_eq!(component.scroll, 0, "and a notch up takes them back");
    }

    /// The cursor moves onto a row whose detail overflows the pane, so a scroll left over from
    /// the last row would show: only the reset clears it.
    #[test]
    fn test_moving_the_cursor_starts_the_detail_at_the_top() {
        let (_tmp, mut ctx) = fixture();
        let mut component = synced(&ctx, None);
        let area = Rect::new(0, 0, 80, 4);
        component.prepare(area);
        let pane = screen_rects(area, Shape::ChipTwoPane).detail;
        component.detail = long_detail();
        component.handle_mouse(
            notch(MouseEventKind::ScrollDown, pane.x + 1, pane.y + 1),
            &mut ctx,
        );
        assert_eq!(component.scroll, WHEEL_ROWS, "the first detail scrolls");

        component.handle_key(key(KeyCode::Down), &mut ctx);
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
}
