use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use crate::error::{Error, Result};
use crate::ignore::IgnoreRules;
use crate::state::{StowAction, StowFlags};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StowStatus {
    Stowed,
    Unstowed,
    Partial,
    Conflict,
    Missing,
    Empty,
    /// The package's ignore list cannot be read or compiled, so its status is unknown.
    Invalid,
}

impl StowStatus {
    pub fn label(&self) -> &'static str {
        match self {
            StowStatus::Stowed => "Stowed",
            StowStatus::Unstowed => "Unstowed",
            StowStatus::Partial => "Partial",
            StowStatus::Conflict => "Conflict",
            StowStatus::Missing => "Missing",
            StowStatus::Empty => "Empty",
            StowStatus::Invalid => "Invalid",
        }
    }

    pub fn is_stowed(&self) -> bool {
        *self == StowStatus::Stowed
    }
}

/// Status of `package` under `target`, counting only files that would be linked with no
/// config ignores applied. `global` is the `.stow-global-ignore` path to read, if any.
/// Use [`check_stowed_status_with_ignores`] to match a group's `ignore` list too.
pub fn check_stowed_status(
    source: &Path,
    target: &Path,
    package: &str,
    global: Option<&Path>,
) -> Result<StowStatus> {
    check_stowed_status_with_ignores(source, target, package, &[], global)
}

/// Like [`check_stowed_status`], with the group's config ignores (`ignores`) applied on
/// top of the package's ignore list, so status counts the same files a link run would.
pub fn check_stowed_status_with_ignores(
    source: &Path,
    target: &Path,
    package: &str,
    ignores: &[String],
    global: Option<&Path>,
) -> Result<StowStatus> {
    if !source.join(package).exists() {
        return Ok(StowStatus::Missing);
    }

    let links = file_links(source, target, package, ignores, global)?;
    if links.is_empty() {
        return Ok(StowStatus::Empty);
    }

    let linked = links
        .iter()
        .filter(|(_, link)| *link == FileLink::Linked)
        .count();
    if links.iter().any(|(_, link)| link.is_conflict()) {
        Ok(StowStatus::Conflict)
    } else if linked == links.len() {
        Ok(StowStatus::Stowed)
    } else if linked == 0 {
        Ok(StowStatus::Unstowed)
    } else {
        Ok(StowStatus::Partial)
    }
}

/// How one file that a link run would link sits in the target.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FileLink {
    /// The target holds a link to the package's file.
    Linked,
    /// Nothing is in the target's place, or only a broken link is.
    Unlinked,
    /// A real file is in the target's place.
    RealFile,
    /// A link is in the target's place, but it does not lead to the package's file.
    WrongLink,
}

impl FileLink {
    /// True when something other than a link to the file is in its place.
    pub fn is_conflict(self) -> bool {
        matches!(self, FileLink::RealFile | FileLink::WrongLink)
    }
}

/// Every file a link run would link for `package` under `source` (see [`linkable_files`]),
/// relative to the package, with how each sits in `target`. Files come in name order.
pub fn file_links(
    source: &Path,
    target: &Path,
    package: &str,
    ignores: &[String],
    global: Option<&Path>,
) -> Result<Vec<(PathBuf, FileLink)>> {
    let package_source = source.join(package);
    let files = linkable_files(&package_source, ignores, global)?;
    Ok(files
        .iter()
        .map(|file| {
            let relative = match file.strip_prefix(&package_source) {
                Ok(p) => p.to_path_buf(),
                Err(_) => file
                    .file_name()
                    .map(PathBuf::from)
                    .unwrap_or_else(|| file.clone()),
            };
            let link = link_state(&target.join(&relative), file);
            (relative, link)
        })
        .collect())
}

