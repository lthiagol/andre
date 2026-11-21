//! The list rows and the detail pane, built from the pending settings. Built in `refresh`
//! (after each key and each tick), so painting only reads them.

use std::borrow::Cow;

use andre_core::{discover_packages, StowAction, StowFlag};
use ratatui::text::Line;

use super::pending::PendingSettings;
use super::types::{Item, Row};
use crate::components::AppContext;
use crate::theme::ThemeColors;
use crate::ui::detail::{fact_line, Detail};
use crate::ui::display_path;
use crate::ui::icons::Icons;

/// Every row of the list, in display order: the section headers and their settings.
pub fn items(pending: &PendingSettings, ctx: &AppContext) -> Vec<Item> {
    let flags = pending.flags;
    let icons = Icons::for_set(pending.icons);
    vec![
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
        item(Row::Packages, "packages", folder_text(&pending.config, ctx)),
        item(
            Row::GlobalIgnore,
            "global ignore",
            pattern_count(pending.config.global_ignore().len()),
        ),
    ]
}

/// The detail pane for `row`: what the setting does, its pending and saved values, its
/// global key and the config file it lives in. The packages folder adds its package count
/// and where links go.
pub fn detail(row: &Row, pending: &PendingSettings, ctx: &AppContext) -> Detail {
    let flags = pending.flags;
    let saved = ctx.core.flags;
    let colors = ThemeColors::from_theme(pending.theme);
    match row {
        Row::Header => Detail::default(),
        Row::Packages => packages_detail(pending, ctx, &colors),
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
            "Patterns that no package links.",
            &pattern_count(pending.config.global_ignore().len()),
            &pattern_count(ctx.core.config.global_ignore().len()),
            None,
            ctx,
            &colors,
        ),
    }
}

/// The packages folder as the list shows it: `~`-relative when it resolves, `not set` when
/// `packages:` is unset.
fn folder_text(config: &andre_core::Config, ctx: &AppContext) -> String {
    config
        .packages_dir(&ctx.config_dir, &ctx.home_dir)
        .map_or_else(
            || "not set".to_string(),
            |path| display_path(&path, &ctx.home_dir),
        )
}

/// The packages folder's detail: what a package is, the folder now and saved, how many
/// packages it holds, and where they link. `(target: in andre.yml)` says the target key is
/// set; it is not offered as a row.
fn packages_detail(pending: &PendingSettings, ctx: &AppContext, colors: &ThemeColors) -> Detail {
    let packages = pending
        .config
        .packages_dir(&ctx.config_dir, &ctx.home_dir)
        .map_or(0, |path| discover_packages(&path).len());
    let mut links_to = display_path(
        &pending.config.target_dir(&ctx.config_dir, &ctx.home_dir),
        &ctx.home_dir,
    );
    if pending.config.target.is_some() {
        links_to.push_str("  (target: in andre.yml)");
    }
    let file = display_path(&ctx.config_path, &ctx.home_dir);
    let paths = format!(
        "relative to {}",
        display_path(&ctx.config_dir, &ctx.home_dir)
    );
    Detail {
        headline: vec![Line::raw("Every folder here is a package.".to_string())],
        facts: vec![
            ("now".into(), folder_text(&pending.config, ctx)),
            ("saved".into(), folder_text(&ctx.core.config, ctx)),
            ("packages".into(), packages.to_string()),
            ("links to".into(), links_to),
        ],
        sections: vec![
            fact_line("file", &file, colors),
            fact_line("paths", &paths, colors),
            Line::raw(String::new()),
            Line::raw("Enter picks another folder.".to_string()),
        ],
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
