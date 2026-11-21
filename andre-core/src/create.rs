//! Package creation: an empty package, or a package made from live files. Live files
//! are moved into the package and linked back, which is what `adopt` did. This module
//! generalises `adopt`: a new package or an existing one, optional ignore files, and
//! optional profile membership.
//!
//! Flow: [`plan_package`] checks everything and touches no disk. [`execute_package`]
//! then runs the steps in order and logs each one. If a step fails, the log is undone
//! in reverse, so a failed run leaves the live files where they were.
//!
//! Known limit: a move uses `rename`, so the package folder must be on the same
//! filesystem as the target folder (as `adopt` already required).

use std::collections::BTreeSet;
use std::fs;
use std::io::{self, Write};
use std::path::{Component, Path, PathBuf};

use crate::engine::{EngineKind, LinkEngine, LinkOp, NativeEngine, StowEngine};
use crate::error::{Error, Result};
use crate::ignore::{IgnoreFile, IgnoreRules, RuleSource};
use crate::profile::{Profile, ProfileError, ProfileSet};
use crate::state::{StowAction, StowFlags};
use crate::stow::{collect_package_files, ignored_by_config};

/// What to create. `source_root` holds the packages, `target_root` is where live files sit.
#[derive(Debug, Clone, Default)]
pub struct NewPackage {
    /// The profile ref the package is written as: `nvim` for a flat config, `home/nvim`
    /// for an interim `groups:` config. Must end in `package`; `plan_package` refuses others.
    pub package_ref: String,
    pub source_root: PathBuf,
    pub target_root: PathBuf,
    pub package: String,
    /// Allow adding to a package that already exists. When false, an existing package
    /// is an error.
    pub allow_existing: bool,
    /// Live files or folders under `target_root` to move into the package and link
    /// back. Empty creates an empty package and links nothing.
    pub files: Vec<PathBuf>,
    /// Write `.gitignore` from the secrets template.
    pub gitignore: bool,
    /// Write `.stow-local-ignore` with GNU's built-in list. The file replaces the
    /// built-in list, so the template copies it in.
    pub stow_local_ignore: bool,
    /// Profiles to add the package to. Each gets `package_ref`.
    pub profiles: Vec<String>,
    /// Engine for the link-back step.
    pub engine: EngineKind,
    /// Pass `--no-folding` to GNU stow for the link-back (the native engine never folds).
    pub no_folding: bool,
    /// Config ignores (substring match), applied to the link-back.
    pub ignores: Vec<String>,
    /// The `.stow-global-ignore` to read when the package has no list of its own. The
    /// caller resolves it (see [`crate::ignore::global_ignore_in`]); `None` means none.
    pub global_ignore: Option<PathBuf>,
}

/// One live path moved into the package. `to` is the matching path inside the package.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileMove {
    pub from: PathBuf,
    pub to: PathBuf,
}

/// A profile that will gain the package. `already_member` edits are listed but skipped.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProfileEdit {
    pub profile: String,
    pub package_ref: String,
    pub already_member: bool,
}

/// What [`execute_package`] will do. Built by [`plan_package`] without touching disk.
#[derive(Debug, Clone)]
pub struct PackagePlan {
    /// The profile ref the package is written as (see [`NewPackage::package_ref`]).
    pub package_ref: String,
    pub package: String,
    pub package_dir: PathBuf,
    /// The package folder already exists. Its files stay; the new ones are added.
    pub package_exists: bool,
    /// Folders to create, parents first.
    pub dirs_to_create: Vec<PathBuf>,
    /// Live paths moved into the package.
    pub moves: Vec<FileMove>,
    /// Live paths that will not be linked back because the package's ignore rules
    /// exclude them. They are moved into the package and vanish from the target.
    pub unlinked: Vec<PathBuf>,
    /// Live paths that will become links back into the package.
    pub links: Vec<PathBuf>,
    pub profile_edits: Vec<ProfileEdit>,
    pub engine: EngineKind,
    no_folding: bool,
    ignore_writes: Vec<(PathBuf, String)>,
    source_root: PathBuf,
    target_root: PathBuf,
    ignores: Vec<String>,
    global_ignore: Option<PathBuf>,
}

impl PackagePlan {
    /// Ignore files that will be created, in the package folder.
    pub fn ignore_files(&self) -> Vec<&Path> {
        self.ignore_writes
            .iter()
            .map(|(p, _)| p.as_path())
            .collect()
    }
}

/// Package name rule: starts with a letter or digit, then letters, digits, `-`, `_` or `.`.
pub fn is_valid_package_name(name: &str) -> bool {
    let mut chars = name.chars();
    matches!(chars.next(), Some(c) if c.is_ascii_alphanumeric())
        && chars.all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.'))
}

/// A profile ref for `package`: the bare name, or `<group>/<name>` for an interim
/// `groups:` config. Anything else is refused, so a profile never names a path.
fn valid_package_ref(package_ref: &str, package: &str) -> bool {
    match package_ref.rsplit_once('/') {
        None => package_ref == package,
        Some((group, name)) => name == package && !group.is_empty() && !group.contains('/'),
    }
}

