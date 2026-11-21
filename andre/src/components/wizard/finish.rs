//! Steps 4–6 of the wizard: the ignore files, the profiles the package joins, and the review
//! that creates it. The review is planned once when the profiles step is left, so what it
//! shows is what [`execute_package`] runs. Paint is in `finish_view.rs`.

use std::collections::BTreeSet;
use std::ffi::OsStr;
use std::path::Path;

use andre_core::create::{execute_package, plan_package, NewPackage, PackagePlan};
use andre_core::ignore::{global_ignore_in, GITIGNORE_FILE, GITIGNORE_SECRETS_TEXT};
use andre_core::profile::{ProfileSet, PROFILES_DIR};
use crossterm::event::{KeyCode, KeyEvent};

use super::{NewPackageWizard, Source, Step};
use crate::components::{AppContext, Transition};
use crate::input::Input;
use crate::ui::{display_path, plural};

/// Rows on the ignore step: `.gitignore`, then `.stow-local-ignore`.
pub const IGNORE_ROWS: usize = 2;

/// What the `.gitignore` row writes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Gitignore {
    /// No file is written.
    Skip,
    /// The secrets template.
    #[default]
    CommonSecrets,
}

impl Gitignore {
    fn flipped(self) -> Self {
        match self {
            Gitignore::Skip => Gitignore::CommonSecrets,
            Gitignore::CommonSecrets => Gitignore::Skip,
        }
    }
}

/// How the review ended. A finished review waits for Enter, which leaves the wizard.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Done {
    /// A new package folder was made.
    Created,
    /// Files were added to a package that already existed.
    Added,
    /// `dry-run` is on: the plan is shown and nothing is written.
    DryRun,
}

/// What a profile holds, as the profiles step shows it: its packages with its includes
/// resolved, and the same count once the new package joins it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct ProfileCount {
    pub packages: usize,
    pub after: usize,
}

/// One line of the review: a verb (`move`, `link`, `skip`, `create`, `edit`) and what it
/// does. The counts line has an empty verb.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReviewLine {
    pub verb: &'static str,
    pub text: String,
}

impl NewPackageWizard {
    pub(super) fn ignore_key(&mut self, key: &KeyEvent, ctx: &AppContext) -> Transition {
        if Input::handle_list_navigation(key, &mut self.ignore_list.cursor, IGNORE_ROWS) {
            return Transition::Handled;
        }
        match key.code {
            KeyCode::Char(' ') => self.toggle_ignore_row(),
            // The arrows change the template, which only the `.gitignore` row has.
            KeyCode::Left | KeyCode::Right if self.ignore_list.cursor == 0 => {
                self.gitignore = self.gitignore.flipped();
            }
            KeyCode::Enter => self.open_profiles(ctx),
            KeyCode::Esc => self.step = Step::Source,
            _ => return Transition::None,
        }
        Transition::Handled
    }

    fn toggle_ignore_row(&mut self) {
        if self.ignore_list.cursor == 0 {
            self.gitignore = self.gitignore.flipped();
        } else {
            self.stow_local_ignore = !self.stow_local_ignore;
        }
    }

    /// Reads the profile names once per visit, so the list does not change under the cursor.
    /// The counts are resolved here, in memory, so the detail paints without reading anything.
    fn open_profiles(&mut self, ctx: &AppContext) {
        self.profile_names = ctx.core.profiles.iter().map(|p| p.name.clone()).collect();
        let known = self.listed_packages();
        let package_ref = self.package_ref();
        self.profile_counts = self
            .profile_names
            .iter()
            .map(|name| {
                let count = profile_count(&ctx.core.profiles, name, &known, &package_ref);
                (name.clone(), count)
            })
            .collect();
        self.profile_list.cursor = 0;
        self.step = Step::Profiles;
    }

    /// The `group/package` ref the package will have.
    pub(super) fn package_ref(&self) -> String {
        format!(
            "{}/{}",
            self.group.clone().unwrap_or_default(),
            self.name.value
        )
    }

