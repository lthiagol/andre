use std::collections::HashMap;
use std::path::Path;

use andre_core::{check_stowed_status_with_ignores, PackageEntry, StowStatus};

/// Cache for stow status results, keyed by package name (`shell`). Writes (prefetch) should
/// happen in `Component::update()`; reads in `render()` should never block on I/O.
pub struct StatusCache {
    inner: HashMap<String, StowStatus>,
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
    pub fn get(&self, name: &str) -> Option<StowStatus> {
        self.inner.get(name).copied()
    }

    /// Prefetch and cache one package's status. The package's ignores (`entry.ignores`,
    /// which is `global.ignore`) apply, so status counts the same files a link run would. `global` is the `.stow-global-ignore` to honour (see
    /// `andre_core::ignore::global_ignore_in`).
    pub fn prefetch(&mut self, entry: &PackageEntry, global: Option<&Path>) -> StowStatus {
        if let Some(&st) = self.inner.get(&entry.name) {
            return st;
        }
        // The only error here is an ignore list that cannot be read or compiled. That
        // is a broken package, not a missing one, so it must not read as Missing.
        let st = check_stowed_status_with_ignores(
            &entry.source,
            &entry.target,
            &entry.name,
            &entry.ignores,
            global,
        )
        .unwrap_or(StowStatus::Invalid);
        self.insert(entry.name.clone(), st);
        st
    }

    /// Prefetch every package in `entries`.
    pub fn prefetch_all(&mut self, entries: &[PackageEntry], global: Option<&Path>) {
        for entry in entries {
            self.prefetch(entry, global);
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

    pub fn insert(&mut self, name: String, value: StowStatus) {
        self.inner.insert(name, value);
        self.generation += 1;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    /// An entry for `name` in `source` linking into `target`, with `ignores` applied.
    fn entry(source: &Path, target: &Path, name: &str, ignores: Vec<String>) -> PackageEntry {
        PackageEntry {
            name: name.to_string(),
            dir: source.join(name),
            source: source.to_path_buf(),
            target: target.to_path_buf(),
            ignores,
        }
    }

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
        let st = cache.prefetch(&entry(&source, &target, "pkg", Vec::new()), None);
        assert_eq!(st, StowStatus::Invalid);
        assert_eq!(cache.get("pkg"), Some(StowStatus::Invalid));
    }

    #[test]
    fn test_missing_source_is_still_missing() {
        let tmp = TempDir::new().unwrap();
        let source = tmp.path().join("packages");
        let target = tmp.path().join("target");
        std::fs::create_dir_all(&source).unwrap();
        std::fs::create_dir_all(&target).unwrap();

        let mut cache = StatusCache::new();
        let st = cache.prefetch(&entry(&source, &target, "nope", Vec::new()), None);
        assert_eq!(st, StowStatus::Missing);
    }

    /// A file that matches `global.ignore` is not counted, so it does not make the package
    /// partial. Without the ignore it does.
    #[cfg(unix)]
    #[test]
    fn test_global_ignore_does_not_make_a_package_partial() {
        let tmp = TempDir::new().unwrap();
        let source = tmp.path().join("packages");
        let target = tmp.path().join("target");
        std::fs::create_dir_all(source.join("pkg")).unwrap();
        std::fs::create_dir_all(&target).unwrap();
        std::fs::write(source.join("pkg/.rc"), "x").unwrap();
        std::fs::write(source.join("pkg/.DS_Store"), "x").unwrap();
        std::os::unix::fs::symlink(source.join("pkg/.rc"), target.join(".rc")).unwrap();

        let mut cache = StatusCache::new();
        let plain = entry(&source, &target, "pkg", Vec::new());
        assert_eq!(cache.prefetch(&plain, None), StowStatus::Partial);

        let mut cache = StatusCache::new();
        let ignoring = entry(&source, &target, "pkg", vec![".DS_Store".to_string()]);
        assert_eq!(cache.prefetch(&ignoring, None), StowStatus::Stowed);
    }

    #[test]
    fn test_prefetch_all_caches_each_package_by_name() {
        let tmp = TempDir::new().unwrap();
        let source = tmp.path().join("packages");
        let target = tmp.path().join("target");
        std::fs::create_dir_all(source.join("alpha")).unwrap();
        std::fs::create_dir_all(source.join("beta")).unwrap();
        std::fs::create_dir_all(&target).unwrap();
        std::fs::write(source.join("alpha/.rc"), "x").unwrap();
        std::fs::write(source.join("beta/.rc"), "x").unwrap();
        let entries = [
            entry(&source, &target, "alpha", Vec::new()),
            entry(&source, &target, "beta", Vec::new()),
        ];

        let mut cache = StatusCache::new();
        cache.prefetch_all(&entries, None);
        assert_eq!(cache.get("alpha"), Some(StowStatus::Unstowed));
        assert_eq!(cache.get("beta"), Some(StowStatus::Unstowed));
        assert_eq!(cache.get("gamma"), None);
    }
}
