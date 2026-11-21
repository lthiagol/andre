//! Profiles: named collections of packages, one YAML file each.
//!
//! A profile lives at `<config-folder>/profiles/<name>.yml` and lists package refs
//! (package names like `nvim`; `group/package` while a `groups:` config is still
//! supported). It may also include other profiles. [`ProfileSet::resolve`]
//! flattens the includes into one ordered, de-duplicated list and records where each
//! entry came from. Cycles and unknown includes are errors; package refs that point
//! at nothing are kept and marked `missing`.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::config::Config;
use crate::create::is_valid_package_name;
use crate::error::{Error, Result};

/// Folder under the config folder that holds profile files.
pub const PROFILES_DIR: &str = "profiles";

/// One profile file. `deny_unknown_fields`: a misspelt key rejects the whole file.
#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Profile {
    /// File stem. Set from the file name on load, never written to the file.
    #[serde(skip)]
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub includes: Vec<String>,
    /// Package refs, in the order the user listed them.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub packages: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProfileError {
    InvalidName(String),
    InvalidPackageRef(String),
    /// `<name>.yaml` exists; profile files must use `.yml`.
    WrongExtension(String),
    Exists(String),
    NotFound(String),
    UnknownInclude {
        profile: String,
        include: String,
    },
    /// Include path from the first profile of the cycle back to itself.
    Cycle(Vec<String>),
    /// Deleting a profile that other profiles still include.
    IncludedBy {
        profile: String,
        by: Vec<String>,
    },
}

impl fmt::Display for ProfileError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ProfileError::InvalidName(name) => write!(
                f,
                "invalid profile name '{}': use lowercase letters, digits, '-' and '_', starting with a letter or digit",
                name
            ),
            ProfileError::InvalidPackageRef(pkg) => {
                write!(f, "invalid package ref '{}': expected a package name like 'nvim'", pkg)
            }
            ProfileError::WrongExtension(file) => {
                write!(f, "'{}': profile files must use the .yml extension", file)
            }
            ProfileError::Exists(name) => write!(f, "profile '{}' already exists", name),
            ProfileError::NotFound(name) => write!(f, "profile '{}' not found", name),
            ProfileError::UnknownInclude { profile, include } => write!(
                f,
                "profile '{}' includes unknown profile '{}'",
                profile, include
            ),
            ProfileError::Cycle(path) => {
                write!(f, "profile include cycle: {}", path.join(" -> "))
            }
            ProfileError::IncludedBy { profile, by } => write!(
                f,
                "profile '{}' is included by {}; remove it from those profiles first",
                profile,
                by.join(", ")
            ),
        }
    }
}

impl std::error::Error for ProfileError {}

/// One file change in a [`ProfileSet`] transaction.
enum Op {
    Write(Profile),
    Remove(String),
}

/// Why a profile file was skipped on load. The rest of the set still loads.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LoadIssue {
    pub file: String,
    pub message: String,
}

