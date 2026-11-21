//! The list rows and the detail pane, built from the pending settings. Built in `refresh`
//! (after each key and each tick), so painting only reads them.

use std::borrow::Cow;

use andre_core::profile::split_package_ref;
use andre_core::{discover_packages, StowAction, StowFlag};
use ratatui::text::Line;

use super::pending::{GroupChange, PendingSettings};
use super::types::{Item, Row};
use crate::components::AppContext;
use crate::theme::ThemeColors;
use crate::ui::detail::{fact_line, Detail};
use crate::ui::display_path;
use crate::ui::icons::Icons;

/// Every row of the list, in display order: headers, then the settings and the groups.
pub fn items(pending: &PendingSettings, ctx: &AppContext) -> Vec<Item> {
    let flags = pending.flags;
    let icons = Icons::for_set(pending.icons);
    let mut items = vec![
        header("stow options", ""),
        item(Row::Action, "action", flags.action.as_str().to_string()),
        item(
            Row::Flag(StowFlag::Verbosity),
            "verbose",
            flags.verbosity.to_string(),
        ),
        item(
            Row::Flag(StowFlag::DryRun),
            "dry-run",
            flag_glyph(flags.dry_run, icons),
        ),
        item(
            Row::Flag(StowFlag::NoFolding),
            "no-fold",
            flag_glyph(flags.no_folding, icons),
        ),
        item(
            Row::Flag(StowFlag::Adopt),
            "adopt",
            flag_glyph(flags.adopt, icons),
        ),
        item(
            Row::Flag(StowFlag::Dotfiles),
            "dotfiles",
            flag_glyph(flags.dotfiles, icons),
        ),
        header("appearance", ""),
        item(Row::Theme, "theme", pending.theme.name().to_string()),
        item(Row::Icons, "icons", pending.icons.name().to_string()),
        header("config", ""),
        item(
            Row::GlobalIgnore,
            "global ignore",
            pattern_count(pending.config.global_ignore().len()),
        ),
        header("groups", "+"),
    ];
    for name in pending.config.groups.keys() {
        let (_, target) = group_paths(pending, name, ctx).unwrap_or_default();
        items.push(item(Row::Group(name.clone()), name, target));
    }
    items
}

/// The detail pane for `row`: what the setting does, its pending and saved values, its
/// global key and the config file it lives in. A group shows its paths and counts.
pub fn detail(row: &Row, pending: &PendingSettings, ctx: &AppContext) -> Detail {
    let flags = pending.flags;
    let saved = ctx.core.flags;
    let colors = ThemeColors::from_theme(pending.theme);
    match row {
        Row::Header => Detail::default(),
        Row::Group(name) => group_detail(name, pending, ctx),
        Row::Action => setting(
            "What the next run does to the packages.",
            flags.action.as_str(),
            saved.action.as_str(),
            Some("s  (anywhere)"),
            ctx,
            &colors,
        ),
        Row::Flag(StowFlag::Verbosity) => setting(
            "How much stow prints, 0\u{2013}5.",
            &flags.verbosity.to_string(),
            &saved.verbosity.to_string(),
            Some("v  (anywhere)"),
            ctx,
            &colors,
        ),
        Row::Flag(StowFlag::DryRun) => setting(
            "Show what would change and write nothing.",
            on_off(flags.dry_run),
            on_off(saved.dry_run),
            Some("d  (anywhere)"),
            ctx,
            &colors,
        ),
        Row::Flag(StowFlag::NoFolding) => setting(
            "Link every file, never a whole folder.",
            on_off(flags.no_folding),
            on_off(saved.no_folding),
            Some("n  (anywhere)"),
            ctx,
            &colors,
        ),
        Row::Flag(StowFlag::Adopt) => setting(
            "Move existing target files into the package, then link (stow --adopt).",
            on_off(flags.adopt),
            on_off(saved.adopt),
            Some("a  (anywhere)"),
            ctx,
            &colors,
        ),
        Row::Flag(StowFlag::Dotfiles) => setting(
            "Treat dot-prefixed package entries as dotfiles (stow --dotfiles).",
            on_off(flags.dotfiles),
            on_off(saved.dotfiles),
            Some("o  (anywhere)"),
            ctx,
            &colors,
        ),
        Row::Theme => setting(
            "Colour theme. Previews live while you move.",
            pending.theme.name(),
            ctx.core.theme.name(),
            Some("t  (anywhere)"),
            ctx,
            &colors,
        ),
        Row::Icons => setting(
            "Glyph set: unicode symbols, or nerd font icons.",
            pending.icons.name(),
            ctx.core.icons.name(),
            None,
            ctx,
            &colors,
        ),
        Row::GlobalIgnore => setting(
            "Patterns no group links. A group's own list adds to them.",
            &pattern_count(pending.config.global_ignore().len()),
            &pattern_count(ctx.core.config.global_ignore().len()),
            None,
            ctx,
            &colors,
        ),
    }
}

