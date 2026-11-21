//! The review: what a run would do, before it runs. The left pane is the selected packages
//! with the status each has now; the right pane is the command and target of the cursor's
//! package. `y` runs the selection on the run screen, `n` or `Esc` goes back. Nothing changes
//! on disk here: commands are previewed and files are counted.

use std::borrow::Cow;
use std::path::Path;

use andre_core::ignore::global_ignore_in;
use andre_core::stow::linkable_files;
use andre_core::{build_stow_args, engine_for, EngineKind, StowAction, StowFlags};
use crossterm::event::{KeyCode, KeyEvent, MouseEvent};
use ratatui::{layout::Rect, style::Style, text::Line, Frame};

use crate::components::execute::ExecuteComponent;
use crate::components::{AppContext, Component, Transition};
use crate::execute::{build_execution_results, spawn_execution};
use crate::input::Input;
use crate::theme::ThemeColors;
use crate::ui::confirm::review_buttons;
use crate::ui::detail::{render_detail, Detail};
use crate::ui::dialog::{button_click, render_action_bar};
use crate::ui::footer::{review_hints, Hint};
use crate::ui::icons::Icons;
use crate::ui::layout::{screen_rects, Shape};
use crate::ui::list_panel::{self, click_list, render_list_panel, Empty, ListView};
use crate::ui::package_list::{package_line, PackageList, PackageRow};
use crate::ui::{display_path, plural};

pub struct ConfirmComponent {
    /// The packages the run covers, by name, in name order.
    pub selected_packages: Vec<String>,
    /// The action this review runs. `None` follows the global action.
    action: Option<StowAction>,
    /// The packages with the status each has now. Statuses are read once, when the review
    /// opens.
    packages: PackageList,
    /// What each package would run. Rebuilt when a global flag changes.
    plan: Plan,
    /// The flags `plan` was built from.
    plan_flags: StowFlags,
    pub list: ListView,
    /// The detail of the cursor's package. Built by `sync`.
    detail: Detail,
    /// The action bar, as the last `prepare` laid it out.
    actions: Option<Rect>,
}

/// What a run does, worked out from disk when the review opens and when a flag changes.
#[derive(Default)]
struct Plan {
    /// `native` or `stow`: the engine the run uses.
    engine: &'static str,
    /// Each selected package's preview, in review order.
    packages: Vec<(String, Preview)>,
}

struct Preview {
    /// Where the package links, as a display path. `None` when the path does not resolve.
    target: Option<String>,
    command: String,
    /// The files the package would link. `None` when its folder cannot be read.
    files: Option<usize>,
}

/// The flags for a review: the global ones, with the action replaced when the review has
/// its own.
fn flags_for(ctx: &AppContext, action: Option<StowAction>) -> StowFlags {
    StowFlags {
        action: action.unwrap_or(ctx.core.flags.action),
        ..ctx.core.flags
    }
}

/// Sorts and de-duplicates the names, so the review lists and runs them in name order.
fn in_review_order(mut names: Vec<String>) -> Vec<String> {
    names.sort();
    names.dedup();
    names
}

/// The rows the review lists. Reads each package's status through the status cache. A name
/// that is not a package, or whose target folder is missing, has no status.
fn build_rows(selected: &[String], ctx: &AppContext) -> PackageList {
    let entries = ctx.package_entries();
    let global = global_ignore_in(&ctx.home_dir);
    let mut cache = ctx.status_cache.borrow_mut();
    let rows = selected
        .iter()
        .map(|name| {
            // A package whose target is missing does not resolve, so its status stays unknown.
            let status = entries
                .iter()
                .find(|entry| &entry.name == name)
                .filter(|entry| entry.target.exists())
                .map(|entry| cache.prefetch(entry, Some(global.as_path())));
            PackageRow {
                name: name.clone(),
                status,
                mark: None,
                meta: String::new(),
            }
        })
        .collect();
    PackageList::new(rows)
}

