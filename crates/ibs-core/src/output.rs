//! Output path planning. Enforces ADR 0004: a source file is never a target.

use crate::{ConflictPolicy, FileError, FileErrorCode, ImageFormat};
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

/// Prefix of in-progress output files (hidden, ignored by input scans).
pub const TEMP_PREFIX: &str = ".ibs-tmp-";
const MAX_RENAME_ATTEMPTS: u32 = 10_000;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Plan {
    /// Write to `target`; `replace` means an existing output file may be replaced.
    Write {
        target: PathBuf,
        replace: bool,
    },
    Skip {
        target: PathBuf,
    },
}

pub struct Planner {
    dir: PathBuf,
    ext: &'static str,
    policy: ConflictPolicy,
    sources: HashSet<PathBuf>,
    claimed: Mutex<HashSet<PathBuf>>,
}

impl Planner {
    pub fn new(
        dir: &Path,
        format: ImageFormat,
        policy: ConflictPolicy,
        inputs: &[PathBuf],
    ) -> Self {
        Self {
            dir: dir.to_path_buf(),
            ext: format.extension(),
            policy,
            sources: inputs.iter().map(|p| identity(p)).collect(),
            claimed: Mutex::new(HashSet::new()),
        }
    }

    /// Chooses the output path for `input`. Targets claimed earlier in the same
    /// batch are always avoided by numbering, whatever the policy.
    pub fn plan(&self, input: &Path) -> Result<Plan, FileError> {
        let stem = input
            .file_stem()
            .map(|s| s.to_string_lossy().into_owned())
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| "image".into());
        let mut claimed = self
            .claimed
            .lock()
            .map_err(|_| FileError::new(FileErrorCode::Internal, "planner lock poisoned"))?;
        let mut candidate = self.dir.join(format!("{stem}.{}", self.ext));
        for n in 1..=MAX_RENAME_ATTEMPTS {
            let key = claim_key(&candidate);
            let is_source = self.sources.contains(&identity(&candidate));
            let exists = std::fs::symlink_metadata(&candidate).is_ok();
            if !claimed.contains(&key) {
                if !is_source && !exists {
                    claimed.insert(key);
                    return Ok(Plan::Write {
                        target: candidate,
                        replace: false,
                    });
                }
                match self.policy {
                    ConflictPolicy::Rename => {}
                    ConflictPolicy::Skip => return Ok(Plan::Skip { target: candidate }),
                    ConflictPolicy::Overwrite if is_source => {
                        return Err(FileError::new(
                            FileErrorCode::WouldOverwriteSource,
                            format!("refusing to overwrite source file {}", candidate.display()),
                        ))
                    }
                    ConflictPolicy::Overwrite => {
                        if !candidate.is_file() {
                            return Err(FileError::new(
                                FileErrorCode::Io,
                                format!("{} exists and is not a file", candidate.display()),
                            ));
                        }
                        claimed.insert(key);
                        return Ok(Plan::Write {
                            target: candidate,
                            replace: true,
                        });
                    }
                }
            }
            candidate = self.dir.join(format!("{stem} ({n}).{}", self.ext));
        }
        Err(FileError::new(
            FileErrorCode::Io,
            "could not find a free output file name",
        ))
    }
}

/// Batch reservation key. Lexical (the output dir is canonical) so it stays
/// stable when an existing target, e.g. a symlink, is replaced mid-batch.
fn claim_key(p: &Path) -> PathBuf {
    fold_case(p.to_path_buf())
}

fn fold_case(p: PathBuf) -> PathBuf {
    if cfg!(any(windows, target_os = "macos")) {
        PathBuf::from(p.to_string_lossy().to_lowercase())
    } else {
        p
    }
}

