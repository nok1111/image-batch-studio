//! Batch execution: bounded worker pool, per-file isolation, cooperative
//! cancellation, write-to-temp → verify → atomic rename.

use crate::codec::{self, EncodeOptions};
use crate::gif;
use crate::output::{Plan, Planner, TEMP_PREFIX};
use crate::{
    FileError, FileErrorCode, FileResult, FileStatus, FileWarning, ImageFormat, Job,
    MetadataPolicy, OutputMode,
};
use rayon::prelude::*;
use serde::Serialize;
use std::io::Write;
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::time::Instant;
use ts_rs::TS;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct JobSummary {
    pub total: u32,
    pub done: u32,
    pub skipped: u32,
    pub failed: u32,
    pub cancelled: u32,
    #[ts(type = "number")]
    pub elapsed_ms: u64,
}

/// Events streamed to the UI. Progress batches results to limit IPC traffic.
#[derive(Debug, Clone, Serialize, TS)]
#[serde(tag = "type", rename_all = "camelCase")]
#[ts(export)]
pub enum JobEvent {
    Progress { results: Vec<FileResult> },
    Finished { summary: JobSummary },
    Aborted { message: String },
}

#[derive(Debug, thiserror::Error)]
pub enum JobError {
    #[error("no input files")]
    NoInputs,
    #[error("quality must be between 1 and 100")]
    InvalidQuality,
    #[error("output folder must be an absolute path")]
    OutputDirNotAbsolute,
    #[error("{0:?} output is not available in this version")]
    FormatUnavailable(ImageFormat),
    #[error("a target size needs a lossy format (JPEG or lossy WebP) and a value above 0")]
    InvalidTargetSize,
    #[error("copying originals cannot be combined with resizing")]
    CopyWithResize,
    #[error("{0}")]
    InvalidOptions(String),
    #[error("cannot use output folder: {0}")]
    OutputDir(std::io::Error),
    #[error("cannot start workers: {0}")]
    Pool(String),
}

fn validate(job: &Job) -> Result<(), JobError> {
    let o = &job.output;
    if job.inputs.is_empty() {
        return Err(JobError::NoInputs);
    }
    if !(1..=100).contains(&o.quality) {
        return Err(JobError::InvalidQuality);
    }
    if let OutputMode::Convert(f) = o.mode {
        if !f.can_encode() {
            return Err(JobError::FormatUnavailable(f));
        }
    }
    if let Some(t) = o.target_size {
        let lossy_ok = match o.mode {
            OutputMode::Convert(f) => {
                matches!(f, ImageFormat::Jpeg | ImageFormat::Webp) && f.is_lossy(o.lossless)
            }
            OutputMode::SameAsSource => true,
            OutputMode::CopyOriginal => false,
        };
        if t == 0 || !lossy_ok {
            return Err(JobError::InvalidTargetSize);
        }
    }
    if let Some(r) = &job.resize {
        if o.mode == OutputMode::CopyOriginal {
            return Err(JobError::CopyWithResize);
        }
        r.validate().map_err(JobError::InvalidOptions)?;
    }
    if let Some(t) = &job.transform {
        if o.mode == OutputMode::CopyOriginal {
            return Err(JobError::CopyWithResize);
        }
        t.validate().map_err(JobError::InvalidOptions)?;
    }
    if let Some(r) = &job.rename {
        r.validate().map_err(JobError::InvalidOptions)?;
    }
    if !Path::new(&o.dir).is_absolute() {
        return Err(JobError::OutputDirNotAbsolute);
    }
    Ok(())
}

/// Validates the job and makes sure the output folder exists.
pub fn prepare(job: &Job) -> Result<PathBuf, JobError> {
    validate(job)?;
    let dir = Path::new(&job.output.dir);
    std::fs::create_dir_all(dir).map_err(JobError::OutputDir)?;
    std::fs::canonicalize(dir).map_err(JobError::OutputDir)
}