    /// Every package the wizard listed, as `group/package`: the form profiles use.
    fn listed_packages(&self) -> BTreeSet<String> {
        self.listing
            .iter()
            .flat_map(|(group, packages)| packages.iter().map(move |p| format!("{group}/{p}")))
            .collect()
    }

    pub(super) fn profiles_key(&mut self, key: &KeyEvent, ctx: &mut AppContext) -> Transition {
        let len = self.profile_names.len();
        if Input::handle_list_navigation(key, &mut self.profile_list.cursor, len) {
            return Transition::Handled;
        }
        match key.code {
            KeyCode::Char(' ') => {
                if let Some(name) = self.profile_names.get(self.profile_list.cursor).cloned() {
                    match self
                        .chosen_profiles
                        .iter()
                        .position(|chosen| chosen == &name)
                    {
                        Some(at) => {
                            self.chosen_profiles.remove(at);
                        }
                        None => self.chosen_profiles.push(name),
                    }
                }
            }
            KeyCode::Enter => self.build_review(ctx),
            KeyCode::Esc => self.step = Step::Ignore,
            _ => return Transition::None,
        }
        Transition::Handled
    }

    /// Plans the package. A refused plan stays on the profiles step, with the reason as a toast.
    fn build_review(&mut self, ctx: &mut AppContext) {
        let request = self.request(ctx);
        let plan = match plan_package(&request, &ctx.core.profiles) {
            Ok(plan) => plan,
            Err(err) => {
                ctx.show_error_toast(err.to_string());
                return;
            }
        };
        self.review = review_lines(&plan, ctx);
        self.plan = Some(plan);
        self.review_scroll = 0;
        self.done = None;
        self.step = Step::Review;
    }

    /// The request the answers so far describe. Live files are the marks of the picker.
    fn request(&self, ctx: &AppContext) -> NewPackage {
        let config = &ctx.core.config;
        let group = self.group.clone().unwrap_or_default();
        let files = match self.source {
            Some(Source::LiveFiles) => self
                .picker
                .as_ref()
                .map(|picker| picker.marks.clone())
                .unwrap_or_default(),
            _ => Vec::new(),
        };
        let profiles = ctx
            .core
            .profiles
            .iter()
            .filter(|profile| self.chosen_profiles.contains(&profile.name))
            .map(|profile| profile.name.clone())
            .collect();
        NewPackage {
            group: group.clone(),
            source_root: config
                .get_group_source(&group, &ctx.config_dir, &ctx.home_dir)
                .unwrap_or_default(),
            target_root: config
                .get_group_target(&group, &ctx.config_dir, &ctx.home_dir)
                .unwrap_or_default(),
            package: self.name.value.clone(),
            allow_existing: self.name_exists(),
            files,
            gitignore: self.gitignore == Gitignore::CommonSecrets,
            stow_local_ignore: self.stow_local_ignore,
            profiles,
            engine: config.effective_engine(),
            no_folding: ctx.core.flags.no_folding,
            ignores: config.effective_ignores(&group),
            global_ignore: Some(global_ignore_in(&ctx.home_dir)),
        }
    }

    /// The lines scroll before and after the run: the result line takes two rows of the panel,
    /// so the last lines may only be reachable after it has shown.
    pub(super) fn review_key(&mut self, key: &KeyEvent, ctx: &mut AppContext) -> Transition {
        let last = self.review_last_scroll();
        match key.code {
            KeyCode::Up => self.review_scroll = self.review_scroll.saturating_sub(1),
            KeyCode::Down => self.review_scroll = (self.review_scroll + 1).min(last),
            KeyCode::Enter | KeyCode::Esc if self.done.is_some() => return Transition::PopAll,
            KeyCode::Enter => self.create(ctx),
            KeyCode::Esc => {
                self.plan = None;
                self.review.clear();
                self.step = Step::Profiles;
            }
            _ => return Transition::None,
        }
        Transition::Handled
    }

    /// The scroll offset at which the last review line reaches the bottom of the panel.
    pub(super) fn review_last_scroll(&self) -> usize {
        self.review
            .len()
            .saturating_sub(usize::from(self.review_height))
    }

