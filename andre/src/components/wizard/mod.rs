//! The new-package wizard, six steps: pick a group, name the package (typing filters the
//! group's packages, and the create row is always first), pick the source (an empty package,
//! or live files moved in through a file picker), choose the ignore files, choose the profiles
//! the package joins, then review and create it.
//!
//! One component holds every answer, so Esc back never loses one. Nothing is written until
//! the review's Enter.

mod finish;
mod finish_view;
mod picker;
mod view;

use std::borrow::Cow;
use std::collections::BTreeMap;

use andre_core::create::{is_valid_package_name, PackagePlan};
use andre_core::discover_packages;
use crossterm::event::{KeyCode, KeyEvent, MouseEvent};
use ratatui::{layout::Rect, Frame};

use crate::components::{AppContext, Component, InputMode, Transition};
use crate::input::{self, Input};
use crate::ui::footer::{
    wizard_done_hints, wizard_files_hints, wizard_group_hints, wizard_ignore_hints,
    wizard_name_hints, wizard_profiles_hints, wizard_review_hints, wizard_source_hints, Hint,
};
use crate::ui::layout::{screen_rects, Shape};
use crate::ui::list_panel::{click_list, ListView};
use crate::ui::text_field::TextField;
use finish::{Done, Gitignore, ProfileCount, ReviewLine, IGNORE_ROWS};
use picker::FilePicker;

/// The column of a row's checkbox in the file, ignore and profile lists: the gutter is column 0.
const MARK_COL: u16 = 1;

/// The steps as the step bar names them.
pub const STEPS: &[&str] = &["group", "name", "source", "ignore", "profiles", "review"];

/// The source choices, in the order the source step lists them.
pub const SOURCES: [Source; 2] = [Source::Empty, Source::LiveFiles];

/// The screen the wizard shows.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub enum Step {
    #[default]
    Group,
    Name,
    Source,
    /// The file picker opened from [`Source::LiveFiles`]. The step bar shows it as step 3.
    Files,
    Ignore,
    Profiles,
    Review,
}

impl Step {
    /// Index of the step in [`STEPS`].
    fn bar_index(self) -> usize {
        match self {
            Step::Group => 0,
            Step::Name => 1,
            Step::Source | Step::Files => 2,
            Step::Ignore => 3,
            Step::Profiles => 4,
            Step::Review => 5,
        }
    }
}

/// Where the package's files come from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Source {
    Empty,
    LiveFiles,
}

/// A row of the name step: the create row first, then the group's packages the typed text
/// matches.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NameRow {
    Create,
    Existing(String),
}

#[derive(Default)]
pub struct NewPackageWizard {
    step: Step,
    /// Every group with its packages in name order. Read once, on the first update or key.
    listing: BTreeMap<String, Vec<String>>,
    loaded: bool,
    group_list: ListView,
    /// The group chosen on step 1.
    group: Option<String>,
    name: TextField,
    name_list: ListView,
    source: Option<Source>,
    source_list: ListView,
    picker: Option<FilePicker>,
    /// Step 4: the `.gitignore` template and whether `.stow-local-ignore` is written.
    gitignore: Gitignore,
    stow_local_ignore: bool,
    ignore_list: ListView,
    /// Step 5: the profile names, read when the step is entered, what each holds, and the
    /// ones chosen.
    profile_names: Vec<String>,
    profile_counts: BTreeMap<String, ProfileCount>,
    chosen_profiles: Vec<String>,
    profile_list: ListView,
    /// Step 6: the plan the review shows, its lines, and how far the lines are scrolled.
    plan: Option<PackagePlan>,
    review: Vec<ReviewLine>,
    review_scroll: usize,
    /// Rows the review's lines get, set in `prepare`.
    review_height: u16,
    /// Set once the review has run (or was a dry run). Enter then leaves the wizard.
    done: Option<Done>,
}

