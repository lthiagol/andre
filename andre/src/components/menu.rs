use std::borrow::Cow;
use std::collections::hash_map::DefaultHasher;
use std::collections::{BTreeSet, HashMap};
use std::hash::{Hash, Hasher};

use andre_core::bootstrap::{self, Bootstrap};
use andre_core::{discover_packages, EngineKind, StowStatus};
use crossterm::event::{KeyCode, KeyEvent, MouseEvent};
use ratatui::{
    layout::Rect,
    style::{Modifier, Style},
    text::{Line, Span},
    Frame,
};
use unicode_width::UnicodeWidthStr;

use crate::components::confirm::ConfirmComponent;
use crate::components::help::HelpComponent;
use crate::components::packages::PackagesComponent;
use crate::components::profiles::ProfilesComponent;
use crate::components::settings::SettingsComponent;
use crate::components::status::StatusComponent;
use crate::components::target_select::unstow_screen;
use crate::components::wizard::NewPackageWizard;
use crate::components::{AppContext, Component, Transition};
use crate::input;
use crate::theme::ThemeColors;
use crate::ui::detail::{render_detail, Detail, VALUE_WIDTH};
use crate::ui::footer::{screen_hints, Hint};
use crate::ui::icons::Icons;
use crate::ui::layout::{screen_rects, Shape};
use crate::ui::list_panel::{self, click_list, render_list_panel, ListView};
use crate::ui::selection;
use crate::ui::{display_path, elide, plural};

/// The main menu's rows. The eight in [`MenuItem::ALL`] are always there, in jump-digit
/// order (`1`..`8`). `Setup` is the fresh-machine item, shown only while
/// [`bootstrap::find`] names a package, and jumped to with `0`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MenuItem {
    Setup,
    Profiles,
    Stow,
    NewPackage,
    Unstow,
    Status,
    Settings,
    Help,
    Quit,
}

impl MenuItem {
    pub const ALL: [MenuItem; 8] = [
        MenuItem::Profiles,
        MenuItem::Stow,
        MenuItem::NewPackage,
        MenuItem::Unstow,
        MenuItem::Status,
        MenuItem::Settings,
        MenuItem::Help,
        MenuItem::Quit,
    ];

    /// The digit that jumps to the item: `0` for `Setup`, `1`..`8` for the rest.
    pub fn digit(self) -> usize {
        match Self::ALL.iter().position(|item| *item == self) {
            Some(index) => index + 1,
            None => 0,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            MenuItem::Setup => "set up this machine",
            MenuItem::Profiles => "profiles",
            MenuItem::Stow => "packages",
            MenuItem::NewPackage => "new package",
            MenuItem::Unstow => "unstow",
            MenuItem::Status => "verify status",
            MenuItem::Settings => "settings",
            MenuItem::Help => "help",
            MenuItem::Quit => "quit",
        }
    }

    fn icon(self, icons: &Icons) -> &'static str {
        match self {
            MenuItem::Setup => icons.menu_setup,
            MenuItem::Profiles => icons.menu_profiles,
            MenuItem::Stow => icons.link,
            MenuItem::NewPackage => icons.menu_new_package,
            MenuItem::Unstow => icons.unlink,
            MenuItem::Status => icons.check,
            MenuItem::Settings => icons.gear,
            MenuItem::Help => icons.question,
            MenuItem::Quit => icons.power,
        }
    }

    fn description(self) -> &'static str {
        match self {
            MenuItem::Setup => {
                "Link this config folder to ~/.config/andre, so plain andre finds it."
            }
            MenuItem::Profiles => {
                "Named sets of packages. Pick one and run the current action on all of it."
            }
            MenuItem::Stow => "Pick groups and packages, then run the current action.",
            MenuItem::NewPackage => "Create a package from files that already sit in a target.",
            MenuItem::Unstow => "Remove symlinks from a target, one link at a time.",
            MenuItem::Status => "Check every package's links against its source tree.",
            MenuItem::Settings => "Edit the config file, groups, theme, icons and stow options.",
            MenuItem::Help => "Keyboard reference and what each screen does.",
            MenuItem::Quit => "Leave andre. Nothing is changed.",
        }
    }
}

