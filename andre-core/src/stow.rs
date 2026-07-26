use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use crate::error::{Error, Result};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StowStatus {
    Stowed,
    Unstowed,
    Partial,
    Conflict,
    Missing,
    Empty,
}

impl StowStatus {
    pub fn icon(&self) -> &'static str {
        match self {
            StowStatus::Stowed => "✓",
            StowStatus::Unstowed => "→",
            StowStatus::Partial => "◐",
            StowStatus::Conflict => "⚠",
            StowStatus::Missing => "?",
            StowStatus::Empty => "∅",
        }
    }

    pub fn label(&self) -> &'static str {
        match self {
            StowStatus::Stowed => "Stowed",
            StowStatus::Unstowed => "Unstowed",
            StowStatus::Partial => "Partial",
            StowStatus::Conflict => "Conflict",
            StowStatus::Missing => "Missing",
            StowStatus::Empty => "Empty",
        }
    }

    pub fn is_stowed(&self) -> bool {
        *self == StowStatus::Stowed
    }
}

pub fn check_stowed_status(source: &Path, target: &Path, package: &str) -> Result<StowStatus> {
    let package_source = source.join(package);

    if !package_source.exists() {
        return Ok(StowStatus::Missing);
    }

    let files = collect_package_files(&package_source);

    if files.is_empty() {
        return Ok(StowStatus::Empty);
    }

    let mut stowed_count = 0;
    let mut conflict_count = 0;

    for file in &files {
        let relative = match file.strip_prefix(&package_source) {
            Ok(p) => p.to_path_buf(),
            Err(_) => file
                .file_name()
                .map(PathBuf::from)
                .unwrap_or_else(|| file.clone()),
        };
        let target_path = target.join(&relative);

        if !target_path.exists() {
            continue;
        }

        let metadata = match fs::symlink_metadata(&target_path) {
            Ok(m) => m,
            Err(_) => {
                continue;
            }
        };

        if metadata.file_type().is_symlink() {
            // Direct symlink — verify it points to the right place
            let link_target = match fs::read_link(&target_path) {
                Ok(lt) => lt,
                Err(_) => {
                    conflict_count += 1;
                    continue;
                }
            };

            let link_target_abs = target_path
                .parent()
                .map(|p| p.join(&link_target))
                .unwrap_or(link_target);
            let link_target_canonical = match fs::canonicalize(&link_target_abs) {
                Ok(p) => p,
                Err(_) => {
                    conflict_count += 1;
                    continue;
                }
            };

            let file_canonical = match fs::canonicalize(file) {
                Ok(p) => p,
                Err(_) => {
                    conflict_count += 1;
                    continue;
                }
            };

            if link_target_canonical == file_canonical {
                stowed_count += 1;
            } else {
                conflict_count += 1;
            }
        } else {
            // Not a direct symlink — check folding: a parent directory symlink
            // may point into the package source (stow default behavior).
            match (fs::canonicalize(&target_path), fs::canonicalize(file)) {
                (Ok(t), Ok(f)) if t == f => stowed_count += 1,
                _ => conflict_count += 1,
            }
        }
    }

    if conflict_count > 0 {
        Ok(StowStatus::Conflict)
    } else if stowed_count == files.len() {
        Ok(StowStatus::Stowed)
    } else if stowed_count == 0 {
        Ok(StowStatus::Unstowed)
    } else {
        Ok(StowStatus::Partial)
    }
}

pub fn collect_package_files(package_dir: &Path) -> Vec<PathBuf> {
    let mut files = Vec::new();

    if !package_dir.is_dir() {
        return files;
    }

    if let Ok(entries) = fs::read_dir(package_dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_file() {
                files.push(path);
            } else if path.is_dir() {
                files.extend(collect_package_files(&path));
            }
        }
    }

    files
}

#[allow(clippy::too_many_arguments)]
pub fn build_stow_args(
    source: &Path,
    target: &Path,
    package: &str,
    action: &str,
    verbosity: u8,
    dry_run: bool,
    no_folding: bool,
    adopt: bool,
    dotfiles: bool,
    ignores: &[String],
) -> Command {
    let mut cmd = Command::new("stow");

    cmd.arg("-d");
    cmd.arg(source.display().to_string());
    cmd.arg("-t");
    cmd.arg(target.display().to_string());

    match action {
        "unstow" => {
            cmd.arg("-D");
        }
        "restow" => {
            cmd.arg("-R");
        }
        _ => {}
    }

    if verbosity > 0 {
        for _ in 0..verbosity {
            cmd.arg("-v");
        }
    }

    if dry_run {
        cmd.arg("-n");
    }

    if no_folding {
        cmd.arg("--no-folding");
    }

    if adopt {
        cmd.arg("--adopt");
    }

    if dotfiles {
        cmd.arg("--dotfiles");
    }

    for ignore in ignores {
        cmd.arg(format!("--ignore={}", ignore));
    }

    cmd.arg(package);

    cmd
}

