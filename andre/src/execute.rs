use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;

use tokio::sync::mpsc;
use tokio::time::Duration;

use andre_core::ignore::global_ignore_in;
use andre_core::{Config, LinkEngine, LinkOp, PackageEntry, StowFlags};

#[derive(Debug, Clone)]
pub struct ExecutionResult {
    /// The package's name as the rows show it, such as `shell`.
    pub name: String,
    pub success: bool,
    pub error_message: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExecutionState {
    Running,
    Completed,
}

#[derive(Debug, Clone)]
pub enum ExecutionUpdate {
    Progress { current: usize },
    Result(ExecutionResult),
    Complete,
}

/// Spawn an async execution that applies each `LinkOp` via the given engine.
/// Heavy engine work runs on `spawn_blocking` with a per-op 60s timeout so a
/// hung stow (or fs op) cannot stall the async runtime indefinitely.
/// The returned receiver closes after `ExecutionUpdate::Complete`.
pub fn spawn_execution(
    ops: Vec<LinkOp>,
    engine: Arc<dyn LinkEngine>,
) -> mpsc::Receiver<ExecutionUpdate> {
    let (tx, rx) = mpsc::channel::<ExecutionUpdate>(ops.len().max(1));

    tokio::spawn(async move {
        for (current, op) in ops.into_iter().enumerate() {
            let _ = tx
                .send(ExecutionUpdate::Progress {
                    current: current + 1,
                })
                .await;

            let engine = engine.clone();
            let op_for_task = op.clone();
            let outcome = tokio::time::timeout(
                Duration::from_secs(60),
                tokio::task::spawn_blocking(move || engine.apply(&op_for_task)),
            )
            .await;

            let error_message = match outcome {
                Ok(Ok(Ok(()))) => None,
                Ok(Ok(Err(e))) => Some(e.to_string()),
                Ok(Err(_join)) => Some("engine task panicked".to_string()),
                Err(_elapsed) => Some("Operation timed out after 60s".to_string()),
            };
            let exec_result = ExecutionResult {
                name: op.package.clone(),
                success: error_message.is_none(),
                error_message,
            };

            let _ = tx.send(ExecutionUpdate::Result(exec_result)).await;
        }

        let _ = tx.send(ExecutionUpdate::Complete).await;
    });

    rx
}

/// One `LinkOp` per selected package that resolves on disk, plus a skipped `ExecutionResult`
/// for each one that does not, in the order `selected` names them. A package is skipped when
/// it does not exist or its target folder is missing.
pub fn build_execution_results(
    selected: &[String],
    config: &Config,
    config_dir: &Path,
    home_dir: &Path,
    flags: &StowFlags,
) -> (Vec<LinkOp>, Vec<ExecutionResult>) {
    let entries: HashMap<String, PackageEntry> = config
        .package_entries(config_dir, home_dir)
        .into_iter()
        .map(|entry| (entry.name.clone(), entry))
        .collect();
    let global = global_ignore_in(home_dir);
    let mut ops = Vec::new();
    let mut skipped = Vec::new();

    for name in selected {
        let skip = |reason: &str| ExecutionResult {
            name: name.clone(),
            success: false,
            error_message: Some(format!("Skipped: {reason}")),
        };
        match entries.get(name) {
            None => skipped.push(skip("package not found")),
            Some(entry) if !entry.target.exists() => {
                skipped.push(skip("target directory not found"))
            }
            Some(entry) => ops.push(LinkOp {
                package: entry.name.clone(),
                source: entry.source.clone(),
                target: entry.target.clone(),
                flags: *flags,
                ignores: entry.ignores.clone(),
                global_ignore: Some(global.clone()),
            }),
        }
    }

    (ops, skipped)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A flat config whose packages folder is `packages` and whose target is `target`.
    fn flat_config(packages: &Path, target: &Path) -> Config {
        let tmp = packages.parent().unwrap();
        let config_path = tmp.join("andre.yml");
        std::fs::write(
            &config_path,
            format!(
                "packages: {}\ntarget: {}\nglobal: {{}}\n",
                packages.display(),
                target.display()
            ),
        )
        .unwrap();
        Config::load(&config_path).unwrap()
    }

    #[test]
    fn test_build_execution_results_skips_missing_target() {
        let tmp = tempfile::TempDir::new().unwrap();
        let packages = tmp.path().join("dotfiles");
        std::fs::create_dir_all(packages.join("test-pkg")).unwrap();
        let config = flat_config(&packages, &tmp.path().join("nonexistent"));

        let (ops, skipped) = build_execution_results(
            &["test-pkg".to_string()],
            &config,
            tmp.path(),
            tmp.path(),
            &StowFlags::default(),
        );

        assert!(ops.is_empty(), "no ops when the target is missing");
        assert_eq!(skipped.len(), 1);
        assert_eq!(skipped[0].name, "test-pkg");
        assert!(!skipped[0].success);
        assert_eq!(
            skipped[0].error_message.as_deref(),
            Some("Skipped: target directory not found")
        );
    }

    #[test]
    fn test_build_execution_results_names_each_op_by_package() {
        let tmp = tempfile::TempDir::new().unwrap();
        let packages = tmp.path().join("dotfiles");
        std::fs::create_dir_all(packages.join("test-pkg")).unwrap();
        let config = flat_config(&packages, tmp.path());

        let (ops, skipped) = build_execution_results(
            &["test-pkg".to_string()],
            &config,
            tmp.path(),
            tmp.path(),
            &StowFlags::default(),
        );

        assert!(skipped.is_empty());
        assert_eq!(ops.len(), 1);
        assert_eq!(ops[0].package, "test-pkg");
        assert_eq!(ops[0].package, "test-pkg");
        assert_eq!(ops[0].source, packages);
    }

    #[test]
    fn test_build_execution_results_skips_a_name_that_is_not_a_package() {
        let tmp = tempfile::TempDir::new().unwrap();
        let packages = tmp.path().join("dotfiles");
        std::fs::create_dir_all(&packages).unwrap();
        let config = flat_config(&packages, tmp.path());

        let (ops, skipped) = build_execution_results(
            &["ghost".to_string()],
            &config,
            tmp.path(),
            tmp.path(),
            &StowFlags::default(),
        );

        assert!(ops.is_empty());
        assert_eq!(skipped.len(), 1);
        assert_eq!(
            skipped[0].error_message.as_deref(),
            Some("Skipped: package not found")
        );
    }

    #[test]
    fn test_build_execution_results_keeps_the_order_it_is_given() {
        let tmp = tempfile::TempDir::new().unwrap();
        let packages = tmp.path().join("dotfiles");
        for name in ["alpha", "beta", "gamma"] {
            std::fs::create_dir_all(packages.join(name)).unwrap();
        }
        let config = flat_config(&packages, tmp.path());
        let selected = ["gamma".to_string(), "alpha".to_string()];

        let (ops, _) = build_execution_results(
            &selected,
            &config,
            tmp.path(),
            tmp.path(),
            &StowFlags::default(),
        );

        let names: Vec<&str> = ops.iter().map(|op| op.package.as_str()).collect();
        assert_eq!(names, ["gamma", "alpha"]);
    }
}