/// How `file` sits at `target_path`. A path that does not exist, a broken link included,
/// counts as unlinked.
fn link_state(target_path: &Path, file: &Path) -> FileLink {
    if !target_path.exists() {
        return FileLink::Unlinked;
    }
    let Ok(metadata) = fs::symlink_metadata(target_path) else {
        return FileLink::Unlinked;
    };

    if !metadata.file_type().is_symlink() {
        // Not a direct symlink — check folding: a parent directory symlink
        // may point into the package source (stow default behavior).
        return match (fs::canonicalize(target_path), fs::canonicalize(file)) {
            (Ok(t), Ok(f)) if t == f => FileLink::Linked,
            _ => FileLink::RealFile,
        };
    }

    // Direct symlink — verify it points to the right place.
    let Ok(link_target) = fs::read_link(target_path) else {
        return FileLink::WrongLink;
    };
    let link_target_abs = target_path
        .parent()
        .map(|p| p.join(&link_target))
        .unwrap_or(link_target);
    let (Ok(link_target_canonical), Ok(file_canonical)) =
        (fs::canonicalize(&link_target_abs), fs::canonicalize(file))
    else {
        return FileLink::WrongLink;
    };
    if link_target_canonical == file_canonical {
        FileLink::Linked
    } else {
        FileLink::WrongLink
    }
}

/// The files a link run would link for `package_dir`: the package's ignore list
/// ([`IgnoreRules::load`], with `global` as the global list) applied, then the group's
/// `ignores` as substrings of the relative path. Both engines and status use this, so
/// they agree on what is linked.
pub fn linkable_files(
    package_dir: &Path,
    ignores: &[String],
    global: Option<&Path>,
) -> Result<Vec<PathBuf>> {
    let rules = IgnoreRules::load(package_dir, global)?;
    Ok(collect_package_files(package_dir, &rules)
        .into_iter()
        .filter(|file| {
            let rel = file.strip_prefix(package_dir).unwrap_or(file);
            !ignored_by_group(rel, ignores)
        })
        .collect())
}

/// True when the group's `ignores` (substring match) exclude `rel`, a path relative to
/// the package. Shared by [`linkable_files`] and the create planner, so previews match.
pub(crate) fn ignored_by_group(rel: &Path, ignores: &[String]) -> bool {
    let rel = rel.to_string_lossy();
    ignores.iter().any(|ig| rel.contains(ig.as_str()))
}

/// Every regular file under `package_dir` that `rules` does not exclude. An excluded
/// directory is not entered. Entries are visited in name order.
pub fn collect_package_files(package_dir: &Path, rules: &IgnoreRules) -> Vec<PathBuf> {
    let mut files = Vec::new();
    if package_dir.is_dir() {
        walk(package_dir, "", rules, &mut files);
    }
    files
}

fn walk(dir: &Path, prefix: &str, rules: &IgnoreRules, files: &mut Vec<PathBuf>) {
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    let mut entries: Vec<_> = entries.flatten().collect();
    entries.sort_by_key(|entry| entry.file_name());
    for entry in entries {
        let name = entry.file_name().to_string_lossy().into_owned();
        let rel = if prefix.is_empty() {
            name
        } else {
            format!("{prefix}/{name}")
        };
        if rules.is_ignored(&rel) {
            continue;
        }
        let path = entry.path();
        if path.is_file() {
            files.push(path);
        } else if path.is_dir() {
            walk(&path, &rel, rules, files);
        }
    }
}

pub fn build_stow_args(
    source: &Path,
    target: &Path,
    package: &str,
    flags: &StowFlags,
    ignores: &[String],
) -> Command {
    let mut cmd = Command::new("stow");

    cmd.arg("-d");
    cmd.arg(source.display().to_string());
    cmd.arg("-t");
    cmd.arg(target.display().to_string());

    match flags.action {
        StowAction::Unstow => {
            cmd.arg("-D");
        }
        StowAction::Restow => {
            cmd.arg("-R");
        }
        StowAction::Stow => {}
    }

    for _ in 0..flags.verbosity {
        cmd.arg("-v");
    }

    if flags.dry_run {
        cmd.arg("-n");
    }

    if flags.no_folding {
        cmd.arg("--no-folding");
    }

    if flags.adopt {
        cmd.arg("--adopt");
    }

    if flags.dotfiles {
        cmd.arg("--dotfiles");
    }

    for ignore in ignores {
        cmd.arg(format!("--ignore={}", ignore));
    }

    cmd.arg(package);

    cmd
}