impl Component for NewPackageWizard {
    fn id(&self) -> &'static str {
        "NewPackage"
    }

    fn crumbs(&self, _ctx: &AppContext) -> Vec<Cow<'static, str>> {
        match self.step {
            Step::Files => vec![Cow::Borrowed("new package"), Cow::Borrowed("files")],
            _ => vec![Cow::Borrowed("new package")],
        }
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }

    fn input_mode(&self) -> InputMode {
        match self.step {
            Step::Name => InputMode::TextInput,
            Step::Group
            | Step::Source
            | Step::Files
            | Step::Ignore
            | Step::Profiles
            | Step::Review => InputMode::Normal,
        }
    }

    /// The wizard has no dialogs. Its name field is part of the name step, so the step's list
    /// takes clicks and wheel notches while the field has focus.
    fn mouse_blocked(&self) -> bool {
        false
    }

    fn update(&mut self, ctx: &mut AppContext) -> Transition {
        self.load(ctx);
        if let Some(picker) = &mut self.picker {
            picker.refresh();
            picker.read_current();
        }
        Transition::None
    }

    fn handle_key(&mut self, key: KeyEvent, ctx: &mut AppContext) -> Transition {
        // Keys can arrive before the next update, so the listing is brought up to date first.
        self.load(ctx);
        let transition = match self.step {
            Step::Group => self.group_key(&key),
            Step::Name => self.name_key(&key),
            Step::Source => self.source_key(&key, ctx),
            Step::Files => self.files_key(&key, ctx),
            Step::Ignore => self.ignore_key(&key, ctx),
            Step::Profiles => self.profiles_key(&key, ctx),
            Step::Review => self.review_key(&key, ctx),
        };
        // The cursor may have moved onto another file.
        if let Some(picker) = &mut self.picker {
            picker.read_current();
        }
        transition
    }

    fn hints(&self, _ctx: &AppContext) -> Vec<Hint> {
        match self.step {
            Step::Group => wizard_group_hints(),
            Step::Name => wizard_name_hints(),
            Step::Source => wizard_source_hints(),
            Step::Files => wizard_files_hints(),
            Step::Ignore => wizard_ignore_hints(),
            Step::Profiles => wizard_profiles_hints(),
            Step::Review if self.done.is_some() => wizard_done_hints(),
            Step::Review => wizard_review_hints(),
        }
    }

    fn handle_mouse(&mut self, ev: MouseEvent, ctx: &mut AppContext) -> Transition {
        self.load(ctx);
        let rows = self.step_rows();
        let Some(view) = self.step_list_mut() else {
            return Transition::None;
        };
        let Some(click) = click_list(view, ev, rows) else {
            return Transition::None;
        };
        let key = match self.step {
            Step::Group | Step::Name | Step::Source => click.activate.then_some(KeyCode::Enter),
            Step::Files => {
                let entry = self
                    .picker
                    .as_ref()
                    .and_then(|p| p.entries().get(click.row));
                match entry {
                    Some(entry) if click.col == MARK_COL && entry.is_selectable() => {
                        Some(KeyCode::Char(' '))
                    }
                    Some(entry) if click.activate && entry.is_dir => Some(KeyCode::Right),
                    Some(entry) if click.activate && entry.is_selectable() => {
                        Some(KeyCode::Char(' '))
                    }
                    _ => None,
                }
            }
            Step::Ignore | Step::Profiles => {
                (click.col == MARK_COL || click.activate).then_some(KeyCode::Char(' '))
            }
            Step::Review => None,
        };
        match key {
            Some(code) => self.handle_key(input::key(code), ctx),
            None => Transition::Handled,
        }
    }

    fn prepare(&mut self, area: Rect) {
        let rects = screen_rects(area, self.shape());
        match self.step {
            Step::Group => {
                let len = self.groups().len();
                self.group_list.prepare(rects.list, len);
            }
            Step::Name => {
                let len = self.name_rows().len();
                let (_, matches) = view::name_panels(rects.list, self.name.error.is_some());
                self.name_list.prepare(matches, len);
            }
            Step::Source => self.source_list.prepare(rects.list, SOURCES.len()),
            Step::Files => {
                if let Some(picker) = &mut self.picker {
                    let len = picker.entries().len();
                    picker.list.prepare(rects.list, len);
                }
            }
            Step::Ignore => self.ignore_list.prepare(rects.list, IGNORE_ROWS),
            Step::Profiles => {
                let len = self.profile_names.len();
                self.profile_list.prepare(rects.list, len);
            }
            Step::Review => {
                let body = finish_view::review_body(rects.list);
                self.review_height = body.height.saturating_sub(self.head_rows());
                self.review_scroll = self.review_scroll.min(self.review_last_scroll());
            }
        }
    }

    fn render(&mut self, frame: &mut Frame, area: Rect, ctx: &AppContext) {
        view::render(frame, area, ctx, self);
    }
}

