use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use crate::ignore::global_ignore_in;
use crate::profile::{split_package_ref, ResolvedProfile};
use crate::{discover_packages, engine::LinkEngine, path::resolve_path, Config, LinkOp, StowFlags};

/// A single group-level stow job: run stow for all packages in one group.
#[derive(Debug, Clone)]
pub struct StowJob {
    pub group: String,
    pub source: PathBuf,
    pub target: PathBuf,
    pub packages: Vec<String>,
    pub ignores: Vec<String>,
    /// The `.stow-global-ignore` path for this run (see [`LinkOp::global_ignore`]).
    pub global_ignore: Option<PathBuf>,
}

/// Build a list of stow jobs from config, skipping groups with missing sources.
pub fn build_plan(config: &Config, config_dir: &Path, home_dir: &Path) -> Vec<StowJob> {
    let mut jobs = Vec::new();
    for (group_name, group) in &config.groups {
        let source = resolve_path(&group.source, config_dir, home_dir);
        let target = resolve_path(&group.target, config_dir, home_dir);

        if !source.exists() {
            continue;
        }

        let packages = discover_packages(&source);
        if packages.is_empty() {
            continue;
        }

        let ignores = config.effective_ignores(group_name);
        jobs.push(StowJob {
            group: group_name.clone(),
            source,
            target,
            packages,
            ignores,
            global_ignore: Some(global_ignore_in(home_dir)),
        });
    }
    jobs
}

/// The jobs for one resolved profile: only the packages it names, each in its own group.
/// Refs that name no package on disk are left out; the caller reports them.
pub fn profile_plan(
    config: &Config,
    config_dir: &Path,
    home_dir: &Path,
    profile: &ResolvedProfile,
) -> Vec<StowJob> {
    let wanted: BTreeSet<(&str, &str)> = profile
        .entries
        .iter()
        .filter(|e| !e.missing)
        .filter_map(|e| split_package_ref(&e.package))
        .collect();

    let mut jobs = build_plan(config, config_dir, home_dir);
    for job in &mut jobs {
        let group = job.group.clone();
        job.packages
            .retain(|pkg| wanted.contains(&(group.as_str(), pkg.as_str())));
    }
    jobs.retain(|job| !job.packages.is_empty());
    jobs
}