#[derive(Default)]
pub struct MainMenuComponent {
    /// The cursor and the window over the items.
    pub list: ListView,
    /// Built by `update`, read while painting.
    detail: Detail,
    /// Fingerprint of everything `detail` and `bootstrap` were built from. `None` until the
    /// first update.
    detail_key: Option<u64>,
    /// The fresh-machine item, if there is one. Set by `update`.
    bootstrap: Option<Bootstrap>,
}

impl Component for MainMenuComponent {
    fn id(&self) -> &'static str {
        "MainMenu"
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }

    fn handle_key(&mut self, key: KeyEvent, ctx: &mut AppContext) -> Transition {
        let items = self.items();

        if input::Input::is_up(&key) {
            self.list.step(false, items.len());
        } else if input::Input::is_down(&key) {
            self.list.step(true, items.len());
        } else if input::Input::is_quit_key(&key) {
            // q and Ctrl+C: checked before the Char arm so the digit
            // dispatcher never swallows Ctrl+C / Ctrl+1 / etc.
            return Transition::Quit;
        } else if key.code == KeyCode::Enter {
            return self.activate(self.selected(), ctx);
        } else if let KeyCode::Char(c) = key.code {
            // Unmodified digit keys jump to and activate the item they are drawn
            // beside: `0` for set up, `1`..`8` for the rest. A digit with no item
            // (`0` without the set-up item, `9`) is ignored.
            if key.modifiers.is_empty() {
                if let Some(d) = c.to_digit(10) {
                    if let Some(index) = items.iter().position(|item| item.digit() == d as usize) {
                        self.list.cursor = index;
                        return self.activate(items[index], ctx);
                    }
                }
            }
        }

        Transition::None
    }

    fn update(&mut self, ctx: &mut AppContext) -> Transition {
        let key = self.fingerprint(ctx);
        if self.detail_key != Some(key) {
            self.bootstrap = bootstrap::find(
                &ctx.core.config,
                &ctx.config_dir,
                &ctx.home_dir,
                ctx.search_dir.as_deref(),
            )
            .filter(|found| review_can_run(found, &ctx.core.config));
            let items = self.items().len();
            self.list.cursor = self.list.cursor.min(items - 1);
            self.detail = build_detail(self.selected(), ctx, self.bootstrap.as_ref());
            self.detail_key = Some(key);
        }
        Transition::None
    }

    fn hints(&self, _ctx: &AppContext) -> Vec<Hint> {
        let mut hints = screen_hints(self.id());
        for hint in &mut hints {
            if hint.label == "jump" {
                hint.key = jump_keys(self.bootstrap.is_some()).into();
            }
        }
        hints
    }

    fn handle_mouse(&mut self, ev: MouseEvent, ctx: &mut AppContext) -> Transition {
        let len = self.items().len();
        match click_list(&mut self.list, ev, len) {
            Some(click) if click.activate => self.handle_key(input::key(KeyCode::Enter), ctx),
            Some(_) => Transition::Handled,
            None => Transition::None,
        }
    }

    fn prepare(&mut self, area: Rect) {
        let rects = screen_rects(area, Shape::TwoPane);
        self.list.prepare(rects.list, self.items().len());
    }

    fn render(&mut self, frame: &mut Frame, area: Rect, ctx: &AppContext) {
        let colors = ThemeColors::from_theme(ctx.core.theme);
        let icons = Icons::for_set(ctx.core.icons);
        let items = self.items();
        let selected = items[self.list.cursor];
        let rects = screen_rects(area, Shape::TwoPane);

        let width = list_panel::inner(rects.list).width;
        let rows: Vec<Line> = items
            .iter()
            .enumerate()
            .map(|(i, item)| list_row(*item, i == self.list.cursor, icons, &colors, width))
            .collect();
        render_list_panel(
            frame, rects.list, &colors, "actions", rows, &self.list, None,
        );
        render_detail(frame, rects.detail, &colors, selected.label(), &self.detail);
    }
}