/// The plan for `selected` under `flags`. Reads each package's folder to count its files.
fn build_plan(selected: &[String], ctx: &AppContext, flags: &StowFlags) -> Plan {
    let config = &ctx.core.config;
    let native = config.effective_engine() == EngineKind::Native;
    let global = global_ignore_in(&ctx.home_dir);
    let entries = ctx.package_entries();
    let packages = selected
        .iter()
        .map(|name| {
            // A package whose target is missing does not resolve, as a run would not stow it.
            let entry = entries
                .iter()
                .find(|entry| &entry.name == name)
                .filter(|entry| entry.target.exists());
            // Native (the default) does not shell out, so it shows no `stow` command.
            let command = if native {
                match entry {
                    Some(_) => format!("native: {name}  (relative symlink)"),
                    None => format!("native: {name} (path unresolved)"),
                }
            } else if let Some(entry) = entry {
                // Paths read as the list shows them (`~/...`). The run uses the real ones.
                let cmd = build_stow_args(
                    Path::new(&display_path(&entry.source, &ctx.home_dir)),
                    Path::new(&display_path(&entry.target, &ctx.home_dir)),
                    &entry.name,
                    flags,
                    &entry.ignores,
                );
                let args: Vec<String> = cmd
                    .get_args()
                    .map(|a| a.to_string_lossy().to_string())
                    .collect();
                format!("{} {}", cmd.get_program().to_string_lossy(), args.join(" "))
            } else {
                format!("stow {name} (path unresolved)")
            };
            let files = entry
                .and_then(|entry| {
                    linkable_files(&entry.dir, &entry.ignores, Some(global.as_path())).ok()
                })
                .map(|files| files.len());
            let target = entry.map(|entry| display_path(&entry.target, &ctx.home_dir));
            (
                name.clone(),
                Preview {
                    target,
                    command,
                    files,
                },
            )
        })
        .collect();
    Plan {
        engine: if native { "native" } else { "stow" },
        packages,
    }
}

fn fact(label: &'static str, value: String) -> (Cow<'static, str>, String) {
    (Cow::Borrowed(label), value)
}

/// One command line. A `stow` command is shown as the shell would run it.
fn command_line(engine: &str, command: &str, colors: &ThemeColors) -> Line<'static> {
    let text = if engine == "stow" {
        format!("$ {command}")
    } else {
        command.to_string()
    };
    Line::styled(text, Style::default().fg(colors.text))
}

/// Whether the run changes files: a dry run does not, and says so.
fn dry_run_line(flags: &StowFlags, colors: &ThemeColors) -> Line<'static> {
    if flags.dry_run {
        Line::styled(
            "dry-run is on: no files change.",
            Style::default().fg(colors.text_dim),
        )
    } else {
        let color = match flags.action {
            StowAction::Unstow => colors.warn,
            _ => colors.text,
        };
        Line::styled("dry-run is off: files change.", Style::default().fg(color))
    }
}

impl ConfirmComponent {
    pub fn new(selected_packages: Vec<String>, ctx: &AppContext) -> Self {
        Self::build(selected_packages, ctx, None)
    }

    /// A review that always stows, whatever the global action is: the bootstrap item.
    pub fn stow_only(selected_packages: Vec<String>, ctx: &AppContext) -> Self {
        Self::build(selected_packages, ctx, Some(StowAction::Stow))
    }

    fn build(selected_packages: Vec<String>, ctx: &AppContext, action: Option<StowAction>) -> Self {
        let selected_packages = in_review_order(selected_packages);
        let flags = flags_for(ctx, action);
        let packages = build_rows(&selected_packages, ctx);
        let plan = build_plan(&selected_packages, ctx, &flags);
        let mut component = Self {
            selected_packages,
            action,
            packages,
            plan,
            plan_flags: flags,
            list: ListView::default(),
            detail: Detail::default(),
            actions: None,
        };
        component.sync(ctx);
        component
    }

    fn flags(&self, ctx: &AppContext) -> StowFlags {
        flags_for(ctx, self.action)
    }

    /// The packages the run covers.
    fn count(&self) -> usize {
        self.selected_packages.len()
    }