#[allow(clippy::too_many_arguments)]
pub fn build_stow_args_multi(
    source: &Path,
    target: &Path,
    packages: &[String],
    action: &str,
    verbosity: u8,
    dry_run: bool,
    no_folding: bool,
    adopt: bool,
    dotfiles: bool,
    ignores: &[String],
) -> Command {
    let mut cmd = Command::new("stow");

    cmd.arg("-d");
    cmd.arg(source.display().to_string());
    cmd.arg("-t");
    cmd.arg(target.display().to_string());

    match action {
        "unstow" => {
            cmd.arg("-D");
        }
        "restow" => {
            cmd.arg("-R");
        }
        _ => {}
    }

    if verbosity > 0 {
        for _ in 0..verbosity {
            cmd.arg("-v");
        }
    }

    if dry_run {
        cmd.arg("-n");
    }

    if no_folding {
        cmd.arg("--no-folding");
    }

    if adopt {
        cmd.arg("--adopt");
    }

    if dotfiles {
        cmd.arg("--dotfiles");
    }

    for ignore in ignores {
        cmd.arg(format!("--ignore={}", ignore));
    }

    for pkg in packages {
        cmd.arg(pkg);
    }

    cmd
}

#[allow(clippy::too_many_arguments)]
pub fn execute_stow(
    source: &Path,
    target: &Path,
    package: &str,
    action: &str,
    verbosity: u8,
    dry_run: bool,
    no_folding: bool,
    adopt: bool,
    dotfiles: bool,
    ignores: &[String],
) -> Result<i32> {
    let mut cmd = build_stow_args(
        source, target, package, action, verbosity, dry_run, no_folding, adopt, dotfiles, ignores,
    );
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

        let status = check_stowed_status(&source, &target, "bash-env").unwrap();
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

        let status = check_stowed_status(&source, &target, "bash-env").unwrap();
        assert_eq!(status, StowStatus::Unstowed);
    }

    #[test]
    fn test_stow_status_missing() {
        let tmp = TempDir::new().unwrap();
        let source = tmp.path().join("packages");
        let target = tmp.path().join("target");

        fs::create_dir_all(&source).unwrap();
        fs::create_dir_all(&target).unwrap();

        let status = check_stowed_status(&source, &target, "nonexistent").unwrap();
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

        let status = check_stowed_status(&source, &target, "empty-pkg").unwrap();
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

        let status = check_stowed_status(&source, &target, "bash-env").unwrap();
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

        let status = check_stowed_status(&source, &target, "bash-env").unwrap();
        assert_eq!(status, StowStatus::Partial);
    }

    #[test]
    fn test_build_stow_args_stow() {
        let source = PathBuf::from("/src/packages");
        let target = PathBuf::from("/home/user");

        let cmd = build_stow_args(
            &source,
            &target,
            "bash-env",
            "stow",
            0,
            false,
            false,
            false,
            false,
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
            "unstow",
            0,
            false,
            false,
            false,
            false,
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
            "stow",
            2,
            false,
            false,
            false,
            false,
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
            "stow",
            0,
            true,
            false,
            false,
            false,
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
            "stow",
            0,
            false,
            false,
            false,
            false,
            &[String::from(".git"), String::from(".DS_Store")],
        );

        let args: Vec<&str> = cmd.get_args().map(|a| a.to_str().unwrap()).collect();
        assert!(args.contains(&"--ignore=.git"));
        assert!(args.contains(&"--ignore=.DS_Store"));
    }

    #[test]
    fn test_stow_status_icon_and_label() {
        assert_eq!(StowStatus::Stowed.icon(), "✓");
        assert_eq!(StowStatus::Stowed.label(), "Stowed");
        assert!(StowStatus::Stowed.is_stowed());

        assert_eq!(StowStatus::Unstowed.icon(), "→");
        assert!(!StowStatus::Unstowed.is_stowed());

        assert_eq!(StowStatus::Conflict.icon(), "⚠");
        assert_eq!(StowStatus::Missing.icon(), "?");
        assert_eq!(StowStatus::Empty.icon(), "∅");
        assert_eq!(StowStatus::Partial.icon(), "◐");
    }

    #[test]
    fn test_build_stow_args_restow() {
        let source = PathBuf::from("/src/packages");
        let target = PathBuf::from("/home/user");

        let cmd = build_stow_args(
            &source,
            &target,
            "bash-env",
            "restow",
            0,
            false,
            false,
            false,
            false,
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
            "stow",
            0,
            false,
            true,
            false,
            false,
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
            "stow",
            0,
            false,
            false,
            true,
            false,
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
            "stow",
            0,
            false,
            false,
            false,
            true,
            &[],
        );

        let args: Vec<&str> = cmd.get_args().map(|a| a.to_str().unwrap()).collect();
        assert!(args.contains(&"--dotfiles"));
    }

    #[test]
    fn test_stow_status_missing_source() {
        let tmp = TempDir::new().unwrap();
        let source = tmp.path().join("nonexistent-source");
        let target = tmp.path().join("target");

        fs::create_dir_all(&target).unwrap();

        let status = check_stowed_status(&source, &target, "bash-env").unwrap();
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

        let status = check_stowed_status(&source, &target, "bash-env").unwrap();
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