/// Where a resolved package came from.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Origin {
    /// Listed in the resolved profile's own `packages`.
    Own,
    /// Reached through the resolved profile's direct include `include`, and listed in
    /// `declared_in`'s own `packages`.
    Included {
        include: String,
        declared_in: String,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedEntry {
    pub package: String,
    pub origin: Origin,
    /// The ref names no package that exists on disk. Not fatal; shown as `missing`.
    pub missing: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedProfile {
    pub name: String,
    /// Own packages first, then each include in listed order (recursively). No duplicates.
    pub entries: Vec<ResolvedEntry>,
}

impl ResolvedProfile {
    pub fn missing(&self) -> impl Iterator<Item = &ResolvedEntry> {
        self.entries.iter().filter(|e| e.missing)
    }
}

/// Every profile in one `profiles/` folder, keyed by name.
#[derive(Debug, Clone, Default)]
pub struct ProfileSet {
    dir: PathBuf,
    profiles: BTreeMap<String, Profile>,
    issues: Vec<LoadIssue>,
}

impl ProfileSet {
    /// An empty set that writes into `dir`.
    pub fn new(dir: PathBuf) -> Self {
        Self {
            dir,
            profiles: BTreeMap::new(),
            issues: Vec::new(),
        }
    }

    /// Loads every `*.yml` in `dir`. A missing folder is an empty set. A file that
    /// fails to parse or has a bad name is recorded in [`ProfileSet::issues`] and
    /// skipped. Only an unreadable folder is an error.
    pub fn load(dir: &Path) -> Result<Self> {
        let mut set = Self::new(dir.to_path_buf());
        if !dir.is_dir() {
            return Ok(set);
        }

        let mut paths = fs::read_dir(dir)?
            .map(|entry| entry.map(|e| e.path()))
            .collect::<std::io::Result<Vec<_>>>()?;
        paths.sort();

        for path in paths {
            if !path.is_file() {
                continue;
            }
            let file = path
                .file_name()
                .map(|f| f.to_string_lossy().into_owned())
                .unwrap_or_default();
            match read_profile(&path) {
                Ok(Some(profile)) => {
                    set.profiles.insert(profile.name.clone(), profile);
                }
                Ok(None) => {}
                Err(err) => set.issues.push(LoadIssue {
                    file,
                    message: err.to_string(),
                }),
            }
        }
        Ok(set)
    }

    pub fn dir(&self) -> &Path {
        &self.dir
    }

    pub fn get(&self, name: &str) -> Option<&Profile> {
        self.profiles.get(name)
    }

    /// Profiles in name order.
    pub fn iter(&self) -> impl Iterator<Item = &Profile> {
        self.profiles.values()
    }

    pub fn issues(&self) -> &[LoadIssue] {
        &self.issues
    }

    pub fn file_path(&self, name: &str) -> PathBuf {
        self.dir.join(format!("{name}.yml"))
    }

    /// Writes a new profile file. Fails if the name is invalid or the file exists.
    pub fn create(&mut self, profile: Profile) -> Result<()> {
        check_profile(&profile)?;
        if self.file_path(&profile.name).exists() {
            return Err(ProfileError::Exists(profile.name).into());
        }
        self.store(profile)
    }

    /// Writes a profile file, replacing the current contents if there are any.
    pub fn save(&mut self, profile: Profile) -> Result<()> {
        check_profile(&profile)?;
        self.store(profile)
    }

    /// Renames a profile's file and rewrites `includes` in every profile that names
    /// it. Returns the profiles that were rewritten.
    pub fn rename(&mut self, old: &str, new: &str) -> Result<Vec<String>> {
        if !is_valid_name(new) {
            return Err(ProfileError::InvalidName(new.to_string()).into());
        }
        if !self.profiles.contains_key(old) {
            return Err(ProfileError::NotFound(old.to_string()).into());
        }
        if old == new {
            return Ok(Vec::new());
        }
        if self.file_path(new).exists() {
            return Err(ProfileError::Exists(new.to_string()).into());
        }

        // The new file is written before the old one is removed, and includers are
        // repointed while `new` already exists, so every intermediate state loads.
        let mut renamed = repoint_include(&self.profiles[old], old, new)
            .unwrap_or_else(|| self.profiles[old].clone());
        renamed.name = new.to_string();
        let mut ops = vec![Op::Write(renamed)];

        let mut changed = Vec::new();
        for profile in self.profiles.values().filter(|p| p.name != old) {
            if let Some(next) = repoint_include(profile, old, new) {
                changed.push(next.name.clone());
                ops.push(Op::Write(next));
            }
        }
        ops.push(Op::Remove(old.to_string()));

        self.transact(ops)?;
        Ok(changed)
    }

    /// Deletes a profile file. Refused while another profile includes it.
    pub fn delete(&mut self, name: &str) -> Result<()> {
        if !self.profiles.contains_key(name) {
            return Err(ProfileError::NotFound(name.to_string()).into());
        }
        let by: Vec<String> = self
            .profiles
            .values()
            .filter(|p| p.name != name && p.includes.iter().any(|inc| inc == name))
            .map(|p| p.name.clone())
            .collect();
        if !by.is_empty() {
            return Err(ProfileError::IncludedBy {
                profile: name.to_string(),
                by,
            }
            .into());
        }

        self.transact(vec![Op::Remove(name.to_string())])
    }

    /// Deletes a profile file and drops it from the includes of every profile that names
    /// it. The includers and the deletion are one change. Returns the profiles rewritten.
    pub fn delete_unlinking(&mut self, name: &str) -> Result<Vec<String>> {
        if !self.profiles.contains_key(name) {
            return Err(ProfileError::NotFound(name.to_string()).into());
        }
        let mut changed = Vec::new();
        let mut ops = Vec::new();
        for profile in self.profiles.values().filter(|p| p.name != name) {
            if let Some(next) = drop_include(profile, name) {
                changed.push(next.name.clone());
                ops.push(Op::Write(next));
            }
        }
        ops.push(Op::Remove(name.to_string()));

        self.transact(ops)?;
        Ok(changed)
    }

    /// Renames a package ref in every profile. Returns the profiles that changed.
    pub fn rename_package(&mut self, old: &str, new: &str) -> Result<Vec<String>> {
        if !is_valid_ref(new) {
            return Err(ProfileError::InvalidPackageRef(new.to_string()).into());
        }
        self.rewrite_packages(|pkg| {
            Some(if pkg == old {
                new.to_string()
            } else {
                pkg.to_string()
            })
        })
    }

    /// Removes a package ref from every profile. Returns the profiles that changed.
    pub fn remove_package(&mut self, pkg: &str) -> Result<Vec<String>> {
        self.rewrite_packages(|p| (p != pkg).then(|| p.to_string()))
    }

    /// Resolves `name` against the packages that exist (`known`, as in [`known_packages`]).
    pub fn resolve(&self, name: &str, known: &BTreeSet<String>) -> Result<ResolvedProfile> {
        if !self.profiles.contains_key(name) {
            return Err(ProfileError::NotFound(name.to_string()).into());
        }
        let mut walk = Walk {
            set: self,
            known,
            stack: Vec::new(),
            done: BTreeSet::new(),
            seen: BTreeSet::new(),
            entries: Vec::new(),
        };
        walk.visit(name, None)?;
        Ok(ResolvedProfile {
            name: name.to_string(),
            entries: walk.entries,
        })
    }

    /// Resolves `draft` as if it were saved under its name, without touching the disk. The
    /// other profiles are the ones in the set. The editor uses it to show unsaved changes.
    pub fn resolve_draft(
        &self,
        draft: &Profile,
        known: &BTreeSet<String>,
    ) -> Result<ResolvedProfile> {
        let mut set = self.clone();
        set.profiles.insert(draft.name.clone(), draft.clone());
        set.resolve(&draft.name, known)
    }

    /// Human-readable problems: skipped files, resolution errors and missing refs.
    pub fn warnings(&self, known: &BTreeSet<String>) -> Vec<String> {
        let mut out: Vec<String> = self
            .issues
            .iter()
            .map(|issue| format!("profile file {}: {}", issue.file, issue.message))
            .collect();
        for profile in self.profiles.values() {
            match self.resolve(&profile.name, known) {
                Ok(resolved) => {
                    for entry in resolved.missing() {
                        out.push(format!(
                            "profile '{}': package '{}' not found",
                            profile.name, entry.package
                        ));
                    }
                }
                Err(err) => out.push(err.to_string()),
            }
        }
        out
    }

    fn store(&mut self, profile: Profile) -> Result<()> {
        self.transact(vec![Op::Write(profile)])
    }

    /// Applies `f` to every package ref (`None` removes it). Drops duplicates, keeps
    /// order, and writes the profiles whose list changed.
    fn rewrite_packages(&mut self, f: impl Fn(&str) -> Option<String>) -> Result<Vec<String>> {
        let mut changed = Vec::new();
        let mut ops = Vec::new();
        for profile in self.profiles.values() {
            let next = dedup(profile.packages.iter().filter_map(|pkg| f(pkg.as_str())));
            if next != profile.packages {
                changed.push(profile.name.clone());
                ops.push(Op::Write(Profile {
                    packages: next,
                    ..profile.clone()
                }));
            }
        }
        self.transact(ops)?;
        Ok(changed)
    }

    /// Writes every op to disk, then applies them to memory. If an op fails, the ops
    /// already written are undone and memory is never touched. Callers name only
    /// profiles that exist (for `Remove`).
    fn transact(&mut self, ops: Vec<Op>) -> Result<()> {
        let mut undo = Vec::with_capacity(ops.len());
        for op in &ops {
            let inverse = match op {
                Op::Write(profile) => match self.profiles.get(&profile.name) {
                    Some(original) => Op::Write(original.clone()),
                    None => Op::Remove(profile.name.clone()),
                },
                Op::Remove(name) => Op::Write(self.profiles[name].clone()),
            };
            if let Err(err) = self.apply(op) {
                self.roll_back(undo);
                return Err(err);
            }
            undo.push(inverse);
        }

        for op in ops {
            match op {
                Op::Write(profile) => {
                    self.profiles.insert(profile.name.clone(), profile);
                }
                Op::Remove(name) => {
                    self.profiles.remove(&name);
                }
            }
        }
        Ok(())
    }

    fn apply(&self, op: &Op) -> Result<()> {
        match op {
            Op::Write(profile) => write_profile(&self.dir, profile),
            Op::Remove(name) => Ok(fs::remove_file(self.file_path(name))?),
        }
    }

    /// Undoes written ops, newest first. If an undo fails, memory is reloaded from
    /// disk so it matches whatever is actually there.
    fn roll_back(&mut self, undo: Vec<Op>) {
        let mut clean = true;
        for op in undo.iter().rev() {
            clean &= self.apply(op).is_ok();
        }
        if !clean {
            if let Ok(fresh) = ProfileSet::load(&self.dir) {
                self.profiles = fresh.profiles;
                self.issues = fresh.issues;
            }
        }
    }
}

/// The names of the packages that exist on disk (see [`Config::package_entries`]).
pub fn known_packages(config: &Config, config_dir: &Path, home_dir: &Path) -> BTreeSet<String> {
    config
        .package_entries(config_dir, home_dir)
        .into_iter()
        .map(|entry| entry.name)
        .collect()
}

/// `[a-z0-9][a-z0-9_-]*`. The rule for profile file stems.
pub fn is_valid_name(name: &str) -> bool {
    let mut chars = name.chars();
    matches!(chars.next(), Some(c) if c.is_ascii_lowercase() || c.is_ascii_digit())
        && chars.all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_' || c == '-')
}

/// Splits `group/package` at its last `/`. Both halves must be non-empty.
pub fn split_package_ref(pkg: &str) -> Option<(&str, &str)> {
    let (group, package) = pkg.rsplit_once('/')?;
    (!group.is_empty() && !package.is_empty()).then_some((group, package))
}

/// A ref is a package name (`nvim`) or, while `groups:` is supported, `group/package`.
fn is_valid_ref(pkg: &str) -> bool {
    is_valid_package_name(pkg) || split_package_ref(pkg).is_some()
}

/// Reads one `*.yml` file. `Ok(None)` for files that are not profiles at all.
fn read_profile(path: &Path) -> Result<Option<Profile>> {
    let file = path
        .file_name()
        .map(|f| f.to_string_lossy().into_owned())
        .unwrap_or_default();
    match path.extension().and_then(|e| e.to_str()) {
        Some("yml") => {}
        Some("yaml") => return Err(ProfileError::WrongExtension(file).into()),
        _ => return Ok(None),
    }

    let name = path
        .file_stem()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_default();
    if !is_valid_name(&name) {
        return Err(ProfileError::InvalidName(name).into());
    }

    let text = fs::read_to_string(path)?;
    let mut profile: Profile = if text.trim().is_empty() {
        Profile::default()
    } else {
        serde_yaml_ng::from_str(&text).map_err(Error::YamlParse)?
    };
    profile.name = name;
    check_refs(&profile)?;
    Ok(Some(profile))
}

fn check_profile(profile: &Profile) -> Result<()> {
    if !is_valid_name(&profile.name) {
        return Err(ProfileError::InvalidName(profile.name.clone()).into());
    }
    check_refs(profile)
}

fn check_refs(profile: &Profile) -> Result<()> {
    for pkg in &profile.packages {
        if !is_valid_ref(pkg) {
            return Err(ProfileError::InvalidPackageRef(pkg.clone()).into());
        }
    }
    Ok(())
}

/// Writes through a `.tmp` file beside the target, then renames it over the target, so
/// a failed or interrupted write leaves the old file intact. `load` ignores the temp
/// file (its extension is not `.yml`).
fn write_profile(dir: &Path, profile: &Profile) -> Result<()> {
    fs::create_dir_all(dir)?;
    let yaml = serde_yaml_ng::to_string(profile).map_err(Error::YamlParse)?;
    let target = dir.join(format!("{}.yml", profile.name));
    let tmp = dir.join(format!(".{}.yml.tmp", profile.name));
    if let Err(err) = write_synced(&tmp, yaml.as_bytes()).and_then(|()| fs::rename(&tmp, &target)) {
        let _ = fs::remove_file(&tmp);
        return Err(err.into());
    }
    Ok(())
}

fn write_synced(path: &Path, bytes: &[u8]) -> std::io::Result<()> {
    let mut file = fs::File::create(path)?;
    file.write_all(bytes)?;
    file.sync_all()
}

/// `profile` without its include of `name`, or `None` if it has none.
fn drop_include(profile: &Profile, name: &str) -> Option<Profile> {
    if !profile.includes.iter().any(|inc| inc == name) {
        return None;
    }
    let mut next = profile.clone();
    next.includes.retain(|inc| inc != name);
    Some(next)
}

/// `profile` with every include of `old` replaced by `new`, or `None` if it has none.
fn repoint_include(profile: &Profile, old: &str, new: &str) -> Option<Profile> {
    if !profile.includes.iter().any(|inc| inc == old) {
        return None;
    }
    let mut next = profile.clone();
    next.includes = dedup(profile.includes.iter().map(|inc| {
        if inc == old {
            new.to_string()
        } else {
            inc.clone()
        }
    }));
    Some(next)
}

/// Keeps the first occurrence of each item.
fn dedup(items: impl Iterator<Item = String>) -> Vec<String> {
    let mut seen = BTreeSet::new();
    items.filter(|item| seen.insert(item.clone())).collect()
}

/// Depth-first walk over includes. `stack` holds the profiles being expanded (cycle
/// check); `done` holds the ones fully expanded (so diamonds are walked once).
struct Walk<'a> {
    set: &'a ProfileSet,
    known: &'a BTreeSet<String>,
    stack: Vec<String>,
    done: BTreeSet<String>,
    seen: BTreeSet<String>,
    entries: Vec<ResolvedEntry>,
}

impl Walk<'_> {
    /// `via` is the root's direct include this profile was reached through, or `None`
    /// for the root itself.
    fn visit(&mut self, name: &str, via: Option<&str>) -> Result<()> {
        if let Some(pos) = self.stack.iter().position(|n| n == name) {
            let mut path = self.stack[pos..].to_vec();
            path.push(name.to_string());
            return Err(ProfileError::Cycle(path).into());
        }
        if self.done.contains(name) {
            return Ok(());
        }

        let set = self.set;
        let Some(profile) = set.profiles.get(name) else {
            return Err(ProfileError::NotFound(name.to_string()).into());
        };

        self.stack.push(name.to_string());
        for pkg in &profile.packages {
            if self.seen.insert(pkg.clone()) {
                let origin = match via {
                    None => Origin::Own,
                    Some(include) => Origin::Included {
                        include: include.to_string(),
                        declared_in: name.to_string(),
                    },
                };
                self.entries.push(ResolvedEntry {
                    package: pkg.clone(),
                    origin,
                    missing: !self.known.contains(pkg),
                });
            }
        }
        for include in &profile.includes {
            if !set.profiles.contains_key(include) {
                return Err(ProfileError::UnknownInclude {
                    profile: name.to_string(),
                    include: include.clone(),
                }
                .into());
            }
            self.visit(include, via.or(Some(include.as_str())))?;
        }
        self.stack.pop();
        self.done.insert(name.to_string());
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn write(dir: &Path, file: &str, body: &str) {
        fs::create_dir_all(dir).unwrap();
        fs::write(dir.join(file), body).unwrap();
    }

    fn known(pkgs: &[&str]) -> BTreeSet<String> {
        pkgs.iter().map(|p| p.to_string()).collect()
    }

    fn profile(name: &str, includes: &[&str], packages: &[&str]) -> Profile {
        Profile {
            name: name.to_string(),
            description: None,
            includes: includes.iter().map(|s| s.to_string()).collect(),
            packages: packages.iter().map(|s| s.to_string()).collect(),
        }
    }

    fn entry_names(resolved: &ResolvedProfile) -> Vec<&str> {
        resolved
            .entries
            .iter()
            .map(|e| e.package.as_str())
            .collect()
    }

    fn load(dir: &Path) -> ProfileSet {
        ProfileSet::load(dir).unwrap()
    }

    #[test]
    fn missing_folder_loads_empty_set() {
        let tmp = TempDir::new().unwrap();
        let set = load(&tmp.path().join("profiles"));
        assert_eq!(set.iter().count(), 0);
        assert!(set.issues().is_empty());
    }

    #[test]
    fn loads_profiles_by_file_stem_in_name_order() {
        let tmp = TempDir::new().unwrap();
        let dir = tmp.path().join("profiles");
        write(
            &dir,
            "work.yml",
            "description: Work\npackages: [home/git]\n",
        );
        write(&dir, "agent-base.yml", "packages: [config/nvim]\n");
        write(&dir, "README.txt", "not a profile");
        fs::create_dir_all(dir.join("subdir.yml")).unwrap();

        let set = load(&dir);
        let names: Vec<&str> = set.iter().map(|p| p.name.as_str()).collect();
        assert_eq!(names, ["agent-base", "work"]);
        let work = set.get("work").unwrap();
        assert_eq!(work.description.as_deref(), Some("Work"));
        assert_eq!(work.packages, ["home/git"]);
        assert!(work.includes.is_empty());
        assert!(set.issues().is_empty());
    }

    #[test]
    fn empty_file_is_an_empty_profile() {
        let tmp = TempDir::new().unwrap();
        let dir = tmp.path().join("profiles");
        write(&dir, "empty.yml", "   \n");
        let set = load(&dir);
        let empty = set.get("empty").unwrap();
        assert_eq!(empty, &profile("empty", &[], &[]));
    }

    #[test]
    fn unknown_field_skips_only_that_file() {
        let tmp = TempDir::new().unwrap();
        let dir = tmp.path().join("profiles");
        write(&dir, "good.yml", "packages: [home/git]\n");
        write(&dir, "bad.yml", "packages: [home/git]\npakages: [x/y]\n");

        let set = load(&dir);
        assert!(set.get("good").is_some());
        assert!(set.get("bad").is_none());
        assert_eq!(set.issues().len(), 1);
        assert_eq!(set.issues()[0].file, "bad.yml");
        assert!(
            set.issues()[0].message.contains("pakages"),
            "message should name the unknown field: {}",
            set.issues()[0].message
        );
    }

    #[test]
    fn invalid_file_names_are_issues() {
        let tmp = TempDir::new().unwrap();
        let dir = tmp.path().join("profiles");
        write(&dir, "Has Space.yml", "packages: []\n");
        write(&dir, "-leading.yml", "packages: []\n");
        write(&dir, "old.yaml", "packages: []\n");

        let set = load(&dir);
        assert_eq!(set.iter().count(), 0);
        let messages: Vec<&str> = set.issues().iter().map(|i| i.message.as_str()).collect();
        assert_eq!(set.issues().len(), 3, "{messages:?}");
        assert!(messages.iter().any(|m| m.contains("invalid profile name")));
        assert!(messages.iter().any(|m| m.contains(".yml extension")));
    }

    #[test]
    fn malformed_package_ref_is_an_issue() {
        let tmp = TempDir::new().unwrap();
        let dir = tmp.path().join("profiles");
        write(&dir, "p.yml", "packages: [bad name]\n");
        let set = load(&dir);
        assert!(set.get("p").is_none());
        assert!(set.issues()[0].message.contains("expected a package name"));
    }

    #[test]
    fn name_rule_matches_spec() {
        for ok in ["a", "0", "agent-work", "a_b-2", "x9"] {
            assert!(is_valid_name(ok), "{ok} should be valid");
        }
        for bad in ["", "-a", "_a", "A", "a b", "a.b", "a/b", "café"] {
            assert!(!is_valid_name(bad), "{bad} should be invalid");
        }
    }

    #[test]
    fn split_package_ref_needs_both_halves() {
        assert_eq!(split_package_ref("home/git"), Some(("home", "git")));
        assert_eq!(split_package_ref("a/b/c"), Some(("a/b", "c")));
        assert_eq!(split_package_ref("home/"), None);
        assert_eq!(split_package_ref("/git"), None);
        assert_eq!(split_package_ref("git"), None);
    }

    #[test]
    fn resolve_own_packages_in_listed_order() {
        let tmp = TempDir::new().unwrap();
        let mut set = ProfileSet::new(tmp.path().join("profiles"));
        set.create(profile("p", &[], &["b/two", "a/one"])).unwrap();

        let resolved = set.resolve("p", &known(&["a/one", "b/two"])).unwrap();
        assert_eq!(entry_names(&resolved), ["b/two", "a/one"]);
        assert!(resolved.entries.iter().all(|e| e.origin == Origin::Own));
        assert!(resolved.entries.iter().all(|e| !e.missing));
    }

    #[test]
    fn resolve_includes_recursively_own_first_then_includes_in_order() {
        let tmp = TempDir::new().unwrap();
        let mut set = ProfileSet::new(tmp.path().join("profiles"));
        set.create(profile("base", &[], &["core/a"])).unwrap();
        set.create(profile("tools", &["base"], &["tools/t"]))
            .unwrap();
        set.create(profile("work", &["tools", "base"], &["work/w", "core/a"]))
            .unwrap();

        let all = known(&["core/a", "tools/t", "work/w"]);
        let resolved = set.resolve("work", &all).unwrap();
        // own: work/w, core/a; tools: (tools/t, then base's core/a = duplicate, skipped);
        // base again is already done.
        assert_eq!(entry_names(&resolved), ["work/w", "core/a", "tools/t"]);
    }

    #[test]
    fn resolve_records_origin_through_nested_includes() {
        let tmp = TempDir::new().unwrap();
        let mut set = ProfileSet::new(tmp.path().join("profiles"));
        set.create(profile("leaf", &[], &["x/leaf"])).unwrap();
        set.create(profile("mid", &["leaf"], &["x/mid"])).unwrap();
        set.create(profile("root", &["mid"], &["x/root"])).unwrap();

        let resolved = set.resolve("root", &known(&[])).unwrap();
        assert_eq!(resolved.entries[0].origin, Origin::Own);
        assert_eq!(
            resolved.entries[1].origin,
            Origin::Included {
                include: "mid".into(),
                declared_in: "mid".into(),
            }
        );
        assert_eq!(
            resolved.entries[2].origin,
            Origin::Included {
                include: "mid".into(),
                declared_in: "leaf".into(),
            }
        );
    }

    #[test]
    fn duplicate_refs_keep_first_origin() {
        let tmp = TempDir::new().unwrap();
        let mut set = ProfileSet::new(tmp.path().join("profiles"));
        set.create(profile("base", &[], &["home/git"])).unwrap();
        set.create(profile("top", &["base"], &["home/git"]))
            .unwrap();

        let resolved = set.resolve("top", &known(&["home/git"])).unwrap();
        assert_eq!(resolved.entries.len(), 1);
        assert_eq!(resolved.entries[0].origin, Origin::Own);
    }

    #[test]
    fn unknown_include_names_the_profile() {
        let tmp = TempDir::new().unwrap();
        let mut set = ProfileSet::new(tmp.path().join("profiles"));
        set.create(profile("top", &["ghost"], &[])).unwrap();

        let err = set.resolve("top", &known(&[])).unwrap_err();
        assert!(matches!(
            err,
            Error::Profile(ProfileError::UnknownInclude { ref profile, ref include })
                if profile == "top" && include == "ghost"
        ));
        let msg = err.to_string();
        assert!(msg.contains("'top'") && msg.contains("'ghost'"), "{msg}");
    }

    #[test]
    fn cycle_reports_its_path() {
        let tmp = TempDir::new().unwrap();
        let mut set = ProfileSet::new(tmp.path().join("profiles"));
        set.create(profile("a", &["b"], &[])).unwrap();
        set.create(profile("b", &["c"], &[])).unwrap();
        set.create(profile("c", &["b"], &[])).unwrap();

        let err = set.resolve("a", &known(&[])).unwrap_err();
        assert_eq!(
            err.to_string(),
            "profile include cycle: b -> c -> b",
            "cycle path starts at the first repeated profile"
        );
    }

    #[test]
    fn resolve_draft_uses_the_draft_without_writing_it() {
        let tmp = TempDir::new().unwrap();
        let mut set = ProfileSet::new(tmp.path().join("profiles"));
        set.create(profile("base", &[], &["home/a"])).unwrap();
        set.create(profile("agent", &[], &["home/b"])).unwrap();

        let draft = profile("agent", &["base"], &["home/b"]);
        let resolved = set
            .resolve_draft(&draft, &known(&["home/a", "home/b"]))
            .unwrap();
        assert_eq!(entry_names(&resolved), vec!["home/b", "home/a"]);

        // Nothing was written: a fresh load still has the stored profile.
        assert!(set.get("agent").unwrap().includes.is_empty());
        let reloaded = ProfileSet::load(&tmp.path().join("profiles")).unwrap();
        assert!(reloaded.get("agent").unwrap().includes.is_empty());
    }

    #[test]
    fn resolve_draft_refuses_a_cycle_through_the_draft() {
        let tmp = TempDir::new().unwrap();
        let mut set = ProfileSet::new(tmp.path().join("profiles"));
        set.create(profile("base", &[], &[])).unwrap();
        set.create(profile("agent", &["base"], &[])).unwrap();

        let draft = profile("base", &["agent"], &[]);
        assert_eq!(
            set.resolve_draft(&draft, &known(&[]))
                .unwrap_err()
                .to_string(),
            "profile include cycle: base -> agent -> base"
        );
    }

    #[test]
    fn self_include_is_a_cycle() {
        let tmp = TempDir::new().unwrap();
        let mut set = ProfileSet::new(tmp.path().join("profiles"));
        set.create(profile("loop", &["loop"], &[])).unwrap();
        assert_eq!(
            set.resolve("loop", &known(&[])).unwrap_err().to_string(),
            "profile include cycle: loop -> loop"
        );
    }

    #[test]
    fn diamond_is_not_a_cycle() {
        let tmp = TempDir::new().unwrap();
        let mut set = ProfileSet::new(tmp.path().join("profiles"));
        set.create(profile("shared", &[], &["s/one"])).unwrap();
        set.create(profile("left", &["shared"], &[])).unwrap();
        set.create(profile("right", &["shared"], &[])).unwrap();
        set.create(profile("top", &["left", "right"], &[])).unwrap();

        let resolved = set.resolve("top", &known(&["s/one"])).unwrap();
        assert_eq!(entry_names(&resolved), ["s/one"]);
    }

    #[test]
    fn missing_refs_are_kept_and_flagged() {
        let tmp = TempDir::new().unwrap();
        let mut set = ProfileSet::new(tmp.path().join("profiles"));
        set.create(profile("p", &[], &["home/git", "gone/pkg"]))
            .unwrap();

        let resolved = set.resolve("p", &known(&["home/git"])).unwrap();
        assert_eq!(resolved.entries.len(), 2);
        let missing: Vec<&str> = resolved.missing().map(|e| e.package.as_str()).collect();
        assert_eq!(missing, ["gone/pkg"]);

        let warnings = set.warnings(&known(&["home/git"]));
        assert_eq!(warnings, ["profile 'p': package 'gone/pkg' not found"]);
    }

    #[test]
    fn resolve_unknown_profile_is_not_found() {
        let tmp = TempDir::new().unwrap();
        let set = ProfileSet::new(tmp.path().join("profiles"));
        assert_eq!(
            set.resolve("nope", &known(&[])).unwrap_err().to_string(),
            "profile 'nope' not found"
        );
    }

    #[test]
    fn warnings_report_issues_and_resolution_errors() {
        let tmp = TempDir::new().unwrap();
        let dir = tmp.path().join("profiles");
        write(&dir, "broken.yml", "bogus: 1\n");
        write(&dir, "dangling.yml", "includes: [ghost]\n");

        let set = load(&dir);
        let warnings = set.warnings(&known(&[]));
        assert_eq!(warnings.len(), 2, "{warnings:?}");
        assert!(warnings[0].starts_with("profile file broken.yml:"));
        assert!(warnings[1].contains("includes unknown profile 'ghost'"));
    }

    #[test]
    fn create_writes_file_and_round_trips() {
        let tmp = TempDir::new().unwrap();
        let dir = tmp.path().join("profiles");
        let mut set = ProfileSet::new(dir.clone());
        set.create(Profile {
            name: "agent-work".into(),
            description: Some("Servers".into()),
            includes: vec!["base".into()],
            packages: vec!["home/git".into()],
        })
        .unwrap();

        let text = fs::read_to_string(dir.join("agent-work.yml")).unwrap();
        assert!(text.contains("description: Servers"), "{text}");
        assert!(!text.contains("name:"), "name must not be written: {text}");

        let reloaded = load(&dir);
        assert_eq!(reloaded.get("agent-work"), set.get("agent-work"));
    }

    #[test]
    fn create_refuses_existing_file_and_bad_input() {
        let tmp = TempDir::new().unwrap();
        let dir = tmp.path().join("profiles");
        let mut set = ProfileSet::new(dir.clone());
        set.create(profile("p", &[], &[])).unwrap();

        assert_eq!(
            set.create(profile("p", &[], &[])).unwrap_err().to_string(),
            "profile 'p' already exists"
        );
        assert!(set.create(profile("Bad Name", &[], &[])).is_err());
        assert!(set.create(profile("ok", &[], &["bad name"])).is_err());
        assert!(!dir.join("ok.yml").exists());
    }

    #[test]
    fn create_refuses_name_of_a_broken_file_on_disk() {
        let tmp = TempDir::new().unwrap();
        let dir = tmp.path().join("profiles");
        write(&dir, "broken.yml", "bogus: 1\n");
        let mut set = load(&dir);
        assert!(set.create(profile("broken", &[], &[])).is_err());
    }

    #[test]
    fn save_replaces_contents() {
        let tmp = TempDir::new().unwrap();
        let dir = tmp.path().join("profiles");
        let mut set = ProfileSet::new(dir.clone());
        set.create(profile("p", &[], &["a/b"])).unwrap();
        set.save(profile("p", &[], &["c/d"])).unwrap();

        assert_eq!(load(&dir).get("p").unwrap().packages, ["c/d"]);
    }

    #[test]
    fn rename_moves_file_and_rewrites_includes() {
        let tmp = TempDir::new().unwrap();
        let dir = tmp.path().join("profiles");
        let mut set = ProfileSet::new(dir.clone());
        set.create(profile("base", &[], &["a/b"])).unwrap();
        set.create(profile("top", &["base"], &[])).unwrap();
        set.create(profile("other", &[], &[])).unwrap();

        let changed = set.rename("base", "core").unwrap();
        assert_eq!(changed, ["top"]);
        assert!(!dir.join("base.yml").exists());
        assert!(dir.join("core.yml").exists());
        assert_eq!(set.get("top").unwrap().includes, ["core"]);
        assert!(set.get("base").is_none());

        let reloaded = load(&dir);
        assert_eq!(reloaded.get("core").unwrap().name, "core");
        assert_eq!(reloaded.get("top").unwrap().includes, ["core"]);
        let resolved = reloaded.resolve("top", &known(&["a/b"])).unwrap();
        assert_eq!(entry_names(&resolved), ["a/b"]);
    }

    #[test]
    fn rename_refuses_taken_or_invalid_target() {
        let tmp = TempDir::new().unwrap();
        let dir = tmp.path().join("profiles");
        let mut set = ProfileSet::new(dir.clone());
        set.create(profile("a", &[], &[])).unwrap();
        set.create(profile("b", &[], &[])).unwrap();

        assert!(matches!(
            set.rename("a", "b"),
            Err(Error::Profile(ProfileError::Exists(_)))
        ));
        assert!(matches!(
            set.rename("a", "Not Valid"),
            Err(Error::Profile(ProfileError::InvalidName(_)))
        ));
        assert!(matches!(
            set.rename("missing", "c"),
            Err(Error::Profile(ProfileError::NotFound(_)))
        ));
        assert!(dir.join("a.yml").exists());
        assert!(dir.join("b.yml").exists());
    }

    #[test]
    fn rename_to_same_name_is_a_no_op() {
        let tmp = TempDir::new().unwrap();
        let mut set = ProfileSet::new(tmp.path().join("profiles"));
        set.create(profile("a", &[], &[])).unwrap();
        assert_eq!(set.rename("a", "a").unwrap(), Vec::<String>::new());
    }

    #[test]
    fn delete_removes_file() {
        let tmp = TempDir::new().unwrap();
        let dir = tmp.path().join("profiles");
        let mut set = ProfileSet::new(dir.clone());
        set.create(profile("gone", &[], &[])).unwrap();
        set.delete("gone").unwrap();
        assert!(!dir.join("gone.yml").exists());
        assert!(set.get("gone").is_none());
        assert!(matches!(
            set.delete("gone"),
            Err(Error::Profile(ProfileError::NotFound(_)))
        ));
    }

    #[test]
    fn delete_refused_while_included() {
        let tmp = TempDir::new().unwrap();
        let dir = tmp.path().join("profiles");
        let mut set = ProfileSet::new(dir.clone());
        set.create(profile("base", &[], &[])).unwrap();
        set.create(profile("x", &["base"], &[])).unwrap();
        set.create(profile("y", &["base"], &[])).unwrap();

        let err = set.delete("base").unwrap_err();
        assert_eq!(
            err.to_string(),
            "profile 'base' is included by x, y; remove it from those profiles first"
        );
        assert!(dir.join("base.yml").exists());
    }

    #[test]
    fn delete_unlinking_removes_the_file_and_its_includes() {
        let tmp = TempDir::new().unwrap();
        let dir = tmp.path().join("profiles");
        let mut set = ProfileSet::new(dir.clone());
        set.create(profile("base", &[], &["a/b"])).unwrap();
        set.create(profile("x", &["base"], &["c/d"])).unwrap();
        set.create(profile("y", &["base", "x"], &[])).unwrap();
        set.create(profile("other", &[], &[])).unwrap();

        let changed = set.delete_unlinking("base").unwrap();
        assert_eq!(changed, ["x", "y"]);
        assert!(!dir.join("base.yml").exists());
        assert!(set.get("base").is_none());
        assert!(set.get("x").unwrap().includes.is_empty());
        assert_eq!(set.get("y").unwrap().includes, ["x"]);
        assert!(set.get("other").unwrap().includes.is_empty());

        let reloaded = load(&dir);
        assert!(reloaded.get("base").is_none());
        assert!(reloaded.get("x").unwrap().includes.is_empty());
        assert_eq!(reloaded.get("y").unwrap().includes, ["x"]);
        assert!(matches!(
            set.delete_unlinking("base"),
            Err(Error::Profile(ProfileError::NotFound(_)))
        ));
    }

    #[test]
    fn rename_package_rewrites_every_profile_and_dedups() {
        let tmp = TempDir::new().unwrap();
        let dir = tmp.path().join("profiles");
        let mut set = ProfileSet::new(dir.clone());
        set.create(profile("a", &[], &["home/git", "home/shell"]))
            .unwrap();
        set.create(profile("b", &[], &["home/shell"])).unwrap();
        set.create(profile("c", &[], &["other/x"])).unwrap();

        let changed = set.rename_package("home/git", "home/vcs").unwrap();
        assert_eq!(changed, ["a"]);
        assert_eq!(set.get("a").unwrap().packages, ["home/vcs", "home/shell"]);

        // Renaming onto an existing ref keeps one copy.
        let changed = set.rename_package("home/shell", "home/vcs").unwrap();
        assert_eq!(changed, ["a", "b"]);
        assert_eq!(set.get("a").unwrap().packages, ["home/vcs"]);
        assert_eq!(load(&dir).get("b").unwrap().packages, ["home/vcs"]);
        assert_eq!(set.get("c").unwrap().packages, ["other/x"]);
    }

    #[test]
    fn rename_package_rejects_bad_target() {
        let tmp = TempDir::new().unwrap();
        let mut set = ProfileSet::new(tmp.path().join("profiles"));
        set.create(profile("a", &[], &["home/git"])).unwrap();
        assert!(matches!(
            set.rename_package("home/git", "bad name"),
            Err(Error::Profile(ProfileError::InvalidPackageRef(_)))
        ));
        assert_eq!(set.get("a").unwrap().packages, ["home/git"]);
    }

    #[test]
    fn remove_package_drops_ref_everywhere() {
        let tmp = TempDir::new().unwrap();
        let dir = tmp.path().join("profiles");
        let mut set = ProfileSet::new(dir.clone());
        set.create(profile("a", &[], &["home/git", "home/shell"]))
            .unwrap();
        set.create(profile("b", &[], &["config/nvim"])).unwrap();

        assert_eq!(set.remove_package("home/git").unwrap(), ["a"]);
        assert_eq!(set.get("a").unwrap().packages, ["home/shell"]);
        assert_eq!(load(&dir).get("a").unwrap().packages, ["home/shell"]);
        // Removing a ref that no profile lists changes nothing.
        assert!(set.remove_package("home/git").unwrap().is_empty());
    }

    #[test]
    fn flat_package_names_are_valid_refs() {
        let tmp = TempDir::new().unwrap();
        let mut set = ProfileSet::new(tmp.path().join("profiles"));
        set.create(profile("p", &[], &["nvim", "home/git"]))
            .unwrap();
        assert_eq!(set.get("p").unwrap().packages, ["nvim", "home/git"]);
        assert!(matches!(
            set.create(profile("q", &[], &["bad name"])),
            Err(Error::Profile(ProfileError::InvalidPackageRef(_)))
        ));
        assert!(matches!(
            set.rename_package("nvim", "bad name"),
            Err(Error::Profile(ProfileError::InvalidPackageRef(_)))
        ));
        assert_eq!(
            ProfileError::InvalidPackageRef("home/nvim/".to_string()).to_string(),
            "invalid package ref 'home/nvim/': expected a package name like 'nvim'"
        );
    }

    #[test]
    fn flat_profile_resolves_against_flat_known_packages() {
        let tmp = TempDir::new().unwrap();
        let config_dir = tmp.path();
        let home = tmp.path().join("home");
        fs::create_dir_all(config_dir.join("dotfiles/nvim")).unwrap();
        fs::create_dir_all(config_dir.join("dotfiles/.git")).unwrap();
        let config: Config = serde_yaml_ng::from_str("packages: dotfiles\nglobal: {}\n").unwrap();

        let known = known_packages(&config, config_dir, &home);
        assert_eq!(known.iter().collect::<Vec<_>>(), ["nvim"]);

        let mut set = ProfileSet::new(config_dir.join("profiles"));
        set.create(profile("p", &[], &["nvim", "ghost"])).unwrap();
        let resolved = set.resolve("p", &known).unwrap();
        let missing: Vec<_> = resolved
            .entries
            .iter()
            .map(|e| (e.package.as_str(), e.missing))
            .collect();
        assert_eq!(missing, [("nvim", false), ("ghost", true)]);
    }

    #[test]
    fn known_packages_lists_packages_that_exist() {
        let tmp = TempDir::new().unwrap();
        let config_dir = tmp.path();
        let home = tmp.path().join("home");
        fs::create_dir_all(config_dir.join("packages/git")).unwrap();
        fs::create_dir_all(config_dir.join("packages/shell")).unwrap();
        fs::write(config_dir.join("packages/loose.txt"), "").unwrap();

        let yaml = "global: {}\npackages: packages\ntarget: out\n";
        let config: Config = serde_yaml_ng::from_str(yaml).unwrap();
        let known = known_packages(&config, config_dir, &home);
        assert_eq!(known.into_iter().collect::<Vec<_>>(), ["git", "shell"]);
    }

    #[test]
    fn path_that_is_a_file_loads_as_empty() {
        let tmp = TempDir::new().unwrap();
        let file = tmp.path().join("profiles");
        fs::write(&file, "not a dir").unwrap();
        // Not a directory: treated as no profiles rather than an error.
        assert_eq!(load(&file).iter().count(), 0);
    }

    #[test]
    fn save_leaves_no_temp_file_and_load_ignores_a_stale_one() {
        let tmp = TempDir::new().unwrap();
        let dir = tmp.path().join("profiles");
        let mut set = ProfileSet::new(dir.clone());
        set.create(profile("p", &[], &["a/b"])).unwrap();
        assert!(!dir.join(".p.yml.tmp").exists());

        // Leftover from an interrupted write, cut off mid-line.
        write(&dir, ".p.yml.tmp", "packages: [c/");
        let reloaded = load(&dir);
        assert_eq!(reloaded.get("p").unwrap().packages, ["a/b"]);
        assert!(reloaded.issues().is_empty());
    }

    #[test]
    fn failed_save_keeps_the_previous_file() {
        let tmp = TempDir::new().unwrap();
        let dir = tmp.path().join("profiles");
        let mut set = ProfileSet::new(dir.clone());
        set.create(profile("p", &[], &["a/b"])).unwrap();

        // A directory where the temp file goes makes the write fail before the rename.
        fs::create_dir(dir.join(".p.yml.tmp")).unwrap();
        assert!(set.save(profile("p", &[], &["c/d"])).is_err());
        assert_eq!(load(&dir).get("p").unwrap().packages, ["a/b"]);
        assert_eq!(set.get("p").unwrap().packages, ["a/b"]);
    }

    #[test]
    fn rename_rolls_back_when_an_includer_cannot_be_written() {
        let tmp = TempDir::new().unwrap();
        let dir = tmp.path().join("profiles");
        let mut set = ProfileSet::new(dir.clone());
        set.create(profile("base", &[], &["a/b"])).unwrap();
        set.create(profile("top", &["base"], &[])).unwrap();

        // `top` cannot be replaced: a directory sits at its path. The new `core` file
        // is written first, so it must be removed again.
        fs::remove_file(dir.join("top.yml")).unwrap();
        fs::create_dir(dir.join("top.yml")).unwrap();

        assert!(set.rename("base", "core").is_err());
        assert!(dir.join("base.yml").exists());
        assert!(!dir.join("core.yml").exists());
        assert!(set.get("base").is_some());
        assert!(set.get("core").is_none());
        assert_eq!(set.get("top").unwrap().includes, ["base"]);
    }

    #[test]
    fn rewrite_rolls_back_profiles_already_written() {
        let tmp = TempDir::new().unwrap();
        let dir = tmp.path().join("profiles");
        let mut set = ProfileSet::new(dir.clone());
        set.create(profile("a", &[], &["home/git", "home/shell"]))
            .unwrap();
        set.create(profile("b", &[], &["home/git"])).unwrap();

        // `b` is rewritten after `a`; blocking it makes `a`'s rewrite the one to undo.
        fs::remove_file(dir.join("b.yml")).unwrap();
        fs::create_dir(dir.join("b.yml")).unwrap();

        assert!(set.remove_package("home/git").is_err());
        assert_eq!(
            load(&dir).get("a").unwrap().packages,
            ["home/git", "home/shell"]
        );
        assert_eq!(set.get("a").unwrap().packages, ["home/git", "home/shell"]);
        assert_eq!(set.get("b").unwrap().packages, ["home/git"]);
    }

    #[cfg(unix)]
    #[test]
    fn failed_write_in_read_only_folder_changes_nothing() {
        use std::os::unix::fs::PermissionsExt;

        let tmp = TempDir::new().unwrap();
        let dir = tmp.path().join("profiles");
        let mut set = ProfileSet::new(dir.clone());
        set.create(profile("base", &[], &["a/b"])).unwrap();
        set.create(profile("top", &["base"], &["a/b"])).unwrap();

        fs::set_permissions(&dir, fs::Permissions::from_mode(0o555)).unwrap();
        let probe = dir.join(".probe");
        if fs::write(&probe, "").is_ok() {
            // Root ignores directory permissions, so there is no failure to provoke.
            fs::remove_file(&probe).unwrap();
            fs::set_permissions(&dir, fs::Permissions::from_mode(0o755)).unwrap();
            return;
        }
        let renamed = set.rename("base", "core");
        let removed = set.remove_package("a/b");
        fs::set_permissions(&dir, fs::Permissions::from_mode(0o755)).unwrap();

        assert!(renamed.is_err());
        assert!(removed.is_err());
        assert!(dir.join("base.yml").exists());
        assert!(!dir.join("core.yml").exists());
        assert_eq!(load(&dir).get("top").unwrap().includes, ["base"]);
        assert_eq!(load(&dir).get("base").unwrap().packages, ["a/b"]);
        assert_eq!(set.get("base").unwrap().packages, ["a/b"]);
        assert_eq!(set.get("top").unwrap().includes, ["base"]);
        assert!(set.get("core").is_none());
    }
}
