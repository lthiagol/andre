use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;

use tokio::sync::mpsc;
use tokio::time::Duration;

use andre_core::{Config, LinkEngine, LinkOp, StowFlags};

#[derive(Debug, Clone)]
pub struct ExecutionResult {
    pub group: String,
    pub package: String,
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
                group: op.group.clone(),
                package: op.package.clone(),
                success: error_message.is_none(),
                error_message,
            };

            let _ = tx.send(ExecutionUpdate::Result(exec_result)).await;
        }

        let _ = tx.send(ExecutionUpdate::Complete).await;
    });

    rx
}

/// One `LinkOp` per selected package whose group resolves on disk, plus a skipped
/// `ExecutionResult` for each package whose source or target directory is missing.
pub fn build_execution_results(
    selected_packages: &HashMap<String, Vec<String>>,
    config: &Config,
    config_dir: &Path,
    home_dir: &Path,
    flags: &StowFlags,
) -> (Vec<LinkOp>, Vec<ExecutionResult>) {
    let mut ops = Vec::new();
    let mut skipped = Vec::new();

    for (group, packages) in selected_packages {
        let source = config.get_group_source(group, config_dir, home_dir);
        let target = config.get_group_target(group, config_dir, home_dir);
        let ignores = config.effective_ignores(group);

        if let (Some(src), Some(tgt)) = (source, target) {
            for pkg in packages {
                ops.push(LinkOp {
                    group: group.clone(),
                    package: pkg.clone(),
                    source: src.clone(),
                    target: tgt.clone(),
                    flags: *flags,
                    ignores: ignores.clone(),
                });
            }
        } else {
            for pkg in packages {
                let reason = if config
                    .get_group_source(group, config_dir, home_dir)
                    .is_none()
                {
                    "source directory not found"
                } else {
                    "target directory not found"
                };
                skipped.push(ExecutionResult {
                    group: group.clone(),
                    package: pkg.clone(),
                    success: false,
                    error_message: Some(format!("Skipped: {}", reason)),
                });
            }
        }
    }

    (ops, skipped)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_config_with_missing_target() -> (Config, tempfile::TempDir) {
        let tmp = tempfile::TempDir::new().unwrap();
        let config_path = tmp.path().join("andre.yml");
        std::fs::write(
            &config_path,
            r#"
global: {}
groups:
  test-group:
    source: /tmp
    target: /nonexistent/target/dir
"#,
        )
        .unwrap();
        let config = Config::load(&config_path).unwrap();
        (config, tmp)
    }

    #[test]
    fn test_build_execution_results_skips_missing_target() {
        let (config, tmp) = make_config_with_missing_target();
        let mut selected = HashMap::new();
        selected.insert("test-group".to_string(), vec!["test-pkg".to_string()]);

        let (ops, skipped) = build_execution_results(
            &selected,
            &config,
            tmp.path(),
            tmp.path(),
            &StowFlags::default(),
        );

        assert!(
            ops.is_empty(),
            "expected no ops when target dir is missing, got {}",
            ops.len()
        );
        assert_eq!(skipped.len(), 1, "expected one skipped result");
        assert!(!skipped[0].success);
        assert!(skipped[0]
            .error_message
            .as_deref()
            .unwrap()
            .contains("target directory"));
    }

    #[test]
    fn test_build_execution_results_skips_missing_source() {
        let tmp = tempfile::TempDir::new().unwrap();
        let config_path = tmp.path().join("andre.yml");
        std::fs::write(
            &config_path,
            r#"
global: {}
groups:
  test-group:
    source: /nonexistent/source
    target: /tmp
"#,
        )
        .unwrap();
        let config = Config::load(&config_path).unwrap();
        let mut selected = HashMap::new();
        selected.insert("test-group".to_string(), vec!["test-pkg".to_string()]);

        let (ops, skipped) = build_execution_results(
            &selected,
            &config,
            tmp.path(),
            tmp.path(),
            &StowFlags::default(),
        );

        assert!(
            ops.is_empty(),
            "expected no ops when source dir is missing, got {}",
            ops.len()
        );
        assert_eq!(skipped.len(), 1, "expected one skipped result");
        assert!(!skipped[0].success);
        assert!(skipped[0]
            .error_message
            .as_deref()
            .unwrap()
            .contains("source directory"));
    }
}