impl MainMenuComponent {
    /// The facts shown for the selected item. Refreshed by `update`.
    pub fn detail(&self) -> &Detail {
        &self.detail
    }

    /// The rows on screen, in order: the set-up item first while it is offered.
    pub fn items(&self) -> Vec<MenuItem> {
        let mut items = Vec::with_capacity(MenuItem::ALL.len() + 1);
        if self.bootstrap.is_some() {
            items.push(MenuItem::Setup);
        }
        items.extend(MenuItem::ALL);
        items
    }

    /// How many rows the menu draws. The top-level UI reads it to decide whether the
    /// big banner still fits.
    pub fn item_count(&self) -> usize {
        self.items().len()
    }

    fn selected(&self) -> MenuItem {
        self.items()[self.list.cursor]
    }

    /// Hash of everything the detail depends on. A change in any of these
    /// means the facts are rebuilt on the next update.
    fn fingerprint(&self, ctx: &AppContext) -> u64 {
        let mut hasher = DefaultHasher::new();
        self.list.cursor.hash(&mut hasher);
        bootstrap::default_folder_taken(&ctx.home_dir).hash(&mut hasher);
        ctx.status_cache.borrow().generation().hash(&mut hasher);
        ctx.core.theme.name().hash(&mut hasher);
        engine_label(ctx.core.config.effective_engine()).hash(&mut hasher);
        ctx.config_path.hash(&mut hasher);
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

    /// Activate the item. Extracted so digit-key dispatch and Enter share the
    /// same transition table.
    fn activate(&self, item: MenuItem, ctx: &AppContext) -> Transition {
        match item {
            MenuItem::Setup => match &self.bootstrap {
                Some(found) => {
                    // Interim: the grouped review keys packages by group, which an
                    // interim `group/package` name carries. G3 passes the name instead.
                    let (group, package) = found
                        .package
                        .split_once('/')
                        .unwrap_or(("", found.package.as_str()));
                    let selected = HashMap::from([(group.to_string(), vec![package.to_string()])]);
                    Transition::Push(Box::new(ConfirmComponent::stow_only(selected, ctx)))
                }
                None => Transition::Handled,
            },
            MenuItem::Profiles => Transition::Push(Box::new(ProfilesComponent::default())),
            MenuItem::Stow => Transition::Push(Box::new(PackagesComponent::default())),
            MenuItem::NewPackage => Transition::Push(Box::new(NewPackageWizard::default())),
            MenuItem::Unstow => Transition::Push(unstow_screen(ctx)),
            MenuItem::Status => Transition::Push(Box::new(StatusComponent::default())),
            MenuItem::Settings => Transition::Push(Box::new(SettingsComponent::default())),
            MenuItem::Help => Transition::Push(Box::new(HelpComponent::default())),
            MenuItem::Quit => Transition::Quit,
        }
    }
}

/// One list row: icon, label, and the jump digit at the right. The shared selection
/// style draws the accent bar and the fill.
fn list_row(
    item: MenuItem,
    selected: bool,
    icons: &Icons,
    colors: &ThemeColors,
    width: u16,
) -> Line<'static> {
    let label = item.label();
    let label_style = if selected {
        Style::default()
            .fg(colors.text)
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(colors.text)
    };
    let digit_style = if selected {
        Style::default().fg(colors.text)
    } else {
        Style::default().fg(colors.text_dim)
    };

    // Icon, two spaces, label, the gap, the digit and one column of margin. The accent
    // bar takes the one column before them.
    let used = 1 + 2 + label.width() + 1 + 1;
    let gap = (width as usize).saturating_sub(used + 1);

    selection::row_with_glyphs(
        vec![Span::styled(
            item.icon(icons),
            Style::default().fg(colors.accent),
        )],
        vec![
            Span::raw("  "),
            Span::styled(label, label_style),
            Span::raw(" ".repeat(gap)),
            Span::styled(item.digit().to_string(), digit_style),
            Span::raw(" "),
        ],
        selected,
        colors,
        width,
    )
}

