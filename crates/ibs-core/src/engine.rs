//! Batch execution: bounded worker pool, per-file isolation, cooperative
//! cancellation, write-to-temp → verify → atomic rename.

use crate::codec;
use crate::output::{cleanup_stale_temps, Plan, Planner, TEMP_PREFIX};
use crate::{FileError, FileErrorCode, FileResult, FileStatus, ImageFormat, Job};
use rayon::prelude::*;
use serde::Serialize;
use std::io::{BufWriter, Write};
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::time::{Duration, Instant};
use ts_rs::TS;

const STALE_TEMP_AGE: Duration = Duration::from_secs(3600);

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
    #[error("cannot use output folder: {0}")]
    OutputDir(std::io::Error),
    #[error("cannot start workers: {0}")]
    Pool(String),
}

/// Validates the job and makes sure the output folder exists.
pub fn prepare(job: &Job) -> Result<PathBuf, JobError> {
    if job.inputs.is_empty() {
        return Err(JobError::NoInputs);
    }
    if !(1..=100).contains(&job.output.quality) {
        return Err(JobError::InvalidQuality);
    }
    if !job.output.format.can_encode() {
        return Err(JobError::FormatUnavailable(job.output.format));
    }
    let dir = Path::new(&job.output.dir);
    if !dir.is_absolute() {
        return Err(JobError::OutputDirNotAbsolute);
    }
    std::fs::create_dir_all(dir).map_err(JobError::OutputDir)?;
    std::fs::canonicalize(dir).map_err(JobError::OutputDir)
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
    cleanup_stale_temps(&dir, STALE_TEMP_AGE);
    let inputs: Vec<PathBuf> = job.inputs.iter().map(PathBuf::from).collect();
    let planner = Planner::new(&dir, job.output.format, job.output.conflict, &inputs);
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
        inputs.par_iter().enumerate().for_each(|(i, input)| {
            let index = i as u32;
            let result = if cancel.load(Ordering::Relaxed) {
                cancelled(index)
            } else {
                catch_unwind(AssertUnwindSafe(|| {
                    process(index, input, job, &dir, &planner, cancel)
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

fn cancelled(index: u32) -> FileResult {
    FileResult {
        index,
        status: FileStatus::Cancelled,
        output: None,
        output_size: None,
        error: None,
    }
}

fn failed(index: u32, error: FileError) -> FileResult {
    FileResult {
        index,
        status: FileStatus::Failed,
        output: None,
        output_size: None,
        error: Some(error),
    }
}

fn process(
    index: u32,
    input: &Path,
    job: &Job,
    dir: &Path,
    planner: &Planner,
    cancel: &AtomicBool,
) -> FileResult {
    match write_one(input, job, dir, planner, cancel) {
        Ok(Some((target, size))) => FileResult {
            index,
            status: FileStatus::Done,
            output: Some(target.to_string_lossy().into_owned()),
            output_size: Some(size),
            error: None,
        },
        Ok(None) => cancelled(index),
        Err(Outcome::Skipped(target)) => FileResult {
            index,
            status: FileStatus::Skipped,
            output: Some(target.to_string_lossy().into_owned()),
            output_size: None,
            error: None,
        },
        Err(Outcome::Failed(e)) => failed(index, e),
    }
}

enum Outcome {
    Skipped(PathBuf),
    Failed(FileError),
}

impl From<FileError> for Outcome {
    fn from(e: FileError) -> Self {
        Self::Failed(e)
    }
}

/// Returns `Ok(None)` when cancelled; the temp file is removed on drop.
fn write_one(
    input: &Path,
    job: &Job,
    dir: &Path,
    planner: &Planner,
    cancel: &AtomicBool,
) -> Result<Option<(PathBuf, u64)>, Outcome> {
    let spec = &job.output;
    let img = codec::decode(input)?;
    codec::check_encodable(spec.format, img.width(), img.height())?;
    if cancel.load(Ordering::Relaxed) {
        return Ok(None);
    }
    let (target, replace) = match planner.plan(input)? {
        Plan::Write { target, replace } => (target, replace),
        Plan::Skip { target } => return Err(Outcome::Skipped(target)),
    };

    let mut tmp = tempfile::Builder::new()
        .prefix(TEMP_PREFIX)
        .suffix(&format!(".{}", spec.format.extension()))
        .tempfile_in(dir)
        .map_err(FileError::io)?;
    {
        let mut w = BufWriter::new(tmp.as_file_mut());
        codec::encode(&img, spec.format, spec.quality, &mut w)?;
        w.flush().map_err(FileError::io)?;
    }
    if cancel.load(Ordering::Relaxed) {
        return Ok(None);
    }
    codec::verify(tmp.path(), img.width(), img.height())?;
    if cancel.load(Ordering::Relaxed) {
        return Ok(None);
    }
    let persisted = if replace {
        tmp.persist(&target)
    } else {
        tmp.persist_noclobber(&target)
    };
    let file = persisted.map_err(|e| FileError::io(e.error))?;
    let size = file.metadata().map_err(FileError::io)?.len();
    Ok(Some((target, size)))
}
