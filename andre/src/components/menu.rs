use std::collections::hash_map::DefaultHasher;
use std::collections::BTreeSet;
use std::hash::{Hash, Hasher};
use std::path::Path;

use andre_core::{discover_packages, EngineKind, StowStatus};
use crossterm::event::{KeyCode, KeyEvent};
use ratatui::{
    layout::{Constraint, Direction, Layout, Margin, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Paragraph, Wrap},
    Frame,
};
use unicode_width::UnicodeWidthStr;

use crate::components::help::HelpComponent;
use crate::components::settings::SettingsComponent;
use crate::components::status::StatusComponent;
use crate::components::stow::GroupSelectComponent;
use crate::components::target_select::TargetSelectComponent;
use crate::components::{AppContext, Component, Transition};
use crate::input;
use crate::theme::ThemeColors;
use crate::ui::icons::Icons;
use crate::ui::selection;

/// Width of the action list. The detail panel takes the rest of the content area.
const LIST_WIDTH: u16 = 34;
/// Columns between the list and the detail panel.
const GUTTER: u16 = 1;

/// The seven rows of the main menu, in jump-digit order (`1`..`7`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MenuItem {
    Stow,
    Adopt,
    Unstow,
    Status,
    Settings,
    Help,
    Quit,
}

impl MenuItem {
    pub const ALL: [MenuItem; 7] = [
        MenuItem::Stow,
        MenuItem::Adopt,
        MenuItem::Unstow,
        MenuItem::Status,
        MenuItem::Settings,
        MenuItem::Help,
        MenuItem::Quit,
    ];

    /// Row label. The first item names the current action (`stow packages`, `restow packages`).
    pub fn label(self, action: &str) -> String {
        match self {
            MenuItem::Stow => format!("{action} packages"),
            MenuItem::Adopt => "adopt files".to_string(),
            MenuItem::Unstow => "unstow".to_string(),
            MenuItem::Status => "verify status".to_string(),
            MenuItem::Settings => "settings".to_string(),
            MenuItem::Help => "help".to_string(),
            MenuItem::Quit => "quit".to_string(),
        }
    }

    fn icon(self, icons: &Icons) -> &'static str {
        match self {
            MenuItem::Stow => icons.link,
            MenuItem::Adopt => icons.import,
            MenuItem::Unstow => icons.unlink,
            MenuItem::Status => icons.check,
            MenuItem::Settings => icons.gear,
            MenuItem::Help => icons.question,
            MenuItem::Quit => icons.power,
        }
    }

    fn description(self) -> &'static str {
        match self {
            MenuItem::Stow => "Pick groups and packages, then run the current action.",
            MenuItem::Adopt => "Import files that already sit in a target into a package.",
            MenuItem::Unstow => "Remove symlinks from a target, one link at a time.",
            MenuItem::Status => "Check every package's links against its source tree.",
            MenuItem::Settings => "Edit the config file, groups, theme, icons and stow options.",
            MenuItem::Help => "Keyboard reference and what each screen does.",
            MenuItem::Quit => "Leave andre. Nothing is changed.",
        }
    }
}

/// What the detail panel shows for the selected item. Built by
/// `MainMenuComponent::update`, never while painting.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Detail {
    pub description: &'static str,
    pub facts: Vec<(&'static str, String)>,
}

#[derive(Default)]
pub struct MainMenuComponent {
    pub index: usize,
    detail: Detail,
    /// Fingerprint of everything `detail` was built from. `None` until the first update.
    detail_key: Option<u64>,
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

