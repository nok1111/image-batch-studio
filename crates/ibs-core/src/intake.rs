use crate::ImageFormat;
use serde::Serialize;
use std::collections::HashSet;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use ts_rs::TS;
use walkdir::WalkDir;

/// Upper bound on files accepted in one scan, to keep memory bounded.
pub const MAX_INPUTS: usize = 100_000;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct InputFile {
    pub path: String,
    pub name: String,
    #[ts(type = "number")]
    pub size: u64,
    pub format: ImageFormat,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ScanResult {
    pub files: Vec<InputFile>,
    /// Files ignored because their type is not supported.
    pub unsupported: u32,
    /// True when `MAX_INPUTS` was reached or the scan was cancelled.
    pub truncated: bool,
}

/// Expands files and folders (recursively) into supported image files.
/// Hidden entries are ignored, symlinked folders are not followed and
/// duplicates are removed. Order: as given, folders sorted by name.
pub fn scan(paths: &[PathBuf], cancel: &AtomicBool) -> ScanResult {
    let mut out = ScanResult::default();
    let mut seen = HashSet::new();
    'roots: for root in paths {
        let walker = WalkDir::new(root)
            .follow_links(false)
            .sort_by_file_name()
            .into_iter()
            .filter_entry(|e| e.depth() == 0 || !is_hidden(e.file_name()));
        for entry in walker.flatten() {
            if cancel.load(Ordering::Relaxed) || out.files.len() >= MAX_INPUTS {
                out.truncated = true;
                break 'roots;
            }
            if !entry.file_type().is_file() {
                continue;
            }
            let path = entry.path();
            let Some(format) = ImageFormat::from_input_path(path) else {
                out.unsupported += 1;
                continue;
            };
            let key = std::fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf());
            if !seen.insert(key) {
                continue;
            }
            let size = entry.metadata().map(|m| m.len()).unwrap_or(0);
            out.files.push(InputFile {
                path: path.to_string_lossy().into_owned(),
                name: entry.file_name().to_string_lossy().into_owned(),
                size,
                format,
            });
        }
    }
    out
}

fn is_hidden(name: &std::ffi::OsStr) -> bool {
    name.to_str().is_some_and(|s| s.starts_with('.'))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    fn touch(p: &Path) {
        std::fs::create_dir_all(p.parent().unwrap()).unwrap();
        std::fs::write(p, b"x").unwrap();
    }

    #[test]
    fn expands_folders_filters_and_dedupes() {
        let dir = tempfile::tempdir().unwrap();
        let d = dir.path();
        touch(&d.join("b.PNG"));
        touch(&d.join("a.jpg"));
        touch(&d.join("notes.txt"));
        touch(&d.join("sub/c.webp"));
        touch(&d.join(".hidden/d.png"));
        touch(&d.join(".e.png"));
        let cancel = AtomicBool::new(false);
        let r = scan(&[d.to_path_buf(), d.join("a.jpg")], &cancel);
        let names: Vec<_> = r.files.iter().map(|f| f.name.as_str()).collect();
        assert_eq!(names, ["a.jpg", "b.PNG", "c.webp"]);
        assert_eq!(r.unsupported, 1);
        assert!(!r.truncated);
        assert_eq!(r.files[1].format, ImageFormat::Png);
    }

    #[test]
    fn missing_path_is_ignored() {
        let r = scan(
            &[PathBuf::from("/definitely/not/here")],
            &AtomicBool::new(false),
        );
        assert!(r.files.is_empty());
    }

    #[test]
    fn cancelled_scan_is_truncated() {
        let dir = tempfile::tempdir().unwrap();
        touch(&dir.path().join("a.png"));
        let r = scan(&[dir.path().to_path_buf()], &AtomicBool::new(true));
        assert!(r.truncated);
        assert!(r.files.is_empty());
    }
}