/// Stow-status counts for every configured group. Reads the status cache only:
/// packages with no cached status count as unknown.
#[derive(Default)]
struct Tally {
    groups: usize,
    packages: usize,
    known: usize,
    stowed: usize,
}

impl Tally {
    /// `stowed of packages` once every package's status is known, otherwise `–`.
    fn stowed_text(&self) -> String {
        if self.known == self.packages {
            format!("{} of {}", self.stowed, self.packages)
        } else {
            "\u{2013}".to_string()
        }
    }
}

fn tally(ctx: &AppContext) -> Tally {
    let cache = ctx.status_cache.borrow();
    let mut tally = Tally {
        groups: ctx.core.config.groups.len(),
        ..Tally::default()
    };
    for name in ctx.core.config.groups.keys() {
        let Some(source) = ctx
            .core
            .config
            .get_group_source(name, &ctx.config_dir, &ctx.home_dir)
        else {
            continue;
        };
        for pkg in discover_packages(&source) {
            tally.packages += 1;
            match cache.get(name, &pkg) {
                Some(StowStatus::Stowed) => {
                    tally.known += 1;
                    tally.stowed += 1;
                }
                Some(_) => tally.known += 1,
                None => {}
            }
        }
    }
    tally
}

/// The distinct target directories of all groups, `~`-relative where possible.
fn targets(ctx: &AppContext) -> String {
    let set: BTreeSet<String> = ctx
        .core
        .config
        .groups
        .keys()
        .filter_map(|name| {
            ctx.core
                .config
                .resolve_group_target(name, &ctx.config_dir, &ctx.home_dir)
        })
        .map(|path| display_path(&path, &ctx.home_dir))
        .collect();
    if set.is_empty() {
        "none configured".to_string()
    } else {
        set.into_iter().collect::<Vec<_>>().join(", ")
    }
}

/// The facts for the set-up item: what would be linked, from where, and that Enter
/// reviews it first.
fn setup_detail(found: &Bootstrap, ctx: &AppContext, colors: &ThemeColors) -> Detail {
    let source = ctx
        .core
        .config
        .package(&found.package, &ctx.config_dir, &ctx.home_dir)
        .map(|entry| display_path(&entry.source, &ctx.home_dir))
        .unwrap_or_else(|| "unresolved".to_string());
    let link = display_path(&found.link, &ctx.home_dir);
    let fact = |value: String| elide(&value, VALUE_WIDTH);
    Detail {
        headline: vec![Line::styled(
            format!(
                "This config folder lives in a repo. Link it to {link} so plain `andre` finds it next time."
            ),
            Style::default().fg(colors.text),
        )],
        facts: vec![
            (
                Cow::Borrowed("package"),
                fact(found.package.clone()),
            ),
            (Cow::Borrowed("links"), fact(link.clone())),
            (Cow::Borrowed("from"), fact(source)),
            (Cow::Borrowed("action"), "stow (always)".to_string()),
        ],
        sections: vec![
            Line::styled("Enter reviews the link first.", Style::default().fg(colors.text)),
            Line::styled(
                format!("Shown until {link} exists."),
                Style::default().fg(colors.text_dim),
            ),
        ],
    }
}