    /// Runs the plan. `dry-run` stops before any write. A failure is a toast, and the wizard
    /// goes back to the profiles step: the plan may be stale, so Enter there plans again.
    fn create(&mut self, ctx: &mut AppContext) {
        let Some(plan) = &self.plan else {
            return;
        };
        if ctx.core.flags.dry_run {
            self.done = Some(Done::DryRun);
            return;
        }
        let done = if plan.package_exists {
            Done::Added
        } else {
            Done::Created
        };
        match execute_package(plan, &mut ctx.core.profiles) {
            Ok(()) => {
                ctx.status_cache.borrow_mut().invalidate_all();
                self.done = Some(done);
            }
            Err(err) => {
                ctx.show_error_toast(err.to_string());
                self.plan = None;
                self.review.clear();
                self.step = Step::Profiles;
            }
        }
    }
}

/// Counts a profile's packages with its includes resolved, and with `package_ref` added. A
/// profile that does not resolve (an include cycle) counts its own packages.
fn profile_count(
    profiles: &ProfileSet,
    name: &str,
    known: &BTreeSet<String>,
    package_ref: &str,
) -> ProfileCount {
    let packages: Vec<String> = match profiles.resolve(name, known) {
        Ok(resolved) => resolved.entries.into_iter().map(|e| e.package).collect(),
        Err(_) => profiles
            .get(name)
            .map(|profile| profile.packages.clone())
            .unwrap_or_default(),
    };
    let joins = packages.iter().any(|package| package == package_ref);
    ProfileCount {
        packages: packages.len(),
        after: packages.len() + usize::from(!joins),
    }
}

/// The review lines of a plan: moves, links, skipped links, ignore files, profile edits, then
/// a counts line. Paths are shown as the user knows them: live paths with `~`, package paths
/// as `group/package/…`.
pub fn review_lines(plan: &PackagePlan, ctx: &AppContext) -> Vec<ReviewLine> {
    let home = ctx.home_dir.as_path();
    let mut lines = Vec::new();
    for movement in &plan.moves {
        let text = format!(
            "{} \u{2192} {}",
            display_path(&movement.from, home),
            in_package(plan, &movement.to)
        );
        lines.push(line("move", text));
    }
    for link in &plan.links {
        let text = format!("{} \u{21e2} package", display_path(link, home));
        lines.push(line("link", text));
    }
    for path in &plan.unlinked {
        let text = format!(
            "{} (ignored: moved in, not linked back)",
            display_path(path, home)
        );
        lines.push(line("skip", text));
    }
    let files = plan.ignore_files();
    for path in &files {
        let text = format!("{}  ({})", in_package(plan, path), ignore_note(path));
        lines.push(line("create", text));
    }
    let edits: Vec<_> = plan
        .profile_edits
        .iter()
        .filter(|edit| !edit.already_member)
        .collect();
    for edit in &edits {
        let text = format!(
            "{PROFILES_DIR}/{}.yml  + {}",
            edit.profile, edit.package_ref
        );
        lines.push(line("edit", text));
    }
    let (moves, links) = (plan.moves.len(), plan.links.len());
    let (file_count, profile_count) = (files.len(), edits.len());
    lines.push(line("", String::new()));
    lines.push(line(
        "",
        format!(
            "{moves} {} \u{b7} {links} {} \u{b7} {file_count} {} \u{b7} {profile_count} {}",
            plural(moves, "move", "moves"),
            plural(links, "link", "links"),
            plural(file_count, "file", "files"),
            plural(profile_count, "profile", "profiles"),
        ),
    ));
    lines
}

fn line(verb: &'static str, text: String) -> ReviewLine {
    ReviewLine { verb, text }
}

/// A path inside the new package, as `group/package/rest`.
fn in_package(plan: &PackagePlan, path: &Path) -> String {
    let rest = path.strip_prefix(&plan.package_dir).unwrap_or(path);
    format!("{}/{}/{}", plan.group, plan.package, rest.display())
}

/// What an ignore file written by the wizard holds, for its review line.
fn ignore_note(path: &Path) -> String {
    if path.file_name() == Some(OsStr::new(GITIGNORE_FILE)) {
        format!(
            "common secrets, {} lines",
            GITIGNORE_SECRETS_TEXT.lines().count()
        )
    } else {
        "built-in list, replaces stow's defaults".to_string()
    }
}

