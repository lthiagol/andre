use std::collections::HashMap;
use std::path::Path;

use andre_core::StowStatus;

/// Cache for stow status results. Writes (prefetch) should happen in
/// `Component::update()`; reads in `render()` should never block on I/O.
pub struct StatusCache {
    inner: HashMap<(String, String), StowStatus>,
    /// Bumped on every change, so readers can tell whether anything they built from the cache is stale.
    generation: u64,
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
            generation: 0,
        }
    }

    /// Changes whenever a status is added or the cache is cleared.
    pub fn generation(&self) -> u64 {
        self.generation
    }

    /// Read cached status. Returns `None` if not yet prefetched.
    pub fn get(&self, group: &str, pkg: &str) -> Option<StowStatus> {
        self.inner
            .get(&(group.to_string(), pkg.to_string()))
            .copied()
    }

    /// Prefetch and cache a single package status. `global` is the `.stow-global-ignore`
    /// to honour (see `andre_core::ignore::global_ignore_in`).
    pub fn prefetch_package(
        &mut self,
        source: &Path,
        target: &Path,
        group: &str,
        pkg: &str,
        global: Option<&Path>,
    ) -> StowStatus {
        let key = (group.to_string(), pkg.to_string());
        if let Some(&st) = self.inner.get(&key) {
            return st;
        }
        // The only error here is an ignore list that cannot be read or compiled. That
        // is a broken package, not a missing one, so it must not read as Missing.
        let st = andre_core::check_stowed_status(source, target, pkg, global)
            .unwrap_or(StowStatus::Invalid);
        self.insert(key, st);
        st
    }

    /// Prefetch all packages in a group.
    pub fn prefetch_group(
        &mut self,
        source: &Path,
        target: &Path,
        group: &str,
        packages: &[String],
        global: Option<&Path>,
    ) {
        for pkg in packages {
            self.prefetch_package(source, target, group, pkg, global);
        }
    }

    pub fn invalidate_all(&mut self) {
        self.inner.clear();
        self.generation += 1;
    }

    // -- test helpers / legacy compat --
    pub fn is_empty(&self) -> bool {
        self.inner.is_empty()
    }

    pub fn insert(&mut self, key: (String, String), value: StowStatus) {
        self.inner.insert(key, value);
        self.generation += 1;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_broken_ignore_list_is_invalid_not_missing() {
        let tmp = TempDir::new().unwrap();
        let source = tmp.path().join("packages");
        let target = tmp.path().join("target");
        std::fs::create_dir_all(source.join("pkg")).unwrap();
        std::fs::create_dir_all(&target).unwrap();
        std::fs::write(source.join("pkg/.rc"), "x").unwrap();
        // Look-around is rejected by the regex engine, so the list does not compile.
        std::fs::write(source.join("pkg/.stow-local-ignore"), "(?=x)y\n").unwrap();

        let mut cache = StatusCache::new();
        let st = cache.prefetch_package(&source, &target, "g", "pkg", None);
        assert_eq!(st, StowStatus::Invalid);
        assert_eq!(cache.get("g", "pkg"), Some(StowStatus::Invalid));
    }

    #[test]
    fn test_missing_source_is_still_missing() {
        let tmp = TempDir::new().unwrap();
        let source = tmp.path().join("packages");
        let target = tmp.path().join("target");
        std::fs::create_dir_all(&source).unwrap();
        std::fs::create_dir_all(&target).unwrap();

        let mut cache = StatusCache::new();
        let st = cache.prefetch_package(&source, &target, "g", "nope", None);
        assert_eq!(st, StowStatus::Missing);
    }
}
