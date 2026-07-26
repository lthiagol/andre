use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use tempfile::TempDir;

#[derive(Debug)]
pub struct CommandResult {
    pub exit_code: i32,
    #[allow(dead_code)]
    pub stdout: String,
    #[allow(dead_code)]
    pub stderr: String,
}

#[derive(Debug)]
#[allow(dead_code)]
pub struct TargetSnapshotEntry {
    pub is_symlink: bool,
    pub target: Option<String>,
    pub is_dir: bool,
    pub is_file: bool,
    pub permissions: String,
}

#[allow(dead_code)]
pub struct TestEnv {
    pub temp_dir: TempDir,
    pub config_path: PathBuf,
    pub target_dir: PathBuf,
    pub packages_dir: PathBuf,
    pub scenario_name: String,
}

#[allow(dead_code)]
impl TestEnv {
    #[allow(clippy::expect_fun_call)]
    pub fn from_scenario(scenario_name: &str) -> Self {
        let temp_dir = TempDir::new().expect(&format!(
            "Failed to create temp dir for scenario {}",
            scenario_name
        ));
        let temp_path = temp_dir.path();

        let fixture_source = Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .unwrap()
            .join("tests")
            .join("e2e")
            .join(scenario_name);
        let packages_source = fixture_source.join("packages");
        let target_source = fixture_source.join("target");

        let config_path = temp_path.join("andre.yml");
        let target_dir = temp_path.join("target");
        let packages_dir = temp_path.join("packages");

        if !config_path.exists() {
            fs::copy(fixture_source.join("andre.yml"), &config_path)
                .expect("Failed to copy andre.yml");
            fs::create_dir_all(&target_dir).expect("Failed to create target dir");
            if packages_source.exists() {
                recurse_copy(&packages_source, &packages_dir).expect("Failed to copy packages");
            }
            if target_source.exists() {
                recurse_copy(&target_source, &target_dir).expect("Failed to copy target dir");
            }
        } else {
            panic!(
                "Fixture scenario '{}' not found at {:?}",
                scenario_name, fixture_source
            );
        }

        Self {
            temp_dir,
            config_path,
            target_dir,
            packages_dir,
            scenario_name: scenario_name.to_string(),
        }
    }

    pub fn stow_binary() -> PathBuf {
        which("stow").expect("stow binary not found in PATH")
    }

    pub fn awesome_stow_binary() -> PathBuf {
        let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
        let root = manifest_dir.parent().unwrap();
        let candidates = [
            root.join("target").join("llvm-cov-target").join("debug"),
            root.join("target").join("debug"),
            root.join("target").join("release"),
        ];
        for dir in &candidates {
            let path = dir.join("andre");
            if path.exists() {
                return path;
            }
        }
        candidates[1].join("andre")
    }

    pub fn run_stow(&self, args: &[&str]) -> std::io::Result<CommandResult> {
        let stow_bin = Self::stow_binary();
        let mut cmd = std::process::Command::new(&stow_bin);
        cmd.args(args)
            .current_dir(self.temp_dir.path())
            .env("HOME", self.temp_dir.path());
        let output = cmd.output()?;
        Ok(CommandResult {
            exit_code: output.status.code().unwrap_or(-1),
            stdout: String::from_utf8_lossy(&output.stdout).to_string(),
            stderr: String::from_utf8_lossy(&output.stderr).to_string(),
        })
    }

    pub fn run_awesome_stow(&self, args: &[&str]) -> std::io::Result<CommandResult> {
        let binary = Self::awesome_stow_binary();
        let mut cmd = std::process::Command::new(&binary);
        cmd.args(args)
            .current_dir(self.temp_dir.path())
            .env("HOME", self.temp_dir.path());
        let output = cmd.output()?;
        Ok(CommandResult {
            exit_code: output.status.code().unwrap_or(-1),
            stdout: String::from_utf8_lossy(&output.stdout).to_string(),
            stderr: String::from_utf8_lossy(&output.stderr).to_string(),
        })
    }

    pub fn target_snapshot(&self) -> HashMap<String, TargetSnapshotEntry> {
        let mut map = HashMap::new();
        if !self.target_dir.exists() {
            return map;
        }
        collect_snapshot(&self.target_dir, &self.target_dir, &mut map);
        map
    }