#[cfg(test)]
mod tests {
    use andre_core::config::Config;
    use andre_core::ignore::GITIGNORE_SECRETS_TEXT;
    use andre_core::profile::ProfileSet;
    use crossterm::event::{KeyCode, KeyModifiers};
    use tempfile::TempDir;

    use super::*;
    use crate::components::{AppContext, Component, Transition};

    fn key(code: KeyCode) -> KeyEvent {
        KeyEvent::new(code, KeyModifiers::NONE)
    }

    /// One group, `home`, from `dotfiles/home` into the temp dir (the home dir), holding
    /// `git`. The profile `personal` lists `home/git`.
    fn fixture() -> (AppContext, TempDir) {
        let dir = TempDir::new().unwrap();
        let config_path = dir.path().join("andre.yml");
        std::fs::write(
            &config_path,
            "global:\n  stow:\n    verbosity: 0\ngroups:\n  home:\n    source: dotfiles/home\n    target: \"~\"\n",
        )
        .unwrap();
        std::fs::create_dir_all(dir.path().join("dotfiles/home/git")).unwrap();
        std::fs::create_dir_all(dir.path().join("profiles")).unwrap();
        std::fs::write(
            dir.path().join("profiles/personal.yml"),
            "packages:\n  - home/git\n",
        )
        .unwrap();
        let config = Config::load(&config_path).unwrap();
        let mut ctx = AppContext::for_tests(config, dir.path());
        ctx.core.profiles = ProfileSet::load(&dir.path().join("profiles")).unwrap();
        (ctx, dir)
    }

    /// Drives the wizard from step 1 to the profiles step for a new empty package `name`.
    fn at_profiles(wizard: &mut NewPackageWizard, ctx: &mut AppContext, name: &str) {
        wizard.update(ctx);
        wizard.handle_key(key(KeyCode::Enter), ctx);
        for c in name.chars() {
            wizard.handle_key(key(KeyCode::Char(c)), ctx);
        }
        for _ in 0..3 {
            wizard.handle_key(key(KeyCode::Enter), ctx);
        }
        assert_eq!(wizard.step, Step::Profiles);
    }

    /// Chooses the one profile, then Enter plans the package.
    fn to_review(wizard: &mut NewPackageWizard, ctx: &mut AppContext) {
        wizard.handle_key(key(KeyCode::Char(' ')), ctx);
        wizard.handle_key(key(KeyCode::Enter), ctx);
        assert_eq!(wizard.step, Step::Review);
    }

    /// The review lines as `verb text`, the counts line without a verb.
    fn review_texts(wizard: &NewPackageWizard) -> Vec<String> {
        wizard
            .review
            .iter()
            .map(|line| {
                if line.verb.is_empty() {
                    line.text.clone()
                } else {
                    format!("{} {}", line.verb, line.text)
                }
            })
            .collect()
    }

    #[test]
    fn test_ignore_step_starts_with_the_secrets_template_and_no_stow_list() {
        let (mut ctx, _dir) = fixture();
        let mut wizard = NewPackageWizard::default();
        at_profiles(&mut wizard, &mut ctx, "tmux");
        wizard.handle_key(key(KeyCode::Esc), &mut ctx);

        assert_eq!(wizard.step, Step::Ignore);
        assert_eq!(wizard.gitignore, Gitignore::CommonSecrets);
        assert!(!wizard.stow_local_ignore);
    }

    #[test]
    fn test_space_on_the_gitignore_row_flips_the_template_and_arrows_do_too() {
        let (mut ctx, _dir) = fixture();
        let mut wizard = NewPackageWizard::default();
        at_profiles(&mut wizard, &mut ctx, "tmux");
        wizard.handle_key(key(KeyCode::Esc), &mut ctx);

        wizard.handle_key(key(KeyCode::Char(' ')), &mut ctx);
        assert_eq!(wizard.gitignore, Gitignore::Skip);
        wizard.handle_key(key(KeyCode::Right), &mut ctx);
        assert_eq!(wizard.gitignore, Gitignore::CommonSecrets);
        wizard.handle_key(key(KeyCode::Left), &mut ctx);
        assert_eq!(wizard.gitignore, Gitignore::Skip);
    }