impl NewPackageWizard {
    /// The rows of the list the current step shows. The review has none.
    fn step_rows(&self) -> usize {
        match self.step {
            Step::Group => self.groups().len(),
            Step::Name => self.name_rows().len(),
            Step::Source => SOURCES.len(),
            Step::Files => self.picker.as_ref().map_or(0, |p| p.entries().len()),
            Step::Ignore => IGNORE_ROWS,
            Step::Profiles => self.profile_names.len(),
            Step::Review => 0,
        }
    }

    /// The list the current step shows, if it has one.
    fn step_list_mut(&mut self) -> Option<&mut ListView> {
        match self.step {
            Step::Group => Some(&mut self.group_list),
            Step::Name => Some(&mut self.name_list),
            Step::Source => Some(&mut self.source_list),
            Step::Files => self.picker.as_mut().map(|p| &mut p.list),
            Step::Ignore => Some(&mut self.ignore_list),
            Step::Profiles => Some(&mut self.profile_list),
            Step::Review => None,
        }
    }
}

impl NewPackageWizard {
    /// The layout of the current step: the review is one wide panel, the rest are two-pane.
    pub fn shape(&self) -> Shape {
        match self.step {
            Step::Review => Shape::WizardWide,
            _ => Shape::Wizard,
        }
    }

    /// Rows above the review's lines: the result and a blank row once the review has ended.
    fn head_rows(&self) -> u16 {
        if self.done.is_some() {
            2
        } else {
            0
        }
    }

    /// Group names in name order.
    fn groups(&self) -> Vec<&str> {
        self.listing.keys().map(String::as_str).collect()
    }

    /// The create row, then every package of the chosen group that contains the typed text
    /// (ignoring case). A package equal to the typed text has the create row instead.
    fn name_rows(&self) -> Vec<NameRow> {
        let typed = self.name.value.to_lowercase();
        let mut rows = vec![NameRow::Create];
        for package in self.group_packages() {
            if package.to_lowercase().contains(&typed) && package != &self.name.value {
                rows.push(NameRow::Existing(package.clone()));
            }
        }
        rows
    }

    /// The packages of the chosen group, in name order. Empty before a group is chosen.
    fn group_packages(&self) -> &[String] {
        self.group
            .as_ref()
            .and_then(|group| self.listing.get(group))
            .map_or(&[], Vec::as_slice)
    }

    /// Whether the typed name is a package already in the chosen group.
    fn name_exists(&self) -> bool {
        self.group_packages().contains(&self.name.value)
    }

    fn group_key(&mut self, key: &KeyEvent) -> Transition {
        let len = self.groups().len();
        if Input::handle_list_navigation(key, &mut self.group_list.cursor, len) {
            return Transition::Handled;
        }
        match key.code {
            KeyCode::Enter => self.choose_group(),
            KeyCode::Esc => return Transition::Pop,
            _ => return Transition::None,
        }
        Transition::Handled
    }

    fn choose_group(&mut self) {
        let Some(group) = self
            .groups()
            .get(self.group_list.cursor)
            .map(|g| g.to_string())
        else {
            return;
        };
        // A picker is the folder of one group, so a different group starts it again.
        if self.group.as_deref() != Some(group.as_str()) {
            self.picker = None;
        }
        self.group = Some(group);
        self.name_list.cursor = 0;
        self.step = Step::Name;
    }

    fn name_key(&mut self, key: &KeyEvent) -> Transition {
        // Only the arrows move here: j and k are text on this step.
        let len = self.name_rows().len();
        match key.code {
            KeyCode::Up | KeyCode::Down => {
                self.name_list.step(key.code == KeyCode::Down, len);
                return Transition::Handled;
            }
            KeyCode::Enter => self.choose_name(),
            KeyCode::Esc => self.step = Step::Group,
            _ => {
                if !self.name.handle_key(key) {
                    return Transition::None;
                }
                self.name_list.cursor = 0;
            }
        }
        Transition::Handled
    }