    #[allow(clippy::expect_fun_call)]
    pub fn assert_symlinks(&self, expected: &[(&str, &str)]) {
        let snapshot = self.target_snapshot();
        for (relative_path, expected_target) in expected {
            let full_path = self.target_dir.join(relative_path);
            let metadata = fs::symlink_metadata(&full_path).unwrap_or_else(|_| {
                panic!(
                    "Expected file at {:?} but it does not exist. Snapshot: {:?}",
                    full_path, snapshot
                )
            });

            assert!(
                metadata.file_type().is_symlink(),
                "Expected symlink at {:?} but found {:?}",
                full_path,
                metadata.file_type()
            );

            let link_target = fs::read_link(&full_path)
                .expect(&format!("Failed to read symlink at {:?}", full_path));
            let link_target_str = link_target.to_string_lossy().to_string();
            assert_eq!(
                link_target_str, *expected_target,
                "Symlink at {:?} points to '{}' expected '{}'",
                full_path, link_target_str, expected_target
            );
        }
    }

    pub fn assert_no_symlinks(&self) {
        let snapshot = self.target_snapshot();
        for (path, entry) in &snapshot {
            assert!(
                !entry.is_symlink,
                "Expected no symlinks but found one at: {}",
                path
            );
        }
    }

    pub fn assert_target_exists(&self, relative_path: &str) -> bool {
        self.target_dir.join(relative_path).exists()
    }

    pub fn count_symlinks(&self) -> usize {
        self.target_snapshot()
            .values()
            .filter(|e| e.is_symlink)
            .count()
    }

    pub fn cleanup_symlinks(&self) -> std::io::Result<()> {
        if !self.target_dir.exists() {
            return Ok(());
        }
        for entry in fs::read_dir(self.target_dir.as_path())? {
            let entry = entry?;
            let path = entry.path();
            let metadata = entry.metadata()?;
            if metadata.file_type().is_symlink() {
                fs::remove_file(&path)?;
            } else if metadata.is_dir() {
                fs::remove_dir_all(&path)?;
            }
        }
        Ok(())
    }
}

fn recurse_copy(src: &Path, dst: &Path) -> std::io::Result<()> {
    if src.is_dir() {
        fs::create_dir_all(dst)?;
        for entry in fs::read_dir(src)? {
            let entry = entry?;
            recurse_copy(&entry.path(), &dst.join(entry.file_name()))?;
        }
    } else {
        fs::copy(src, dst)?;
    }
    Ok(())
}

fn collect_snapshot(base: &Path, current: &Path, map: &mut HashMap<String, TargetSnapshotEntry>) {
    if !current.is_dir() {
        return;
    }
    for entry in fs::read_dir(current).unwrap() {
        let entry = entry.unwrap();
        let path = entry.path();
        let relative = path
            .strip_prefix(base)
            .unwrap()
            .to_string_lossy()
            .to_string();

        let metadata = entry.metadata().unwrap();
        let file_type = metadata.file_type();

        let is_symlink = file_type.is_symlink();
        let is_dir = file_type.is_dir();
        let is_file = file_type.is_file();

        let target = if is_symlink {
            Some(fs::read_link(&path).unwrap().to_string_lossy().to_string())
        } else {
            None
        };

        let permissions = format_permissions(&metadata);

        map.insert(
            relative,
            TargetSnapshotEntry {
                is_symlink,
                target,
                is_dir,
                is_file,
                permissions,
            },
        );

        if is_dir && !is_symlink {
            collect_snapshot(base, &path, map);
        }
    }
}

fn format_permissions(metadata: &fs::Metadata) -> String {
    use std::os::unix::fs::PermissionsExt;
    let mode = metadata.permissions().mode();
    format!(
        "{}{}{}",
        if mode & 0o400 != 0 { "r" } else { "-" },
        if mode & 0o200 != 0 { "w" } else { "-" },
        if mode & 0o100 != 0 { "x" } else { "-" }
    )
}

fn which(bin: &str) -> Option<PathBuf> {
    std::env::var_os("PATH").and_then(|path| {
        std::env::split_paths(&path)
            .filter_map(|dir| {
                let candidate = dir.join(bin);
                if candidate.exists() {
                    Some(candidate)
                } else {
                    None
                }
            })
            .next()
    })
}

#[cfg(test)]
#[allow(clippy::expect_fun_call, clippy::needless_borrows_for_generic_args)]
mod tests {
    use super::*;

    #[test]
    fn test_temp_dir_created() {
        let env = TestEnv::from_scenario("01-basic-stow");
        assert!(env.temp_dir.path().exists());
        assert!(env.config_path.exists());
    }

    #[test]
    fn test_target_snapshot_empty() {
        let env = TestEnv::from_scenario("01-basic-stow");
        let snapshot = env.target_snapshot();
        assert!(snapshot.is_empty());
    }

    #[test]
    fn test_stow_binary_exists() {
        let stow_path = TestEnv::stow_binary();
        assert!(stow_path.exists(), "stow binary should be in PATH");
    }
}