    #[test]
    fn test_arrows_on_the_stow_row_leave_the_template_alone() {
        let (mut ctx, _dir) = fixture();
        let mut wizard = NewPackageWizard::default();
        at_profiles(&mut wizard, &mut ctx, "tmux");
        wizard.handle_key(key(KeyCode::Esc), &mut ctx);
        wizard.handle_key(key(KeyCode::Down), &mut ctx);
        wizard.handle_key(key(KeyCode::Right), &mut ctx);

        assert_eq!(wizard.gitignore, Gitignore::CommonSecrets);
        assert!(!wizard.stow_local_ignore);
    }

    #[test]
    fn test_space_on_the_stow_row_toggles_the_built_in_list() {
        let (mut ctx, _dir) = fixture();
        let mut wizard = NewPackageWizard::default();
        at_profiles(&mut wizard, &mut ctx, "tmux");
        wizard.handle_key(key(KeyCode::Esc), &mut ctx);
        wizard.handle_key(key(KeyCode::Down), &mut ctx);
        wizard.handle_key(key(KeyCode::Char(' ')), &mut ctx);

        assert!(wizard.stow_local_ignore);
    }

    #[test]
    fn test_enter_on_the_ignore_step_reads_the_profiles_and_opens_step_five() {
        let (mut ctx, _dir) = fixture();
        let mut wizard = NewPackageWizard::default();
        at_profiles(&mut wizard, &mut ctx, "tmux");
        wizard.handle_key(key(KeyCode::Esc), &mut ctx);
        wizard.handle_key(key(KeyCode::Enter), &mut ctx);

        assert_eq!(wizard.step, Step::Profiles);
        assert_eq!(wizard.profile_names, vec!["personal".to_string()]);
    }

    #[test]
    fn test_space_chooses_and_unchooses_a_profile() {
        let (mut ctx, _dir) = fixture();
        let mut wizard = NewPackageWizard::default();
        at_profiles(&mut wizard, &mut ctx, "tmux");

        wizard.handle_key(key(KeyCode::Char(' ')), &mut ctx);
        assert_eq!(wizard.chosen_profiles, vec!["personal".to_string()]);
        wizard.handle_key(key(KeyCode::Char(' ')), &mut ctx);
        assert!(wizard.chosen_profiles.is_empty());
    }

    #[test]
    fn test_review_of_an_empty_package_lists_the_ignore_file_and_the_profile_edit() {
        let (mut ctx, _dir) = fixture();
        let mut wizard = NewPackageWizard::default();
        at_profiles(&mut wizard, &mut ctx, "tmux");
        to_review(&mut wizard, &mut ctx);

        let lines = review_texts(&wizard);
        let template_lines = GITIGNORE_SECRETS_TEXT.lines().count();
        assert!(
            lines.contains(&format!(
                "create home/tmux/.gitignore  (common secrets, {template_lines} lines)"
            )),
            "{lines:?}"
        );
        assert!(
            lines.contains(&"edit profiles/personal.yml  + home/tmux".to_string()),
            "{lines:?}"
        );
        assert_eq!(
            lines.last().unwrap(),
            "0 moves · 0 links · 1 file · 1 profile"
        );
    }

    #[test]
    fn test_none_chosen_and_no_template_plans_a_bare_package_with_no_edits() {
        let (mut ctx, _dir) = fixture();
        let mut wizard = NewPackageWizard::default();
        at_profiles(&mut wizard, &mut ctx, "tmux");
        wizard.handle_key(key(KeyCode::Esc), &mut ctx);
        wizard.handle_key(key(KeyCode::Char(' ')), &mut ctx);
        wizard.handle_key(key(KeyCode::Enter), &mut ctx);
        wizard.handle_key(key(KeyCode::Enter), &mut ctx);

        assert_eq!(wizard.step, Step::Review);
        assert_eq!(
            review_texts(&wizard).last().unwrap(),
            "0 moves · 0 links · 0 files · 0 profiles"
        );
    }