    /// Enter on the name step: the create row needs a valid new name; an existing row takes
    /// that package's name, so its files are added to it.
    fn choose_name(&mut self) {
        match self.name_rows().get(self.name_list.cursor) {
            Some(NameRow::Existing(package)) => {
                let package = package.clone();
                self.name = TextField {
                    cursor: package.chars().count(),
                    value: package,
                    error: None,
                };
            }
            Some(NameRow::Create) => {
                if self.name.value.is_empty() {
                    self.name.error = Some("name is needed".to_string());
                    return;
                }
                if !is_valid_package_name(&self.name.value) {
                    self.name.error = Some("letters, digits, - _ . only".to_string());
                    return;
                }
            }
            None => return,
        }
        self.source_list.cursor = SOURCES
            .iter()
            .position(|source| Some(*source) == self.source)
            .unwrap_or(0);
        self.step = Step::Source;
    }

    fn source_key(&mut self, key: &KeyEvent, ctx: &mut AppContext) -> Transition {
        if Input::handle_list_navigation(key, &mut self.source_list.cursor, SOURCES.len()) {
            return Transition::Handled;
        }
        match key.code {
            KeyCode::Enter => self.choose_source(ctx),
            KeyCode::Esc => self.step = Step::Name,
            _ => return Transition::None,
        }
        Transition::Handled
    }

    /// Enter on the source step. Live files open the picker at the group's target; a group
    /// without a target folder says so and keeps the step.
    fn choose_source(&mut self, ctx: &mut AppContext) {
        let Some(source) = SOURCES.get(self.source_list.cursor).copied() else {
            return;
        };
        if source == Source::LiveFiles && self.picker.is_none() {
            let target = self.group.as_ref().and_then(|group| {
                ctx.core
                    .config
                    .get_group_target(group, &ctx.config_dir, &ctx.home_dir)
            });
            let Some(target) = target else {
                let group = self.group.clone().unwrap_or_default();
                ctx.show_error_toast(format!("{group} has no target folder"));
                return;
            };
            self.picker = Some(FilePicker::new(target));
        }
        self.source = Some(source);
        self.step = match source {
            Source::Empty => Step::Ignore,
            Source::LiveFiles => Step::Files,
        };
    }

    fn files_key(&mut self, key: &KeyEvent, ctx: &mut AppContext) -> Transition {
        let Some(picker) = &mut self.picker else {
            self.step = Step::Source;
            return Transition::Handled;
        };
        let len = picker.entries().len();
        if Input::handle_list_navigation(key, &mut picker.list.cursor, len) {
            return Transition::Handled;
        }
        match key.code {
            KeyCode::Right => picker.open_dir(),
            KeyCode::Left | KeyCode::Backspace => picker.go_up(),
            KeyCode::Char(' ') => {
                if let Err(message) = picker.toggle_mark() {
                    ctx.show_error_toast(message);
                }
            }
            KeyCode::Enter => self.choose_files(ctx),
            KeyCode::Esc => self.step = Step::Source,
            _ => return Transition::None,
        }
        Transition::Handled
    }

    /// Enter on the picker: live files need at least one mark. Empty packages are chosen on
    /// the source step, so an empty mark list here is refused rather than read as one.
    fn choose_files(&mut self, ctx: &mut AppContext) {
        let marked = self
            .picker
            .as_ref()
            .is_some_and(|picker| !picker.marks.is_empty());
        if !marked {
            ctx.show_error_toast("mark files to move in, or go back and pick an empty package");
            return;
        }
        self.step = Step::Ignore;
    }

    /// Reads every group's packages once. Reading the disk is allowed here, not in paint.
    fn load(&mut self, ctx: &AppContext) {
        if self.loaded {
            return;
        }
        self.loaded = true;
        for name in ctx.core.config.groups.keys() {
            let packages = ctx
                .core
                .config
                .get_group_source(name, &ctx.config_dir, &ctx.home_dir)
                .as_deref()
                .map(discover_packages)
                .unwrap_or_default();
            self.listing.insert(name.clone(), packages);
        }
    }
}

#[cfg(test)]
mod tests {
    use crossterm::event::{KeyCode, KeyModifiers};
    use tempfile::TempDir;

