use std::fs;
use std::path::{Path, PathBuf};

use crate::error::{Error, Result};
use crate::stow::build_stow_args;

#[derive(Debug, Clone)]
pub enum AdoptionAction {
    CreateDir,
    MoveFile,
    StowPackage,
}

impl AdoptionAction {
    pub fn label(&self) -> &'static str {
        match self {
            AdoptionAction::CreateDir => "Create directory",
            AdoptionAction::MoveFile => "Move file",
            AdoptionAction::StowPackage => "Stow package",
        }
    }
}

#[derive(Debug, Clone)]
pub struct AdoptionStep {
    pub action: AdoptionAction,
    pub description: String,
}

#[derive(Debug, Clone)]
pub struct AdoptionPlan {
    pub group: String,
    pub target_path: PathBuf,
    pub source_path: PathBuf,
    pub package_name: String,
    pub package_dir: PathBuf,
    pub selected_files: Vec<PathBuf>,
    pub steps: Vec<AdoptionStep>,
}

pub fn plan_adoption(
    target_path: &Path,
    source_path: &Path,
    package_name: &str,
    selected_files: &[PathBuf],
) -> Result<AdoptionPlan> {
    if selected_files.is_empty() {
        return Err(Error::StowFailed(
            "No files selected for adoption".to_string(),
        ));
    }

    let package_dir = source_path.join(package_name);

    let mut steps: Vec<AdoptionStep> = Vec::new();

    if !package_dir.exists() {
        steps.push(AdoptionStep {
            action: AdoptionAction::CreateDir,
            description: format!("mkdir -p {}", package_dir.display()),
        });
    }

    for file in selected_files {
        let rel = file
            .strip_prefix(target_path)
            .map_err(|e| Error::StowFailed(format!("File not under target: {}", e)))?;

        if let Some(parent) = rel.parent() {
            if parent != Path::new("") && parent != Path::new(".") {
                let dest_parent = package_dir.join(parent);
                steps.push(AdoptionStep {
                    action: AdoptionAction::CreateDir,
                    description: format!("mkdir -p {}", dest_parent.display()),
                });
            }
        }

        let dest = package_dir.join(rel);
        let dest_exists = dest.exists();
        if dest_exists {
            return Err(Error::StowFailed(format!(
                "File already exists in package: {}",
                dest.display()
            )));
        }

        steps.push(AdoptionStep {
            action: AdoptionAction::MoveFile,
            description: format!("mv {} -> {}", file.display(), dest.display()),
        });
    }

    steps.push(AdoptionStep {
        action: AdoptionAction::StowPackage,
        description: format!(
            "stow -d {} -t {} {}",
            source_path.display(),
            target_path.display(),
            package_name
        ),
    });

    Ok(AdoptionPlan {
        group: String::new(),
        target_path: target_path.to_path_buf(),
        source_path: source_path.to_path_buf(),
        package_name: package_name.to_string(),
        package_dir,
        selected_files: selected_files.to_vec(),
        steps,
    })
}

#[allow(clippy::too_many_arguments)]
pub fn execute_adoption(
    plan: &AdoptionPlan,
    verbosity: u8,
    dry_run: bool,
    no_folding: bool,
    adopt: bool,
    dotfiles: bool,
    ignores: &[String],
) -> Result<()> {
    if !plan.package_dir.exists() {
        fs::create_dir_all(&plan.package_dir)?;
    }

    for file in &plan.selected_files {
        let rel = file
            .strip_prefix(&plan.target_path)
            .map_err(|e| Error::StowFailed(format!("File not under target: {}", e)))?;

        let dest = plan.package_dir.join(rel);

        if let Some(parent) = dest.parent() {
            if !parent.exists() {
                fs::create_dir_all(parent)?;
            }
        }

        if dest.exists() {
            return Err(Error::StowFailed(format!(
                "File already exists in package: {}",
                dest.display()
            )));
        }

        if dry_run {
            continue;
        }

        fs::rename(file, &dest)?;
    }

    if dry_run {
        return Ok(());
    }

    let mut cmd = build_stow_args(
        &plan.source_path,
        &plan.target_path,
        &plan.package_name,
        "stow",
        verbosity,
        dry_run,
        no_folding,
        adopt,
        dotfiles,
        ignores,
    );

    let status = cmd.status().map_err(Error::Io)?;
    if !status.success() {
        return Err(Error::Subprocess(status.code().unwrap_or(1)));
    }

    Ok(())
}

pub fn list_target_entries(target_path: &Path) -> Result<Vec<TargetEntry>> {
    let mut entries = Vec::new();
    let dir = fs::read_dir(target_path)?;
    for entry in dir {
        let entry = entry?;
        let path = entry.path();
        let name = entry.file_name().to_string_lossy().to_string();
        let is_symlink = path.is_symlink();
        let is_dir = path.is_dir();
        let is_artifact = name == ".DS_Store" || name == "__MACOSX";
        entries.push(TargetEntry {
            name,
            is_symlink,
            is_dir,
            is_artifact,
        });
    }
    entries.sort_by(|a, b| {
        let a_sel = !a.is_symlink && !a.is_artifact;
        let b_sel = !b.is_symlink && !b.is_artifact;
        b_sel
            .cmp(&a_sel)
            .then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase()))
    });
    Ok(entries)
}