/// Checks the request and works out every change. Nothing on disk is touched.
pub fn plan_package(req: &NewPackage, profiles: &ProfileSet) -> Result<PackagePlan> {
    if !is_valid_package_name(&req.package) {
        return Err(Error::Package(format!(
            "invalid package name '{}': use letters, digits, '-', '_' and '.', starting with a letter or digit",
            req.package
        )));
    }
    if !valid_package_ref(&req.package_ref, &req.package) {
        return Err(Error::Package(format!(
            "invalid package ref '{}' for package '{}'",
            req.package_ref, req.package
        )));
    }
    if !req.source_root.is_dir() {
        return Err(Error::SourceNotFound(req.source_root.clone()));
    }

    let package_dir = req.source_root.join(&req.package);
    let package_exists = match fs::symlink_metadata(&package_dir) {
        Ok(meta) if !meta.is_dir() => {
            return Err(Error::Package(format!(
                "{} exists and is not a folder",
                package_dir.display()
            )));
        }
        Ok(_) if !req.allow_existing => {
            return Err(Error::Package(format!(
                "package '{}' already exists in {}",
                req.package,
                req.source_root.display()
            )));
        }
        Ok(_) => true,
        Err(_) => false,
    };

    let mut ignore_writes = Vec::new();
    for (wanted, kind) in [
        (req.stow_local_ignore, IgnoreFile::StowLocal),
        (req.gitignore, IgnoreFile::GitIgnore),
    ] {
        if !wanted {
            continue;
        }
        let path = kind.path_in(&package_dir);
        if fs::symlink_metadata(&path).is_ok() {
            return Err(Error::Package(format!(
                "{} already exists; not overwritten",
                path.display()
            )));
        }
        ignore_writes.push((path, kind.template()));
    }

    let profile_edits = plan_profile_edits(req, profiles)?;

    let mut sorted = req.files.clone();
    sorted.sort();
    if sorted.windows(2).any(|pair| pair[0] == pair[1]) {
        return Err(Error::Package("a file is listed twice".into()));
    }
    if sorted.iter().enumerate().any(|(i, a)| {
        sorted
            .iter()
            .enumerate()
            .any(|(j, b)| i != j && a.starts_with(b))
    }) {
        return Err(Error::Package(
            "one selected item is inside another; select only the outer one".into(),
        ));
    }

    let rules = rules_after(req, &package_dir)?;
    // GNU only runs when files are moved and linked back.
    if req.engine == EngineKind::Stow && !req.files.is_empty() {
        refuse_if_stow_links_more(req, &package_dir, &rules, &ignore_writes)?;
    }

    let mut dirs_to_create = Vec::new();
    if !package_exists {
        dirs_to_create.push(package_dir.clone());
    }
    let mut moves = Vec::new();
    let mut rels = Vec::new();
    for file in &req.files {
        let rel = plan_one_file(file, req, &package_dir, &mut dirs_to_create)?;
        moves.push(FileMove {
            from: file.clone(),
            to: package_dir.join(&rel),
        });
        rels.push(rel);
    }

    let mut unlinked = Vec::new();
    let mut links = Vec::new();
    for (movement, rel) in moves.iter().zip(&rels) {
        let before = unlinked.len();
        collect_excluded(&movement.from, rel, &rules, &req.ignores, &mut unlinked);
        if unlinked.len() == before {
            links.push(movement.from.clone());
        }
    }

    Ok(PackagePlan {
        package_ref: req.package_ref.clone(),
        package: req.package.clone(),
        package_dir,
        package_exists,
        dirs_to_create,
        moves,
        unlinked,
        links,
        profile_edits,
        engine: req.engine,
        no_folding: req.no_folding,
        ignore_writes,
        source_root: req.source_root.clone(),
        target_root: req.target_root.clone(),
        ignores: req.ignores.clone(),
        global_ignore: req.global_ignore.clone(),
    })
}

/// Runs the plan. On failure every completed step is undone (newest first) and the
/// original error is returned. If an undo step fails too, the error names the leftovers.
pub fn execute_package(plan: &PackagePlan, profiles: &mut ProfileSet) -> Result<()> {
    #[cfg(test)]
    {
        STEPS.with(|s| s.set(0));
    }

    let mut log = Vec::new();
    match apply(plan, profiles, &mut log) {
        Ok(()) => Ok(()),
        Err(cause) => {
            let problems = undo(&log, profiles);
            if problems.is_empty() {
                Err(cause)
            } else {
                Err(Error::Package(format!(
                    "{cause}; rollback incomplete: {}",
                    problems.join("; ")
                )))
            }
        }
    }
}

fn plan_profile_edits(req: &NewPackage, profiles: &ProfileSet) -> Result<Vec<ProfileEdit>> {
    let package_ref = &req.package_ref;
    let mut seen = BTreeSet::new();
    let mut edits = Vec::new();
    for name in &req.profiles {
        if !seen.insert(name.as_str()) {
            continue;
        }
        let profile = profiles
            .get(name)
            .ok_or_else(|| Error::Profile(ProfileError::NotFound(name.clone())))?;
        edits.push(ProfileEdit {
            profile: name.clone(),
            package_ref: package_ref.clone(),
            already_member: profile.packages.contains(package_ref),
        });
    }
    Ok(edits)
}

/// Checks one live path and returns its path relative to the target (and so inside
/// the package). Adds the folders the move needs to `dirs`.
fn plan_one_file(
    file: &Path,
    req: &NewPackage,
    package_dir: &Path,
    dirs: &mut Vec<PathBuf>,
) -> Result<PathBuf> {
    let rel = file.strip_prefix(&req.target_root).map_err(|_| {
        Error::Package(format!(
            "{} is not under {}",
            file.display(),
            req.target_root.display()
        ))
    })?;
    if rel.as_os_str().is_empty() || rel.components().any(|c| !matches!(c, Component::Normal(_))) {
        return Err(Error::Package(format!(
            "cannot move {}: select a file or folder inside {}",
            file.display(),
            req.target_root.display()
        )));
    }

    let meta = fs::symlink_metadata(file)
        .map_err(|_| Error::Package(format!("{} does not exist", file.display())))?;
    if meta.file_type().is_symlink() {
        return Err(Error::Package(format!(
            "{} is a symbolic link; only real files and folders can be moved",
            file.display()
        )));
    }
    if !meta.is_file() && !meta.is_dir() {
        return Err(Error::Package(format!(
            "{} is neither a file nor a folder",
            file.display()
        )));
    }
    if is_inside(file, &req.source_root) {
        return Err(Error::Package(format!(
            "{} is already inside the packages folder",
            file.display()
        )));
    }

    let mut acc = package_dir.to_path_buf();
    if let Some(parent) = rel.parent() {
        for part in parent.components() {
            acc.push(part);
            match fs::symlink_metadata(&acc) {
                Ok(m) if !m.is_dir() => {
                    return Err(Error::Package(format!(
                        "{} is a file where a folder is needed",
                        acc.display()
                    )));
                }
                Ok(_) => {}
                Err(_) => {
                    if !dirs.contains(&acc) {
                        dirs.push(acc.clone());
                    }
                }
            }
        }
    }

    let dest = package_dir.join(rel);
    if fs::symlink_metadata(&dest).is_ok() {
        return Err(Error::Package(format!(
            "{} already exists in the package",
            dest.display()
        )));
    }
    Ok(rel.to_path_buf())
}

