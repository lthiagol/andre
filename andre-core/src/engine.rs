//! Link engine abstraction (M07): one seam for symlink semantics.
//!
//! `LinkEngine` is implemented by `NativeEngine` (default; pure-Rust file-level
//! symlinks, no GNU stow) and `StowEngine` (shells out to GNU stow). The TUI and
//! yolo paths build a `LinkOp` per package and call `apply`/`plan`; they never
//! construct a `stow` command directly.

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use crate::error::{Error, Result};
use crate::state::StowAction;
use crate::stow::{collect_package_files, execute_stow};

/// A single package link operation. All fields the engines need to plan/apply.
#[derive(Debug, Clone)]
pub struct LinkOp {
    pub group: String,
    pub package: String,
    pub source: PathBuf,
    pub target: PathBuf,
    pub action: StowAction,
    pub dry_run: bool,
    pub no_folding: bool,
    pub adopt: bool,
    pub dotfiles: bool,
    pub verbosity: u8,
    pub ignores: Vec<String>,
}

/// A planned file-level operation (dry-run preview). Producing this must not
/// touch the filesystem.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LinkAction {
    CreateSymlink { link: PathBuf, source_file: PathBuf },
    RemoveSymlink { path: PathBuf },
}

/// Link engine abstraction. Methods are synchronous; async callers wrap heavy
/// work in `spawn_blocking`.
pub trait LinkEngine: Send + Sync {
    fn name(&self) -> &'static str;
    /// Apply the operation. When `op.dry_run` is set, engines plan without
    /// writing (preserving the product's dry-run = no-side-effects contract).
    fn apply(&self, op: &LinkOp) -> Result<()>;
    /// Plan the file-level operations without touching the filesystem.
    /// The stow engine cannot produce a structured plan and returns an empty vec.
    fn plan(&self, op: &LinkOp) -> Result<Vec<LinkAction>>;
}

/// Select an engine by config string. Unrecognized values fall back to native
/// (`Config::validate` emits a warning for unknown engines).
pub fn engine_for(engine: &str) -> Arc<dyn LinkEngine> {
    if engine.eq_ignore_ascii_case("stow") {
        Arc::new(StowEngine)
    } else {
        Arc::new(NativeEngine)
    }
}

// ---------------------------------------------------------------------------
// StowEngine — wraps the existing GNU stow shell-out.
// ---------------------------------------------------------------------------

pub struct StowEngine;

impl LinkEngine for StowEngine {
    fn name(&self) -> &'static str {
        "stow"
    }

    fn apply(&self, op: &LinkOp) -> Result<()> {
        if op.dry_run {
            // Preserve the product contract: dry-run never touches the filesystem
            // (historically the TUI simulated success without invoking stow -n).
            return Ok(());
        }
        execute_stow(
            &op.source,
            &op.target,
            &op.package,
            op.action.as_str(),
            op.verbosity,
            op.dry_run,
            op.no_folding,
            op.adopt,
            op.dotfiles,
            &op.ignores,
        )?;
        Ok(())
    }

    fn plan(&self, _op: &LinkOp) -> Result<Vec<LinkAction>> {
        // GNU stow planning is opaque (its -n preview is not structured). The
        // native engine owns structured planning; callers needing a file-level
        // preview should use engine: native.
        Ok(Vec::new())
    }
}

// ---------------------------------------------------------------------------
// NativeEngine — pure-Rust, no_folding-first file-level symlinks.
// ---------------------------------------------------------------------------

pub struct NativeEngine;

impl NativeEngine {
    /// Recursively collect package files, applying literal-substring ignores.
    ///
    /// NOTE (v1 limitation): stow `--ignore` is a regex; native v1 matches each
    /// ignore as a substring of the file's relative path. `.*\.swp`-style regex
    /// patterns are NOT honored — recommend `engine: stow` for regex ignores.
    fn collect(source_pkg: &Path, ignores: &[String]) -> Vec<PathBuf> {
        collect_package_files(source_pkg)
            .into_iter()
            .filter(|file| {
                let rel = file
                    .strip_prefix(source_pkg)
                    .unwrap_or(file)
                    .to_string_lossy();
                !ignores.iter().any(|ig| rel.contains(ig.as_str()))
            })
            .collect()
    }