    /// Brings the plan, the detail and the cursor up to date with the flags.
    fn sync(&mut self, ctx: &AppContext) {
        let flags = self.flags(ctx);
        if flags != self.plan_flags {
            self.plan = build_plan(&self.selected_packages, ctx, &flags);
            self.plan_flags = flags;
        }
        self.list.cursor = self
            .list
            .cursor
            .min(self.packages.rows().len().saturating_sub(1));
        self.detail = self.detail_for(ctx);
    }

    /// The name of the package under the cursor.
    fn cursor_name(&self) -> Option<&str> {
        self.packages
            .rows()
            .get(self.list.cursor)
            .map(|row| row.name.as_str())
    }

    /// The detail for the cursor's package: its command and target.
    fn detail_for(&self, ctx: &AppContext) -> Detail {
        let colors = ThemeColors::from_theme(ctx.core.theme);
        let flags = self.flags(ctx);
        match self.cursor_name() {
            Some(name) => self.package_detail(name, &flags, &colors),
            None => Detail::default(),
        }
    }

    fn package_detail(&self, name: &str, flags: &StowFlags, colors: &ThemeColors) -> Detail {
        let Some((_, preview)) = self
            .plan
            .packages
            .iter()
            .find(|(package, _)| package == name)
        else {
            return Detail::default();
        };
        let links = match preview.files {
            Some(files) => format!("{files} {}", plural(files, "file", "files")),
            None => "unknown".to_string(),
        };
        Detail {
            headline: vec![Line::styled(
                format!("{} {name}", flags.action.as_str()),
                Style::default().fg(colors.text),
            )],
            facts: vec![
                fact("engine", self.plan.engine.to_string()),
                fact(
                    "target",
                    preview
                        .target
                        .clone()
                        .unwrap_or_else(|| "not found".to_string()),
                ),
                fact("links", links),
            ],
            sections: vec![
                command_line(self.plan.engine, &preview.command, colors),
                Line::raw(""),
                dry_run_line(flags, colors),
            ],
        }
    }

    fn detail_title(&self) -> String {
        self.cursor_name().unwrap_or("review").to_string()
    }

    /// Runs the selection on the run screen, which keeps the run's packages and their
    /// order so it can show the one in flight.
    fn run(&self, ctx: &AppContext) -> Transition {
        let flags = self.flags(ctx);
        let (ops, skipped) = build_execution_results(
            &self.selected_packages,
            &ctx.core.config,
            &ctx.config_dir,
            &ctx.home_dir,
            &flags,
        );
        let planned = ops.iter().map(|op| op.package.clone()).collect();
        // Select the link engine from config (native default, stow opt-in).
        let engine = engine_for(ctx.core.config.effective_engine());
        let total = ops.len() + skipped.len();
        let rx = spawn_execution(ops, engine);
        Transition::Replace(Box::new(
            ExecuteComponent::new(rx, total, skipped).for_run(flags.action.as_str(), planned),
        ))
    }
}

