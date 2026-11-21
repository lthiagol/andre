//! Which package a stowed link belongs to: the first folder under the packages folder that
//! the link points into. Nothing here reads the disk, so paint and tests stay hermetic.

use std::path::{Component as PathPart, Path, PathBuf};

/// The package whose files the link points into, or `None` when it points outside the
/// packages folder. `target` is the link's raw target, as `read_link` gives it.
pub(super) fn owning_package(link: &Path, target: &str, packages: &Path) -> Option<String> {
    let inside = inside_packages(link, target, packages)?;
    let package = inside.components().next()?;
    Some(package.as_os_str().to_string_lossy().into_owned())
}

/// The link's target from the packages folder, such as `shell/.bashrc`, or `None` when it
/// points outside the packages folder or at its root.
pub(super) fn inside_packages(link: &Path, target: &str, packages: &Path) -> Option<PathBuf> {
    let resolved = resolve_link(link, target);
    let inside = resolved.strip_prefix(normalize(packages)).ok()?;
    inside
        .components()
        .next()
        .is_some()
        .then(|| inside.to_path_buf())
}

/// Where a link points, as an absolute path. A relative target is taken from the link's
/// folder. Nothing is read from the disk.
pub(super) fn resolve_link(link: &Path, target: &str) -> PathBuf {
    let target = Path::new(target);
    if target.is_absolute() {
        return normalize(target);
    }
    match link.parent() {
        Some(folder) => normalize(&folder.join(target)),
        None => normalize(target),
    }
}

/// `path` with `.` dropped and each `..` taken from the part before it. Nothing is read
/// from the disk, so a link through a symlinked folder keeps its written path.
pub(super) fn normalize(path: &Path) -> PathBuf {
    let mut out = PathBuf::new();
    for part in path.components() {
        match part {
            PathPart::ParentDir => {
                out.pop();
            }
            PathPart::CurDir => {}
            other => out.push(other.as_os_str()),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_normalize_takes_dot_dot_from_the_text() {
        assert_eq!(
            normalize(Path::new("/a/b/../c/./d")),
            PathBuf::from("/a/c/d")
        );
    }

    #[test]
    fn test_resolve_link_takes_a_relative_target_from_the_links_folder() {
        assert_eq!(
            resolve_link(Path::new("/home/u/.bashrc"), "dotfiles/shell/.bashrc"),
            PathBuf::from("/home/u/dotfiles/shell/.bashrc")
        );
        assert_eq!(
            resolve_link(Path::new("/home/u/.config/x"), "../../src/y"),
            PathBuf::from("/home/src/y")
        );
    }

    #[test]
    fn test_owner_is_the_first_folder_under_the_packages_folder() {
        let packages = Path::new("/home/u/dotfiles");
        assert_eq!(
            owning_package(
                Path::new("/home/u/.bashrc"),
                "dotfiles/shell/.bashrc",
                packages
            ),
            Some("shell".to_string())
        );
        assert_eq!(
            owning_package(
                Path::new("/home/u/.config/nvim"),
                "../dotfiles/nvim/.config/nvim",
                packages
            ),
            Some("nvim".to_string())
        );
    }

    #[test]
    fn test_inside_is_the_target_from_the_packages_folder() {
        let packages = Path::new("/home/u/dotfiles");
        assert_eq!(
            inside_packages(
                Path::new("/home/u/.bashrc"),
                "dotfiles/test/.bashrc",
                packages
            ),
            Some(PathBuf::from("test/.bashrc"))
        );
        assert_eq!(
            inside_packages(Path::new("/home/u/.profile"), "/etc/profile", packages),
            None
        );
        assert_eq!(
            inside_packages(Path::new("/home/u/.x"), "dotfiles", packages),
            None
        );
    }

    #[test]
    fn test_no_owner_outside_the_packages_folder_or_at_its_root() {
        let packages = Path::new("/home/u/dotfiles");
        assert_eq!(
            owning_package(Path::new("/home/u/.profile"), "/etc/profile", packages),
            None
        );
        assert_eq!(
            owning_package(Path::new("/home/u/.x"), "dotfiles", packages),
            None,
            "the packages folder itself is no package"
        );
    }
}