/// Encoded format for `input`, or `None` for byte copies.
fn target_format(job: &Job, input: &Path) -> Result<Option<ImageFormat>, FileError> {
    match job.output.mode {
        OutputMode::Convert(f) => Ok(Some(f)),
        OutputMode::CopyOriginal => Ok(None),
        OutputMode::SameAsSource => ImageFormat::from_input_path(input)
            .map(Some)
            .ok_or_else(|| FileError::new(FileErrorCode::Unsupported, "unknown file extension")),
    }
}

/// Wanted output stem and extension, before conflict handling.
fn desired_name(job: &Job, index: usize, input: &Path) -> Result<(String, String), FileError> {
    let ext = match job.output.mode {
        OutputMode::Convert(f) => f.extension().to_string(),
        _ => {
            target_format(job, input)?;
            input
                .extension()
                .map(|e| e.to_string_lossy().into_owned())
                .unwrap_or_default()
        }
    };
    let stem = input
        .file_stem()
        .map(|s| s.to_string_lossy().into_owned())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "image".into());
    let stem = match &job.rename {
        Some(r) => {
            let modified = std::fs::metadata(input).and_then(|m| m.modified()).ok();
            r.render(&stem, index as u32, modified)
        }
        None => stem,
    };
    Ok((stem, ext))
}

fn plan_one(job: &Job, planner: &Planner, index: usize, input: &Path) -> Result<Plan, FileError> {
    let (stem, ext) = desired_name(job, index, input)?;
    planner.plan_named(&stem, &ext)
}