fn build_detail(item: MenuItem, ctx: &AppContext, bootstrap: Option<&Bootstrap>) -> Detail {
    let colors = ThemeColors::from_theme(ctx.core.theme);
    if let (MenuItem::Setup, Some(found)) = (item, bootstrap) {
        return setup_detail(found, ctx, &colors);
    }
    let facts: Vec<(&'static str, String)> = match item {
        MenuItem::Setup => Vec::new(),
        MenuItem::Profiles => {
            let t = tally(ctx);
            vec![
                ("profiles", ctx.core.profiles.iter().count().to_string()),
                (
                    "packages",
                    format!(
                        "{} in {} {}",
                        t.packages,
                        t.groups,
                        plural(t.groups, "group", "groups")
                    ),
                ),
                ("stowed", t.stowed_text()),
            ]
        }
        MenuItem::Stow | MenuItem::Status => {
            let t = tally(ctx);
            let mut facts = Vec::new();
            if item == MenuItem::Stow {
                facts.push(("groups", t.groups.to_string()));
            }
            facts.push(("packages", t.packages.to_string()));
            facts.push(("stowed", t.stowed_text()));
            facts
        }
        MenuItem::NewPackage => vec![
            ("groups", ctx.core.config.groups.len().to_string()),
            ("targets", targets(ctx)),
        ],
        MenuItem::Unstow => vec![("targets", targets(ctx))],
        MenuItem::Settings => vec![
            ("config", display_path(&ctx.config_path, &ctx.home_dir)),
            (
                "engine",
                engine_label(ctx.core.config.effective_engine()).to_string(),
            ),
            ("theme", ctx.core.theme.name().to_string()),
        ],
        MenuItem::Help => vec![(
            "keys",
            format!(
                "\u{2191}\u{2193} move, {} jump, Enter open, q quit",
                jump_keys(bootstrap.is_some())
            ),
        )],
        MenuItem::Quit => Vec::new(),
    };
    Detail {
        headline: vec![Line::styled(
            item.description(),
            Style::default().fg(colors.text),
        )],
        facts: facts
            .into_iter()
            .map(|(label, value)| (Cow::Borrowed(label), value))
            .collect(),
        sections: Vec::new(),
    }
}

/// Whether the set-up review can stow `found` yet. The review still takes grouped names, so
/// it needs a `group/` part, which a flat package lacks. A config with no `packages:` key is
/// the grouped one, so it passes too. Until G3 moves the review to names, flat packages are
/// not offered.
fn review_can_run(found: &Bootstrap, config: &andre_core::Config) -> bool {
    found.package.contains('/') || config.packages.is_none()
}

/// The jump keys the footer and the help fact name: `0–8` while the set-up item is offered.
fn jump_keys(set_up_offered: bool) -> &'static str {
    if set_up_offered {
        "0\u{2013}8"
    } else {
        "1\u{2013}8"
    }
}

