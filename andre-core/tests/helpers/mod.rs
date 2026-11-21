#![allow(dead_code)]

use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use tempfile::TempDir;

#[derive(Debug)]
pub struct CommandResult {
    pub exit_code: i32,
    pub stdout: String,
    pub stderr: String,
}

#[derive(Debug)]
pub struct TargetSnapshotEntry {
    pub is_symlink: bool,
    pub target: Option<String>,
    pub is_dir: bool,
    pub is_file: bool,
    pub permissions: String,
}

pub struct TestEnv {
    pub temp_dir: TempDir,
    pub config_path: PathBuf,
    pub target_dir: PathBuf,
    pub packages_dir: PathBuf,
    pub scenario_name: String,
}

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
        let profiles_source = fixture_source.join("profiles");
        let target_source = fixture_source.join("target");

        if !fixture_source.is_dir() {
            panic!(
                "Fixture scenario '{}' not found at {:?}",
                scenario_name, fixture_source
            );
        }

        let config_path = temp_path.join("andre.yml");
        let target_dir = temp_path.join("target");
        let packages_dir = temp_path.join("packages");

        fs::copy(fixture_source.join("andre.yml"), &config_path).expect("Failed to copy andre.yml");
        fs::create_dir_all(&target_dir).expect("Failed to create target dir");
        if packages_source.exists() {
            recurse_copy(&packages_source, &packages_dir).expect("Failed to copy packages");
        }
        if profiles_source.exists() {
            recurse_copy(&profiles_source, &temp_path.join("profiles"))
                .expect("Failed to copy profiles");
        }
        if target_source.exists() {
            recurse_copy(&target_source, &target_dir).expect("Failed to copy target dir");
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

    pub fn andre_binary() -> PathBuf {
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

    pub fn run_andre(&self, args: &[&str]) -> std::io::Result<CommandResult> {
        // andre no longer looks in its working directory for a config, so the scenario's
        // andre.yml is named explicitly.
        let config = self.config_path.to_string_lossy().into_owned();
        let mut full = vec!["--config", config.as_str()];
        full.extend_from_slice(args);
        run(&Self::andre_binary(), self.temp_dir.path(), &full)
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
}

fn run(bin: &Path, cwd: &Path, args: &[&str]) -> std::io::Result<CommandResult> {
    let output = std::process::Command::new(bin)
        .args(args)
        .current_dir(cwd)
        .env("HOME", cwd)
        .output()?;
    Ok(CommandResult {
        exit_code: output.status.code().unwrap_or(-1),
        stdout: String::from_utf8_lossy(&output.stdout).to_string(),
        stderr: String::from_utf8_lossy(&output.stderr).to_string(),
    })
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