/// Plans every output in input order, so names are deterministic and a
/// preview matches the run exactly.
fn plan_all(job: &Job, dir: &Path, inputs: &[PathBuf]) -> Vec<Result<Plan, FileError>> {
    let format = match job.output.mode {
        OutputMode::Convert(f) => f,
        _ => ImageFormat::Png,
    };
    let planner = Planner::new(dir, format, job.output.conflict, inputs);
    inputs
        .iter()
        .enumerate()
        .map(|(i, p)| plan_one(job, &planner, i, p))
        .collect()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum PlannedAction {
    Write,
    /// An existing output file will be replaced (policy `overwrite`).
    Replace,
    Skip,
    Fail,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct PlannedOutput {
    pub index: u32,
    pub action: PlannedAction,
    pub target: Option<String>,
    /// The wanted name was taken, so a ` (n)` suffix was added.
    pub renumbered: bool,
    pub error: Option<FileError>,
}

/// Output names a run would use, without writing anything.
pub fn preview(job: &Job) -> Result<Vec<PlannedOutput>, JobError> {
    validate(job)?;
    let dir = Path::new(&job.output.dir);
    let dir = std::fs::canonicalize(dir).unwrap_or_else(|_| dir.to_path_buf());
    let inputs: Vec<PathBuf> = job.inputs.iter().map(PathBuf::from).collect();
    let plans = plan_all(job, &dir, &inputs);
    Ok(plans
        .into_iter()
        .enumerate()
        .map(|(i, plan)| {
            let wanted = desired_name(job, i, &inputs[i])
                .ok()
                .map(|(stem, ext)| dir.join(format!("{stem}.{ext}")));
            let (action, target, error) = match plan {
                Ok(Plan::Write { target, replace }) => (
                    if replace {
                        PlannedAction::Replace
                    } else {
                        PlannedAction::Write
                    },
                    Some(target),
                    None,
                ),
                Ok(Plan::Skip { target }) => (PlannedAction::Skip, Some(target), None),
                Err(e) => (PlannedAction::Fail, None, Some(e)),
            };
            let renumbered = target.is_some() && wanted.is_some() && target != wanted;
            PlannedOutput {
                index: i as u32,
                action,
                target: target.map(|t| t.to_string_lossy().into_owned()),
                renumbered,
                error,
            }
        })
        .collect())
}

/// Processes every input; `on_result` is called once per input from worker
/// threads. `concurrency` 0 means one worker per logical CPU.
pub fn run<F>(
    job: &Job,
    concurrency: usize,
    cancel: &AtomicBool,
    on_result: F,
) -> Result<JobSummary, JobError>
where
    F: Fn(FileResult) + Sync,
{
    let started = Instant::now();
    let dir = prepare(job)?;
    let inputs: Vec<PathBuf> = job.inputs.iter().map(PathBuf::from).collect();
    let plans = plan_all(job, &dir, &inputs);
    let threads = match concurrency {
        0 => std::thread::available_parallelism().map_or(1, |n| n.get()),
        n => n,
    };
    let pool = rayon::ThreadPoolBuilder::new()
        .num_threads(threads)
        .thread_name(|i| format!("ibs-worker-{i}"))
        .build()
        .map_err(|e| JobError::Pool(e.to_string()))?;

    let counts: [AtomicU32; 4] = Default::default();
    pool.install(|| {
        inputs
            .par_iter()
            .zip(plans.par_iter())
            .enumerate()
            .for_each(|(i, (input, plan))| {
                let index = i as u32;
                let result = if cancel.load(Ordering::Relaxed) {
                    cancelled(index)
                } else {
                    catch_unwind(AssertUnwindSafe(|| {
                        process(index, input, plan, job, &dir, cancel)
                    }))
                    .unwrap_or_else(|_| {
                        failed(
                            index,
                            FileError::new(
                                FileErrorCode::Internal,
                                "unexpected error while processing",
                            ),
                        )
                    })
                };
                let slot = match result.status {
                    FileStatus::Done => 0,
                    FileStatus::Skipped => 1,
                    FileStatus::Failed => 2,
                    FileStatus::Cancelled => 3,
                };
                counts[slot].fetch_add(1, Ordering::Relaxed);
                on_result(result);
            });
    });

    let [done, skipped, failed, cancelled] = counts.map(AtomicU32::into_inner);
    Ok(JobSummary {
        total: inputs.len() as u32,
        done,
        skipped,
        failed,
        cancelled,
        elapsed_ms: started.elapsed().as_millis() as u64,
    })
}

fn result(index: u32, status: FileStatus) -> FileResult {
    FileResult {
        index,
        status,
        output: None,
        output_size: None,
        error: None,
        warnings: Vec::new(),
    }
}

fn cancelled(index: u32) -> FileResult {
    result(index, FileStatus::Cancelled)
}

fn failed(index: u32, error: FileError) -> FileResult {
    FileResult {
        error: Some(error),
        ..result(index, FileStatus::Failed)
    }
}

fn process(
    index: u32,
    input: &Path,
    plan: &Result<Plan, FileError>,
    job: &Job,
    dir: &Path,
    cancel: &AtomicBool,
) -> FileResult {
    let (target, replace) = match plan {
        Ok(Plan::Write { target, replace }) => (target, *replace),
        Ok(Plan::Skip { target }) => {
            return FileResult {
                output: Some(target.to_string_lossy().into_owned()),
                ..result(index, FileStatus::Skipped)
            }
        }
        Err(e) => return failed(index, e.clone()),
    };
    match write_one(input, job, dir, target, replace, cancel) {
        Ok(Some(w)) => FileResult {
            output: Some(target.to_string_lossy().into_owned()),
            output_size: Some(w.size),
            warnings: w.warnings,
            ..result(index, FileStatus::Done)
        },
        Ok(None) => cancelled(index),
        Err(e) => failed(index, e),
    }
}

struct Written {
    size: u64,
    warnings: Vec<FileWarning>,
}

/// Returns `Ok(None)` when cancelled; the temp file is removed on drop.
fn write_one(
    input: &Path,
    job: &Job,
    dir: &Path,
    target: &Path,
    replace: bool,
    cancel: &AtomicBool,
) -> Result<Option<Written>, FileError> {
    let spec = &job.output;
    let source_len = std::fs::metadata(input).map_err(FileError::io)?.len();
    let format = target_format(job, input)?;
    let ext = target
        .extension()
        .map(|e| e.to_string_lossy().into_owned())
        .unwrap_or_default();
    let mut tmp = tempfile::Builder::new()
        .prefix(TEMP_PREFIX)
        .suffix(&format!(".{ext}"))
        .tempfile_in(dir)
        .map_err(FileError::io)?;
    let mut warnings = Vec::new();

    match format {
        None => {
            let mut src = std::fs::File::open(input).map_err(FileError::io)?;
            std::io::copy(&mut src, tmp.as_file_mut()).map_err(FileError::io)?;
            tmp.as_file_mut().flush().map_err(FileError::io)?;
            if cancel.load(Ordering::Relaxed) {
                return Ok(None);
            }
            let copied = std::fs::metadata(tmp.path()).map_err(FileError::io)?.len();
            if copied != source_len {
                return Err(FileError::new(FileErrorCode::Verify, "copy size mismatch"));
            }
            codec::decode(tmp.path()).map_err(|e| {
                FileError::new(FileErrorCode::Verify, format!("copy does not decode: {e}"))
            })?;
        }
        Some(format) => {
            // Pure metadata strip on a GIF goes byte-level so animation
            // survives — decoding would collapse it to the first frame.
            if format == ImageFormat::Gif
                && spec.metadata == MetadataPolicy::Strip
                && job.resize.is_none()
                && job.transform.is_none()
            {
                let bytes = std::fs::read(input).map_err(FileError::io)?;
                if bytes.starts_with(b"GIF8") {
                    let stripped = gif::strip_metadata(&bytes)?;
                    let (w, h) = gif::screen_size(&stripped).unwrap_or((0, 0));
                    tmp.as_file_mut()
                        .write_all(&stripped)
                        .map_err(FileError::io)?;
                    tmp.as_file_mut().flush().map_err(FileError::io)?;
                    if cancel.load(Ordering::Relaxed) {
                        return Ok(None);
                    }
                    codec::verify(tmp.path(), format, w, h)?;
                    if cancel.load(Ordering::Relaxed) {
                        return Ok(None);
                    }
                    let persisted = if replace {
                        tmp.persist(target)
                    } else {
                        tmp.persist_noclobber(target)
                    };
                    let file = persisted.map_err(|e| FileError::io(e.error))?;
                    let size = file.metadata().map_err(FileError::io)?.len();
                    return Ok(Some(Written { size, warnings }));
                }
            }
            let decoded = codec::decode(input)?;
            if decoded.animated {
                warnings.push(FileWarning::FirstFrameOnly);
            }
            let mut img = match &job.transform {
                Some(t) => t.apply(&decoded.image)?,
                None => decoded.image,
            };
            if let Some(r) = &job.resize {
                img = r.apply(&img);
            }
            codec::check_encodable(format, img.width(), img.height())?;
            if cancel.load(Ordering::Relaxed) {
                return Ok(None);
            }
            let keep = spec.metadata == MetadataPolicy::Keep;
            let opts = EncodeOptions {
                format,
                quality: spec.quality,
                lossless: spec.lossless,
                icc: decoded.icc.as_deref().filter(|_| keep),
                exif: decoded.exif.as_deref().filter(|_| keep),
            };
            let encoded = match spec.target_size {
                Some(t) if format.is_lossy(spec.lossless) && format != ImageFormat::Avif => {
                    let sized = codec::encode_to_size(&img, &opts, t)?;
                    if sized.target_missed {
                        warnings.push(FileWarning::TargetNotReached);
                    }
                    sized.encoded
                }
                Some(_) => {
                    warnings.push(FileWarning::TargetSizeUnsupported);
                    codec::encode(&img, &opts)?
                }
                None => codec::encode(&img, &opts)?,
            };
            if encoded.metadata_dropped {
                warnings.push(FileWarning::MetadataDropped);
            }
            tmp.as_file_mut()
                .write_all(&encoded.bytes)
                .map_err(FileError::io)?;
            tmp.as_file_mut().flush().map_err(FileError::io)?;
            if cancel.load(Ordering::Relaxed) {
                return Ok(None);
            }
            codec::verify(tmp.path(), format, img.width(), img.height())?;
            if spec.mode == OutputMode::SameAsSource && encoded.bytes.len() as u64 > source_len {
                warnings.push(FileWarning::LargerThanSource);
            }
        }
    }
    if cancel.load(Ordering::Relaxed) {
        return Ok(None);
    }
    let persisted = if replace {
        tmp.persist(target)
    } else {
        tmp.persist_noclobber(target)
    };
    let file = persisted.map_err(|e| FileError::io(e.error))?;
    let size = file.metadata().map_err(FileError::io)?.len();
    Ok(Some(Written { size, warnings }))
}