pub fn list_target_dirs(target_path: &Path) -> Result<Vec<TargetEntry>> {
    let mut entries: Vec<TargetEntry> = list_target_entries(target_path)?
        .into_iter()
        .filter(|e| e.is_dir && !e.is_artifact)
        .collect();
    entries.sort_by_key(|a| a.name.to_lowercase());
    Ok(entries)
}

#[derive(Debug, Clone)]
pub struct TargetEntry {
    pub name: String,
    pub is_symlink: bool,
    pub is_dir: bool,
    pub is_artifact: bool,
}

impl TargetEntry {
    pub fn is_selectable(&self) -> bool {
        !self.is_symlink && !self.is_artifact
    }

    pub fn kind_icon(&self) -> &'static str {
        if self.is_artifact {
            return "x";
        }
        if self.is_symlink {
            return "~";
        }
        if self.is_dir {
            return "/";
        }
        "\u{2022}"
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_plan_adoption_basic() {
        let tmp = TempDir::new().unwrap();
        let target = tmp.path().join("target");
        let source = tmp.path().join("source");
        fs::create_dir_all(&target).unwrap();
        fs::create_dir_all(&source).unwrap();

        let file = target.join(".bashrc");
        fs::write(&file, "alias l='ls -la'").unwrap();

        let plan = plan_adoption(&target, &source, "shell", std::slice::from_ref(&file)).unwrap();

        assert_eq!(plan.package_name, "shell");
        assert_eq!(plan.selected_files.len(), 1);
        assert!(plan
            .steps
            .iter()
            .any(|s| matches!(s.action, AdoptionAction::CreateDir)));
        assert!(plan
            .steps
            .iter()
            .any(|s| matches!(s.action, AdoptionAction::MoveFile)));
        assert!(plan
            .steps
            .iter()
            .any(|s| matches!(s.action, AdoptionAction::StowPackage)));
    }

    #[test]
    fn test_plan_adoption_nested() {
        let tmp = TempDir::new().unwrap();
        let target = tmp.path().join("target");
        let source = tmp.path().join("source");
        let nested = target.join(".config").join("nvim");
        fs::create_dir_all(&nested).unwrap();
        fs::create_dir_all(&source).unwrap();

        let file = nested.join("init.lua");
        fs::write(&file, "vim.cmd('set number')").unwrap();

        let plan = plan_adoption(&target, &source, "nvim", std::slice::from_ref(&file)).unwrap();

        assert_eq!(plan.selected_files.len(), 1);
        let mkdir_count = plan
            .steps
            .iter()
            .filter(|s| matches!(s.action, AdoptionAction::CreateDir))
            .count();
        assert!(mkdir_count >= 2);
    }

    #[test]
    fn test_plan_adoption_empty_files() {
        let result = plan_adoption(Path::new("/tmp"), Path::new("/tmp/src"), "pkg", &[]);
        assert!(result.is_err());
    }

    #[test]
    fn test_plan_adoption_file_not_under_target() {
        let tmp = TempDir::new().unwrap();
        let target = tmp.path().join("target");
        let source = tmp.path().join("source");
        fs::create_dir_all(&target).unwrap();
        fs::create_dir_all(&source).unwrap();

        let file = tmp.path().join("outside.txt");
        fs::write(&file, "hello").unwrap();

        let result = plan_adoption(&target, &source, "pkg", &[file]);
        assert!(result.is_err());
    }

    #[test]
    fn test_execute_adoption_creates_symlinks() {
        let tmp = TempDir::new().unwrap();
        let target = tmp.path().join("target");
        let source = tmp.path().join("source");
        fs::create_dir_all(&target).unwrap();
        fs::create_dir_all(&source).unwrap();

        let file = target.join(".bashrc");
        let original = "export PATH=/opt/bin:$PATH";
        fs::write(&file, original).unwrap();

        let plan = plan_adoption(&target, &source, "shell", std::slice::from_ref(&file)).unwrap();
        execute_adoption(&plan, 0, false, false, false, false, &[]).unwrap();

        let dest = source.join("shell").join(".bashrc");
        assert!(dest.exists(), "File should exist in package dir");
        let content = fs::read_to_string(&dest).unwrap();
        assert_eq!(content, original, "Content should match original");
        assert!(file.is_symlink(), "Original path should now be a symlink");
    }

    #[test]
    fn test_list_target_entries() {
        let tmp = TempDir::new().unwrap();
        fs::write(tmp.path().join("file.txt"), "hello").unwrap();
        fs::create_dir(tmp.path().join("subdir")).unwrap();

        let entries = list_target_entries(tmp.path()).unwrap();
        assert_eq!(entries.len(), 2);
        let file_entry = entries.iter().find(|e| e.name == "file.txt").unwrap();
        assert!(file_entry.is_selectable());
        assert!(!file_entry.is_dir);
    }

    #[test]
    fn test_target_entry_ds_store_not_selectable() {
        let entry = TargetEntry {
            name: ".DS_Store".to_string(),
            is_symlink: false,
            is_dir: false,
            is_artifact: true,
        };
        assert!(!entry.is_selectable());
    }
}