/// Comparable identity of a path: canonical when it exists, otherwise the
/// canonical parent plus file name; case-insensitive on Windows and macOS.
fn identity(p: &Path) -> PathBuf {
    let resolved = std::fs::canonicalize(p).unwrap_or_else(|_| {
        match (
            p.parent().and_then(|d| std::fs::canonicalize(d).ok()),
            p.file_name(),
        ) {
            (Some(parent), Some(name)) => parent.join(name),
            _ => p.to_path_buf(),
        }
    });
    fold_case(resolved)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn setup() -> (tempfile::TempDir, PathBuf) {
        let d = tempfile::tempdir().unwrap();
        let src = d.path().join("photo.png");
        std::fs::write(&src, b"src").unwrap();
        (d, src)
    }

    #[test]
    fn same_name_as_source_is_never_written() {
        let (d, src) = setup();
        let sources = [src.clone()];
        let p = Planner::new(
            d.path(),
            ImageFormat::Png,
            ConflictPolicy::Overwrite,
            &sources,
        );
        assert_eq!(
            p.plan(&src).unwrap_err().code,
            FileErrorCode::WouldOverwriteSource
        );
        let p = Planner::new(d.path(), ImageFormat::Png, ConflictPolicy::Skip, &sources);
        assert!(matches!(p.plan(&src).unwrap(), Plan::Skip { .. }));
        let p = Planner::new(d.path(), ImageFormat::Png, ConflictPolicy::Rename, &sources);
        assert_eq!(
            p.plan(&src).unwrap(),
            Plan::Write {
                target: d.path().join("photo (1).png"),
                replace: false
            }
        );
    }

    #[test]
    fn source_reached_through_another_spelling_is_detected() {
        let (d, src) = setup();
        let sub = d.path().join("sub");
        std::fs::create_dir(&sub).unwrap();
        let dotted = sub.join("..");
        let p = Planner::new(
            &dotted,
            ImageFormat::Png,
            ConflictPolicy::Overwrite,
            std::slice::from_ref(&src),
        );
        assert_eq!(
            p.plan(&src).unwrap_err().code,
            FileErrorCode::WouldOverwriteSource
        );
    }

    #[test]
    fn existing_outputs_follow_policy() {
        let (d, src) = setup();
        std::fs::write(d.path().join("photo.jpg"), b"old").unwrap();
        let mk = |policy| {
            Planner::new(
                d.path(),
                ImageFormat::Jpeg,
                policy,
                std::slice::from_ref(&src),
            )
        };
        assert_eq!(
            mk(ConflictPolicy::Overwrite).plan(&src).unwrap(),
            Plan::Write {
                target: d.path().join("photo.jpg"),
                replace: true
            }
        );
        assert!(matches!(
            mk(ConflictPolicy::Skip).plan(&src).unwrap(),
            Plan::Skip { .. }
        ));
        assert_eq!(
            mk(ConflictPolicy::Rename).plan(&src).unwrap(),
            Plan::Write {
                target: d.path().join("photo (1).jpg"),
                replace: false
            }
        );
    }

    #[test]
    fn batch_duplicates_are_numbered_even_when_overwriting() {
        let d = tempfile::tempdir().unwrap();
        let a = d.path().join("in/a.png");
        let b = d.path().join("in/a.bmp");
        let out = d.path().join("out");
        std::fs::create_dir_all(&out).unwrap();
        let p = Planner::new(
            &out,
            ImageFormat::Jpeg,
            ConflictPolicy::Overwrite,
            &[a.clone(), b.clone()],
        );
        let t1 = p.plan(&a).unwrap();
        let t2 = p.plan(&b).unwrap();
        assert_eq!(
            t1,
            Plan::Write {
                target: out.join("a.jpg"),
                replace: false
            }
        );
        assert_eq!(
            t2,
            Plan::Write {
                target: out.join("a (1).jpg"),
                replace: false
            }
        );
    }

    #[cfg(unix)]
    #[test]
    fn claims_survive_replacing_a_symlinked_target() {
        let (d, _) = setup();
        let out = d.path().join("out");
        std::fs::create_dir(&out).unwrap();
        std::fs::write(out.join("old.jpg"), b"old").unwrap();
        std::os::unix::fs::symlink(out.join("old.jpg"), out.join("a.jpg")).unwrap();
        let inputs = [d.path().join("a.png"), d.path().join("a.bmp")];
        let p = Planner::new(&out, ImageFormat::Jpeg, ConflictPolicy::Overwrite, &inputs);
        let first = p.plan(&inputs[0]).unwrap();
        assert_eq!(
            first,
            Plan::Write {
                target: out.join("a.jpg"),
                replace: true
            }
        );
        std::fs::remove_file(out.join("a.jpg")).unwrap();
        std::fs::write(out.join("a.jpg"), b"new").unwrap();
        let second = p.plan(&inputs[1]).unwrap();
        assert_eq!(
            second,
            Plan::Write {
                target: out.join("a (1).jpg"),
                replace: false
            }
        );
    }
}