    use super::*;
    use crate::components::AppContext;
    use andre_core::config::Config;

    fn key(code: KeyCode) -> KeyEvent {
        KeyEvent::new(code, KeyModifiers::NONE)
    }

    fn type_text(wizard: &mut NewPackageWizard, ctx: &mut AppContext, text: &str) {
        for c in text.chars() {
            wizard.handle_key(key(KeyCode::Char(c)), ctx);
        }
    }

    /// Groups `config` (target `~/.config`) and `home` (target `~`), both under `home/`.
    /// `home` holds `nvim` and `git`; `config` holds `nvim`. The temp dir is the home dir.
    fn fixture() -> (AppContext, TempDir) {
        let dir = TempDir::new().unwrap();
        let config_path = dir.path().join("andre.yml");
        std::fs::write(
            &config_path,
            "global:\n  stow:\n    verbosity: 0\ngroups:\n  home:\n    source: dotfiles/home\n    target: \"~\"\n  config:\n    source: dotfiles/config\n    target: \"~/.config\"\n",
        )
        .unwrap();
        for package in [
            "dotfiles/home/nvim",
            "dotfiles/home/git",
            "dotfiles/config/nvim",
        ] {
            std::fs::create_dir_all(dir.path().join(package)).unwrap();
        }
        let config = Config::load(&config_path).unwrap();
        let ctx = AppContext::for_tests(config, dir.path());
        (ctx, dir)
    }

    /// The wizard on step 1 with its groups read.
    fn start() -> (NewPackageWizard, AppContext, TempDir) {
        let (mut ctx, dir) = fixture();
        let mut wizard = NewPackageWizard::default();
        wizard.update(&mut ctx);
        (wizard, ctx, dir)
    }

    /// Chooses the group at `index` (groups sort as `config`, `home`).
    fn choose(wizard: &mut NewPackageWizard, ctx: &mut AppContext, index: usize) {
        wizard.group_list.cursor = index;
        wizard.handle_key(key(KeyCode::Enter), ctx);
    }

    #[test]
    fn test_group_step_lists_the_configured_groups_in_name_order() {
        let (wizard, _ctx, _dir) = start();
        assert_eq!(wizard.groups(), vec!["config", "home"]);
        assert_eq!(wizard.step, Step::Group);
    }

    #[test]
    fn test_enter_on_a_group_opens_the_name_step_with_its_packages() {
        let (mut wizard, mut ctx, _dir) = start();
        choose(&mut wizard, &mut ctx, 1);

        assert_eq!(wizard.step, Step::Name);
        assert_eq!(wizard.group.as_deref(), Some("home"));
        assert_eq!(wizard.group_packages(), ["git", "nvim"]);
    }

    #[test]
    fn test_name_step_is_text_input_and_the_other_steps_are_normal() {
        let (mut wizard, mut ctx, _dir) = start();
        assert_eq!(wizard.input_mode(), InputMode::Normal);
        choose(&mut wizard, &mut ctx, 1);
        assert_eq!(wizard.input_mode(), InputMode::TextInput);
    }

    #[test]
    fn test_typing_filters_packages_and_the_create_row_stays_first() {
        let (mut wizard, mut ctx, _dir) = start();
        choose(&mut wizard, &mut ctx, 1);
        type_text(&mut wizard, &mut ctx, "nv");

        assert_eq!(
            wizard.name_rows(),
            vec![NameRow::Create, NameRow::Existing("nvim".to_string())]
        );
        assert_eq!(wizard.name_list.cursor, 0);
    }

    #[test]
    fn test_j_is_typed_into_the_name_and_does_not_move_the_cursor() {
        let (mut wizard, mut ctx, _dir) = start();
        choose(&mut wizard, &mut ctx, 1);
        type_text(&mut wizard, &mut ctx, "j");

        assert_eq!(wizard.name.value, "j");
        assert_eq!(wizard.step, Step::Name);
    }

    #[test]
    fn test_enter_on_an_empty_create_row_is_refused_and_keeps_the_step() {
        let (mut wizard, mut ctx, _dir) = start();
        choose(&mut wizard, &mut ctx, 1);
        wizard.name_list.cursor = 0;
        wizard.handle_key(key(KeyCode::Enter), &mut ctx);

        assert_eq!(wizard.step, Step::Name);
        assert_eq!(wizard.name.error.as_deref(), Some("name is needed"));
    }