/// Sets `HOME` on a stow child process, so GNU stow reads the `.stow-global-ignore` in
/// `home` and not the host's. Only the child is affected; this process's environment is
/// untouched. `None` leaves the child with the inherited `HOME`.
pub(crate) fn set_child_home(cmd: &mut Command, home: Option<&Path>) {
    if let Some(home) = home {
        cmd.env("HOME", home);
    }
}

/// Runs GNU stow for one package. `home` is the home whose global ignore list stow
/// should read (see [`set_child_home`]).
pub fn execute_stow(
    source: &Path,
    target: &Path,
    package: &str,
    flags: &StowFlags,
    ignores: &[String],
    home: Option<&Path>,
) -> Result<i32> {
    let mut cmd = build_stow_args(source, target, package, flags, ignores);
    set_child_home(&mut cmd, home);
    let output = cmd.output().map_err(|e| {
        if e.kind() == std::io::ErrorKind::NotFound {
            Error::StowNotFound
        } else {
            Error::StowFailed(e.to_string())
        }
    })?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(Error::StowFailed(stderr.to_string()));
    }

    Ok(output.status.code().unwrap_or(0))
}

pub fn unlink_stowed_symlink(path: &Path) -> Result<bool> {
    if !path.is_symlink() {
        return Ok(false);
    }
    std::fs::remove_file(path)?;
    Ok(true)
}

#[cfg(test)]
#[allow(clippy::needless_borrows_for_generic_args, clippy::expect_fun_call)]
mod tests {
    use tempfile::TempDir;

    use super::*;

    #[cfg(unix)]
    fn make_symlink(target: &Path, link: &Path) -> std::io::Result<()> {
        std::os::unix::fs::symlink(target, link)
    }

    #[cfg(windows)]
    fn make_symlink(target: &Path, link: &Path) -> std::io::Result<()> {
        std::os::windows::fs::symlink_file(target, link)
    }

    #[test]
    fn test_stow_status_stowed() {
        let tmp = TempDir::new().unwrap();
        let source = tmp.path().join("packages");
        let target = tmp.path().join("target");
        let pkg_src = source.join("bash-env");

        fs::create_dir_all(&pkg_src).unwrap();
        fs::write(pkg_src.join(".bashrc"), "content").unwrap();
        fs::create_dir_all(&target).unwrap();

        let link_path = target.join(".bashrc");
        make_symlink(&pkg_src.join(".bashrc"), &link_path).unwrap();

        let status = check_stowed_status(&source, &target, "bash-env", None).unwrap();
        assert_eq!(status, StowStatus::Stowed);
    }

    #[test]
    fn test_stow_status_unstowed() {
        let tmp = TempDir::new().unwrap();
        let source = tmp.path().join("packages");
        let target = tmp.path().join("target");
        let pkg_src = source.join("bash-env");

        fs::create_dir_all(&pkg_src).unwrap();
        fs::write(pkg_src.join(".bashrc"), "content").unwrap();
        fs::create_dir_all(&target).unwrap();

        let status = check_stowed_status(&source, &target, "bash-env", None).unwrap();
        assert_eq!(status, StowStatus::Unstowed);
    }

    #[test]
    fn test_stow_status_missing() {
        let tmp = TempDir::new().unwrap();
        let source = tmp.path().join("packages");
        let target = tmp.path().join("target");

        fs::create_dir_all(&source).unwrap();
        fs::create_dir_all(&target).unwrap();

        let status = check_stowed_status(&source, &target, "nonexistent", None).unwrap();
        assert_eq!(status, StowStatus::Missing);
    }

