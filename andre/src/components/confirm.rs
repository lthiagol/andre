//! The review: what a run would do, before it runs. The left pane is the selected groups and
//! packages with the status each has now; the right pane is the command and target of the
//! cursor's row. `y` runs the selection on the run screen, `n` or `Esc` goes back. Nothing
//! changes on disk here: commands are previewed and files are counted.

use std::borrow::Cow;
use std::collections::{BTreeMap, HashMap};
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
use crate::ui::tree::{tree_line_without_checkboxes, Tree, TreeGroup, TreePackage, TreeRow};
use crate::ui::{display_path, plural};

pub struct ConfirmComponent {
    pub selected_packages: HashMap<String, Vec<String>>,
    /// The action this review runs. `None` follows the global action.
    action: Option<StowAction>,
    /// Groups and packages with the status each has now. Statuses are read once, when the
    /// review opens.
    tree: Tree,
    /// The tree's visible rows, as of the last `sync`.
    rows: Vec<TreeRow>,
    /// What each package would run. Rebuilt when a global flag changes.
    plan: Plan,
    /// The flags `plan` was built from.
    plan_flags: StowFlags,
    pub list: ListView,
    /// The detail of the cursor's row. Built by `sync`.
    detail: Detail,
    /// The action bar, as the last `prepare` laid it out.
    actions: Option<Rect>,
}

/// What a run does, worked out from disk when the review opens and when a flag changes.
#[derive(Default)]
struct Plan {
    /// `native` or `stow`: the engine the run uses.
    engine: &'static str,
    groups: BTreeMap<String, GroupPlan>,
}

struct GroupPlan {
    /// Where the group links, as a display path. `None` when the path does not resolve.
    target: Option<String>,
    /// Each selected package's command, in selection order.
    packages: Vec<(String, Preview)>,
}