    /// True when `link` is a symlink whose resolved target lies inside `pkg_src`.
    /// Matches the ownership rule used by `check_stowed_status` / GNU stow:
    /// native must not remove or replace links owned by another package (or
    /// hand-managed paths).
    fn link_owned_by_package(link: &Path, pkg_src: &Path) -> bool {
        let Ok(meta) = fs::symlink_metadata(link) else {
            return false;
        };
        if !meta.file_type().is_symlink() {
            return false;
        }
        let Ok(link_target) = fs::read_link(link) else {
            return false;
        };
        let abs = link
            .parent()
            .map(|p| p.join(&link_target))
            .unwrap_or(link_target);
        match (fs::canonicalize(&abs), fs::canonicalize(pkg_src)) {
            (Ok(dest), Ok(pkg)) => dest.starts_with(&pkg),
            _ => false,
        }
    }
}

impl LinkEngine for NativeEngine {
    fn name(&self) -> &'static str {
        "native"
    }

    fn plan(&self, op: &LinkOp) -> Result<Vec<LinkAction>> {
        // Native v1 is file-level only; adopt and dotfiles would change behavior
        // we do not implement. Folding is inherent (native never folds), so
        // no_folding does not require an error.
        if op.adopt {
            return Err(Error::NativeUnsupported("adopt".into()));
        }
        if op.dotfiles {
            return Err(Error::NativeUnsupported("dotfiles".into()));
        }

        let pkg_src = op.source.join(&op.package);
        if !pkg_src.exists() {
            return Err(Error::SourceNotFound(pkg_src));
        }

        let files = Self::collect(&pkg_src, &op.ignores);
        let mut actions = Vec::with_capacity(files.len());
        for f in files {
            let rel = f.strip_prefix(&pkg_src).unwrap_or(&f).to_path_buf();
            let link = op.target.join(&rel);
            match op.action {
                StowAction::Stow => actions.push(LinkAction::CreateSymlink {
                    link,
                    source_file: f,
                }),
                StowAction::Restow => {
                    // restow = unstow then stow. Only remove links we own.
                    if Self::link_owned_by_package(&link, &pkg_src) {
                        actions.push(LinkAction::RemoveSymlink { path: link.clone() });
                    }
                    actions.push(LinkAction::CreateSymlink {
                        link,
                        source_file: f,
                    });
                }
                StowAction::Unstow => {
                    // Only plan removal for package-owned links (stow parity).
                    if Self::link_owned_by_package(&link, &pkg_src) {
                        actions.push(LinkAction::RemoveSymlink { path: link });
                    }
                }
            }
        }
        Ok(actions)
    }

    fn apply(&self, op: &LinkOp) -> Result<()> {
        let pkg_src = op.source.join(&op.package);
        let actions = self.plan(op)?;
        if op.dry_run {
            return Ok(());
        }
        for action in actions {
            match action {
                LinkAction::CreateSymlink { link, source_file } => {
                    if let Some(parent) = link.parent() {
                        fs::create_dir_all(parent)?;
                    }
                    // Create a portable RELATIVE symlink (stow-compatible): the
                    // target is expressed relative to the link's parent, computed
                    // from canonicalized ends so it is valid regardless of CWD.
                    let canon_source =
                        fs::canonicalize(&source_file).unwrap_or(source_file.clone());
                    let canon_parent = link
                        .parent()
                        .and_then(|p| fs::canonicalize(p).ok())
                        .unwrap_or_else(|| source_file.clone());
                    let rel_target =
                        pathdiff::diff_paths(&canon_source, &canon_parent).unwrap_or(canon_source);
                    // Conflict unless missing or an owned package symlink we may replace.
                    if link.is_symlink() {
                        if Self::link_owned_by_package(&link, &pkg_src) {
                            fs::remove_file(&link)?;
                        } else {
                            return Err(Error::StowFailed(format!(
                                "target already exists (foreign or dangling symlink): {}",
                                link.display()
                            )));
                        }
                    } else if link.exists() {
                        return Err(Error::StowFailed(format!(
                            "target already exists and is not a symlink: {}",
                            link.display()
                        )));
                    }
                    symlink(&rel_target, &link)?;
                }
                LinkAction::RemoveSymlink { path } => {
                    // Defense in depth: plan already filters ownership.
                    if Self::link_owned_by_package(&path, &pkg_src) {
                        fs::remove_file(&path)?;
                    }
                }
            }
        }
        Ok(())
    }
}

#[cfg(unix)]
fn symlink(src: &Path, link: &Path) -> Result<()> {
    std::os::unix::fs::symlink(src, link).map_err(Error::from)
}

