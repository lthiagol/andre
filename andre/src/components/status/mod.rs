use std::collections::BTreeSet;

use andre_core::ignore::global_ignore_in;
use andre_core::profile::known_packages;
use andre_core::{discover_packages, path::resolve_path, StowStatus};
use crossterm::event::KeyEvent;
use ratatui::{layout::Rect, Frame};

use crate::components::{AppContext, Component, Transition};
use crate::input::Input;
use crate::theme::ThemeColors;
use crate::ui::icons::Icons;
use crate::ui::{status_layout, StatusRow};

#[derive(Default)]
pub struct StatusComponent {
    invalidated: bool,
    scroll_index: usize,
    viewport_height: usize,
    rows: Vec<StatusRow>,
    /// Show only the packages this profile resolves to. `None` lists every package.
    profile: Option<String>,
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

/// The `group/package` refs that profile `name` runs: its resolved refs that exist on disk.
/// A profile that does not resolve runs none.
fn profile_refs(ctx: &AppContext, name: &str) -> BTreeSet<String> {
    let known = known_packages(&ctx.core.config, &ctx.config_dir, &ctx.home_dir);
    match ctx.core.profiles.resolve(name, &known) {
        Ok(resolved) => resolved
            .entries
            .iter()
            .filter(|entry| !entry.missing)
            .map(|entry| entry.package.clone())
            .collect(),
        Err(_) => BTreeSet::new(),
    }
}

/// Lists every configured group's packages with their cached stow status, or only those
/// of `profile` when it is given. Runs once per visit, after the status cache has been
/// refreshed.
fn build_rows(ctx: &AppContext, profile: Option<&str>) -> Vec<StatusRow> {
    let only = profile.map(|name| profile_refs(ctx, name));
    let mut rows = Vec::new();
    let cache = ctx.status_cache.borrow();
    for (group_name, group) in &ctx.core.config.groups {
        let source = resolve_path(&group.source, &ctx.config_dir, &ctx.home_dir);
        let target = resolve_path(&group.target, &ctx.config_dir, &ctx.home_dir);
        let packages: Vec<String> = discover_packages(&source)
            .into_iter()
            .filter(|name| {
                only.as_ref()
                    .is_none_or(|refs| refs.contains(&format!("{group_name}/{name}")))
            })
            .collect();
        if packages.is_empty() {
            continue;
        }

        rows.push(StatusRow::Group(group_name.clone()));
        let linked = source.exists() && target.exists();
        for name in packages {
            let status = if linked {
                cache.get(group_name, &name).unwrap_or(StowStatus::Missing)
            } else {
                StowStatus::Missing
            };
            rows.push(StatusRow::Package { name, status });
        }
        rows.push(StatusRow::Spacer);
    }
    if rows.is_empty() {
        rows.push(match profile {
            Some(name) => StatusRow::NoProfilePackages(name.to_string()),
            None => StatusRow::NoPackages,
        });
    }
    rows
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

    fn update(&mut self, ctx: &mut AppContext) -> Transition {
        if !self.invalidated {
            ctx.status_cache.borrow_mut().invalidate_all();
            let global = global_ignore_in(&ctx.home_dir);
            // Prefetch all groups' status
            for (group_name, group) in &ctx.core.config.groups {
                let source = resolve_path(&group.source, &ctx.config_dir, &ctx.home_dir);
                let target = resolve_path(&group.target, &ctx.config_dir, &ctx.home_dir);
                if source.exists() && target.exists() {
                    let packages = discover_packages(&source);
                    let mut cache = ctx.status_cache.borrow_mut();
                    cache.prefetch_group(&source, &target, group_name, &packages, Some(&global));
                }
            }
            self.rows = build_rows(ctx, self.profile.as_deref());
            self.invalidated = true;
        }
        Transition::None
    }

    fn handle_key(&mut self, key: KeyEvent, _ctx: &mut AppContext) -> Transition {
        if key.code == crossterm::event::KeyCode::Enter
            || key.code == crossterm::event::KeyCode::Esc
        {
            return Transition::Pop;
        }
        Input::handle_list_scroll(
            &key,
            &mut self.scroll_index,
            self.rows.len(),
            self.viewport_height,
        );
        Transition::None
    }

    fn render(&mut self, frame: &mut Frame, area: Rect, ctx: &AppContext) {
        let colors = ThemeColors::from_theme(ctx.core.theme);
        crate::ui::render_status(
            frame,
            area,
            &colors,
            Icons::for_set(ctx.core.icons),
            &self.rows,
            self.scroll_index,
            self.viewport_height,
        );
    }

    fn prepare(&mut self, area: Rect) {
        self.viewport_height = status_layout(area).0.height.saturating_sub(2).max(1) as usize;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use andre_core::config::Config;
    use crossterm::event::{KeyCode, KeyModifiers};

    /// One group `dots` with packages `alpha` and `beta`; both directories exist, so statuses are cached.
    fn fixture() -> (tempfile::TempDir, AppContext) {
        let tmp = tempfile::TempDir::new().unwrap();
        std::fs::create_dir_all(tmp.path().join("src/alpha")).unwrap();
        std::fs::create_dir_all(tmp.path().join("src/beta")).unwrap();
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

    #[test]
    fn test_build_rows_lists_each_package_under_its_group() {
        let (_tmp, ctx) = fixture();
        let rows = build_rows(&ctx, None);

        assert_eq!(rows[0], StatusRow::Group("dots".into()));
        let packages: Vec<&str> = rows
            .iter()
            .filter_map(|row| match row {
                StatusRow::Package { name, .. } => Some(name.as_str()),
                _ => None,
            })
            .collect();
        assert_eq!(packages, vec!["alpha", "beta"]);
        assert_eq!(rows.last(), Some(&StatusRow::Spacer));
    }

    #[test]
    fn test_build_rows_uses_prefetched_status() {
        let (_tmp, ctx) = fixture();
        ctx.status_cache
            .borrow_mut()
            .insert(("dots".into(), "alpha".into()), StowStatus::Stowed);
        let rows = build_rows(&ctx, None);

        assert!(rows.contains(&StatusRow::Package {
            name: "alpha".into(),
            status: StowStatus::Stowed,
        }));
    }

    #[test]
    fn test_build_rows_placeholder_when_no_packages() {
        let tmp = tempfile::TempDir::new().unwrap();
        let ctx = AppContext::for_tests(Config::default_empty(), tmp.path());
        assert_eq!(build_rows(&ctx, None), vec![StatusRow::NoPackages]);
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

    #[test]
    fn test_build_rows_for_a_profile_lists_its_resolved_packages_only() {
        let (_tmp, ctx) = profile_fixture();
        let rows = build_rows(&ctx, Some("one"));

        let packages: Vec<&str> = rows
            .iter()
            .filter_map(|row| match row {
                StatusRow::Package { name, .. } => Some(name.as_str()),
                _ => None,
            })
            .collect();
        assert_eq!(
            packages,
            vec!["alpha", "beta"],
            "own first, then the include"
        );
        assert_eq!(rows[0], StatusRow::Group("dots".into()));
    }

    #[test]
    fn test_build_rows_for_a_profile_that_does_not_resolve_is_empty() {
        let (_tmp, ctx) = profile_fixture();
        assert_eq!(
            build_rows(&ctx, Some("missing")),
            vec![StatusRow::NoProfilePackages("missing".into())]
        );
    }

    #[test]
    fn test_build_rows_for_a_valid_profile_with_no_packages_on_disk_names_the_profile() {
        let (_tmp, mut ctx) = profile_fixture();
        ctx.core
            .profiles
            .create(andre_core::profile::Profile {
                name: "bare".into(),
                packages: vec!["dots/ghost".into()],
                ..Default::default()
            })
            .unwrap();
        assert_eq!(
            build_rows(&ctx, Some("bare")),
            vec![StatusRow::NoProfilePackages("bare".into())]
        );
    }

    #[test]
    fn test_for_profile_filters_the_rows_it_builds_on_visit() {
        let (_tmp, mut ctx) = profile_fixture();
        let mut component = StatusComponent::for_profile("two");
        component.update(&mut ctx);

        let packages: Vec<&str> = component
            .rows
            .iter()
            .filter_map(|row| match row {
                StatusRow::Package { name, .. } => Some(name.as_str()),
                _ => None,
            })
            .collect();
        assert_eq!(packages, vec!["beta"]);
    }

    #[test]
    fn test_prepare_sizes_the_viewport_to_the_rows_the_list_shows() {
        // 80x24 terminal: the content area is 16 rows. The legend takes 9 and the
        // borders 2, so the list shows 5 rows, and that is the page size.
        let mut component = StatusComponent::default();
        component.prepare(Rect::new(0, 0, 80, 16));
        assert_eq!(component.viewport_height, 5);

        // Too short for a legend: the list takes the whole area.
        component.prepare(Rect::new(0, 0, 80, 10));
        assert_eq!(component.viewport_height, 8);
    }

    #[test]
    fn test_scroll_stops_at_the_last_row_when_rows_fit() {
        let (_tmp, mut ctx) = fixture();
        let mut component = StatusComponent {
            rows: build_rows(&ctx, None),
            ..Default::default()
        };
        // 80x24: the list shows 6 rows, more than the 4 built here, so every row fits.
        component.prepare(Rect::new(0, 0, 80, 24));
        let last = component.rows.len() - 1;

        for _ in 0..10 {
            component.handle_key(KeyEvent::new(KeyCode::Down, KeyModifiers::NONE), &mut ctx);
        }
        assert_eq!(component.scroll_index, last);

        component.handle_key(KeyEvent::new(KeyCode::Up, KeyModifiers::NONE), &mut ctx);
        assert_eq!(component.scroll_index, last - 1);
    }

    #[test]
    fn test_update_builds_rows_from_fresh_status() {
        let (tmp, mut ctx) = fixture();
        // `alpha` really is stowed: its file is linked from the target.
        std::fs::write(tmp.path().join("src/alpha/f"), "").unwrap();
        std::os::unix::fs::symlink(tmp.path().join("src/alpha/f"), tmp.path().join("tgt/f"))
            .unwrap();
        // A stale status from an earlier visit; the visit must replace it before rows are built.
        ctx.status_cache
            .borrow_mut()
            .insert(("dots".into(), "alpha".into()), StowStatus::Conflict);

        let mut component = StatusComponent::default();
        component.update(&mut ctx);
        assert!(component.rows.contains(&StatusRow::Package {
            name: "alpha".into(),
            status: StowStatus::Stowed,
        }));

        // Rows are built once per visit: unstowing on disk does not change them until the next visit.
        std::fs::remove_file(tmp.path().join("tgt/f")).unwrap();
        component.update(&mut ctx);
        assert!(component.rows.contains(&StatusRow::Package {
            name: "alpha".into(),
            status: StowStatus::Stowed,
        }));
    }
}
