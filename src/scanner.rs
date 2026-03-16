use anyhow::Result;
use std::path::{Path, PathBuf};

pub fn scan_org_files(dir: &Path) -> Result<Vec<PathBuf>> {
    let mut files = Vec::new();
    scan_org_files_recursive(dir, &mut files, 0)?;
    files.sort();
    Ok(files)
}

pub fn scan_org_files_recursive(dir: &Path, files: &mut Vec<PathBuf>, depth: usize) -> Result<()> {
    const MAX_DEPTH: usize = 5;
    if depth > MAX_DEPTH {
        return Ok(());
    }

    if let Some(dir_name) = dir.file_name().and_then(|n| n.to_str()) {
        if dir_name.starts_with('.') {
            return Ok(());
        }

        const SKIP_DIRS: &[&str] = &[
            "node_modules",
            "target",
            "build",
            "dist",
            ".git",
            ".svn",
            "__pycache__",
            "venv",
            "env",
            "Library",
            "Applications",
            "System",
        ];

        if SKIP_DIRS.contains(&dir_name) {
            return Ok(());
        }
    }

    if let Ok(entries) = std::fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_file() && path.extension().and_then(|s| s.to_str()) == Some("org") {
                files.push(path);
            } else if path.is_dir() {
                scan_org_files_recursive(&path, files, depth + 1)?;
            }
        }
    }
    Ok(())
}