    #[test]
    fn test_invalid_name_is_refused_with_the_rule() {
        let (mut wizard, mut ctx, _dir) = start();
        choose(&mut wizard, &mut ctx, 1);
        type_text(&mut wizard, &mut ctx, "a b");
        wizard.handle_key(key(KeyCode::Enter), &mut ctx);

        assert_eq!(wizard.step, Step::Name);
        assert_eq!(
            wizard.name.error.as_deref(),
            Some("letters, digits, - _ . only")
        );
    }

    #[test]
    fn test_enter_on_an_existing_row_takes_its_name_and_adds_to_it() {
        let (mut wizard, mut ctx, _dir) = start();
        choose(&mut wizard, &mut ctx, 1);
        type_text(&mut wizard, &mut ctx, "nv");
        wizard.handle_key(key(KeyCode::Down), &mut ctx);
        wizard.handle_key(key(KeyCode::Enter), &mut ctx);

        assert_eq!(wizard.name.value, "nvim");
        assert_eq!(wizard.step, Step::Source);
    }

    #[test]
    fn test_enter_on_the_create_row_keeps_a_new_name() {
        let (mut wizard, mut ctx, _dir) = start();
        choose(&mut wizard, &mut ctx, 1);
        type_text(&mut wizard, &mut ctx, "nv");
        wizard.handle_key(key(KeyCode::Enter), &mut ctx);

        assert_eq!(wizard.name.value, "nv");
        assert!(!wizard.name_exists());
        assert_eq!(wizard.step, Step::Source);
    }

    #[test]
    fn test_esc_on_step_two_returns_to_step_one_with_the_group_still_selected() {
        let (mut wizard, mut ctx, _dir) = start();
        choose(&mut wizard, &mut ctx, 1);
        wizard.handle_key(key(KeyCode::Esc), &mut ctx);

        assert_eq!(wizard.step, Step::Group);
        assert_eq!(wizard.group.as_deref(), Some("home"));
        assert_eq!(wizard.group_list.cursor, 1);
    }

    #[test]
    fn test_esc_keeps_the_typed_name_for_when_the_name_step_comes_back() {
        let (mut wizard, mut ctx, _dir) = start();
        choose(&mut wizard, &mut ctx, 1);
        type_text(&mut wizard, &mut ctx, "nv");
        wizard.handle_key(key(KeyCode::Esc), &mut ctx);
        wizard.handle_key(key(KeyCode::Enter), &mut ctx);

        assert_eq!(wizard.step, Step::Name);
        assert_eq!(wizard.name.value, "nv");
    }

    #[test]
    fn test_esc_on_step_one_leaves_the_wizard() {
        let (mut wizard, mut ctx, _dir) = start();
        let transition = wizard.handle_key(key(KeyCode::Esc), &mut ctx);
        assert!(matches!(transition, Transition::Pop));
    }

    #[test]
    fn test_empty_source_is_chosen_and_enter_goes_on_to_the_ignore_step() {
        let (mut wizard, mut ctx, _dir) = start();
        choose(&mut wizard, &mut ctx, 1);
        type_text(&mut wizard, &mut ctx, "tmux");
        wizard.handle_key(key(KeyCode::Enter), &mut ctx);
        wizard.handle_key(key(KeyCode::Enter), &mut ctx);

        assert_eq!(wizard.source, Some(Source::Empty));
        assert_eq!(wizard.step, Step::Ignore);
    }

    #[test]
    fn test_enter_on_the_picker_without_marks_is_refused_with_the_reason() {
        let (mut wizard, mut ctx, _dir) = start();
        choose(&mut wizard, &mut ctx, 1);
        type_text(&mut wizard, &mut ctx, "tmux");
        wizard.handle_key(key(KeyCode::Enter), &mut ctx);
        wizard.source_list.cursor = 1;
        wizard.handle_key(key(KeyCode::Enter), &mut ctx);
        wizard.handle_key(key(KeyCode::Enter), &mut ctx);

        assert_eq!(wizard.step, Step::Files);
        let toast = ctx.toast.as_ref().unwrap();
        assert_eq!(
            toast.message,
            "mark files to move in, or go back and pick an empty package"
        );
    }