    #[test]
    fn test_stow_status_empty() {
        let tmp = TempDir::new().unwrap();
        let source = tmp.path().join("packages");
        let target = tmp.path().join("target");
        let pkg_src = source.join("empty-pkg");

        fs::create_dir_all(&pkg_src).unwrap();
        fs::create_dir_all(&target).unwrap();

        let status = check_stowed_status(&source, &target, "empty-pkg", None).unwrap();
        assert_eq!(status, StowStatus::Empty);
    }

    #[test]
    fn test_stow_status_conflict() {
        let tmp = TempDir::new().unwrap();
        let source = tmp.path().join("packages");
        let target = tmp.path().join("target");
        let pkg_src = source.join("bash-env");

        fs::create_dir_all(&pkg_src).unwrap();
        fs::write(pkg_src.join(".bashrc"), "content").unwrap();
        fs::create_dir_all(&target).unwrap();

        fs::write(target.join(".bashrc"), "different content").unwrap();

        let status = check_stowed_status(&source, &target, "bash-env", None).unwrap();
        assert_eq!(status, StowStatus::Conflict);
    }

    #[test]
    fn test_stow_status_partial() {
        let tmp = TempDir::new().unwrap();
        let source = tmp.path().join("packages");
        let target = tmp.path().join("target");
        let pkg_src = source.join("bash-env");

        fs::create_dir_all(&pkg_src).unwrap();
        fs::write(pkg_src.join(".bashrc"), "content").unwrap();
        fs::write(pkg_src.join(".vimrc"), "content").unwrap();
        fs::create_dir_all(&target).unwrap();

        make_symlink(&pkg_src.join(".bashrc"), &target.join(".bashrc")).unwrap();

        let status = check_stowed_status(&source, &target, "bash-env", None).unwrap();
        assert_eq!(status, StowStatus::Partial);
    }

    #[test]
    fn test_file_links_say_how_each_file_sits_in_the_target() {
        let tmp = TempDir::new().unwrap();
        let source = tmp.path().join("packages");
        let target = tmp.path().join("target");
        let pkg_src = source.join("bash-env");

        fs::create_dir_all(&pkg_src).unwrap();
        for name in [".a", ".b", ".c", ".d", ".e"] {
            fs::write(pkg_src.join(name), "content").unwrap();
        }
        fs::create_dir_all(&target).unwrap();
        make_symlink(&pkg_src.join(".a"), &target.join(".a")).unwrap();
        fs::write(target.join(".c"), "real file").unwrap();
        let elsewhere = tmp.path().join("elsewhere");
        fs::write(&elsewhere, "other").unwrap();
        make_symlink(&elsewhere, &target.join(".d")).unwrap();
        // A broken link is not counted, as before: it reads as unlinked.
        make_symlink(&tmp.path().join("gone"), &target.join(".e")).unwrap();

        let links = file_links(&source, &target, "bash-env", &[], None).unwrap();
        assert_eq!(
            links,
            vec![
                (PathBuf::from(".a"), FileLink::Linked),
                (PathBuf::from(".b"), FileLink::Unlinked),
                (PathBuf::from(".c"), FileLink::RealFile),
                (PathBuf::from(".d"), FileLink::WrongLink),
                (PathBuf::from(".e"), FileLink::Unlinked),
            ]
        );
        let status = check_stowed_status(&source, &target, "bash-env", None).unwrap();
        assert_eq!(
            status,
            StowStatus::Conflict,
            "a real file wins over the links"
        );
    }

    #[test]
    fn test_build_stow_args_stow() {
        let source = PathBuf::from("/src/packages");
        let target = PathBuf::from("/home/user");

        let cmd = build_stow_args(
            &source,
            &target,
            "bash-env",
            &StowFlags {
                action: StowAction::Stow,
                verbosity: 0,
                dry_run: false,
                no_folding: false,
                adopt: false,
                dotfiles: false,
            },
            &[],
        );

        let args: Vec<&str> = cmd.get_args().map(|a| a.to_str().unwrap()).collect();
        assert!(args.contains(&"-d"));
        assert!(args.contains(&"/src/packages"));
        assert!(args.contains(&"-t"));
        assert!(args.contains(&"/home/user"));
        assert!(args.contains(&"bash-env"));
    }