fn engine_label(kind: EngineKind) -> &'static str {
    match kind {
        EngineKind::Native => "native",
        EngineKind::Stow => "stow",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use andre_core::config::Config;

    /// A context whose only group `home` lives under a temp dir, holding `packages`.
    /// The temp dir is returned so it outlives the test body.
    fn fixture(packages: &[&str]) -> (tempfile::TempDir, AppContext) {
        let dir = tempfile::TempDir::new().unwrap();
        let source = dir.path().join("dotfiles/home");
        for pkg in packages {
            std::fs::create_dir_all(source.join(pkg)).unwrap();
        }
        let config_path = dir.path().join("andre.yml");
        std::fs::write(
            &config_path,
            "global: {}\ngroups:\n  home:\n    source: dotfiles/home\n    target: \"~\"\n",
        )
        .unwrap();
        let config = Config::load(&config_path).unwrap();
        let ctx = AppContext::for_tests(config, dir.path());
        (dir, ctx)
    }

    fn detail_of(item: MenuItem, ctx: &mut AppContext) -> Detail {
        let index = MenuItem::ALL.iter().position(|i| *i == item).unwrap();
        let mut menu = MainMenuComponent::default();
        menu.list.cursor = index;
        menu.update(ctx);
        menu.detail().clone()
    }

    fn fact<'a>(detail: &'a Detail, label: &str) -> &'a str {
        detail
            .facts
            .iter()
            .find(|(l, _)| *l == label)
            .map(|(_, v)| v.as_str())
            .unwrap_or_else(|| panic!("no {label:?} fact in {detail:?}"))
    }

    fn headline(detail: &Detail) -> String {
        detail
            .headline
            .iter()
            .map(|line| {
                line.spans
                    .iter()
                    .map(|s| s.content.as_ref())
                    .collect::<String>()
            })
            .collect::<Vec<_>>()
            .join("\n")
    }

    #[test]
    fn test_menu_items_are_labelled_in_digit_order() {
        let labels: Vec<&str> = MenuItem::ALL.iter().map(|item| item.label()).collect();
        assert_eq!(
            labels,
            [
                "profiles",
                "packages",
                "new package",
                "unstow",
                "verify status",
                "settings",
                "help",
                "quit",
            ]
        );
    }

    #[test]
    fn test_digits_are_zero_for_set_up_and_one_to_eight_for_the_rest() {
        assert_eq!(MenuItem::Setup.digit(), 0);
        let digits: Vec<usize> = MenuItem::ALL.iter().map(|item| item.digit()).collect();
        assert_eq!(digits, (1..=8).collect::<Vec<_>>());
    }

    #[test]
    fn test_profiles_facts_count_profiles_packages_and_stowed() {
        let (_dir, mut ctx) = fixture(&["bash", "nvim"]);
        let detail = detail_of(MenuItem::Profiles, &mut ctx);
        assert_eq!(fact(&detail, "profiles"), "0");
        assert_eq!(fact(&detail, "packages"), "2 in 1 group");
        assert_eq!(fact(&detail, "stowed"), "\u{2013}");

        let dir = ctx.config_dir.join("profiles");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("base.yml"), "packages: [home/bash]\n").unwrap();
        ctx.core.profiles = andre_core::profile::ProfileSet::load(&dir).unwrap();
        let detail = detail_of(MenuItem::Profiles, &mut ctx);
        assert_eq!(fact(&detail, "profiles"), "1");
    }

    #[test]
    fn test_profile_change_rebuilds_the_profiles_facts() {
        let (_dir, mut ctx) = fixture(&["bash"]);
        let mut menu = MainMenuComponent::default();
        menu.list.cursor = 0;
        menu.update(&mut ctx);
        let before = menu.detail_key;

        let dir = ctx.config_dir.join("profiles");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("base.yml"), "packages: [home/bash]\n").unwrap();
        ctx.core.profiles = andre_core::profile::ProfileSet::load(&dir).unwrap();
        menu.update(&mut ctx);
        assert_ne!(
            menu.detail_key, before,
            "the profile set is part of the key"
        );
    }

    #[test]
    fn test_stow_facts_count_groups_and_packages() {
        let (_dir, mut ctx) = fixture(&["bash", "nvim"]);
        let detail = detail_of(MenuItem::Stow, &mut ctx);
        assert_eq!(fact(&detail, "groups"), "1");
        assert_eq!(fact(&detail, "packages"), "2");
        assert_eq!(fact(&detail, "stowed"), "\u{2013}", "nothing cached yet");
    }

    #[test]
    fn test_stowed_fact_counts_only_once_every_package_is_cached() {
        let (_dir, mut ctx) = fixture(&["bash", "nvim"]);
        ctx.status_cache
            .borrow_mut()
            .insert(("home".to_string(), "bash".to_string()), StowStatus::Stowed);
        let detail = detail_of(MenuItem::Status, &mut ctx);
        assert_eq!(fact(&detail, "stowed"), "\u{2013}", "nvim is still unknown");

        ctx.status_cache.borrow_mut().insert(
            ("home".to_string(), "nvim".to_string()),
            StowStatus::Unstowed,
        );
        let detail = detail_of(MenuItem::Status, &mut ctx);
        assert_eq!(fact(&detail, "stowed"), "1 of 2");
    }

    #[test]
    fn test_new_package_and_unstow_list_the_targets_relative_to_home() {
        let (_dir, mut ctx) = fixture(&["bash"]);
        let new_package = detail_of(MenuItem::NewPackage, &mut ctx);
        assert_eq!(fact(&new_package, "groups"), "1");
        assert_eq!(fact(&new_package, "targets"), "~");

        let unstow = detail_of(MenuItem::Unstow, &mut ctx);
        assert_eq!(fact(&unstow, "targets"), "~");
    }

    #[test]
    fn test_no_groups_means_no_targets() {
        let (_dir, mut ctx) = fixture(&[]);
        ctx.core.config.groups.clear();
        let unstow = detail_of(MenuItem::Unstow, &mut ctx);
        assert_eq!(fact(&unstow, "targets"), "none configured");
    }

    #[test]
    fn test_settings_facts_show_config_engine_and_theme() {
        let (_dir, mut ctx) = fixture(&[]);
        let detail = detail_of(MenuItem::Settings, &mut ctx);
        assert_eq!(fact(&detail, "config"), "~/andre.yml");
        assert_eq!(fact(&detail, "engine"), "native");
        assert_eq!(fact(&detail, "theme"), "default");
    }

    #[test]
    fn test_help_shows_a_key_reminder_and_quit_has_no_facts() {
        let (_dir, mut ctx) = fixture(&[]);
        let help = detail_of(MenuItem::Help, &mut ctx);
        assert!(fact(&help, "keys").contains("1\u{2013}8 jump"));
        assert!(fact(&help, "keys").contains("Enter open"));
        let quit = detail_of(MenuItem::Quit, &mut ctx);
        assert!(quit.facts.is_empty());
        assert!(!headline(&quit).is_empty());
    }

    #[test]
    fn test_help_names_zero_to_eight_while_set_up_is_offered() {
        let (_dir, ctx) = fixture(&[]);
        let found = Bootstrap {
            package: "config/andre".into(),
            link: ctx.home_dir.join(".config").join("andre"),
        };
        let help = build_detail(MenuItem::Help, &ctx, Some(&found));
        assert!(fact(&help, "keys").contains("0\u{2013}8 jump"));
    }

    /// A flat packages folder whose package `andre` holds the config folder at
    /// `.config/andre`, with the home folder empty (so set-up is pending).
    fn flat_bootstrap_ctx() -> (tempfile::TempDir, AppContext) {
        let dir = tempfile::TempDir::new().unwrap();
        let home = dir.path().join("home");
        let folder = dir.path().join("dotfiles/andre/.config/andre");
        std::fs::create_dir_all(&folder).unwrap();
        std::fs::create_dir_all(&home).unwrap();
        std::fs::write(folder.join("andre.yml"), "global: {}\n").unwrap();
        let mut config = Config::default_empty();
        config.packages = Some(dir.path().join("dotfiles").display().to_string());
        let mut ctx = AppContext::for_tests(config, &home);
        ctx.config_dir = folder.clone();
        ctx.config_path = folder.join("andre.yml");
        (dir, ctx)
    }

    #[test]
    fn test_flat_package_holding_the_config_folder_offers_no_set_up_row() {
        let (_dir, mut ctx) = flat_bootstrap_ctx();
        let mut menu = MainMenuComponent::default();
        menu.update(&mut ctx);
        assert!(
            !menu.items().contains(&MenuItem::Setup),
            "the review cannot stow a flat package yet"
        );
        assert_eq!(menu.item_count(), MenuItem::ALL.len());
    }

    #[test]
    fn test_grouped_package_holding_the_config_folder_offers_set_up() {
        let (dir, mut ctx) = flat_bootstrap_ctx();
        let mut config = Config::default_empty();
        config.add_group(
            "dotfiles".into(),
            dir.path().join("dotfiles").display().to_string(),
            ctx.home_dir.display().to_string(),
        );
        ctx.core.config = config;
        let mut menu = MainMenuComponent::default();
        menu.update(&mut ctx);
        assert_eq!(menu.items().first(), Some(&MenuItem::Setup));
    }

    #[test]
    fn test_update_rebuilds_only_when_something_it_reads_changes() {
        let (_dir, mut ctx) = fixture(&["bash"]);
        let mut menu = MainMenuComponent::default();
        menu.update(&mut ctx);
        let first = menu.detail_key;
        assert!(first.is_some());

        menu.update(&mut ctx);
        assert_eq!(menu.detail_key, first, "an unchanged state keeps its key");

        menu.list.cursor = 1;
        menu.update(&mut ctx);
        assert_ne!(menu.detail_key, first, "selection is part of the key");
        assert_eq!(headline(menu.detail()), MenuItem::Stow.description());
    }
}
