use std::collections::BTreeSet;
use std::path::Path;

use crate::error::Result;
use crate::profile::ResolvedProfile;
use crate::{engine::LinkEngine, Config, LinkOp, PackageEntry, StowFlags};

/// Every package, in name order: what `--yolo` stows.
pub fn build_plan(config: &Config, config_dir: &Path, home_dir: &Path) -> Vec<PackageEntry> {
    config.package_entries(config_dir, home_dir)
}

/// The packages one resolved profile names, in name order. Refs that name no package on
/// disk are left out; the caller reports them.
pub fn profile_plan(
    config: &Config,
    config_dir: &Path,
    home_dir: &Path,
    profile: &ResolvedProfile,
) -> Vec<PackageEntry> {
    let wanted: BTreeSet<&str> = profile
        .entries
        .iter()
        .filter(|e| !e.missing)
        .map(|e| e.package.as_str())
        .collect();
    build_plan(config, config_dir, home_dir)
        .into_iter()
        .filter(|entry| wanted.contains(entry.name.as_str()))
        .collect()
}

/// Stows one package with `engine`. `global_ignore` is the `.stow-global-ignore` to read
/// (see [`LinkOp::global_ignore`]).
pub fn run_package(
    entry: &PackageEntry,
    engine: &dyn LinkEngine,
    flags: &StowFlags,
    global_ignore: Option<&Path>,
) -> Result<()> {
    engine.apply(&LinkOp {
        package: entry.name.clone(),
        source: entry.source.clone(),
        target: entry.target.clone(),
        flags: *flags,
        ignores: entry.ignores.clone(),
        global_ignore: global_ignore.map(Path::to_path_buf),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::profile::{Origin, ResolvedEntry};
    use crate::NativeEngine;
    use std::fs;
    use tempfile::TempDir;

    /// A flat config: `dotfiles/` holds `git` and `nvim`, and the hidden `.git`, and links
    /// to `target/`.
    fn flat_fixture() -> (TempDir, Config) {
        let tmp = TempDir::new().unwrap();
        let dotfiles = tmp.path().join("dotfiles");
        fs::create_dir_all(dotfiles.join("nvim")).unwrap();
        fs::create_dir_all(dotfiles.join("git")).unwrap();
        fs::create_dir_all(dotfiles.join(".git")).unwrap();
        fs::write(dotfiles.join("git/.gitconfig"), "[user]").unwrap();
        fs::write(dotfiles.join("nvim/init.lua"), "").unwrap();
        let mut config = Config::default_empty();
        config.packages = Some(dotfiles.to_string_lossy().into_owned());
        config.target = Some(tmp.path().join("target").to_string_lossy().into_owned());
        (tmp, config)
    }

    fn entry(package: &str, missing: bool) -> ResolvedEntry {
        ResolvedEntry {
            package: package.to_string(),
            origin: Origin::Own,
            missing,
        }
    }

    #[test]
    fn test_build_plan_is_every_package_in_name_order() {
        let (tmp, config) = flat_fixture();
        let plan = build_plan(&config, tmp.path(), tmp.path());
        let names: Vec<&str> = plan.iter().map(|e| e.name.as_str()).collect();
        assert_eq!(
            names,
            vec!["git", "nvim"],
            "the hidden .git is not a package"
        );
        assert_eq!(plan[0].target, tmp.path().join("target"));
        assert_eq!(plan[0].dir, tmp.path().join("dotfiles").join("git"));
    }

    #[test]
    fn test_build_plan_is_empty_without_a_packages_folder() {
        let tmp = TempDir::new().unwrap();
        let config = Config::default_empty();
        assert!(build_plan(&config, tmp.path(), tmp.path()).is_empty());
    }

    #[test]
    fn test_run_package_dry_run_succeeds_and_links_nothing() {
        let (tmp, config) = flat_fixture();
        let plan = build_plan(&config, tmp.path(), tmp.path());
        let git = plan.iter().find(|e| e.name == "git").unwrap();
        let flags = StowFlags {
            dry_run: true,
            ..StowFlags::default()
        };
        run_package(git, &NativeEngine, &flags, None).unwrap();
        assert!(!tmp.path().join("target/.gitconfig").exists());
    }

    #[test]
    fn test_run_package_native_creates_the_symlink() {
        let (tmp, config) = flat_fixture();
        fs::create_dir_all(tmp.path().join("target")).unwrap();
        let plan = build_plan(&config, tmp.path(), tmp.path());
        let git = plan.iter().find(|e| e.name == "git").unwrap();
        run_package(git, &NativeEngine, &StowFlags::default(), None).unwrap();
        assert!(
            tmp.path().join("target/.gitconfig").is_symlink(),
            "native run_package must create the symlink"
        );
    }

    #[test]
    fn test_profile_plan_keeps_only_the_profile_packages() {
        let (tmp, config) = flat_fixture();
        let profile = ResolvedProfile {
            name: "work".into(),
            entries: vec![entry("git", false), entry("ghost", true)],
        };
        let names: Vec<String> = profile_plan(&config, tmp.path(), tmp.path(), &profile)
            .into_iter()
            .map(|e| e.name)
            .collect();
        assert_eq!(names, vec!["git"]);
    }

    #[test]
    fn test_profile_plan_is_empty_when_no_ref_exists() {
        let (tmp, config) = flat_fixture();
        let profile = ResolvedProfile {
            name: "empty".into(),
            entries: vec![entry("ghost", true)],
        };
        assert!(profile_plan(&config, tmp.path(), tmp.path(), &profile).is_empty());
    }
}
