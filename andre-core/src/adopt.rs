use std::fs;
use std::path::Path;

use crate::error::Result;

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