    #[test]
    fn test_a_refused_plan_stays_on_the_profiles_step_with_the_reason() {
        let (mut ctx, dir) = fixture();
        std::fs::create_dir_all(dir.path().join("dotfiles/home/tmux")).unwrap();
        std::fs::write(dir.path().join("dotfiles/home/tmux/.gitignore"), "mine\n").unwrap();
        let mut wizard = NewPackageWizard::default();
        at_profiles(&mut wizard, &mut ctx, "tmux");
        wizard.handle_key(key(KeyCode::Enter), &mut ctx);

        assert_eq!(wizard.step, Step::Profiles);
        let toast = ctx.toast.as_ref().unwrap();
        assert!(
            toast.message.contains("already exists; not overwritten"),
            "{}",
            toast.message
        );
        assert_eq!(
            std::fs::read_to_string(dir.path().join("dotfiles/home/tmux/.gitignore")).unwrap(),
            "mine\n",
            "the existing file is not overwritten"
        );
    }

    #[test]
    fn test_live_files_review_lists_the_moves_and_the_links_back() {
        let (mut ctx, dir) = fixture();
        std::fs::write(dir.path().join(".bashrc"), "alias l='ls'\n").unwrap();
        let mut wizard = NewPackageWizard::default();
        wizard.update(&mut ctx);
        wizard.handle_key(key(KeyCode::Enter), &mut ctx);
        for c in "shell".chars() {
            wizard.handle_key(key(KeyCode::Char(c)), &mut ctx);
        }
        wizard.handle_key(key(KeyCode::Enter), &mut ctx);
        wizard.handle_key(key(KeyCode::Down), &mut ctx);
        wizard.handle_key(key(KeyCode::Enter), &mut ctx);
        // The picker's listing is read in update, once per frame, before keys.
        wizard.update(&mut ctx);
        wizard.handle_key(key(KeyCode::Char(' ')), &mut ctx);
        wizard.handle_key(key(KeyCode::Enter), &mut ctx);
        wizard.handle_key(key(KeyCode::Enter), &mut ctx);
        wizard.handle_key(key(KeyCode::Enter), &mut ctx);

        assert_eq!(wizard.step, Step::Review);
        let lines = review_texts(&wizard);
        assert!(
            lines.contains(&"move ~/.bashrc \u{2192} home/shell/.bashrc".to_string()),
            "{lines:?}"
        );
        assert!(
            lines.contains(&"link ~/.bashrc \u{21e2} package".to_string()),
            "{lines:?}"
        );
        assert_eq!(
            lines.last().unwrap(),
            "1 move \u{b7} 1 link \u{b7} 1 file \u{b7} 0 profiles"
        );
    }

    #[test]
    fn test_create_writes_the_package_then_enter_leaves_the_wizard() {
        let (mut ctx, dir) = fixture();
        let mut wizard = NewPackageWizard::default();
        at_profiles(&mut wizard, &mut ctx, "tmux");
        to_review(&mut wizard, &mut ctx);
        let transition = wizard.handle_key(key(KeyCode::Enter), &mut ctx);

        assert_eq!(wizard.done, Some(Done::Created));
        assert!(dir.path().join("dotfiles/home/tmux").is_dir());
        assert!(
            std::fs::read_to_string(dir.path().join("dotfiles/home/tmux/.gitignore"))
                .unwrap()
                .contains("*.env")
        );
        let profiles = ProfileSet::load(&dir.path().join("profiles")).unwrap();
        assert!(profiles
            .get("personal")
            .unwrap()
            .packages
            .contains(&"home/tmux".to_string()));
        assert!(matches!(transition, Transition::Handled));

        let leave = wizard.handle_key(key(KeyCode::Enter), &mut ctx);
        assert!(matches!(leave, Transition::PopAll));
    }

