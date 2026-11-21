//! Fresh-machine bootstrap. The config folder may live inside a package of a group,
//! so that stowing the package links it to `~/.config/andre`, where plain `andre`
//! looks next time.
//!
//! [`find`] names that package. It is `None` once `~/.config/andre` exists in any form,
//! so the offer goes away by itself after the link is made.

use std::fs;
use std::path::{Path, PathBuf};

use crate::config::Config;
use crate::packages::discover_packages;

/// The package to stow so that plain `andre` finds the loaded config folder.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Bootstrap {
    pub group: String,
    pub package: String,
    /// `~/.config/andre` under the home directory: the path the package links to.
    pub link: PathBuf,
}

/// `~/.config/andre` under `home_dir`: the folder plain `andre` searches.
pub fn default_folder(home_dir: &Path) -> PathBuf {
    home_dir.join(".config").join("andre")
}

/// Whether [`default_folder`] exists in any form. A broken link counts, since it takes
/// the path just the same.
pub fn default_folder_taken(home_dir: &Path) -> bool {
    fs::symlink_metadata(default_folder(home_dir)).is_ok()
}

/// The package whose folder holds `config_dir` at the path that links to
/// [`default_folder`]. `search_dir` is the folder plain `andre` reads its config from
/// (`$XDG_CONFIG_HOME/andre`, or `None` when unknown). `None` is returned when that is not
/// [`default_folder`], since a link there would not be read. Also `None` when that path is
/// already taken, or no package holds it.
pub fn find(
    config: &Config,
    config_dir: &Path,
    home_dir: &Path,
    search_dir: Option<&Path>,
) -> Option<Bootstrap> {
    let link = default_folder(home_dir);
    if search_dir != Some(link.as_path()) || default_folder_taken(home_dir) {
        return None;
    }
    let folder = fs::canonicalize(config_dir).ok()?;

    for group in config.groups.keys() {
        let (Some(source), Some(target)) = (
            config.resolve_group_source(group, config_dir, home_dir),
            config.resolve_group_target(group, config_dir, home_dir),
        ) else {
            continue;
        };
        // The part of the link path that sits under the group's target: stow links
        // `target/<inside>` to `package/<inside>`.
        let Ok(inside) = link.strip_prefix(&target) else {
            continue;
        };
        for package in discover_packages(&source) {
            let candidate = source.join(&package).join(inside);
            if fs::canonicalize(&candidate).ok().as_deref() == Some(folder.as_path()) {
                return Some(Bootstrap {
                    group: group.clone(),
                    package,
                    link,
                });
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    /// A repo `dotfiles/config` whose package `andre` holds the config folder at
    /// `.config/andre`, and a group `config` targeting the home directory.
    fn repo_fixture() -> (TempDir, Config, PathBuf, PathBuf) {
        let tmp = TempDir::new().unwrap();
        let home = tmp.path().join("home");
        let source = tmp.path().join("dotfiles").join("config");
        let folder = source.join("andre").join(".config").join("andre");
        fs::create_dir_all(&folder).unwrap();
        fs::write(folder.join("andre.yml"), "global: {}\ngroups: {}\n").unwrap();
        fs::create_dir_all(&home).unwrap();

        let mut config = Config::default_empty();
        config.add_group(
            "config".into(),
            source.display().to_string(),
            home.display().to_string(),
        );
        (tmp, config, folder, home)
    }

    /// The folder plain `andre` reads when `$XDG_CONFIG_HOME` is unset.
    fn search(home: &Path) -> PathBuf {
        default_folder(home)
    }

    #[test]
    fn names_the_package_that_holds_the_config_folder() {
        let (_tmp, config, folder, home) = repo_fixture();
        let found = find(&config, &folder, &home, Some(&search(&home))).expect("bootstrap offered");
        assert_eq!(found.group, "config");
        assert_eq!(found.package, "andre");
        assert_eq!(found.link, home.join(".config").join("andre"));
    }

    #[test]
    fn nothing_is_offered_once_the_default_folder_exists() {
        let (_tmp, config, folder, home) = repo_fixture();
        fs::create_dir_all(home.join(".config").join("andre")).unwrap();
        assert_eq!(find(&config, &folder, &home, Some(&search(&home))), None);
    }

    #[cfg(unix)]
    #[test]
    fn a_broken_link_at_the_default_folder_also_hides_the_offer() {
        let (_tmp, config, folder, home) = repo_fixture();
        fs::create_dir_all(home.join(".config")).unwrap();
        std::os::unix::fs::symlink(home.join("gone"), home.join(".config").join("andre")).unwrap();
        assert_eq!(find(&config, &folder, &home, Some(&search(&home))), None);
    }

    #[test]
    fn nothing_is_offered_when_the_target_does_not_cover_the_link() {
        let (tmp, _config, folder, home) = repo_fixture();
        let mut config = Config::default_empty();
        config.add_group(
            "config".into(),
            tmp.path()
                .join("dotfiles")
                .join("config")
                .display()
                .to_string(),
            home.join("elsewhere").display().to_string(),
        );
        assert_eq!(find(&config, &folder, &home, Some(&search(&home))), None);
    }

    #[test]
    fn nothing_is_offered_when_xdg_config_home_points_elsewhere() {
        let (tmp, config, folder, home) = repo_fixture();
        // Plain `andre` reads `$XDG_CONFIG_HOME/andre` here, so a link at `~/.config/andre`
        // would not be read, even while that folder exists.
        let xdg = tmp.path().join("xdg").join("andre");
        fs::create_dir_all(&xdg).unwrap();
        assert_eq!(find(&config, &folder, &home, Some(&xdg)), None);
    }

    #[test]
    fn nothing_is_offered_when_the_search_folder_is_unknown() {
        let (_tmp, config, folder, home) = repo_fixture();
        assert_eq!(find(&config, &folder, &home, None), None);
    }

    #[test]
    fn nothing_is_offered_when_no_package_holds_the_config_folder() {
        let (tmp, config, _folder, home) = repo_fixture();
        let other = tmp.path().join("other-repo");
        fs::create_dir_all(&other).unwrap();
        assert_eq!(find(&config, &other, &home, Some(&search(&home))), None);
    }
}