/// A group's source and target as the screen shows them: `~`-relative when they resolve,
/// the raw config text otherwise. `None` when the group does not exist.
pub fn group_paths(
    pending: &PendingSettings,
    name: &str,
    ctx: &AppContext,
) -> Option<(String, String)> {
    let group = pending.config.groups.get(name)?;
    let source = pending
        .config
        .resolve_group_source(name, &ctx.config_dir, &ctx.home_dir)
        .map(|path| display_path(&path, &ctx.home_dir))
        .unwrap_or_else(|| group.source.clone());
    let target = pending
        .config
        .resolve_group_target(name, &ctx.config_dir, &ctx.home_dir)
        .map(|path| display_path(&path, &ctx.home_dir))
        .unwrap_or_else(|| group.target.clone());
    Some((source, target))
}

fn group_detail(name: &str, pending: &PendingSettings, ctx: &AppContext) -> Detail {
    let Some((source_text, target_text)) = group_paths(pending, name, ctx) else {
        return Detail::default();
    };
    let source = pending
        .config
        .resolve_group_source(name, &ctx.config_dir, &ctx.home_dir);
    let packages = source.map_or(0, |path| discover_packages(&path).len());
    let own_ignore = pending
        .config
        .get_group_ignore(name)
        .map_or(0, <[String]>::len);
    let profiles = profiles_using(ctx, &saved_name(pending, name));
    Detail {
        headline: vec![Line::raw(format!("Group `{name}`"))],
        facts: vec![
            ("source".into(), source_text),
            ("target".into(), target_text),
            ("ignore".into(), pattern_count(own_ignore)),
            ("packages".into(), packages.to_string()),
            (
                "profiles".into(),
                match profiles {
                    0 => "none".to_string(),
                    n => format!("used by {n}"),
                },
            ),
        ],
        sections: Vec::new(),
    }
}

/// A setting's detail: what it does, now and saved values, its key, and where it lives. The
/// file and paths are a block of their own, after a blank line.
fn setting(
    what: &str,
    now: &str,
    saved: &str,
    key: Option<&str>,
    ctx: &AppContext,
    colors: &ThemeColors,
) -> Detail {
    let mut facts: Vec<(Cow<'static, str>, String)> = vec![
        ("now".into(), now.to_string()),
        ("saved".into(), saved.to_string()),
    ];
    if let Some(key) = key {
        facts.push(("key".into(), key.to_string()));
    }
    let file = display_path(&ctx.config_path, &ctx.home_dir);
    let paths = format!(
        "relative to {}",
        display_path(&ctx.config_dir, &ctx.home_dir)
    );
    Detail {
        headline: vec![Line::raw(what.to_string())],
        facts,
        sections: vec![
            fact_line("file", &file, colors),
            fact_line("paths", &paths, colors),
        ],
    }
}

/// How many saved profiles name a package of `group`.
fn profiles_using(ctx: &AppContext, group: &str) -> usize {
    ctx.core
        .profiles
        .iter()
        .filter(|profile| profile.packages.iter().any(|pkg| in_group(pkg, group)))
        .count()
}

/// The package refs into `group`, across every saved profile. Removing the group drops
/// them on save. The saved profiles still name a renamed group by its old name, so the
/// count uses that name.
pub fn refs_into(ctx: &AppContext, pending: &PendingSettings, group: &str) -> usize {
    let saved = saved_name(pending, group);
    ctx.core
        .profiles
        .iter()
        .flat_map(|profile| profile.packages.iter())
        .filter(|pkg| in_group(pkg, &saved))
        .count()
}

/// `group`'s name in the saved profiles: the pending renames, undone newest first.
fn saved_name(pending: &PendingSettings, group: &str) -> String {
    let mut name = group.to_string();
    for change in pending.group_changes.iter().rev() {
        if let GroupChange::Rename { from, to } = change {
            if *to == name {
                name = from.clone();
            }
        }
    }
    name
}

fn in_group(pkg: &str, group: &str) -> bool {
    split_package_ref(pkg).is_some_and(|(g, _)| g == group)
}

/// The index of `action` in `StowAction::ALL`.
pub fn action_index(action: StowAction) -> usize {
    StowAction::ALL
        .iter()
        .position(|a| *a == action)
        .unwrap_or(0)
}

fn header(title: &str, meta: &str) -> Item {
    Item {
        row: Row::Header,
        label: title.to_string(),
        value: meta.to_string(),
    }
}

fn item(row: Row, label: &str, value: String) -> Item {
    Item {
        row,
        label: label.to_string(),
        value,
    }
}

fn flag_glyph(on: bool, icons: &Icons) -> String {
    if on { icons.flag_on } else { icons.flag_off }.to_string()
}

fn on_off(on: bool) -> &'static str {
    if on {
        "on"
    } else {
        "off"
    }
}

pub fn pattern_count(count: usize) -> String {
    let noun = if count == 1 { "pattern" } else { "patterns" };
    format!("{count} {noun}")
}
