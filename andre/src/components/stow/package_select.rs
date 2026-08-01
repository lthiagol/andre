use std::collections::HashMap;

use crossterm::event::{KeyCode, KeyEvent};
use ratatui::{layout::Rect, Frame};

use crate::components::confirm::ConfirmComponent;
use crate::components::{AppContext, Component, Transition};
use crate::theme::ThemeColors;
use crate::ui::packages::FlatItem;
use crate::ui::render_package_select;
use andre_core::StowStatus;

#[derive(Default)]
pub struct PackageSelectComponent {
    pub selected_groups: Vec<String>,
    pub selected_packages: HashMap<String, Vec<String>>,
    pub cursor: usize,
    pub entered_at: Option<std::time::Instant>,
    pub scanner_frame: usize,
    viewport_height: usize,
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
        }
    }

    fn build_items(&self, ctx: &AppContext) -> Vec<FlatItem> {
        let mut items = Vec::new();
        for group_name in &self.selected_groups {
            if let Some(_group) = ctx.core.config.groups.get(group_name) {
                let source =
                    ctx.core
                        .config
                        .get_group_source(group_name, &ctx.config_dir, &ctx.home_dir);
                let packages = match &source {
                    Some(s) => andre_core::discover_packages(s),
                    None => Vec::new(),
                };
                let group_selected = self.selected_packages.get(group_name);
                for pkg_name in packages {
                    let pkg_selected = group_selected
                        .map(|v| v.contains(&pkg_name))
                        .unwrap_or(false);
                    let status = match &source {
                        Some(_s) => {
                            let target = ctx.core.config.get_group_target(
                                group_name,
                                &ctx.config_dir,
                                &ctx.home_dir,
                            );
                            match target {
                                Some(_t) => ctx
                                    .status_cache
                                    .borrow()
                                    .get(group_name, &pkg_name)
                                    .unwrap_or(StowStatus::Missing),
                                None => StowStatus::Missing,
                            }
                        }
                        None => StowStatus::Missing,
                    };
                    let preview = source
                        .as_ref()
                        .map(|s| andre_core::get_package_preview(s, &pkg_name, 3))
                        .unwrap_or_default();
                    items.push(FlatItem {
                        group_name: group_name.clone(),
                        package_name: pkg_name.clone(),
                        selected: pkg_selected,
                        status,
                        preview,
                    });
                }
            }
        }
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
        let items = self.build_items(ctx);
        let len = items.len();

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
            if let Some(item) = items.get(self.cursor) {
                let group = &item.group_name;
                let pkg = &item.package_name;
                let entry = self.selected_packages.entry(group.clone()).or_default();
                if entry.contains(pkg) {
                    entry.retain(|p| p != pkg);
                } else {
                    entry.push(pkg.clone());
                }
                if entry.is_empty() {
                    self.selected_packages.remove(group);
                }
            }
        } else if key.code == KeyCode::Enter {
            if !self.selected_packages.is_empty() {
                let action = ctx.core.effective_action().to_string();
                return Transition::Replace(
                    Box::new(ConfirmComponent::new(
                        self.selected_packages.clone(),
                        action,
                    )),
                    None,
                );
            }
        } else if key.code == KeyCode::Esc {
            return Transition::Pop;
        }

        Transition::None
    }

    fn update(&mut self, ctx: &mut AppContext) -> Transition {
        self.scanner_frame = self.scanner_frame.wrapping_add(1);
        for group_name in &self.selected_groups {
            if let Some(_group) = ctx.core.config.groups.get(group_name) {
                let source =
                    ctx.core
                        .config
                        .get_group_source(group_name, &ctx.config_dir, &ctx.home_dir);
                let target =
                    ctx.core
                        .config
                        .get_group_target(group_name, &ctx.config_dir, &ctx.home_dir);
                if let (Some(s), Some(t)) = (source, target) {
                    let packages = andre_core::discover_packages(&s);
                    let mut cache = ctx.status_cache.borrow_mut();
                    cache.prefetch_group(&s, &t, group_name, &packages);
                }
            }
        }
        Transition::None
    }

    fn render(&mut self, frame: &mut Frame, area: Rect, ctx: &AppContext) {
        let flat_items = self.build_items(ctx);
        let colors = ThemeColors::from_theme(ctx.core.theme);
        let title = format!(" {} > Select Packages ", ctx.breadcrumb);
        render_package_select(
            frame,
            area,
            &colors,
            &title,
            &flat_items,
            self.cursor,
            self.entered_at,
            self.scanner_frame,
        );
    }

    fn prepare(&mut self, area: Rect) {
        self.viewport_height = area.height.saturating_sub(2).max(1) as usize;
    }
}