    fn handle_key(&mut self, key: KeyEvent, _ctx: &mut AppContext) -> Transition {
        let items_len = MenuItem::ALL.len();

        if input::Input::is_up(&key) {
            self.index = (self.index + items_len - 1) % items_len;
        } else if input::Input::is_down(&key) {
            self.index = (self.index + 1) % items_len;
        } else if input::Input::is_quit_key(&key) {
            // q and Ctrl+C: checked before the Char arm so the digit
            // dispatcher never swallows Ctrl+C / Ctrl+1 / etc.
            return Transition::Quit;
        } else if key.code == KeyCode::Enter {
            return Self::activate(MenuItem::ALL[self.index]);
        } else if let KeyCode::Char(c) = key.code {
            // Unmodified digit keys 1..=7 jump to and activate the Nth menu
            // item, matching the labels rendered in `render`. '0' and '8'..='9'
            // are out of range and ignored so they cannot crash input.
            if key.modifiers.is_empty() {
                if let Some(d) = c.to_digit(10) {
                    if (1..=items_len as u32).contains(&d) {
                        let idx = (d - 1) as usize;
                        self.index = idx;
                        return Self::activate(MenuItem::ALL[idx]);
                    }
                }
            }
        }

        Transition::None
    }

    fn update(&mut self, ctx: &mut AppContext) -> Transition {
        let key = self.fingerprint(ctx);
        if self.detail_key != Some(key) {
            self.detail = build_detail(MenuItem::ALL[self.index], ctx);
            self.detail_key = Some(key);
        }
        Transition::None
    }

    fn render(&mut self, frame: &mut Frame, area: Rect, ctx: &AppContext) {
        let colors = ThemeColors::from_theme(ctx.core.theme);
        let icons = Icons::for_set(ctx.core.icons);
        let action = ctx.core.flags.action.as_str();
        let selected = MenuItem::ALL[self.index];

        let columns = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([
                Constraint::Length(LIST_WIDTH),
                Constraint::Length(GUTTER),
                Constraint::Min(1),
            ])
            .split(area);
        let (list_area, detail_area) = (columns[0], columns[2]);

        let list_block = crate::ui::themed_panel(&colors).title(" actions ");
        let list_inner = list_block.inner(list_area);
        frame.render_widget(list_block, list_area);
        let rows: Vec<Line> = MenuItem::ALL
            .iter()
            .enumerate()
            .map(|(i, item)| {
                list_row(
                    *item,
                    i,
                    i == self.index,
                    action,
                    icons,
                    &colors,
                    list_inner.width,
                )
            })
            .collect();
        frame.render_widget(Paragraph::new(rows), list_inner);

        let detail_block =
            crate::ui::themed_panel(&colors).title(format!(" {} ", selected.label(action)));
        let detail_inner = detail_block.inner(detail_area);
        frame.render_widget(detail_block, detail_area);
        let mut lines = vec![Line::from(Span::styled(
            self.detail.description,
            Style::default().fg(colors.text),
        ))];
        if !self.detail.facts.is_empty() {
            lines.push(Line::raw(""));
        }
        for (label, value) in &self.detail.facts {
            lines.push(Line::from(vec![
                Span::styled(format!("{label:<10}"), Style::default().fg(colors.text_dim)),
                Span::styled(value.clone(), Style::default().fg(colors.accent)),
            ]));
        }
        frame.render_widget(
            Paragraph::new(lines).wrap(Wrap { trim: false }),
            detail_inner.inner(Margin::new(1, 0)),
        );
    }
}

impl MainMenuComponent {
    /// The facts shown for the selected item. Refreshed by `update`.
    pub fn detail(&self) -> &Detail {
        &self.detail
    }

    /// Hash of everything the detail depends on. A change in any of these
    /// means the facts are rebuilt on the next update.
    fn fingerprint(&self, ctx: &AppContext) -> u64 {
        let mut hasher = DefaultHasher::new();
        self.index.hash(&mut hasher);
        ctx.status_cache.borrow().generation().hash(&mut hasher);
        ctx.core.flags.action.as_str().hash(&mut hasher);
        ctx.core.theme.name().hash(&mut hasher);
        engine_label(ctx.core.config.effective_engine()).hash(&mut hasher);
        ctx.config_path.hash(&mut hasher);
        for (name, group) in &ctx.core.config.groups {
            name.hash(&mut hasher);
            group.source.hash(&mut hasher);
            group.target.hash(&mut hasher);
        }
        hasher.finish()
    }

