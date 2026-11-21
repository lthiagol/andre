//! Fresh-machine bootstrap. The config folder may live inside a package of the packages folder,
//! so that stowing the package links it to `~/.config/andre`, where plain `andre`
//! looks next time.
//!
//! [`find`] names that package. It is `None` once `~/.config/andre` exists in any form,
//! so the offer goes away by itself after the link is made.

use std::fs;
use std::path::{Path, PathBuf};

use crate::config::Config;

/// The package to stow so that plain `andre` finds the loaded config folder.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Bootstrap {
    /// The package's name as the UI shows it: `andre`.
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
/// already taken, when the target is not the home folder, or no package holds it.
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
    if config.target_dir(config_dir, home_dir) != home_dir {
        return None;
    }
    let folder = fs::canonicalize(config_dir).ok()?;

    for entry in config.package_entries(config_dir, home_dir) {
        // The part of the link path that sits under the target: stow links
        // `target/<inside>` to `package/<inside>`.
        let Ok(inside) = link.strip_prefix(&entry.target) else {
            continue;
        };
        let candidate = entry.dir.join(inside);
        if fs::canonicalize(&candidate).ok().as_deref() == Some(folder.as_path()) {
            return Some(Bootstrap {
                package: entry.name,
                link,
            });
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    /// A flat packages folder `dotfiles/` whose package `andre` holds the config folder at
    /// `.config/andre`. The target is the home directory (the default).
    fn repo_fixture() -> (TempDir, Config, PathBuf, PathBuf) {
        let tmp = TempDir::new().unwrap();
        let home = tmp.path().join("home");
        let packages = tmp.path().join("dotfiles");
        let folder = packages.join("andre").join(".config").join("andre");
        fs::create_dir_all(&folder).unwrap();
        fs::write(folder.join("andre.yml"), "global: {}\n").unwrap();
        fs::create_dir_all(&home).unwrap();

        let mut config = Config::default_empty();
        config.packages = Some(packages.display().to_string());
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
        let (_tmp, mut config, folder, home) = repo_fixture();
        config.target = Some(home.join("elsewhere").display().to_string());
        assert_eq!(find(&config, &folder, &home, Some(&search(&home))), None);
    }

    #[test]
    fn nothing_is_offered_when_the_target_is_not_the_home_folder() {
        let (tmp, mut config, _folder, home) = repo_fixture();
        // Stowing `andre/andre` into `~/.config` would link the same path, but only a
        // home target is offered.
        let folder = tmp.path().join("dotfiles").join("andre").join("andre");
        fs::create_dir_all(&folder).unwrap();
        config.target = Some(home.join(".config").display().to_string());
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