#[cfg(windows)]
fn symlink(src: &Path, link: &Path) -> Result<()> {
    std::os::windows::fs::symlink_file(src, link).map_err(Error::from)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    fn pkg(tmp: &TempDir) -> (PathBuf, PathBuf, PathBuf) {
        let source = tmp.path().join("packages");
        let target = tmp.path().join("target");
        let pkg_src = source.join("bash-env");
        fs::create_dir_all(&pkg_src).unwrap();
        fs::create_dir_all(&target).unwrap();
        fs::write(pkg_src.join(".bashrc"), "export FOO=1\n").unwrap();
        (source, target, pkg_src)
    }

    fn op(source: PathBuf, target: PathBuf, action: StowAction) -> LinkOp {
        LinkOp {
            group: "home".into(),
            package: "bash-env".into(),
            source,
            target,
            action,
            dry_run: false,
            no_folding: false,
            adopt: false,
            dotfiles: false,
            verbosity: 0,
            ignores: vec![],
        }
    }

    #[test]
    fn native_plan_stow_lists_create_symlink_without_writing() {
        let tmp = TempDir::new().unwrap();
        let (source, target, _pkg_src) = pkg(&tmp);
        let engine = NativeEngine;
        let actions = engine.plan(&op(source, target, StowAction::Stow)).unwrap();
        assert_eq!(
            actions,
            vec![LinkAction::CreateSymlink {
                link: tmp.path().join("target/.bashrc"),
                source_file: tmp.path().join("packages/bash-env/.bashrc"),
            }]
        );
        // Planning must not have created the link.
        assert!(!tmp.path().join("target/.bashrc").exists());
    }

    #[test]
    fn native_apply_stow_creates_symlink() {
        let tmp = TempDir::new().unwrap();
        let (source, target, _pkg_src) = pkg(&tmp);
        let engine = NativeEngine;
        engine
            .apply(&op(source.clone(), target.clone(), StowAction::Stow))
            .unwrap();
        let link = target.join(".bashrc");
        assert!(link.is_symlink(), "native stow must create a symlink");
        let dest = fs::read_link(&link).unwrap();
        assert!(
            dest.to_string_lossy().contains("bash-env"),
            "symlink should point into the package, got {}",
            dest.display()
        );
    }

    #[test]
    fn native_dry_run_plans_without_writing() {
        let tmp = TempDir::new().unwrap();
        let (source, target, _pkg_src) = pkg(&tmp);
        let mut o = op(source, target, StowAction::Stow);
        o.dry_run = true;
        let engine = NativeEngine;
        let actions = engine.plan(&o).unwrap();
        assert_eq!(actions.len(), 1, "dry-run plan still lists the action");
        engine.apply(&o).unwrap();
        assert!(
            !tmp.path().join("target/.bashrc").exists(),
            "dry-run apply must not write"
        );
    }

    #[test]
    fn native_unstow_removes_symlink() {
        let tmp = TempDir::new().unwrap();
        let (source, target, _pkg_src) = pkg(&tmp);
        let engine = NativeEngine;
        engine
            .apply(&op(source.clone(), target.clone(), StowAction::Stow))
            .unwrap();
        assert!(target.join(".bashrc").is_symlink());
        let actions = engine
            .plan(&op(source.clone(), target.clone(), StowAction::Unstow))
            .unwrap();
        assert!(matches!(actions[0], LinkAction::RemoveSymlink { .. }));
        engine
            .apply(&op(source, target, StowAction::Unstow))
            .unwrap();
        assert!(
            !tmp.path().join("target/.bashrc").exists(),
            "unstow must remove the symlink"
        );
    }

    #[test]
    fn native_restow_recreates_symlink() {
        let tmp = TempDir::new().unwrap();
        let (source, target, _pkg_src) = pkg(&tmp);
        let engine = NativeEngine;
        engine
            .apply(&op(source.clone(), target.clone(), StowAction::Stow))
            .unwrap();
        let link = tmp.path().join("target/.bashrc");
        let first = fs::read_link(&link).unwrap();
        // restow plans remove-then-create for an existing link.
        let actions = engine
            .plan(&op(source.clone(), target.clone(), StowAction::Restow))
            .unwrap();
        assert_eq!(actions.len(), 2, "restow should plan remove + create");
        engine
            .apply(&op(source, target, StowAction::Restow))
            .unwrap();
        assert!(link.is_symlink());
        assert_eq!(first, fs::read_link(&link).unwrap());
    }

    #[test]
    fn native_rejects_adopt_and_dotfiles() {
        let tmp = TempDir::new().unwrap();
        let (source, target, _pkg_src) = pkg(&tmp);
        let engine = NativeEngine;
        let mut adopt_op = op(source.clone(), target.clone(), StowAction::Stow);
        adopt_op.adopt = true;
        let err = engine.plan(&adopt_op).unwrap_err();
        assert!(
            err.to_string().contains("global.engine: stow"),
            "adopt error should recommend stow engine: {}",
            err
        );

        let mut dot_op = op(source, target, StowAction::Stow);
        dot_op.dotfiles = true;
        let err = engine.plan(&dot_op).unwrap_err();
        assert!(err.to_string().contains("dotfiles"));
    }

    #[test]
    fn native_ignores_honored_as_substring() {
        let tmp = TempDir::new().unwrap();
        let source = tmp.path().join("packages");
        let pkg_src = source.join("bash-env");
        let target = tmp.path().join("target");
        fs::create_dir_all(&pkg_src).unwrap();
        fs::create_dir_all(&target).unwrap();
        fs::write(pkg_src.join(".bashrc"), "x").unwrap();
        fs::write(pkg_src.join(".DS_Store"), "x").unwrap();

        let mut o = op(source, target, StowAction::Stow);
        o.ignores = vec![".DS_Store".to_string()];
        let actions = NativeEngine.plan(&o).unwrap();
        assert_eq!(actions.len(), 1, "ignored file must be excluded from plan");
        assert!(actions[0].clone().link().ends_with(".bashrc"));
    }

    #[test]
    fn engine_for_factory_selects_by_name() {
        assert_eq!(engine_for("native").name(), "native");
        assert_eq!(engine_for("NATIVE").name(), "native");
        assert_eq!(engine_for("stow").name(), "stow");
        // unknown falls back to native
        assert_eq!(engine_for("bogus").name(), "native");
    }

    #[test]
    fn native_unstow_does_not_remove_foreign_symlink() {
        let tmp = TempDir::new().unwrap();
        let source = tmp.path().join("packages");
        let target = tmp.path().join("target");
        let pkg_a = source.join("pkg-a");
        let pkg_b = source.join("pkg-b");
        fs::create_dir_all(&pkg_a).unwrap();
        fs::create_dir_all(&pkg_b).unwrap();
        fs::create_dir_all(&target).unwrap();
        fs::write(pkg_a.join(".bashrc"), "a").unwrap();
        fs::write(pkg_b.join(".bashrc"), "b").unwrap();

        let engine = NativeEngine;
        // Stow A so target/.bashrc → pkg-a.
        engine
            .apply(&LinkOp {
                group: "home".into(),
                package: "pkg-a".into(),
                source: source.clone(),
                target: target.clone(),
                action: StowAction::Stow,
                dry_run: false,
                no_folding: false,
                adopt: false,
                dotfiles: false,
                verbosity: 0,
                ignores: vec![],
            })
            .unwrap();
        let link = target.join(".bashrc");
        assert!(link.is_symlink());
        let owned = fs::read_link(&link).unwrap();

        // Unstow B must NOT remove A's link (ownership).
        engine
            .apply(&LinkOp {
                group: "home".into(),
                package: "pkg-b".into(),
                source: source.clone(),
                target: target.clone(),
                action: StowAction::Unstow,
                dry_run: false,
                no_folding: false,
                adopt: false,
                dotfiles: false,
                verbosity: 0,
                ignores: vec![],
            })
            .unwrap();
        assert!(
            link.is_symlink(),
            "unstow of pkg-b must leave pkg-a's symlink alone"
        );
        assert_eq!(fs::read_link(&link).unwrap(), owned);

        // Stow B over A's link must conflict, not steal.
        let err = engine
            .apply(&LinkOp {
                group: "home".into(),
                package: "pkg-b".into(),
                source,
                target,
                action: StowAction::Stow,
                dry_run: false,
                no_folding: false,
                adopt: false,
                dotfiles: false,
                verbosity: 0,
                ignores: vec![],
            })
            .unwrap_err();
        assert!(
            err.to_string().contains("foreign") || err.to_string().contains("already exists"),
            "stow over foreign link must conflict: {err}"
        );
        assert_eq!(fs::read_link(&link).unwrap(), owned);
    }

    // Small helper to inspect a LinkAction's link path in tests.
    impl LinkAction {
        fn link(&self) -> &Path {
            match self {
                LinkAction::CreateSymlink { link, .. } => link,
                LinkAction::RemoveSymlink { path } => path,
            }
        }
    }

    // ---- M09 gap tests ----
    //
    // (1) StowEngine apply surfaces a clear missing-binary error when the
    //     `stow` executable is unresolvable on PATH. Reproducible on developer
    //     machines that DO have stow installed by isolating PATH to a tempdir
    //     that lacks a `stow` binary.
    // (2) StowEngine apply with `dry_run = true` must NOT create or remove
    //     files under the target. The early-return contract in `apply` makes
    //     this a filesystem-touching assertion, not just a code-path check.
    //
    // Race-safety note: the missing-binary test mutates the process PATH.
    // The other tests in this binary use NativeEngine (no shell-out) or temp
    // directories, so they do not observe the override today. cargo test DOES
    // run tests within a binary in parallel (default test-threads = num-cpus);
    // nextest 0.9+ runs each test in its own process. `catch_unwind` here
    // covers the panic case (assertion failure) — it does NOT serialize
    // against other parallel tests. Tests in `tests/e2e.rs` and
    // `tests/integration.rs` are separate binaries and isolated by nextest /
    // by cargo test's per-binary process model. Any future andre-core unit
    // test that reads PATH MUST wrap its `env::var` in catch_unwind+set_var.

    fn stow_op(source: PathBuf, target: PathBuf) -> LinkOp {
        LinkOp {
            group: "home".into(),
            package: "bash-env".into(),
            source,
            target,
            action: StowAction::Stow,
            dry_run: false,
            no_folding: false,
            adopt: false,
            dotfiles: false,
            verbosity: 0,
            ignores: vec![],
        }
    }

    #[test]
    fn stow_engine_missing_binary_returns_clear_error() {
        let tmp = TempDir::new().unwrap();
        // An empty directory on PATH makes `Command::new("stow")` return
        // NotFound, which `execute_stow` surfaces as `Error::StowNotFound`.
        let no_stow = tmp.path().join("no_stow_here");
        fs::create_dir_all(&no_stow).unwrap();

        let source = tmp.path().join("packages");
        let pkg_src = source.join("bash-env");
        let target = tmp.path().join("target");
        fs::create_dir_all(&pkg_src).unwrap();
        fs::write(pkg_src.join(".bashrc"), "x").unwrap();
        fs::create_dir_all(&target).unwrap();

        let saved = std::env::var_os("PATH");
        std::env::set_var("PATH", &no_stow);

        let result = std::panic::catch_unwind(|| {
            let engine = StowEngine;
            let err = engine.apply(&stow_op(source, target)).unwrap_err();
            let msg = err.to_string().to_lowercase();
            assert!(
                msg.contains("stow"),
                "error should mention stow, got: {}",
                err
            );
            assert!(
                matches!(err, Error::StowNotFound)
                    || msg.contains("not found")
                    || msg.contains("no such file"),
                "error should indicate missing binary, got: {}",
                err
            );
        });

        // Always restore PATH, even if the assertion panicked.
        match saved {
            Some(p) => std::env::set_var("PATH", p),
            None => std::env::remove_var("PATH"),
        }
        if let Err(e) = result {
            std::panic::resume_unwind(e);
        }
    }

    #[test]
    fn stow_engine_dry_run_apply_does_not_touch_filesystem() {
        let tmp = TempDir::new().unwrap();
        let source = tmp.path().join("packages");
        let pkg_src = source.join("bash-env");
        let target = tmp.path().join("target");
        fs::create_dir_all(&pkg_src).unwrap();
        fs::write(pkg_src.join(".bashrc"), "x").unwrap();
        fs::create_dir_all(&target).unwrap();

        let mut op = stow_op(source, target.clone());
        op.dry_run = true;

        let engine = StowEngine;
        // Dry-run must succeed even if stow is unresolvable: the engine's
        // early-return contract means we never shell out. We do NOT strip PATH
        // here because that would mask a regression where the dry-run branch
        // accidentally calls execute_stow.
        engine.apply(&op).unwrap();

        // Target must be untouched: no symlinks, no files, no nested dirs.
        let entries: Vec<_> = fs::read_dir(&target)
            .unwrap()
            .filter_map(|e| e.ok())
            .map(|e| e.file_name())
            .collect();
        assert!(
            entries.is_empty(),
            "dry-run must not create anything under target; found: {:?}",
            entries
        );
        assert!(
            !target.join(".bashrc").exists(),
            "dry-run must not create the stowed file"
        );
    }
}
