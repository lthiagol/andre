use std::collections::HashMap;

use crossterm::event::{KeyCode, KeyEvent};
use ratatui::{layout::Rect, Frame};

use crate::components::confirm::ConfirmComponent;
use crate::components::{AppContext, Component, Transition};
use crate::theme::ThemeColors;
use crate::ui::icons::Icons;
use crate::ui::packages::{align_columns, FlatItem};
use crate::ui::{render_package_select, status_layout};
use andre_core::StowStatus;

#[derive(Default)]
pub struct PackageSelectComponent {
    pub selected_groups: Vec<String>,
    pub selected_packages: HashMap<String, Vec<String>>,
    pub cursor: usize,
    pub entered_at: Option<std::time::Instant>,
    pub scanner_frame: usize,
    viewport_height: usize,
    /// Rows for paint. Rebuilt by `sync_items`, never by `render`.
    items: Vec<FlatItem>,
    /// Status cache generation the rows were built against.
    items_generation: Option<u64>,
}

impl PackageSelectComponent {
    pub fn new(selected_groups: Vec<String>) -> Self {
        Self {
            selected_groups,
            selected_packages: HashMap::new(),
            cursor: 0,
            entered_at: Some(std::time::Instant::now()),
            scanner_frame: 0,
            viewport_height: 10,
            items: Vec::new(),
            items_generation: None,
        }
    }

    /// Rebuilds the cached rows when they were never built or the status cache has changed.
    /// Directory scans and previews happen here, not on paint or on every keypress.
    fn sync_items(&mut self, ctx: &AppContext) {
        let generation = ctx.status_cache.borrow().generation();
        if self.items_generation == Some(generation) {
            return;
        }
        self.items = self.build_items(ctx);
        self.items_generation = Some(ctx.status_cache.borrow().generation());
    }

    /// Per selected group: resolve its paths and scan its packages once, prefetch
    /// their statuses, then build the rows from that same scan.
    fn build_items(&self, ctx: &AppContext) -> Vec<FlatItem> {
        let mut items = Vec::new();
        for group_name in &self.selected_groups {
            if !ctx.core.config.groups.contains_key(group_name) {
                continue;
            }
            let source =
                ctx.core
                    .config
                    .get_group_source(group_name, &ctx.config_dir, &ctx.home_dir);
            let target =
                ctx.core
                    .config
                    .get_group_target(group_name, &ctx.config_dir, &ctx.home_dir);
            let packages = match &source {
                Some(s) => andre_core::discover_packages(s),
                None => Vec::new(),
            };
            if let (Some(s), Some(t)) = (&source, &target) {
                ctx.status_cache
                    .borrow_mut()
                    .prefetch_group(s, t, group_name, &packages);
            }
            let group_selected = self.selected_packages.get(group_name);
            for pkg_name in packages {
                let pkg_selected = group_selected
                    .map(|v| v.contains(&pkg_name))
                    .unwrap_or(false);
                let status = match (&source, &target) {
                    (Some(_), Some(_)) => ctx
                        .status_cache
                        .borrow()
                        .get(group_name, &pkg_name)
                        .unwrap_or(StowStatus::Missing),
                    _ => StowStatus::Missing,
                };
                let preview = source
                    .as_ref()
                    .map(|s| andre_core::get_package_preview(s, &pkg_name, 3))
                    .unwrap_or_default();
                items.push(FlatItem {
                    group_name: group_name.clone(),
                    package_name: pkg_name,
                    selected: pkg_selected,
                    status,
                    preview,
                    package_display: String::new(),
                    group_display: String::new(),
                    package_pad: 0,
                    group_pad: 0,
                });
            }
        }
        align_columns(&mut items);
        items
    }
}