/// Execute a single group-level job via the given engine, one package per op.
/// Returns `(succeeded, failed)` counts.
pub fn run_job(job: &StowJob, engine: &dyn LinkEngine, flags: &StowFlags) -> (usize, usize) {
    let mut ok = 0;
    let mut fail = 0;
    for pkg in &job.packages {
        let op = LinkOp {
            group: job.group.clone(),
            package: pkg.clone(),
            source: job.source.clone(),
            target: job.target.clone(),
            flags: *flags,
            ignores: job.ignores.clone(),
            global_ignore: job.global_ignore.clone(),
        };
        match engine.apply(&op) {
            Ok(()) => ok += 1,
            Err(_) => fail += 1,
        }
    }
    (ok, fail)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn test_build_plan_empty_groups() {
        let config = Config::default_empty();
        let tmp = tempfile::TempDir::new().unwrap();
        let plan = build_plan(&config, tmp.path(), tmp.path());
        assert!(plan.is_empty());
    }

    #[test]
    fn test_build_plan_skips_missing_source() {
        let mut config = Config::default_empty();
        config.add_group("test".into(), "/nonexistent/src".into(), "/tmp".into());
        let tmp = tempfile::TempDir::new().unwrap();
        let plan = build_plan(&config, tmp.path(), tmp.path());
        assert!(plan.is_empty(), "should skip group with missing source");
    }

    #[test]
    fn test_build_plan_includes_valid_group() {
        let tmp = tempfile::TempDir::new().unwrap();
        let src = tmp.path().join("dotfiles").join("home");
        fs::create_dir_all(src.join("pkg1")).unwrap();
        fs::write(src.join("pkg1/.bashrc"), "").unwrap();

        let mut config = Config::default_empty();
        config.add_group(
            "home".into(),
            src.to_string_lossy().into(),
            tmp.path().join("target").to_string_lossy().into(),
        );

        let plan = build_plan(&config, tmp.path(), tmp.path());
        assert_eq!(plan.len(), 1);
        assert_eq!(plan[0].group, "home");
        assert_eq!(plan[0].packages, vec!["pkg1"]);
    }

    #[test]
    fn test_run_job_dry_run_succeeds() {
        use crate::NativeEngine;
        let tmp = tempfile::TempDir::new().unwrap();
        let src = tmp.path().join("dotfiles").join("home");
        fs::create_dir_all(src.join("pkg1")).unwrap();
        fs::write(src.join("pkg1/.bashrc"), "alias x=ls").unwrap();
        let target = tmp.path().join("target");
        fs::create_dir_all(&target).unwrap();

        let job = StowJob {
            group: "home".to_string(),
            source: src,
            target,
            packages: vec!["pkg1".to_string()],
            ignores: vec![],
            global_ignore: None,
        };

        let engine = NativeEngine;
        let (ok, fail) = run_job(
            &job,
            &engine,
            &StowFlags {
                dry_run: true,
                ..StowFlags::default()
            },
        );
        assert_eq!(ok, 1);
        assert_eq!(fail, 0);
        // dry-run must not have created the symlink
        assert!(!tmp.path().join("target/.bashrc").exists());
    }

    #[test]
    fn test_run_job_native_stow_creates_symlink() {
        use crate::NativeEngine;
        let tmp = tempfile::TempDir::new().unwrap();
        let src = tmp.path().join("dotfiles").join("home");
        fs::create_dir_all(src.join("pkg1")).unwrap();
        fs::write(src.join("pkg1/.bashrc"), "alias x=ls").unwrap();
        let target = tmp.path().join("target");
        fs::create_dir_all(&target).unwrap();

        let job = StowJob {
            group: "home".to_string(),
            source: src,
            target,
            packages: vec!["pkg1".to_string()],
            ignores: vec![],
            global_ignore: None,
        };

        let engine = NativeEngine;
        let (ok, fail) = run_job(
            &job,
            &engine,
            &StowFlags {
                dry_run: false,
                ..StowFlags::default()
            },
        );
        assert_eq!((ok, fail), (1, 0));
        assert!(
            tmp.path().join("target/.bashrc").is_symlink(),
            "native run_job must create the symlink"
        );
    }

    #[test]
    fn test_profile_plan_keeps_only_the_profile_packages() {
        use crate::profile::{Origin, ResolvedEntry, ResolvedProfile};
        let tmp = tempfile::TempDir::new().unwrap();
        let src = tmp.path().join("dotfiles");
        for pkg in ["bash", "git", "nvim"] {
            fs::create_dir_all(src.join("home").join(pkg)).unwrap();
            fs::write(src.join("home").join(pkg).join(".rc"), "").unwrap();
        }
        fs::create_dir_all(src.join("tools").join("jq")).unwrap();
        let mut config = Config::default_empty();
        config.add_group(
            "home".into(),
            src.join("home").to_string_lossy().into(),
            tmp.path().join("target").to_string_lossy().into(),
        );
        config.add_group(
            "tools".into(),
            src.join("tools").to_string_lossy().into(),
            tmp.path().join("target").to_string_lossy().into(),
        );

        let entry = |package: &str, missing: bool| ResolvedEntry {
            package: package.to_string(),
            origin: Origin::Own,
            missing,
        };
        let profile = ResolvedProfile {
            name: "work".into(),
            entries: vec![
                entry("home/git", false),
                entry("tools/jq", false),
                entry("home/ghost", true),
            ],
        };

        let plan = profile_plan(&config, tmp.path(), tmp.path(), &profile);
        let planned: Vec<(&str, Vec<&str>)> = plan
            .iter()
            .map(|job| {
                (
                    job.group.as_str(),
                    job.packages.iter().map(String::as_str).collect(),
                )
            })
            .collect();
        assert_eq!(planned, vec![("home", vec!["git"]), ("tools", vec!["jq"])]);
    }

    #[test]
    fn test_profile_plan_is_empty_when_no_ref_exists() {
        use crate::profile::{Origin, ResolvedEntry, ResolvedProfile};
        let config = Config::default_empty();
        let tmp = tempfile::TempDir::new().unwrap();
        let profile = ResolvedProfile {
            name: "empty".into(),
            entries: vec![ResolvedEntry {
                package: "home/git".into(),
                origin: Origin::Own,
                missing: true,
            }],
        };
        assert!(profile_plan(&config, tmp.path(), tmp.path(), &profile).is_empty());
    }
}