/// True when `path` (not a symlink) resolves to a location inside `root`.
fn is_inside(path: &Path, root: &Path) -> bool {
    match (fs::canonicalize(path), fs::canonicalize(root)) {
        (Ok(p), Ok(r)) => p.starts_with(r),
        _ => false,
    }
}

/// Rules as they will stand once the package is written: the new `.stow-local-ignore`
/// if one is being created, otherwise what [`IgnoreRules::load`] finds.
fn rules_after(req: &NewPackage, package_dir: &Path) -> Result<IgnoreRules> {
    if req.stow_local_ignore {
        return IgnoreRules::from_text(
            &IgnoreFile::StowLocal.template(),
            RuleSource::Local,
            "the new .stow-local-ignore",
        );
    }
    IgnoreRules::load(package_dir, req.global_ignore.as_deref())
}

/// GNU stow links every file in the package, not only the moved ones. Refuses when it
/// would link a file this request does not move: an existing package file, or an ignore
/// file the request writes. A failed run undoes only the links under moved paths, so
/// such a link would be left behind. Files whose target path is already taken are not
/// counted: GNU checks for conflicts before it links anything.
fn refuse_if_stow_links_more(
    req: &NewPackage,
    package_dir: &Path,
    rules: &IgnoreRules,
    ignore_writes: &[(PathBuf, String)],
) -> Result<()> {
    let existing = collect_package_files(package_dir, rules)
        .into_iter()
        .filter_map(|file| file.strip_prefix(package_dir).ok().map(Path::to_path_buf));
    // The rules a written ignore file will be read under cannot be checked on disk yet,
    // so each one is checked against the same rules as the existing files.
    let written = ignore_writes
        .iter()
        .filter_map(|(path, _)| path.strip_prefix(package_dir).ok().map(Path::to_path_buf))
        .filter(|rel| !rules.excludes(rel));
    let extra = existing
        .chain(written)
        .filter(|rel| !ignored_by_config(rel, &req.ignores))
        .find(|rel| fs::symlink_metadata(req.target_root.join(rel)).is_err());
    match extra {
        None => Ok(()),
        Some(rel) => Err(Error::Package(format!(
            "GNU stow would also link {} (not one of the moved files); use engine: native to link only the moved files",
            rel.display()
        ))),
    }
}

/// Pushes every live path under `live` that `rules` or the config's `ignores` exclude.
/// `rel` is the path of `live` relative to the package. An excluded folder is reported
/// whole, not entered.
fn collect_excluded(
    live: &Path,
    rel: &Path,
    rules: &IgnoreRules,
    ignores: &[String],
    out: &mut Vec<PathBuf>,
) {
    if rules.excludes(rel) || ignored_by_config(rel, ignores) {
        out.push(live.to_path_buf());
        return;
    }
    let is_dir = fs::symlink_metadata(live).is_ok_and(|m| m.is_dir());
    if !is_dir {
        return;
    }
    let Ok(entries) = fs::read_dir(live) else {
        return;
    };
    let mut names: Vec<_> = entries.flatten().map(|e| e.file_name()).collect();
    names.sort();
    for name in names {
        collect_excluded(&live.join(&name), &rel.join(&name), rules, ignores, out);
    }
}

/// One logged step, enough to reverse it.
enum Undo {
    Dir(PathBuf),
    Move(FileMove),
    File(PathBuf),
    Profile(Profile),
}

fn apply(plan: &PackagePlan, profiles: &mut ProfileSet, log: &mut Vec<Undo>) -> Result<()> {
    for movement in &plan.moves {
        let gone = fs::symlink_metadata(&movement.from).is_err();
        let taken = fs::symlink_metadata(&movement.to).is_ok();
        if gone || taken {
            return Err(Error::Package(format!(
                "{} changed since the plan was made",
                movement.from.display()
            )));
        }
    }

    for dir in &plan.dirs_to_create {
        if fs::symlink_metadata(dir).is_ok() {
            continue;
        }
        step()?;
        fs::create_dir(dir)?;
        log.push(Undo::Dir(dir.clone()));
    }

    for movement in &plan.moves {
        step()?;
        fs::rename(&movement.from, &movement.to)?;
        log.push(Undo::Move(movement.clone()));
    }

    for (path, content) in &plan.ignore_writes {
        step()?;
        create_new(path, content)?;
        log.push(Undo::File(path.clone()));
    }

    if !plan.moves.is_empty() {
        step()?;
        let op = LinkOp {
            name: plan.package_ref.clone(),
            package: plan.package.clone(),
            source: plan.source_root.clone(),
            target: plan.target_root.clone(),
            flags: StowFlags {
                action: StowAction::Stow,
                no_folding: plan.no_folding,
                ..StowFlags::default()
            },
            ignores: plan.ignores.clone(),
            global_ignore: plan.global_ignore.clone(),
        };
        match plan.engine {
            EngineKind::Native => {
                // Link back only the moved paths. The rest of the package stays as it was.
                let moved: Vec<PathBuf> = plan
                    .moves
                    .iter()
                    .filter_map(|m| m.to.strip_prefix(&plan.package_dir).ok())
                    .map(Path::to_path_buf)
                    .collect();
                NativeEngine.apply_within(&op, &moved)?;
            }
            // plan_package refused any package file GNU would link beyond the moved ones.
            EngineKind::Stow => StowEngine.apply(&op)?,
        }
        // A second checkpoint after the links exist, so rollback of links is tested too.
        step()?;
    }

    for edit in &plan.profile_edits {
        if edit.already_member {
            continue;
        }
        step()?;
        let original = profiles
            .get(&edit.profile)
            .cloned()
            .ok_or_else(|| Error::Profile(ProfileError::NotFound(edit.profile.clone())))?;
        let mut updated = original.clone();
        updated.packages.push(edit.package_ref.clone());
        profiles.save(updated)?;
        log.push(Undo::Profile(original));
    }

    Ok(())
}