impl Component for PackageSelectComponent {
    fn id(&self) -> &'static str {
        "PackageSelect"
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }

    fn handle_key(&mut self, key: KeyEvent, ctx: &mut AppContext) -> Transition {
        self.sync_items(ctx);
        let len = self.items.len();

        if len > 0
            && crate::input::Input::handle_list_scroll(
                &key,
                &mut self.cursor,
                len,
                self.viewport_height,
            )
        {
            return Transition::None;
        }

        if key.code == KeyCode::Char(' ') {
            if let Some(item) = self.items.get_mut(self.cursor) {
                let entry = self
                    .selected_packages
                    .entry(item.group_name.clone())
                    .or_default();
                if entry.contains(&item.package_name) {
                    entry.retain(|p| p != &item.package_name);
                } else {
                    entry.push(item.package_name.clone());
                }
                if entry.is_empty() {
                    self.selected_packages.remove(&item.group_name);
                }
                item.selected = self
                    .selected_packages
                    .get(&item.group_name)
                    .is_some_and(|pkgs| pkgs.contains(&item.package_name));
            }
        } else if key.code == KeyCode::Enter {
            if !self.selected_packages.is_empty() {
                return Transition::Replace(Box::new(ConfirmComponent::new(
                    self.selected_packages.clone(),
                    ctx,
                )));
            }
        } else if key.code == KeyCode::Esc {
            return Transition::Pop;
        }

        Transition::None
    }

    fn update(&mut self, ctx: &mut AppContext) -> Transition {
        self.scanner_frame = self.scanner_frame.wrapping_add(1);
        self.sync_items(ctx);
        Transition::None
    }

    fn render(&mut self, frame: &mut Frame, area: Rect, ctx: &AppContext) {
        let colors = ThemeColors::from_theme(ctx.core.theme);
        render_package_select(
            frame,
            area,
            &colors,
            Icons::for_set(ctx.core.icons),
            " select packages ",
            &self.items,
            self.cursor,
            self.entered_at,
            self.scanner_frame,
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
    use crossterm::event::KeyModifiers;

    /// One group `dots` whose source holds the packages `alpha` and `beta`.
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

    fn row_index(component: &PackageSelectComponent, package: &str) -> usize {
        component
            .items
            .iter()
            .position(|item| item.package_name == package)
            .expect("package row exists")
    }

    fn press(component: &mut PackageSelectComponent, ctx: &mut AppContext, code: KeyCode) {
        component.handle_key(KeyEvent::new(code, KeyModifiers::NONE), ctx);
    }

    #[test]
    fn test_space_marks_cached_row_without_rebuilding() {
        let (_tmp, mut ctx) = fixture();
        let mut component = PackageSelectComponent::new(vec!["dots".into()]);
        component.update(&mut ctx);
        let built_at = component.items_generation;

        component.cursor = row_index(&component, "alpha");
        press(&mut component, &mut ctx, KeyCode::Char(' '));
        assert!(component.items[row_index(&component, "alpha")].selected);
        assert!(!component.items[row_index(&component, "beta")].selected);

        component.update(&mut ctx);
        assert!(component.items[row_index(&component, "alpha")].selected);
        assert_eq!(component.items_generation, built_at);

        press(&mut component, &mut ctx, KeyCode::Char(' '));
        assert!(!component.items[row_index(&component, "alpha")].selected);
        assert!(component.selected_packages.is_empty());
    }

    #[test]
    fn test_status_change_refreshes_cached_rows() {
        let (_tmp, mut ctx) = fixture();
        let mut component = PackageSelectComponent::new(vec!["dots".into()]);
        component.update(&mut ctx);
        let built_at = component.items_generation;

        ctx.status_cache.borrow_mut().insert(
            ("dots".into(), "alpha".into()),
            andre_core::StowStatus::Stowed,
        );
        component.update(&mut ctx);

        let alpha = &component.items[row_index(&component, "alpha")];
        assert_eq!(alpha.status, andre_core::StowStatus::Stowed);
        assert_ne!(component.items_generation, built_at);
    }

    #[test]
    fn test_handle_key_builds_rows_when_update_has_not_run() {
        let (_tmp, mut ctx) = fixture();
        let mut component = PackageSelectComponent::new(vec!["dots".into()]);
        press(&mut component, &mut ctx, KeyCode::Down);
        assert_eq!(component.items.len(), 2);
    }

    #[test]
    fn test_prepare_sizes_the_viewport_to_the_rows_the_list_shows() {
        // 80x24 terminal: the content area is 16 rows. The legend takes 8 and the
        // borders 2, so the list shows 6 rows, and that is the page size.
        let mut component = PackageSelectComponent::new(vec!["dots".into()]);
        component.prepare(Rect::new(0, 0, 80, 16));
        assert_eq!(component.viewport_height, 6);

        // Too short for a legend: the list takes the whole area.
        component.prepare(Rect::new(0, 0, 80, 10));
        assert_eq!(component.viewport_height, 8);
    }
}
