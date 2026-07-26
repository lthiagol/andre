use std::path::{Path, PathBuf};

use crate::{
    discover_packages, engine::LinkEngine, path::resolve_path, Config, LinkOp, StowAction,
};

/// A single group-level stow job: run stow for all packages in one group.
#[derive(Debug, Clone)]
pub struct StowJob {
    pub group: String,
    pub source: PathBuf,
    pub target: PathBuf,
    pub packages: Vec<String>,
    pub ignores: Vec<String>,
}

/// Aggregate result from running a set of jobs.
#[derive(Debug, Clone, Default)]
pub struct YoloResult {
    pub total: usize,
    pub succeeded: usize,
    pub failed: usize,
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
        });
    }
    jobs
}

/// Execute a single group-level job via the given engine, one package per op.
/// Returns `(succeeded, failed)` counts.
#[allow(clippy::too_many_arguments)]
pub fn run_job(
    job: &StowJob,
    engine: &dyn LinkEngine,
    action: StowAction,
    verbosity: u8,
    dry_run: bool,
    no_folding: bool,
    adopt: bool,
    dotfiles: bool,
) -> (usize, usize) {
    let mut ok = 0;
    let mut fail = 0;
    for pkg in &job.packages {
        let op = LinkOp {
            group: job.group.clone(),
            package: pkg.clone(),
            source: job.source.clone(),
            target: job.target.clone(),
            action,
            dry_run,
            no_folding,
            adopt,
            dotfiles,
            verbosity,
            ignores: job.ignores.clone(),
        };
        match engine.apply(&op) {
            Ok(()) => ok += 1,
            Err(_) => fail += 1,
        }
    }
    (ok, fail)
}

/// Execute all jobs via the engine, collecting aggregate results.
#[allow(clippy::too_many_arguments)]
pub fn run_all(
    jobs: &[StowJob],
    engine: &dyn LinkEngine,
    action: StowAction,
    verbosity: u8,
    dry_run: bool,
    no_folding: bool,
    adopt: bool,
    dotfiles: bool,
) -> YoloResult {
    let mut result = YoloResult::default();
    for job in jobs {
        result.total += job.packages.len();
        let (ok, fail) = run_job(
            job, engine, action, verbosity, dry_run, no_folding, adopt, dotfiles,
        );
        result.succeeded += ok;
        result.failed += fail;
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::StowAction;
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
        };

        let engine = NativeEngine;
        let (ok, fail) = run_job(
            &job,
            &engine,
            StowAction::Stow,
            0,
            true,
            false,
            false,
            false,
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
        };

        let engine = NativeEngine;
        let (ok, fail) = run_job(
            &job,
            &engine,
            StowAction::Stow,
            0,
            false,
            false,
            false,
            false,
        );
        assert_eq!((ok, fail), (1, 0));
        assert!(
            tmp.path().join("target/.bashrc").is_symlink(),
            "native run_job must create the symlink"
        );
    }
}