    #[test]
    fn test_dry_run_writes_nothing_and_says_so() {
        let (mut ctx, dir) = fixture();
        ctx.core.flags.dry_run = true;
        let mut wizard = NewPackageWizard::default();
        at_profiles(&mut wizard, &mut ctx, "tmux");
        to_review(&mut wizard, &mut ctx);
        wizard.handle_key(key(KeyCode::Enter), &mut ctx);

        assert_eq!(wizard.done, Some(Done::DryRun));
        assert!(!dir.path().join("dotfiles/home/tmux").exists());
    }

    #[test]
    fn test_esc_on_the_review_goes_back_to_profiles_and_drops_the_plan() {
        let (mut ctx, _dir) = fixture();
        let mut wizard = NewPackageWizard::default();
        at_profiles(&mut wizard, &mut ctx, "tmux");
        to_review(&mut wizard, &mut ctx);
        wizard.handle_key(key(KeyCode::Esc), &mut ctx);

        assert_eq!(wizard.step, Step::Profiles);
        assert!(wizard.plan.is_none());
        assert!(wizard.review.is_empty());
    }

    #[test]
    fn test_review_scroll_stops_at_the_last_line() {
        let (mut ctx, _dir) = fixture();
        let mut wizard = NewPackageWizard::default();
        at_profiles(&mut wizard, &mut ctx, "tmux");
        to_review(&mut wizard, &mut ctx);
        wizard.review_height = 1;
        for _ in 0..50 {
            wizard.handle_key(key(KeyCode::Down), &mut ctx);
        }

        assert_eq!(wizard.review_scroll, wizard.review.len() - 1);
    }

    #[test]
    fn test_the_lines_still_scroll_after_the_result_line_shows() {
        let (mut ctx, _dir) = fixture();
        let mut wizard = NewPackageWizard::default();
        at_profiles(&mut wizard, &mut ctx, "tmux");
        to_review(&mut wizard, &mut ctx);
        wizard.handle_key(key(KeyCode::Enter), &mut ctx);
        assert_eq!(wizard.done, Some(Done::Created));

        // The result line takes two rows, so the panel shows fewer lines than the plan has.
        wizard.review_height = 3;
        for _ in 0..50 {
            wizard.handle_key(key(KeyCode::Down), &mut ctx);
        }
        assert_eq!(wizard.review_scroll, wizard.review.len() - 3);
        wizard.handle_key(key(KeyCode::Up), &mut ctx);
        assert_eq!(wizard.review_scroll, wizard.review.len() - 4);
        assert_eq!(
            wizard.done,
            Some(Done::Created),
            "scrolling does not leave the wizard"
        );
    }

    #[test]
    fn test_profile_counts_include_what_the_profile_includes_and_the_new_package() {
        let (mut ctx, dir) = fixture();
        std::fs::write(
            dir.path().join("profiles/laptop.yml"),
            "includes:\n  - personal\n",
        )
        .unwrap();
        ctx.core.profiles = ProfileSet::load(&dir.path().join("profiles")).unwrap();
        let mut wizard = NewPackageWizard::default();
        at_profiles(&mut wizard, &mut ctx, "tmux");

        // `personal` lists home/git. `laptop` reaches it only through the include.
        assert_eq!(
            wizard.profile_counts["personal"],
            ProfileCount {
                packages: 1,
                after: 2
            }
        );
        assert_eq!(
            wizard.profile_counts["laptop"],
            ProfileCount {
                packages: 1,
                after: 2
            }
        );
    }

    #[test]
    fn test_a_package_the_profile_already_reaches_is_not_counted_twice() {
        let (mut ctx, dir) = fixture();
        std::fs::write(
            dir.path().join("profiles/laptop.yml"),
            "includes:\n  - personal\n",
        )
        .unwrap();
        ctx.core.profiles = ProfileSet::load(&dir.path().join("profiles")).unwrap();
        let mut wizard = NewPackageWizard::default();
        // `git` exists in the group, so the package adds files to it. `personal` lists it.
        at_profiles(&mut wizard, &mut ctx, "git");

        assert_eq!(
            wizard.profile_counts["personal"],
            ProfileCount {
                packages: 1,
                after: 1
            }
        );
        assert_eq!(
            wizard.profile_counts["laptop"],
            ProfileCount {
                packages: 1,
                after: 1
            }
        );
    }
}