struct Preview {
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

/// The selection, groups in name order.
fn sorted(selected: &HashMap<String, Vec<String>>) -> BTreeMap<&str, &[String]> {
    selected
        .iter()
        .map(|(group, packages)| (group.as_str(), packages.as_slice()))
        .collect()
}

/// The tree the review lists. Reads each group's statuses through the status cache.
fn build_tree(selected: &HashMap<String, Vec<String>>, ctx: &AppContext) -> Tree {
    let config = &ctx.core.config;
    let global = global_ignore_in(&ctx.home_dir);
    let mut cache = ctx.status_cache.borrow_mut();
    let groups = sorted(selected)
        .into_iter()
        .map(|(name, packages)| {
            let source = config.get_group_source(name, &ctx.config_dir, &ctx.home_dir);
            let target = config.get_group_target(name, &ctx.config_dir, &ctx.home_dir);
            if let (Some(source), Some(target)) = (&source, &target) {
                cache.prefetch_group(source, target, name, packages, Some(global.as_path()));
            }
            TreeGroup {
                name: name.to_string(),
                packages: packages
                    .iter()
                    .map(|package| TreePackage {
                        name: package.clone(),
                        status: cache.get(name, package),
                        mark: None,
                        meta: String::new(),
                    })
                    .collect(),
            }
        })
        .collect();
    Tree::new(groups)
}

/// The plan for `selected` under `flags`. Reads each package's folder to count its files.
fn build_plan(
    selected: &HashMap<String, Vec<String>>,
    ctx: &AppContext,
    flags: &StowFlags,
) -> Plan {
    let config = &ctx.core.config;
    let native = config.effective_engine() == EngineKind::Native;
    let global = global_ignore_in(&ctx.home_dir);
    let groups = sorted(selected)
        .into_iter()
        .map(|(group, packages)| {
            let source = config.get_group_source(group, &ctx.config_dir, &ctx.home_dir);
            let target = config.get_group_target(group, &ctx.config_dir, &ctx.home_dir);
            let ignores = config.effective_ignores(group);
            let packages = packages
                .iter()
                .map(|pkg| {
                    // Native (the default) does not shell out, so it shows no `stow` command.
                    let command = if native {
                        match (&source, &target) {
                            (Some(_), Some(_)) => format!("native: {pkg}  (relative symlink)"),
                            _ => format!("native: {pkg} (path unresolved)"),
                        }
                    } else if let (Some(src), Some(tgt)) = (&source, &target) {
                        // Paths read as the list shows them (`~/...`). The run uses the real ones.
                        let cmd = build_stow_args(
                            Path::new(&display_path(src, &ctx.home_dir)),
                            Path::new(&display_path(tgt, &ctx.home_dir)),
                            pkg,
                            flags,
                            &ignores,
                        );
                        let args: Vec<String> = cmd
                            .get_args()
                            .map(|a| a.to_string_lossy().to_string())
                            .collect();
                        format!("{} {}", cmd.get_program().to_string_lossy(), args.join(" "))
                    } else {
                        format!("stow {pkg} (path unresolved)")
                    };
                    let files = source
                        .as_ref()
                        .and_then(|src| {
                            linkable_files(&src.join(pkg), &ignores, Some(global.as_path())).ok()
                        })
                        .map(|files| files.len());
                    (pkg.clone(), Preview { command, files })
                })
                .collect();
            let target = target
                .as_ref()
                .map(|path| display_path(path, &ctx.home_dir));
            (group.to_string(), GroupPlan { target, packages })
        })
        .collect();
    Plan {
        engine: if native { "native" } else { "stow" },
        groups,
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
    pub fn new(selected_packages: HashMap<String, Vec<String>>, ctx: &AppContext) -> Self {
        Self::build(selected_packages, ctx, None)
    }

    /// A review that always stows, whatever the global action is: the bootstrap item.
    pub fn stow_only(selected_packages: HashMap<String, Vec<String>>, ctx: &AppContext) -> Self {
        Self::build(selected_packages, ctx, Some(StowAction::Stow))
    }

    fn build(
        selected_packages: HashMap<String, Vec<String>>,
        ctx: &AppContext,
        action: Option<StowAction>,
    ) -> Self {
        let flags = flags_for(ctx, action);
        let tree = build_tree(&selected_packages, ctx);
        let plan = build_plan(&selected_packages, ctx, &flags);
        let mut component = Self {
            selected_packages,
            action,
            tree,
            rows: Vec::new(),
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
        self.selected_packages.values().map(Vec::len).sum()
    }

    /// Brings the rows, the plan and the detail up to date with the tree and the flags.
    fn sync(&mut self, ctx: &AppContext) {
        let flags = self.flags(ctx);
        if flags != self.plan_flags {
            self.plan = build_plan(&self.selected_packages, ctx, &flags);
            self.plan_flags = flags;
        }
        self.rows = self.tree.rows();
        self.list.cursor = self.list.cursor.min(self.rows.len().saturating_sub(1));
        self.detail = self.detail_for(ctx);
    }

    /// The row a fold on `row` lands on: the group header it sits under.
    fn group_header(&self, row: usize) -> usize {
        self.rows
            .iter()
            .take(row + 1)
            .rposition(|r| matches!(r, TreeRow::Group { .. }))
            .unwrap_or(row)
    }

    /// The detail for the cursor's row: a package's command and target, or a group's
    /// commands for all of its packages.
    fn detail_for(&self, ctx: &AppContext) -> Detail {
        let colors = ThemeColors::from_theme(ctx.core.theme);
        let flags = self.flags(ctx);
        match self.rows.get(self.list.cursor) {
            Some(TreeRow::Package { group, name, .. }) => {
                self.package_detail(group, name, &flags, &colors)
            }
            Some(TreeRow::Group { name, .. }) => self.group_detail(name, &flags, &colors),
            None => Detail::default(),
        }
    }

    fn package_detail(
        &self,
        group: &str,
        name: &str,
        flags: &StowFlags,
        colors: &ThemeColors,
    ) -> Detail {
        let Some(plan) = self.plan.groups.get(group) else {
            return Detail::default();
        };
        let Some((_, preview)) = plan.packages.iter().find(|(package, _)| package == name) else {
            return Detail::default();
        };
        let links = match preview.files {
            Some(files) => format!("{files} {}", plural(files, "file", "files")),
            None => "unknown".to_string(),
        };
        Detail {
            headline: vec![Line::styled(
                format!("{} {group}/{name}", flags.action.as_str()),
                Style::default().fg(colors.text),
            )],
            facts: vec![
                fact("engine", self.plan.engine.to_string()),
                fact(
                    "target",
                    plan.target
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

    fn group_detail(&self, group: &str, flags: &StowFlags, colors: &ThemeColors) -> Detail {
        let Some(plan) = self.plan.groups.get(group) else {
            return Detail::default();
        };
        let mut sections: Vec<Line<'static>> = plan
            .packages
            .iter()
            .map(|(_, preview)| command_line(self.plan.engine, &preview.command, colors))
            .collect();
        if !sections.is_empty() {
            sections.push(Line::raw(""));
            sections.push(dry_run_line(flags, colors));
        }
        Detail {
            headline: vec![Line::styled(
                format!("{} {group}", flags.action.as_str()),
                Style::default().fg(colors.text),
            )],
            facts: vec![
                fact(
                    "target",
                    plan.target
                        .clone()
                        .unwrap_or_else(|| "not found".to_string()),
                ),
                fact("packages", plan.packages.len().to_string()),
            ],
            sections,
        }
    }

    fn detail_title(&self) -> String {
        match self.rows.get(self.list.cursor) {
            Some(TreeRow::Package { group, name, .. }) => format!("{group}/{name}"),
            Some(TreeRow::Group { name, .. }) => name.clone(),
            None => "review".to_string(),
        }
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
        let planned = ops
            .iter()
            .map(|op| (op.group.clone(), op.package.clone()))
            .collect();
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
        if Input::handle_list_navigation(&key, &mut self.list.cursor, self.rows.len()) {
            self.sync(ctx);
            return Transition::Handled;
        }
        match key.code {
            KeyCode::Right => self.tree.fold(self.list.cursor, false),
            KeyCode::Left => {
                // On a package, Left moves to its group's header. On a header, it folds the group.
                let header = self.group_header(self.list.cursor);
                if header != self.list.cursor {
                    self.list.cursor = header;
                } else {
                    self.tree.fold(header, true);
                }
            }
            _ => return Transition::None,
        }
        self.sync(ctx);
        Transition::Handled
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
        let len = self.rows.len();
        match click_list(&mut self.list, ev, len) {
            Some(_) => Transition::Handled,
            None => Transition::None,
        }
    }

    fn prepare(&mut self, area: Rect) {
        let rects = screen_rects(area, Shape::Review);
        self.list.prepare(rects.list, self.rows.len());
        self.actions = rects.actions;
    }

    fn render(&mut self, frame: &mut Frame, area: Rect, ctx: &AppContext) {
        let colors = ThemeColors::from_theme(ctx.core.theme);
        let icons = Icons::for_set(ctx.core.icons);
        let rects = screen_rects(area, Shape::Review);

        let width = list_panel::inner(rects.list).width;
        let lines: Vec<Line> = self
            .rows
            .iter()
            .enumerate()
            .map(|(i, row)| {
                tree_line_without_checkboxes(row, i == self.list.cursor, icons, &colors, width)
            })
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

    /// A group `dots` whose source and target both exist, with the package `alpha`.
    fn fixture() -> (tempfile::TempDir, AppContext) {
        let tmp = tempfile::TempDir::new().unwrap();
        std::fs::create_dir_all(tmp.path().join("src/alpha")).unwrap();
        std::fs::create_dir_all(tmp.path().join("tgt")).unwrap();
        let mut config = Config::default_empty();
        config.add_group(
            "dots".into(),
            tmp.path().join("src").display().to_string(),
            tmp.path().join("tgt").display().to_string(),
        );
        let ctx = AppContext::for_tests(config, tmp.path());
        (tmp, ctx)
    }

    fn selected(group: &str, package: &str) -> HashMap<String, Vec<String>> {
        HashMap::from([(group.to_string(), vec![package.to_string()])])
    }

    #[test]
    fn test_review_lists_each_group_then_its_packages() {
        let (_tmp, ctx) = fixture();
        let component = ConfirmComponent::new(selected("dots", "alpha"), &ctx);

        assert!(matches!(
            &component.rows[..],
            [
                TreeRow::Group { name, folded: false, .. },
                TreeRow::Package { group, name: package, mark: None, .. },
            ] if name == "dots" && group == "dots" && package == "alpha"
        ));
    }

    #[test]
    fn test_native_preview_names_the_relative_link_when_both_paths_resolve() {
        let (_tmp, ctx) = fixture();
        let component = ConfirmComponent::new(selected("dots", "alpha"), &ctx);
        let (_, preview) = &component.plan.groups["dots"].packages[0];

        assert_eq!(preview.command, "native: alpha  (relative symlink)");
        assert_eq!(preview.files, Some(0), "the package folder is empty");
    }

    #[test]
    fn test_native_preview_marks_unresolved_group_paths() {
        let (_tmp, mut ctx) = fixture();
        ctx.core.config.remove_group("dots");
        let component = ConfirmComponent::new(selected("dots", "alpha"), &ctx);

        assert_eq!(
            component.plan.groups["dots"].packages[0].1.command,
            "native: alpha (path unresolved)"
        );
        assert_eq!(component.plan.groups["dots"].target, None);
    }

    #[test]
    fn test_dry_run_toggle_rebuilds_the_stow_preview() {
        let (_tmp, mut ctx) = fixture();
        ctx.core.config.global.engine = Some("stow".into());
        let mut component = ConfirmComponent::new(selected("dots", "alpha"), &ctx);
        assert!(!component.plan.groups["dots"].packages[0]
            .1
            .command
            .contains(" -n "));

        ctx.core.flags.dry_run = true;
        component.update(&mut ctx);
        assert!(component.plan.groups["dots"].packages[0]
            .1
            .command
            .contains(" -n "));
    }

    #[test]
    fn test_stow_preview_shows_paths_as_the_list_does() {
        let (tmp, mut ctx) = fixture();
        ctx.core.config.global.engine = Some("stow".into());
        let component = ConfirmComponent::new(selected("dots", "alpha"), &ctx);
        let command = &component.plan.groups["dots"].packages[0].1.command;

        assert!(command.starts_with("stow -d ~/src -t ~/tgt "), "{command}");
        assert!(
            !command.contains(&tmp.path().display().to_string()),
            "{command}"
        );
    }

    /// The run applies packages in the order the review lists them: groups by name, then
    /// each group's packages in selection order.
    #[test]
    fn test_run_order_is_the_review_order_across_groups() {
        let tmp = tempfile::TempDir::new().unwrap();
        let names = ["zeta", "alpha", "mid", "beta", "omega", "gamma"];
        let mut config = Config::default_empty();
        for name in names {
            let src = tmp.path().join("src").join(name);
            let tgt = tmp.path().join("tgt").join(name);
            std::fs::create_dir_all(src.join("pkg-b")).unwrap();
            std::fs::create_dir_all(src.join("pkg-a")).unwrap();
            std::fs::create_dir_all(&tgt).unwrap();
            config.add_group(
                name.into(),
                src.display().to_string(),
                tgt.display().to_string(),
            );
        }
        let selected: HashMap<String, Vec<String>> = names
            .iter()
            .map(|name| {
                (
                    name.to_string(),
                    vec!["pkg-b".to_string(), "pkg-a".to_string()],
                )
            })
            .collect();
        let ctx = AppContext::for_tests(config, tmp.path());

        let component = ConfirmComponent::new(selected.clone(), &ctx);
        let reviewed: Vec<(String, String)> = component
            .rows
            .iter()
            .filter_map(|row| match row {
                TreeRow::Package { group, name, .. } => Some((group.clone(), name.clone())),
                TreeRow::Group { .. } => None,
            })
            .collect();
        let (ops, skipped) = build_execution_results(
            &selected,
            &ctx.core.config,
            &ctx.config_dir,
            &ctx.home_dir,
            &StowFlags::default(),
        );
        let run: Vec<(String, String)> = ops
            .iter()
            .map(|op| (op.group.clone(), op.package.clone()))
            .collect();

        assert!(skipped.is_empty());
        assert_eq!(reviewed.len(), 12);
        assert_eq!(run, reviewed);
    }

    #[test]
    fn test_unstow_review_says_files_change_when_dry_run_is_off() {
        let (_tmp, mut ctx) = fixture();
        ctx.core.flags.action = StowAction::Unstow;
        let component = ConfirmComponent::new(selected("dots", "alpha"), &ctx);

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