    #[test]
    fn test_build_stow_args_unstow() {
        let source = PathBuf::from("/src/packages");
        let target = PathBuf::from("/home/user");

        let cmd = build_stow_args(
            &source,
            &target,
            "bash-env",
            &StowFlags {
                action: StowAction::Unstow,
                verbosity: 0,
                dry_run: false,
                no_folding: false,
                adopt: false,
                dotfiles: false,
            },
            &[],
        );

        let args: Vec<&str> = cmd.get_args().map(|a| a.to_str().unwrap()).collect();
        assert!(args.contains(&"-D"));
    }

    #[test]
    fn test_build_stow_args_verbosity_multi() {
        let source = PathBuf::from("/src/packages");
        let target = PathBuf::from("/home/user");

        let cmd = build_stow_args(
            &source,
            &target,
            "bash-env",
            &StowFlags {
                action: StowAction::Stow,
                verbosity: 2,
                dry_run: false,
                no_folding: false,
                adopt: false,
                dotfiles: false,
            },
            &[],
        );

        let args: Vec<&str> = cmd.get_args().map(|a| a.to_str().unwrap()).collect();
        let v_count = args.iter().filter(|a| *a == &"-v").count();
        assert_eq!(v_count, 2);
    }

    #[test]
    fn test_build_stow_args_dry_run() {
        let source = PathBuf::from("/src/packages");
        let target = PathBuf::from("/home/user");

        let cmd = build_stow_args(
            &source,
            &target,
            "bash-env",
            &StowFlags {
                action: StowAction::Stow,
                verbosity: 0,
                dry_run: true,
                no_folding: false,
                adopt: false,
                dotfiles: false,
            },
            &[],
        );

        let args: Vec<&str> = cmd.get_args().map(|a| a.to_str().unwrap()).collect();
        assert!(args.contains(&"-n"));
    }

    #[test]
    fn test_build_stow_args_ignores() {
        let source = PathBuf::from("/src/packages");
        let target = PathBuf::from("/home/user");

        let cmd = build_stow_args(
            &source,
            &target,
            "bash-env",
            &StowFlags {
                action: StowAction::Stow,
                verbosity: 0,
                dry_run: false,
                no_folding: false,
                adopt: false,
                dotfiles: false,
            },
            &[String::from(".git"), String::from(".DS_Store")],
        );

        let args: Vec<&str> = cmd.get_args().map(|a| a.to_str().unwrap()).collect();
        assert!(args.contains(&"--ignore=.git"));
        assert!(args.contains(&"--ignore=.DS_Store"));
    }

    #[test]
    fn test_stow_status_label_and_is_stowed() {
        assert_eq!(StowStatus::Stowed.label(), "Stowed");
        assert!(StowStatus::Stowed.is_stowed());

        assert!(!StowStatus::Unstowed.is_stowed());
    }

    #[test]
    fn test_build_stow_args_restow() {
        let source = PathBuf::from("/src/packages");
        let target = PathBuf::from("/home/user");

        let cmd = build_stow_args(
            &source,
            &target,
            "bash-env",
            &StowFlags {
                action: StowAction::Restow,
                verbosity: 0,
                dry_run: false,
                no_folding: false,
                adopt: false,
                dotfiles: false,
            },
            &[],
        );

        let args: Vec<&str> = cmd.get_args().map(|a| a.to_str().unwrap()).collect();
        assert!(args.contains(&"-R"));
    }

