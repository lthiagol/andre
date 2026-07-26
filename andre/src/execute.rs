use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use tokio::sync::mpsc;
use tokio::time::Duration;

use andre_core::{Config, LinkEngine, LinkOp};

type CommandEntry = (String, String, PathBuf, PathBuf, Vec<String>);

#[derive(Debug, Clone)]
pub struct ExecutionResult {
    pub group: String,
    pub package: String,
    pub success: bool,
    pub error_message: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[allow(dead_code)]
pub enum ExecutionState {
    #[default]
    Pending,
    Running,
    Cancelling,
    ErrorPrompt,
    Completed,
}

#[derive(Debug, Clone)]
pub enum ExecutionUpdate {
    Progress {
        current: usize,
        total: usize,
        _package: String,
    },
    Result(ExecutionResult),
    Complete,
}

pub struct AsyncExecutor {
    _tx: mpsc::Sender<ExecutionUpdate>,
    _abort_handle: tokio::task::AbortHandle,
}

impl AsyncExecutor {
    /// Spawn an async execution that applies each `LinkOp` via the given engine.
    /// Heavy engine work runs on `spawn_blocking` with a per-op 60s timeout so a
    /// hung stow (or fs op) cannot stall the async runtime indefinitely.
    pub fn spawn(
        ops: Vec<LinkOp>,
        engine: Arc<dyn LinkEngine>,
    ) -> (Self, mpsc::Receiver<ExecutionUpdate>) {
        let (tx, rx) = mpsc::channel::<ExecutionUpdate>(ops.len().max(1));
        let tx_clone = tx.clone();
        let total = ops.len();

        let handle = tokio::spawn(async move {
            for (current, op) in ops.into_iter().enumerate() {
                let _ = tx_clone
                    .send(ExecutionUpdate::Progress {
                        current: current + 1,
                        total,
                        _package: format!("{} / {}", op.group, op.package),
                    })
                    .await;

                let engine = engine.clone();
                let op_for_task = op.clone();
                let outcome = tokio::time::timeout(
                    Duration::from_secs(60),
                    tokio::task::spawn_blocking(move || engine.apply(&op_for_task)),
                )
                .await;

                let exec_result = match outcome {
                    Ok(Ok(Ok(()))) => ExecutionResult {
                        group: op.group.clone(),
                        package: op.package.clone(),
                        success: true,
                        error_message: None,
                    },
                    Ok(Ok(Err(e))) => ExecutionResult {
                        group: op.group.clone(),
                        package: op.package.clone(),
                        success: false,
                        error_message: Some(e.to_string()),
                    },
                    Ok(Err(_join)) => ExecutionResult {
                        group: op.group.clone(),
                        package: op.package.clone(),
                        success: false,
                        error_message: Some("engine task panicked".to_string()),
                    },
                    Err(_elapsed) => ExecutionResult {
                        group: op.group.clone(),
                        package: op.package.clone(),
                        success: false,
                        error_message: Some("Operation timed out after 60s".to_string()),
                    },
                };

                let _ = tx_clone.send(ExecutionUpdate::Result(exec_result)).await;
            }

            let _ = tx_clone.send(ExecutionUpdate::Complete).await;
        });

        (
            Self {
                _tx: tx,
                _abort_handle: handle.abort_handle(),
            },
            rx,
        )
    }

    #[allow(dead_code)]
    pub fn abort(&self) {
        self._abort_handle.abort();
    }
}

pub fn build_execution_results(
    selected_packages: &HashMap<String, Vec<String>>,
    config: &Config,
    config_dir: &Path,
    home_dir: &Path,
) -> (Vec<CommandEntry>, Vec<ExecutionResult>) {
    let mut commands = Vec::new();
    let mut skipped = Vec::new();

    for (group, packages) in selected_packages {
        let source = config.get_group_source(group, config_dir, home_dir);
        let target = config.get_group_target(group, config_dir, home_dir);
        let ignores = config.effective_ignores(group);

        if let (Some(src), Some(tgt)) = (source, target) {
            for pkg in packages {
                commands.push((
                    group.clone(),
                    pkg.clone(),
                    src.clone(),
                    tgt.clone(),
                    ignores.clone(),
                ));
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

    (commands, skipped)
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

        let (commands, skipped) =
            build_execution_results(&selected, &config, tmp.path(), tmp.path());

        assert!(
            commands.is_empty(),
            "expected no commands when target dir is missing, got {}",
            commands.len()
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

        let (commands, skipped) =
            build_execution_results(&selected, &config, tmp.path(), tmp.path());

        assert!(
            commands.is_empty(),
            "expected no commands when source dir is missing, got {}",
            commands.len()
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
