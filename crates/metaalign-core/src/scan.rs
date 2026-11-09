//! Folder scanning helpers.

use std::path::{Path, PathBuf};

use walkdir::WalkDir;

use crate::office::supported_extensions;

/// Filters applied while walking a directory tree.
#[derive(Clone, Debug)]
pub struct ScanFilter {
    pub recursive: bool,
    /// Lower-case extensions without the dot. Empty means "everything".
    pub extensions: Vec<String>,
    /// Skip Office lock files (`~$document.docx`).
    pub skip_lock_files: bool,
}

impl Default for ScanFilter {
    fn default() -> Self {
        Self {
            recursive: true,
            extensions: supported_extensions()
                .iter()
                .map(|extension| extension.to_string())
                .collect(),
            skip_lock_files: true,
        }
    }
}

impl ScanFilter {
    /// Name/extension test only — callers are responsible for `is_file()`.
    pub fn matches(&self, path: &Path) -> bool {
        if self.skip_lock_files {
            if let Some(name) = path.file_name().and_then(|name| name.to_str()) {
                if name.starts_with("~$") {
                    return false;
                }
            }
        }
        if self.extensions.is_empty() {
            return true;
        }
        match path.extension().and_then(|extension| extension.to_str()) {
            Some(extension) => {
                let lowered = extension.to_ascii_lowercase();
                self.extensions.iter().any(|item| item == &lowered)
            }
            None => false,
        }
    }
}

/// Result of a directory walk.
#[derive(Clone, Debug, Default)]
pub struct ScanResult {
    pub files: Vec<PathBuf>,
    pub errors: Vec<String>,
}

/// Walk every root and collect matching files.
pub fn collect(roots: &[PathBuf], filter: &ScanFilter) -> ScanResult {
    let mut result = ScanResult::default();
    let mut seen = std::collections::HashSet::new();

    for root in roots {
        if root.is_file() {
            if filter.matches(root) && seen.insert(root.clone()) {
                result.files.push(root.clone());
            }
            continue;
        }
        let walker = WalkDir::new(root)
            .follow_links(false)
            .max_depth(if filter.recursive { usize::MAX } else { 1 })
            .into_iter();
        for entry in walker.filter_map(|entry| entry.ok()) {
            if !entry.file_type().is_file() {
                continue;
            }
            let path = entry.path();
            if filter.matches(path) && seen.insert(path.to_path_buf()) {
                result.files.push(path.to_path_buf());
            }
        }
    }

    result
        .files
        .sort_by(|left, right| left.to_string_lossy().cmp(&right.to_string_lossy()));
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn filters_by_extension_and_lock_files() {
        let filter = ScanFilter {
            recursive: true,
            extensions: vec!["docx".to_string()],
            skip_lock_files: true,
        };
        assert!(filter.matches(Path::new("D:/docs/报告.docx")));
        assert!(!filter.matches(Path::new("D:/docs/报告.pdf")));
        assert!(!filter.matches(Path::new("D:/docs/~$报告.docx")));
    }
}