/// Undoes the log, newest first. Returns one message per step that could not be undone.
fn undo(log: &[Undo], profiles: &mut ProfileSet) -> Vec<String> {
    let mut problems = Vec::new();
    for entry in log.iter().rev() {
        let result = match entry {
            Undo::Profile(original) => profiles.save(original.clone()).map_err(|e| e.to_string()),
            Undo::Move(movement) => {
                restore_move(movement).map_err(|e| format!("{}: {e}", movement.from.display()))
            }
            Undo::File(path) => {
                fs::remove_file(path).map_err(|e| format!("{}: {e}", path.display()))
            }
            Undo::Dir(path) => fs::remove_dir(path).map_err(|e| format!("{}: {e}", path.display())),
        };
        if let Err(message) = result {
            problems.push(message);
        }
    }
    problems
}

/// Puts a moved path back. Refuses to overwrite anything that now sits at the original
/// path, since `rename` would replace it silently.
fn restore_move(movement: &FileMove) -> std::result::Result<(), String> {
    remove_links_into(&movement.from, &movement.to)?;
    if fs::symlink_metadata(&movement.from).is_ok() {
        return Err("something is at the original path now; left in the package".into());
    }
    fs::rename(&movement.to, &movement.from).map_err(|e| e.to_string())
}

/// Removes links under `path` that point into `dest`, and then the folders left empty.
/// Links are the only thing the engine adds under a moved path, so this leaves the
/// tree as it was before the move. Anything else stops the removal with an error.
fn remove_links_into(path: &Path, dest: &Path) -> std::result::Result<(), String> {
    let meta = match fs::symlink_metadata(path) {
        Ok(meta) => meta,
        Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(()),
        Err(e) => return Err(e.to_string()),
    };
    if meta.file_type().is_symlink() {
        if points_into(path, dest) {
            fs::remove_file(path).map_err(|e| e.to_string())?;
        }
        return Ok(());
    }
    if meta.is_dir() {
        let entries = fs::read_dir(path).map_err(|e| e.to_string())?;
        for entry in entries {
            let entry = entry.map_err(|e| e.to_string())?;
            remove_links_into(&entry.path(), dest)?;
        }
        fs::remove_dir(path).map_err(|e| e.to_string())?;
    }
    Ok(())
}

fn points_into(link: &Path, dest: &Path) -> bool {
    match (fs::canonicalize(link), fs::canonicalize(dest)) {
        (Ok(l), Ok(d)) => l.starts_with(d),
        _ => false,
    }
}

fn create_new(path: &Path, content: &str) -> Result<()> {
    let mut file = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)?;
    file.write_all(content.as_bytes())?;
    Ok(())
}