    #[test]
    fn test_build_stow_args_no_folding() {
        let source = PathBuf::from("/src/packages");
        let target = PathBuf::from("/home/user");

        let cmd = build_stow_args(
            &source,
            &target,
            "bash-env",
            &StowFlags {
                action: StowAction::Stow,
                verbosity: 0,
                dry_run: false,
                no_folding: true,
                adopt: false,
                dotfiles: false,
            },
            &[],
        );

        let args: Vec<&str> = cmd.get_args().map(|a| a.to_str().unwrap()).collect();
        assert!(args.contains(&"--no-folding"));
    }

    #[test]
    fn test_build_stow_args_adopt() {
        let source = PathBuf::from("/src/packages");
        let target = PathBuf::from("/home/user");

        let cmd = build_stow_args(
            &source,
            &target,
            "bash-env",
            &StowFlags {
                action: StowAction::Stow,
                verbosity: 0,
                dry_run: false,
                no_folding: false,
                adopt: true,
                dotfiles: false,
            },
            &[],
        );

        let args: Vec<&str> = cmd.get_args().map(|a| a.to_str().unwrap()).collect();
        assert!(args.contains(&"--adopt"));
    }

    #[test]
    fn test_build_stow_args_dotfiles() {
        let source = PathBuf::from("/src/packages");
        let target = PathBuf::from("/home/user");

        let cmd = build_stow_args(
            &source,
            &target,
            "bash-env",
            &StowFlags {
                action: StowAction::Stow,
                verbosity: 0,
                dry_run: false,
                no_folding: false,
                adopt: false,
                dotfiles: true,
            },
            &[],
        );

        let args: Vec<&str> = cmd.get_args().map(|a| a.to_str().unwrap()).collect();
        assert!(args.contains(&"--dotfiles"));
    }

    #[test]
    fn test_stow_status_invalid_ignore_list_is_an_error_not_missing() {
        let tmp = TempDir::new().unwrap();
        let source = tmp.path().join("packages");
        let target = tmp.path().join("target");
        fs::create_dir_all(source.join("bash-env")).unwrap();
        fs::create_dir_all(&target).unwrap();
        fs::write(source.join("bash-env/.bashrc"), "rc").unwrap();
        fs::write(source.join("bash-env/.stow-local-ignore"), "(?=x)y\n").unwrap();
        assert!(check_stowed_status(&source, &target, "bash-env", None).is_err());
    }

    #[test]
    fn test_stow_status_missing_source() {
        let tmp = TempDir::new().unwrap();
        let source = tmp.path().join("nonexistent-source");
        let target = tmp.path().join("target");

        fs::create_dir_all(&target).unwrap();

        let status = check_stowed_status(&source, &target, "bash-env", None).unwrap();
        assert_eq!(status, StowStatus::Missing);
    }

    #[test]
    fn test_stow_status_missing_target() {
        let tmp = TempDir::new().unwrap();
        let source = tmp.path().join("packages");
        let target = tmp.path().join("nonexistent-target");
        let pkg_src = source.join("bash-env");

        fs::create_dir_all(&pkg_src).unwrap();
        fs::write(pkg_src.join(".bashrc"), "content").unwrap();

        let status = check_stowed_status(&source, &target, "bash-env", None).unwrap();
        assert_eq!(status, StowStatus::Unstowed);
    }

    #[test]
    fn test_unlink_stowed_symlink_removes_link() {
        let tmp = TempDir::new().unwrap();
        let link = tmp.path().join("mylink");
        let target = tmp.path().join("target.txt");
        fs::write(&target, "content").unwrap();
        std::os::unix::fs::symlink(&target, &link).unwrap();

        assert!(link.is_symlink());
        let result = unlink_stowed_symlink(&link).unwrap();
        assert!(result);
        assert!(!link.exists());
        assert!(target.exists());
    }

    #[test]
    fn test_unlink_stowed_symlink_non_symlink_returns_false() {
        let tmp = TempDir::new().unwrap();
        let file = tmp.path().join("regular.txt");
        fs::write(&file, "content").unwrap();

        let result = unlink_stowed_symlink(&file).unwrap();
        assert!(!result);
        assert!(file.exists());
    }
}
