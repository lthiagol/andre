use std::collections::HashMap;
use std::path::Path;

use andre_core::StowStatus;

/// Cache for stow status results. Writes (prefetch) should happen in
/// `Component::update()`; reads in `render()` should never block on I/O.
pub struct StatusCache {
    inner: HashMap<(String, String), StowStatus>,
}

impl Default for StatusCache {
    fn default() -> Self {
        Self::new()
    }
}

impl StatusCache {
    pub fn new() -> Self {
        Self {
            inner: HashMap::new(),
        }
    }

    /// Read cached status. Returns `None` if not yet prefetched.
    pub fn get(&self, group: &str, pkg: &str) -> Option<StowStatus> {
        self.inner
            .get(&(group.to_string(), pkg.to_string()))
            .copied()
    }

    /// Prefetch and cache a single package status.
    pub fn prefetch_package(
        &mut self,
        source: &Path,
        target: &Path,
        group: &str,
        pkg: &str,
    ) -> StowStatus {
        let key = (group.to_string(), pkg.to_string());
        if let Some(&st) = self.inner.get(&key) {
            return st;
        }
        let st =
            andre_core::check_stowed_status(source, target, pkg).unwrap_or(StowStatus::Missing);
        self.inner.insert(key, st);
        st
    }

    /// Prefetch all packages in a group.
    pub fn prefetch_group(
        &mut self,
        source: &Path,
        target: &Path,
        group: &str,
        packages: &[String],
    ) {
        for pkg in packages {
            self.prefetch_package(source, target, group, pkg);
        }
    }

    pub fn invalidate_all(&mut self) {
        self.inner.clear();
    }

    pub fn invalidate_group(&mut self, group: &str) {
        self.inner.retain(|(g, _), _| g != group);
    }

    // -- test helpers / legacy compat --
    pub fn is_empty(&self) -> bool {
        self.inner.is_empty()
    }

    pub fn insert(&mut self, key: (String, String), value: StowStatus) {
        self.inner.insert(key, value);
    }
}