// Fault injection for rollback tests: the Nth step of an execute call fails.
#[cfg(test)]
thread_local! {
    static FAIL_STEP: std::cell::Cell<Option<usize>> = const { std::cell::Cell::new(None) };
    static STEPS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

/// Marks the start of one mutating step. Under test, fails the configured step.
fn step() -> Result<()> {
    #[cfg(test)]
    {
        let n = STEPS.with(|s| {
            let n = s.get();
            s.set(n + 1);
            n
        });
        if FAIL_STEP.with(|f| f.get()) == Some(n) {
            return Err(Error::Package(format!("injected failure at step {n}")));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    /// `root/packages` (source), `root/home` (target), one profile `base` in `root/profiles`.
    struct Fixture {
        _tmp: TempDir,
        root: PathBuf,
        source: PathBuf,
        target: PathBuf,
        profiles: ProfileSet,
    }

    impl Fixture {
        fn new() -> Self {
            let tmp = TempDir::new().unwrap();
            let root = tmp.path().to_path_buf();
            let source = root.join("packages").join("home");
            let target = root.join("home");
            fs::create_dir_all(&source).unwrap();
            fs::create_dir_all(&target).unwrap();
            let profiles_dir = root.join("profiles");
            fs::create_dir_all(&profiles_dir).unwrap();
            let mut profiles = ProfileSet::load(&profiles_dir).unwrap();
            profiles
                .create(Profile {
                    name: "base".into(),
                    packages: vec!["other/pkg".into()],
                    ..Profile::default()
                })
                .unwrap();
            Self {
                _tmp: tmp,
                root,
                source,
                target,
                profiles,
            }
        }

        fn req(&self, package: &str) -> NewPackage {
            NewPackage {
                package_ref: format!("home/{package}"),
                source_root: self.source.clone(),
                target_root: self.target.clone(),
                package: package.into(),
                engine: EngineKind::Native,
                ..NewPackage::default()
            }
        }

        fn live(&self, rel: &str, content: &str) -> PathBuf {
            let path = self.target.join(rel);
            fs::create_dir_all(path.parent().unwrap()).unwrap();
            fs::write(&path, content).unwrap();
            path
        }

        fn profile_file(&self, name: &str) -> String {
            fs::read_to_string(self.profiles.file_path(name)).unwrap()
        }

        /// Everything under `target` and `source`, as sorted relative paths with kind.
        fn snapshot(&self) -> Vec<String> {
            let mut out = Vec::new();
            for (label, dir) in [("target", &self.target), ("source", &self.source)] {
                walk_snapshot(dir, dir, label, &mut out);
            }
            out.sort();
            out
        }
    }

    fn walk_snapshot(root: &Path, dir: &Path, label: &str, out: &mut Vec<String>) {
        let Ok(entries) = fs::read_dir(dir) else {
            return;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            let rel = path.strip_prefix(root).unwrap().display().to_string();
            let meta = fs::symlink_metadata(&path).unwrap();
            let kind = if meta.file_type().is_symlink() {
                "link"
            } else if meta.is_dir() {
                "dir"
            } else {
                "file"
            };
            out.push(format!("{label}:{rel}:{kind}"));
            if meta.is_dir() && !meta.file_type().is_symlink() {
                walk_snapshot(root, &path, label, out);
            }
        }
    }

    #[test]
    fn package_name_rules() {
        assert!(is_valid_package_name("nvim"));
        assert!(is_valid_package_name("bash-env_2.d"));
        assert!(!is_valid_package_name(""));
        assert!(!is_valid_package_name(".hidden"));
        assert!(!is_valid_package_name("-dash"));
        assert!(!is_valid_package_name("a/b"));
        assert!(!is_valid_package_name("a b"));
    }

    #[test]
    fn empty_package_is_created_with_no_links() {
        let mut fx = Fixture::new();
        let req = fx.req("shell");
        let plan = plan_package(&req, &fx.profiles).unwrap();
        assert!(!plan.package_exists);
        assert!(plan.moves.is_empty());
        assert!(plan.links.is_empty());
        execute_package(&plan, &mut fx.profiles).unwrap();
        assert!(fx.source.join("shell").is_dir());
        assert_eq!(fs::read_dir(fx.source.join("shell")).unwrap().count(), 0);
    }

    #[test]
    fn live_files_move_into_new_package_and_link_back() {
        let mut fx = Fixture::new();
        let rc = fx.live(".bashrc", "export A=1\n");
        let nested = fx.live(".config/nvim/init.lua", "set nu\n");
        let mut req = fx.req("shell");
        req.files = vec![rc.clone(), nested.clone()];
        let plan = plan_package(&req, &fx.profiles).unwrap();
        assert_eq!(plan.moves.len(), 2);
        assert_eq!(plan.links.len(), 2);
        assert!(plan.unlinked.is_empty());

        execute_package(&plan, &mut fx.profiles).unwrap();

        let pkg = fx.source.join("shell");
        assert_eq!(
            fs::read_to_string(pkg.join(".bashrc")).unwrap(),
            "export A=1\n"
        );
        assert_eq!(
            fs::read_to_string(pkg.join(".config/nvim/init.lua")).unwrap(),
            "set nu\n"
        );
        assert!(rc.is_symlink(), "original path is a link back");
        assert_eq!(fs::read_to_string(&rc).unwrap(), "export A=1\n");
        assert!(nested.is_symlink());
    }

    #[test]
    fn folder_moves_whole_and_links_inside() {
        let mut fx = Fixture::new();
        fx.live("dir/a.txt", "a");
        fx.live("dir/sub/b.txt", "b");
        let mut req = fx.req("dirpkg");
        req.files = vec![fx.target.join("dir")];
        let plan = plan_package(&req, &fx.profiles).unwrap();
        execute_package(&plan, &mut fx.profiles).unwrap();
        assert!(fx.source.join("dirpkg/dir/sub/b.txt").is_file());
        assert!(fx.target.join("dir/a.txt").is_symlink());
        assert!(fx.target.join("dir/sub/b.txt").is_symlink());
    }

    #[test]
    fn adds_files_to_existing_package_only_when_allowed() {
        let mut fx = Fixture::new();
        fs::create_dir_all(fx.source.join("shell")).unwrap();
        fs::write(fx.source.join("shell/.profile"), "p").unwrap();
        let rc = fx.live(".bashrc", "rc");

        let mut refused = fx.req("shell");
        refused.files = vec![rc.clone()];
        let err = plan_package(&refused, &fx.profiles).unwrap_err();
        assert!(err.to_string().contains("already exists"), "{err}");

        let mut req = fx.req("shell");
        req.allow_existing = true;
        req.files = vec![rc.clone()];
        let plan = plan_package(&req, &fx.profiles).unwrap();
        assert!(plan.package_exists);
        assert!(plan.dirs_to_create.is_empty());
        execute_package(&plan, &mut fx.profiles).unwrap();
        assert!(
            fx.source.join("shell/.profile").is_file(),
            "existing files stay"
        );
        assert!(rc.is_symlink());
    }

    #[test]
    fn gitignore_and_stow_local_ignore_templates_are_written() {
        let mut fx = Fixture::new();
        let mut req = fx.req("secrets-pkg");
        req.gitignore = true;
        req.stow_local_ignore = true;
        let plan = plan_package(&req, &fx.profiles).unwrap();
        assert_eq!(plan.ignore_files().len(), 2);
        execute_package(&plan, &mut fx.profiles).unwrap();
        let pkg = fx.source.join("secrets-pkg");
        let gitignore = fs::read_to_string(pkg.join(".gitignore")).unwrap();
        assert!(gitignore.contains("*.key"));
        let local = fs::read_to_string(pkg.join(".stow-local-ignore")).unwrap();
        assert!(local.contains(r"\.gitignore"), "built-in defaults kept");
    }

    #[test]
    fn ignored_files_are_moved_but_reported_as_not_linked() {
        let mut fx = Fixture::new();
        let env_file = fx.live(".env", "TOKEN=1");
        let rc = fx.live(".bashrc", "rc");
        // `.gitignore` only keeps files out of git; stow's list decides what is linked.
        fs::create_dir_all(fx.source.join("shell")).unwrap();
        fs::write(fx.source.join("shell/.stow-local-ignore"), "\\.env\n").unwrap();
        let mut req = fx.req("shell");
        req.allow_existing = true;
        req.files = vec![env_file.clone(), rc.clone()];
        let plan = plan_package(&req, &fx.profiles).unwrap();
        assert_eq!(plan.unlinked, vec![env_file.clone()]);
        assert_eq!(plan.links, vec![rc.clone()]);
        execute_package(&plan, &mut fx.profiles).unwrap();
        assert!(!env_file.exists(), "moved, so not left in the target");
        assert!(fx.source.join("shell/.env").is_file());
        assert!(rc.is_symlink());
    }

    #[test]
    fn profile_membership_is_added_and_existing_member_is_noop() {
        let mut fx = Fixture::new();
        let mut req = fx.req("shell");
        req.profiles = vec!["base".into(), "base".into()];
        let plan = plan_package(&req, &fx.profiles).unwrap();
        assert_eq!(plan.profile_edits.len(), 1, "duplicates collapse");
        assert!(!plan.profile_edits[0].already_member);
        execute_package(&plan, &mut fx.profiles).unwrap();
        let saved = fx.profiles.get("base").unwrap();
        assert_eq!(saved.packages, vec!["other/pkg", "home/shell"]);
        assert!(fx.profile_file("base").contains("home/shell"));

        req.allow_existing = true;
        let again = plan_package(&req, &fx.profiles).unwrap();
        assert!(again.profile_edits[0].already_member);
        execute_package(&again, &mut fx.profiles).unwrap();
        assert_eq!(fx.profiles.get("base").unwrap().packages.len(), 2);
    }

    #[test]
    fn flat_package_ref_is_the_profile_ref() {
        let mut fx = Fixture::new();
        let rc = fx.live(".bashrc", "rc");
        let mut req = fx.req("shell");
        req.package_ref = "shell".into();
        req.files = vec![rc.clone()];
        req.profiles = vec!["base".into()];
        let plan = plan_package(&req, &fx.profiles).unwrap();
        assert_eq!(plan.profile_edits[0].package_ref, "shell");
        execute_package(&plan, &mut fx.profiles).unwrap();
        assert_eq!(
            fx.profiles.get("base").unwrap().packages,
            vec!["other/pkg", "shell"]
        );
        assert!(rc.is_symlink());
    }

    #[test]
    fn valid_package_ref_is_a_name_or_one_group_then_the_name() {
        assert!(valid_package_ref("nvim", "nvim"));
        assert!(valid_package_ref("home/nvim", "nvim"));
        assert!(!valid_package_ref("", "nvim"));
        assert!(!valid_package_ref("/nvim", "nvim"));
        assert!(!valid_package_ref("a/b/nvim", "nvim"));
        assert!(!valid_package_ref("home/git", "nvim"));
        assert!(!valid_package_ref("nvim", "git"));
    }

    #[test]
    fn unknown_profile_is_refused_before_any_change() {
        let fx = Fixture::new();
        let rc = fx.live(".bashrc", "rc");
        let mut req = fx.req("shell");
        req.files = vec![rc.clone()];
        req.profiles = vec!["missing".into()];
        let err = plan_package(&req, &fx.profiles).unwrap_err();
        assert!(err.to_string().contains("'missing' not found"), "{err}");
        assert!(rc.is_file() && !rc.is_symlink());
        assert!(!fx.source.join("shell").exists());
    }

    #[test]
    fn validation_refuses_bad_requests_without_touching_disk() {
        let fx = Fixture::new();
        let rc = fx.live(".bashrc", "rc");
        fx.live(".config/x", "x");
        let outside = fx.root.join("outside.txt");
        fs::write(&outside, "x").unwrap();
        let link = fx.target.join(".link");
        std::os::unix::fs::symlink(&rc, &link).unwrap();
        fs::create_dir_all(fx.source.join("taken")).unwrap();
        fs::write(fx.source.join("taken-file"), "x").unwrap();
        let before = fx.snapshot();

        let cases: Vec<(&str, NewPackage)> = vec![
            (
                "bad name",
                NewPackage {
                    package: "a/b".into(),
                    package_ref: "home/a/b".into(),
                    ..fx.req("x")
                },
            ),
            (
                "taken",
                NewPackage {
                    package: "taken".into(),
                    package_ref: "home/taken".into(),
                    ..fx.req("x")
                },
            ),
            (
                "file path",
                NewPackage {
                    package: "taken-file".into(),
                    package_ref: "home/taken-file".into(),
                    ..fx.req("x")
                },
            ),
            (
                "outside target",
                NewPackage {
                    files: vec![outside.clone()],
                    ..fx.req("p")
                },
            ),
            (
                "symlink",
                NewPackage {
                    files: vec![link.clone()],
                    ..fx.req("p")
                },
            ),
            (
                "missing file",
                NewPackage {
                    files: vec![fx.target.join("nope")],
                    ..fx.req("p")
                },
            ),
            (
                "target itself",
                NewPackage {
                    files: vec![fx.target.clone()],
                    ..fx.req("p")
                },
            ),
            (
                "duplicate",
                NewPackage {
                    files: vec![rc.clone(), rc.clone()],
                    ..fx.req("p")
                },
            ),
            (
                "nested selection",
                NewPackage {
                    files: vec![
                        fx.target.clone().join(".config"),
                        fx.target.join(".config/x"),
                    ],
                    ..fx.req("p")
                },
            ),
            (
                "bad package ref",
                NewPackage {
                    package_ref: "a/b/p".into(),
                    ..fx.req("p")
                },
            ),
            (
                "ref not ending in the package",
                NewPackage {
                    package_ref: "home/other".into(),
                    ..fx.req("p")
                },
            ),
        ];
        for (label, req) in cases {
            assert!(
                plan_package(&req, &fx.profiles).is_err(),
                "{label} should be refused"
            );
        }
        assert_eq!(fx.snapshot(), before, "validation must not change disk");
    }

    #[test]
    fn refuses_files_inside_the_packages_folder() {
        let tmp = TempDir::new().unwrap();
        let packages = tmp.path().join("packages");
        let source = packages.join("home");
        fs::create_dir_all(&source).unwrap();
        fs::write(source.join("tracked"), "x").unwrap();
        let profiles = ProfileSet::new(tmp.path().join("profiles"));
        // The target folder contains the packages folder, so the file is under both.
        let req = NewPackage {
            package_ref: "home/new".into(),
            source_root: source.clone(),
            target_root: packages,
            package: "new".into(),
            files: vec![source.join("tracked")],
            ..NewPackage::default()
        };
        let err = plan_package(&req, &profiles).unwrap_err();
        assert!(
            err.to_string().contains("inside the packages folder"),
            "{err}"
        );
    }

    #[test]
    fn existing_ignore_file_is_never_overwritten() {
        let fx = Fixture::new();
        fs::create_dir_all(fx.source.join("pkg")).unwrap();
        fs::write(fx.source.join("pkg/.gitignore"), "mine\n").unwrap();
        let mut req = fx.req("pkg");
        req.allow_existing = true;
        req.gitignore = true;
        let err = plan_package(&req, &fx.profiles).unwrap_err();
        assert!(err.to_string().contains("not overwritten"), "{err}");
    }

    #[test]
    fn file_where_a_folder_is_needed_is_refused() {
        let fx = Fixture::new();
        let file = fx.live(".config/nvim/init.lua", "x");
        fs::create_dir_all(fx.source.join("pkg/.config")).unwrap();
        fs::write(fx.source.join("pkg/.config/nvim"), "not a dir").unwrap();
        let mut req = fx.req("pkg");
        req.allow_existing = true;
        req.files = vec![file];
        let err = plan_package(&req, &fx.profiles).unwrap_err();
        assert!(
            err.to_string().contains("where a folder is needed"),
            "{err}"
        );
    }

    /// Runs the same request once per failure point. Every run must leave the disk and
    /// the profile files exactly as they were before the call.
    #[test]
    fn rollback_restores_everything_at_every_failure_point() {
        let mut total_steps = None;
        for fail_at in 0..32 {
            let mut fx = Fixture::new();
            let rc = fx.live(".bashrc", "rc");
            let nested = fx.live(".config/nvim/init.lua", "nv");
            let mut req = fx.req("shell");
            req.files = vec![rc.clone(), nested.clone()];
            req.gitignore = true;
            req.stow_local_ignore = true;
            req.profiles = vec!["base".into()];
            let plan = plan_package(&req, &fx.profiles).unwrap();

            let before = fx.snapshot();
            let profile_before = fx.profile_file("base");
            FAIL_STEP.with(|f| f.set(Some(fail_at)));
            let result = execute_package(&plan, &mut fx.profiles);
            FAIL_STEP.with(|f| f.set(None));
            let steps = STEPS.with(|s| s.get());

            if fail_at >= steps {
                // The fault never fired: the run completed.
                result.unwrap();
                total_steps = Some(steps);
                break;
            }
            let err = result.unwrap_err();
            assert!(
                err.to_string().contains("injected failure"),
                "step {fail_at}: {err}"
            );
            assert_eq!(
                fx.snapshot(),
                before,
                "step {fail_at}: disk not restored after failure"
            );
            assert_eq!(
                fx.profile_file("base"),
                profile_before,
                "step {fail_at}: profile file not restored"
            );
            assert_eq!(fx.profiles.get("base").unwrap().packages, vec!["other/pkg"]);
            assert!(rc.is_file() && !rc.is_symlink(), "step {fail_at}");
        }
        assert!(
            total_steps.is_some(),
            "the run must complete within 32 steps"
        );
        assert!(
            total_steps.unwrap() >= 8,
            "expected several steps, got {total_steps:?}"
        );
    }

    #[test]
    fn undo_never_overwrites_something_now_at_the_original_path() {
        let fx = Fixture::new();
        let to = fx.source.join("pkg/.bashrc");
        fs::create_dir_all(to.parent().unwrap()).unwrap();
        fs::write(&to, "kept").unwrap();
        let from = fx.target.join(".bashrc");
        fs::write(&from, "new").unwrap();
        let movement = FileMove {
            from: from.clone(),
            to: to.clone(),
        };
        assert!(restore_move(&movement).is_err());
        assert_eq!(fs::read_to_string(&from).unwrap(), "new");
        assert_eq!(fs::read_to_string(&to).unwrap(), "kept");
    }

    /// The package already holds `a.conf` (not stowed) and `z.conf`, and `~/z.conf` is a
    /// real file. The request moves `~/m.conf` in. A run that fails after its link step
    /// must leave the target as it was: no link for `a.conf`, `~/z.conf` untouched.
    #[test]
    fn failed_run_into_existing_package_leaves_target_unchanged() {
        let mut fx = Fixture::new();
        fs::create_dir_all(fx.source.join("shell")).unwrap();
        fs::write(fx.source.join("shell/a.conf"), "a").unwrap();
        fs::write(fx.source.join("shell/z.conf"), "z").unwrap();
        fx.live("z.conf", "real file");
        let m = fx.live("m.conf", "m");
        let mut req = fx.req("shell");
        req.allow_existing = true;
        req.files = vec![m.clone()];
        let plan = plan_package(&req, &fx.profiles).unwrap();

        let before = fx.snapshot();
        // Steps: 0 move, 1 before linking, 2 after linking. Fail after the links exist.
        FAIL_STEP.with(|f| f.set(Some(2)));
        let result = execute_package(&plan, &mut fx.profiles);
        FAIL_STEP.with(|f| f.set(None));
        assert!(result.is_err());
        assert_eq!(fx.snapshot(), before, "target and package restored");
        assert!(!fx.target.join("a.conf").exists());
        assert!(m.is_file() && !m.is_symlink());
    }

    #[test]
    fn existing_unstowed_package_files_are_not_linked() {
        let mut fx = Fixture::new();
        fs::create_dir_all(fx.source.join("shell")).unwrap();
        fs::write(fx.source.join("shell/a.conf"), "a").unwrap();
        fs::write(fx.source.join("shell/z.conf"), "z").unwrap();
        let z = fx.live("z.conf", "real file");
        let m = fx.live("m.conf", "m");
        let mut req = fx.req("shell");
        req.allow_existing = true;
        req.files = vec![m.clone()];
        let plan = plan_package(&req, &fx.profiles).unwrap();
        assert_eq!(plan.links, vec![m.clone()], "only the moved file is linked");

        execute_package(&plan, &mut fx.profiles).unwrap();
        assert!(m.is_symlink());
        assert!(
            !fx.target.join("a.conf").exists(),
            "unstowed file stays unlinked"
        );
        assert!(z.is_file() && !z.is_symlink(), "real target file untouched");
    }

    #[test]
    fn config_ignore_is_previewed_and_applied_the_same_way() {
        let mut fx = Fixture::new();
        let env_local = fx.live(".env.local", "TOKEN=1");
        let mut req = fx.req("shell");
        req.ignores = vec![".env".into()];
        req.files = vec![env_local.clone()];
        let plan = plan_package(&req, &fx.profiles).unwrap();
        assert_eq!(plan.unlinked, vec![env_local.clone()]);
        assert!(plan.links.is_empty());

        execute_package(&plan, &mut fx.profiles).unwrap();
        assert!(fx.source.join("shell/.env.local").is_file());
        assert!(!env_local.exists(), "not linked back, as the plan said");
    }

    #[test]
    fn global_list_comes_from_the_request() {
        let fx = Fixture::new();
        fs::write(fx.root.join(".stow-global-ignore"), "secret\n").unwrap();
        let secret = fx.live("secret", "s");
        let mut req = fx.req("shell");
        req.files = vec![secret.clone()];
        let plan = plan_package(&req, &fx.profiles).unwrap();
        assert_eq!(plan.links, vec![secret.clone()], "no global list given");

        req.global_ignore = Some(fx.root.join(".stow-global-ignore"));
        let plan = plan_package(&req, &fx.profiles).unwrap();
        assert_eq!(plan.unlinked, vec![secret]);
    }

    #[test]
    fn stow_engine_refuses_when_it_would_link_existing_files() {
        let fx = Fixture::new();
        fs::create_dir_all(fx.source.join("shell")).unwrap();
        fs::write(fx.source.join("shell/a.conf"), "a").unwrap();
        let m = fx.live("m.conf", "m");
        let mut req = fx.req("shell");
        req.allow_existing = true;
        req.files = vec![m];
        req.engine = EngineKind::Stow;
        let err = plan_package(&req, &fx.profiles).unwrap_err();
        assert!(err.to_string().contains("a.conf"), "{err}");
        assert!(err.to_string().contains("engine: native"), "{err}");

        // Once a.conf is already linked, GNU has nothing new to do for it.
        std::os::unix::fs::symlink(fx.source.join("shell/a.conf"), fx.target.join("a.conf"))
            .unwrap();
        plan_package(&req, &fx.profiles).unwrap();
    }

    #[test]
    fn stow_engine_refuses_an_ignore_file_the_global_list_would_link() {
        let mut fx = Fixture::new();
        // The global list does not exclude .gitignore, so GNU would link the one written.
        let global = fx.root.join(".stow-global-ignore");
        fs::write(&global, "secret\n").unwrap();
        let m = fx.live("m.conf", "m");
        let mut req = fx.req("shell");
        req.files = vec![m.clone()];
        req.gitignore = true;
        req.global_ignore = Some(global);
        req.engine = EngineKind::Stow;
        let err = plan_package(&req, &fx.profiles).unwrap_err();
        assert!(err.to_string().contains(".gitignore"), "{err}");
        assert!(err.to_string().contains("engine: native"), "{err}");

        // Native links only the moved file and leaves the written .gitignore unlinked.
        req.engine = EngineKind::Native;
        let plan = plan_package(&req, &fx.profiles).unwrap();
        assert_eq!(plan.links, vec![m]);
        execute_package(&plan, &mut fx.profiles).unwrap();
        assert!(fx.source.join("shell/.gitignore").is_file());
        assert!(!fx.target.join(".gitignore").exists());
    }

    #[test]
    fn stow_engine_accepts_an_ignore_file_the_rules_exclude() {
        let fx = Fixture::new();
        // The built-in list excludes .gitignore, so GNU does not link the one written.
        let m = fx.live("m.conf", "m");
        let mut req = fx.req("shell");
        req.files = vec![m];
        req.gitignore = true;
        req.engine = EngineKind::Stow;
        plan_package(&req, &fx.profiles).unwrap();
    }

    #[test]
    fn stow_engine_accepts_an_empty_package_with_an_ignore_file() {
        let mut fx = Fixture::new();
        // GNU never runs without moved files, so the global list cannot refuse it.
        let global = fx.root.join(".stow-global-ignore");
        fs::write(&global, "secret\n").unwrap();
        let mut req = fx.req("fresh");
        req.gitignore = true;
        req.global_ignore = Some(global);
        req.engine = EngineKind::Stow;
        let plan = plan_package(&req, &fx.profiles).unwrap();
        assert!(plan.links.is_empty());
        execute_package(&plan, &mut fx.profiles).unwrap();
        assert!(fx.source.join("fresh/.gitignore").is_file());
        assert!(!fx.target.join(".gitignore").exists());
    }

    #[test]
    fn full_run_after_rollback_succeeds() {
        let mut fx = Fixture::new();
        let rc = fx.live(".bashrc", "rc");
        let mut req = fx.req("shell");
        req.files = vec![rc.clone()];
        let plan = plan_package(&req, &fx.profiles).unwrap();
        FAIL_STEP.with(|f| f.set(Some(1)));
        assert!(execute_package(&plan, &mut fx.profiles).is_err());
        FAIL_STEP.with(|f| f.set(None));
        execute_package(&plan, &mut fx.profiles).unwrap();
        assert!(rc.is_symlink());
    }
}