    /// Activate the item. Extracted so digit-key dispatch and Enter share the
    /// same transition table.
    fn activate(item: MenuItem) -> Transition {
        match item {
            MenuItem::Stow => Transition::Push(Box::new(GroupSelectComponent::default())),
            MenuItem::Adopt => Transition::Push(Box::new(TargetSelectComponent::adopt())),
            MenuItem::Unstow => Transition::Push(Box::new(TargetSelectComponent::unstow())),
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
    index: usize,
    selected: bool,
    action: &str,
    icons: &Icons,
    colors: &ThemeColors,
    width: u16,
) -> Line<'static> {
    let label = item.label(action);
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

    selection::row(
        vec![
            Span::styled(item.icon(icons), Style::default().fg(colors.accent)),
            Span::raw("  "),
            Span::styled(label, label_style),
            Span::raw(" ".repeat(gap)),
            Span::styled((index + 1).to_string(), digit_style),
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

fn build_detail(item: MenuItem, ctx: &AppContext) -> Detail {
    let facts = match item {
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
        MenuItem::Adopt => vec![
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
            "\u{2191}\u{2193} move, 1\u{2013}7 jump, Enter open, q quit".to_string(),
        )],
        MenuItem::Quit => Vec::new(),
    };
    Detail {
        description: item.description(),
        facts,
    }
}

fn engine_label(kind: EngineKind) -> &'static str {
    match kind {
        EngineKind::Native => "native",
        EngineKind::Stow => "stow",
    }
}

/// `path` with the home directory shown as `~`.
fn display_path(path: &Path, home: &Path) -> String {
    match path.strip_prefix(home) {
        Ok(rest) if rest.as_os_str().is_empty() => "~".to_string(),
        Ok(rest) => format!("~/{}", rest.display()),
        Err(_) => path.display().to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use andre_core::config::Config;
    use andre_core::StowAction;

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
        let mut menu = MainMenuComponent {
            index,
            ..MainMenuComponent::default()
        };
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

    #[test]
    fn test_first_item_names_the_current_action() {
        assert_eq!(MenuItem::Stow.label("stow"), "stow packages");
        assert_eq!(MenuItem::Stow.label("restow"), "restow packages");
        assert_eq!(MenuItem::Stow.label("unstow"), "unstow packages");
        assert_eq!(MenuItem::Adopt.label("stow"), "adopt files");
        assert_eq!(MenuItem::Status.label("stow"), "verify status");
        assert_eq!(MenuItem::Settings.label("stow"), "settings");
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
    fn test_adopt_and_unstow_list_the_targets_relative_to_home() {
        let (_dir, mut ctx) = fixture(&["bash"]);
        let adopt = detail_of(MenuItem::Adopt, &mut ctx);
        assert_eq!(fact(&adopt, "groups"), "1");
        assert_eq!(fact(&adopt, "targets"), "~");

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
        assert!(fact(&help, "keys").contains("Enter open"));
        let quit = detail_of(MenuItem::Quit, &mut ctx);
        assert!(quit.facts.is_empty());
        assert!(!quit.description.is_empty());
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

        menu.index = 1;
        menu.update(&mut ctx);
        assert_ne!(menu.detail_key, first, "selection is part of the key");
        assert_eq!(menu.detail().description, MenuItem::Adopt.description());
    }

    #[test]
    fn test_action_change_relabels_and_rebuilds_the_first_item() {
        let (_dir, mut ctx) = fixture(&["bash"]);
        let mut menu = MainMenuComponent::default();
        menu.update(&mut ctx);
        let before = menu.detail_key;

        ctx.core.flags.action = StowAction::Restow;
        menu.update(&mut ctx);
        assert_ne!(menu.detail_key, before, "the action is part of the key");
    }

    #[test]
    fn test_display_path_uses_tilde_only_under_home() {
        let home = Path::new("/home/me");
        assert_eq!(display_path(Path::new("/home/me"), home), "~");
        assert_eq!(
            display_path(Path::new("/home/me/.config"), home),
            "~/.config"
        );
        assert_eq!(display_path(Path::new("/etc/andre"), home), "/etc/andre");
    }
}