impl Component for ConfirmComponent {
    fn id(&self) -> &'static str {
        "Confirm"
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }

    fn handle_key(&mut self, key: KeyEvent, ctx: &mut AppContext) -> Transition {
        match key.code {
            KeyCode::Char('y') | KeyCode::Char('Y') => return self.run(ctx),
            KeyCode::Char('n') | KeyCode::Char('N') | KeyCode::Esc => return Transition::Pop,
            _ => {}
        }
        if Input::handle_list_navigation(&key, &mut self.list.cursor, self.packages.rows().len()) {
            self.sync(ctx);
            return Transition::Handled;
        }
        Transition::None
    }

    fn update(&mut self, ctx: &mut AppContext) -> Transition {
        // The global flag keys stay live on this screen, so the plan follows them.
        self.sync(ctx);
        Transition::None
    }

    fn hints(&self, ctx: &AppContext) -> Vec<Hint> {
        review_hints(self.flags(ctx).action.as_str())
    }

    fn handle_mouse(&mut self, ev: MouseEvent, ctx: &mut AppContext) -> Transition {
        if let Some(actions) = self.actions {
            let buttons = review_buttons(self.flags(ctx).action.as_str(), self.count());
            if let Some(key) = button_click(actions, &buttons, ev) {
                return self.handle_key(key, ctx);
            }
        }
        let len = self.packages.rows().len();
        match click_list(&mut self.list, ev, len) {
            Some(_) => Transition::Handled,
            None => Transition::None,
        }
    }

    fn prepare(&mut self, area: Rect) {
        let rects = screen_rects(area, Shape::Review);
        self.list.prepare(rects.list, self.packages.rows().len());
        self.actions = rects.actions;
    }

    fn render(&mut self, frame: &mut Frame, area: Rect, ctx: &AppContext) {
        let colors = ThemeColors::from_theme(ctx.core.theme);
        let icons = Icons::for_set(ctx.core.icons);
        let rects = screen_rects(area, Shape::Review);

        let width = list_panel::inner(rects.list).width;
        let lines: Vec<Line> = self
            .packages
            .rows()
            .iter()
            .enumerate()
            .map(|(i, row)| package_line(row, i == self.list.cursor, false, icons, &colors, width))
            .collect();
        let action = self.flags(ctx).action.as_str();
        let count = self.count();
        let title = format!("{action} {count} {}", plural(count, "package", "packages"));
        let empty = Empty {
            message: "nothing selected",
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
        render_detail(
            frame,
            rects.detail,
            &colors,
            &self.detail_title(),
            &self.detail,
        );
        if let Some(actions) = rects.actions {
            render_action_bar(frame, actions, &colors, &review_buttons(action, count));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use andre_core::config::Config;

    /// A flat packages folder `src` with the package `alpha`, linking into `tgt`.
    fn fixture() -> (tempfile::TempDir, AppContext) {
        let tmp = tempfile::TempDir::new().unwrap();
        std::fs::create_dir_all(tmp.path().join("src/alpha")).unwrap();
        std::fs::create_dir_all(tmp.path().join("tgt")).unwrap();
        let mut config = Config::default_empty();
        config.set_packages(Some(tmp.path().join("src").display().to_string()));
        config.target = Some(tmp.path().join("tgt").display().to_string());
        let ctx = AppContext::for_tests(config, tmp.path());
        (tmp, ctx)
    }

    fn names(list: &PackageList) -> Vec<String> {
        list.rows().iter().map(|row| row.name.clone()).collect()
    }

    #[test]
    fn test_review_lists_each_package_in_name_order() {
        let (tmp, ctx) = fixture();
        std::fs::create_dir_all(tmp.path().join("src/beta")).unwrap();
        let component = ConfirmComponent::new(vec!["beta".into(), "alpha".into()], &ctx);

        assert_eq!(names(&component.packages), ["alpha", "beta"]);
        assert_eq!(component.selected_packages, ["alpha", "beta"]);
    }

    #[test]
    fn test_review_drops_a_repeated_name() {
        let (_tmp, ctx) = fixture();
        let component = ConfirmComponent::new(vec!["alpha".into(), "alpha".into()], &ctx);
        assert_eq!(component.count(), 1);
    }

    #[test]
    fn test_native_preview_names_the_relative_link_when_both_paths_resolve() {
        let (_tmp, ctx) = fixture();
        let component = ConfirmComponent::new(vec!["alpha".into()], &ctx);
        let (_, preview) = &component.plan.packages[0];

        assert_eq!(preview.command, "native: alpha  (relative symlink)");
        assert_eq!(preview.files, Some(0), "the package folder is empty");
    }

    #[test]
    fn test_native_preview_marks_an_unresolved_target() {
        let (tmp, mut ctx) = fixture();
        ctx.core.config.target = Some(tmp.path().join("gone").display().to_string());
        let component = ConfirmComponent::new(vec!["alpha".into()], &ctx);
        let (_, preview) = &component.plan.packages[0];

        assert_eq!(preview.command, "native: alpha (path unresolved)");
        assert_eq!(preview.target, None);
    }

    #[test]
    fn test_a_package_whose_target_is_missing_has_no_status() {
        let (tmp, mut ctx) = fixture();
        ctx.core.config.target = Some(tmp.path().join("gone").display().to_string());
        let component = ConfirmComponent::new(vec!["alpha".into()], &ctx);
        assert_eq!(component.packages.rows()[0].status, None);
    }

    #[test]
    fn test_a_name_that_is_not_a_package_has_no_status_and_is_skipped() {
        let (_tmp, ctx) = fixture();
        let component = ConfirmComponent::new(vec!["ghost".into()], &ctx);
        assert_eq!(component.packages.rows()[0].status, None);
        assert_eq!(
            component.plan.packages[0].1.command,
            "native: ghost (path unresolved)"
        );

        let (ops, skipped) = build_execution_results(
            &component.selected_packages,
            &ctx.core.config,
            &ctx.config_dir,
            &ctx.home_dir,
            &StowFlags::default(),
        );
        assert!(ops.is_empty());
        assert_eq!(skipped[0].name, "ghost");
    }

    #[test]
    fn test_dry_run_toggle_rebuilds_the_stow_preview() {
        let (_tmp, mut ctx) = fixture();
        ctx.core.config.global.engine = Some("stow".into());
        let mut component = ConfirmComponent::new(vec!["alpha".into()], &ctx);
        assert!(!component.plan.packages[0].1.command.contains(" -n "));

        ctx.core.flags.dry_run = true;
        component.update(&mut ctx);
        assert!(component.plan.packages[0].1.command.contains(" -n "));
    }

    #[test]
    fn test_stow_preview_shows_paths_as_the_list_does() {
        let (tmp, mut ctx) = fixture();
        ctx.core.config.global.engine = Some("stow".into());
        let component = ConfirmComponent::new(vec!["alpha".into()], &ctx);
        let command = &component.plan.packages[0].1.command;

        assert!(command.starts_with("stow -d ~/src -t ~/tgt "), "{command}");
        assert!(
            command.ends_with(" alpha"),
            "the package is named by its folder: {command}"
        );
        assert!(
            !command.contains(&tmp.path().display().to_string()),
            "{command}"
        );
    }

    /// The run applies packages in the order the review lists them: name order.
    #[test]
    fn test_run_order_is_the_review_order() {
        let tmp = tempfile::TempDir::new().unwrap();
        let names = ["zeta", "alpha", "mid", "beta", "omega", "gamma"];
        let packages = tmp.path().join("src");
        for name in names {
            std::fs::create_dir_all(packages.join(name)).unwrap();
        }
        std::fs::create_dir_all(tmp.path().join("tgt")).unwrap();
        let mut config = Config::default_empty();
        config.set_packages(Some(packages.display().to_string()));
        config.target = Some(tmp.path().join("tgt").display().to_string());
        let ctx = AppContext::for_tests(config, tmp.path());

        let selected: Vec<String> = names.iter().map(|name| name.to_string()).collect();
        let component = ConfirmComponent::new(selected.clone(), &ctx);
        let reviewed = names_of(&component);
        let (ops, skipped) = build_execution_results(
            &component.selected_packages,
            &ctx.core.config,
            &ctx.config_dir,
            &ctx.home_dir,
            &StowFlags::default(),
        );
        let run: Vec<String> = ops.iter().map(|op| op.package.clone()).collect();

        assert!(skipped.is_empty());
        assert_eq!(reviewed.len(), 6);
        assert_eq!(run, reviewed);
    }

    fn names_of(component: &ConfirmComponent) -> Vec<String> {
        names(&component.packages)
    }

    #[test]
    fn test_unstow_review_says_files_change_when_dry_run_is_off() {
        let (_tmp, mut ctx) = fixture();
        ctx.core.flags.action = StowAction::Unstow;
        let component = ConfirmComponent::new(vec!["alpha".into()], &ctx);

        let flags = component.flags(&ctx);
        let line = dry_run_line(&flags, &ThemeColors::from_theme(ctx.core.theme));
        assert_eq!(
            line.spans[0].content, "dry-run is off: files change.",
            "{line:?}"
        );
        assert_eq!(
            line.style.fg,
            Some(ThemeColors::from_theme(ctx.core.theme).warn)
        );
    }
}
