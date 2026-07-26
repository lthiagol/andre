use std::fs;
use std::path::Path;

pub fn discover_packages(source: &Path) -> Vec<String> {
    if !source.is_dir() {
        return Vec::new();
    }

    let mut packages = Vec::new();

    if let Ok(entries) = fs::read_dir(source) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                if let Some(name) = path.file_name() {
                    packages.push(name.to_string_lossy().to_string());
                }
            }
        }
    }

    packages.sort();
    packages
}

pub fn get_package_preview(source: &Path, package: &str, max_items: usize) -> Vec<String> {
    let package_path = source.join(package);

    if !package_path.is_dir() || max_items == 0 {
        return Vec::new();
    }

    let mut items = Vec::new();

    if let Ok(entries) = fs::read_dir(&package_path) {
        for entry in entries.flatten() {
            items.push(entry.file_name().to_string_lossy().to_string());
            if items.len() >= max_items {
                break;
            }
        }
    }

    items.sort();
    items
}

#[cfg(test)]
mod tests {
    use std::fs;
    use tempfile::TempDir;

    use super::*;

    #[test]
    fn test_discover_packages_empty() {
        let tmp = TempDir::new().unwrap();
        let source = tmp.path();

        let packages = discover_packages(source);
        assert!(packages.is_empty());
    }

    #[test]
    fn test_discover_packages_some() {
        let tmp = TempDir::new().unwrap();
        let source = tmp.path();

        fs::create_dir_all(source.join("bash-env")).unwrap();
        fs::create_dir_all(source.join("vim-env")).unwrap();
        fs::create_dir_all(source.join("git-env")).unwrap();

        let packages = discover_packages(source);
        assert_eq!(packages, vec!["bash-env", "git-env", "vim-env"]);
    }

    #[test]
    fn test_discover_packages_ignores_files() {
        let tmp = TempDir::new().unwrap();
        let source = tmp.path();

        fs::create_dir_all(source.join("package1")).unwrap();
        fs::write(source.join("some-file.txt"), "content").unwrap();

        let packages = discover_packages(source);
        assert_eq!(packages, vec!["package1"]);
    }

    #[test]
    fn test_get_package_preview() {
        let tmp = TempDir::new().unwrap();
        let source = tmp.path();

        let pkg_dir = source.join("my-package");
        fs::create_dir_all(&pkg_dir).unwrap();
        fs::write(pkg_dir.join(".bashrc"), "content").unwrap();
        fs::write(pkg_dir.join(".vimrc"), "content").unwrap();
        fs::write(pkg_dir.join("README.md"), "content").unwrap();

        let preview = get_package_preview(source, "my-package", 2);
        assert_eq!(preview.len(), 2);
        assert!(
            preview.contains(&String::from(".bashrc")) || preview.contains(&String::from(".vimrc"))
        );
    }

    #[test]
    fn test_get_package_preview_nonexistent() {
        let tmp = TempDir::new().unwrap();
        let source = tmp.path();

        let preview = get_package_preview(source, "nonexistent", 5);
        assert_eq!(preview.len(), 0);
    }

    #[test]
    fn test_discover_packages_nonexistent_dir() {
        let source = Path::new("/this/path/does/not/exist/at/all");
        let packages = discover_packages(source);
        assert!(packages.is_empty());
    }

    #[test]
    fn test_get_package_preview_zero_max_items() {
        let tmp = TempDir::new().unwrap();
        let source = tmp.path();

        let pkg_dir = source.join("my-package");
        fs::create_dir_all(&pkg_dir).unwrap();
        fs::write(pkg_dir.join(".bashrc"), "content").unwrap();

        let preview = get_package_preview(source, "my-package", 0);
        assert_eq!(preview.len(), 0);
    }

    #[test]
    fn test_discover_packages_mixed_files_and_dirs() {
        let tmp = TempDir::new().unwrap();
        let source = tmp.path();

        fs::create_dir_all(source.join("valid-pkg")).unwrap();
        fs::write(source.join("not-a-package.txt"), "content").unwrap();
        fs::write(source.join("also-not.txt"), "content").unwrap();

        let packages = discover_packages(source);
        assert_eq!(packages, vec!["valid-pkg"]);
    }
}