    #[test]
    fn test_live_files_opens_the_picker_at_the_group_target() {
        let (mut wizard, mut ctx, dir) = start();
        choose(&mut wizard, &mut ctx, 1);
        type_text(&mut wizard, &mut ctx, "tmux");
        wizard.handle_key(key(KeyCode::Enter), &mut ctx);
        wizard.handle_key(key(KeyCode::Down), &mut ctx);
        wizard.handle_key(key(KeyCode::Enter), &mut ctx);

        assert_eq!(wizard.step, Step::Files);
        let picker = wizard.picker.as_ref().unwrap();
        assert_eq!(picker.browser.path, dir.path());
    }

    #[test]
    fn test_group_whose_target_folder_is_missing_refuses_live_files_and_keeps_the_step() {
        let (mut wizard, mut ctx, _dir) = start();
        ctx.core.config.groups.get_mut("home").unwrap().target = "no-such-folder".to_string();
        choose(&mut wizard, &mut ctx, 1);
        type_text(&mut wizard, &mut ctx, "tmux");
        wizard.handle_key(key(KeyCode::Enter), &mut ctx);
        wizard.source_list.cursor = 1;
        wizard.handle_key(key(KeyCode::Enter), &mut ctx);

        assert_eq!(wizard.step, Step::Source);
        assert!(wizard.picker.is_none());
    }

    #[test]
    fn test_esc_on_the_picker_returns_to_the_source_step() {
        let (mut wizard, mut ctx, _dir) = start();
        choose(&mut wizard, &mut ctx, 1);
        type_text(&mut wizard, &mut ctx, "tmux");
        wizard.handle_key(key(KeyCode::Enter), &mut ctx);
        wizard.source_list.cursor = 1;
        wizard.handle_key(key(KeyCode::Enter), &mut ctx);
        wizard.handle_key(key(KeyCode::Esc), &mut ctx);

        assert_eq!(wizard.step, Step::Source);
        assert_eq!(wizard.source_list.cursor, 1);
    }

    #[test]
    fn test_changing_the_group_drops_the_picker_of_the_old_one() {
        let (mut wizard, mut ctx, _dir) = start();
        choose(&mut wizard, &mut ctx, 1);
        type_text(&mut wizard, &mut ctx, "tmux");
        wizard.handle_key(key(KeyCode::Enter), &mut ctx);
        wizard.source_list.cursor = 1;
        wizard.handle_key(key(KeyCode::Enter), &mut ctx);
        wizard.handle_key(key(KeyCode::Esc), &mut ctx);
        wizard.handle_key(key(KeyCode::Esc), &mut ctx);
        wizard.handle_key(key(KeyCode::Esc), &mut ctx);
        choose(&mut wizard, &mut ctx, 0);

        assert!(wizard.picker.is_none());
        assert_eq!(wizard.group.as_deref(), Some("config"));
    }

    #[cfg(unix)]
    #[test]
    fn test_space_on_a_symlink_marks_nothing_and_says_why() {
        let (mut wizard, mut ctx, dir) = start();
        std::fs::write(dir.path().join("notes.txt"), "").unwrap();
        std::os::unix::fs::symlink(dir.path().join("notes.txt"), dir.path().join("spell")).unwrap();
        choose(&mut wizard, &mut ctx, 1);
        type_text(&mut wizard, &mut ctx, "tmux");
        wizard.handle_key(key(KeyCode::Enter), &mut ctx);
        wizard.source_list.cursor = 1;
        wizard.handle_key(key(KeyCode::Enter), &mut ctx);
        wizard.update(&mut ctx);
        let picker = wizard.picker.as_mut().unwrap();
        picker.list.cursor = picker
            .entries()
            .iter()
            .position(|entry| entry.name == "spell")
            .unwrap();

        wizard.handle_key(key(KeyCode::Char(' ')), &mut ctx);

        assert!(wizard.picker.as_ref().unwrap().marks.is_empty());
        let toast = ctx.toast.as_ref().unwrap();
        assert_eq!(toast.message, "symlinks cannot be moved in");
    }
}
