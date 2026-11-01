//! Throughput benchmark: N generated JPEGs → WebP (or another format).
//!
//! cargo run --release -p ibs-core --example bench -- [count] [width] [height] [quality] [threads]
//! Defaults: 1000 images of 4000×3000 (12 MP), WebP quality 80, all CPUs.

use ibs_core::{
    run, ConflictPolicy, FileStatus, ImageFormat, Job, MetadataPolicy, OutputMode, OutputSpec,
};
use image::{Rgb, RgbImage};
use rayon::prelude::*;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::time::Instant;

fn arg(i: usize, default: u32) -> u32 {
    std::env::args()
        .nth(i)
        .and_then(|a| a.parse().ok())
        .unwrap_or(default)
}

fn main() {
    let (count, w, h, quality, threads) = (
        arg(1, 1000),
        arg(2, 4000),
        arg(3, 3000),
        arg(4, 80),
        arg(5, 0),
    );
    let dir = tempfile::tempdir().expect("temp dir");
    let src_dir = dir.path().join("in");
    std::fs::create_dir_all(&src_dir).unwrap();
    eprintln!("generating {count} JPEG files of {w}×{h}…");
    let inputs: Vec<String> = (0..count)
        .into_par_iter()
        .map(|i| {
            let img = RgbImage::from_fn(w, h, |x, y| {
                let n = (x ^ y.wrapping_mul(31)).wrapping_add(i) as u8;
                Rgb([((x / 16) as u8) ^ (n / 8), (y / 16) as u8, n / 4])
            });
            let p = src_dir.join(format!("img-{i:05}.jpg"));
            img.save(&p).unwrap();
            p.to_string_lossy().into_owned()
        })
        .collect();
    let input_bytes: u64 = inputs
        .iter()
        .map(|p| std::fs::metadata(p).unwrap().len())
        .sum();
    let job = Job {
        inputs,
        output: OutputSpec {
            dir: dir.path().join("out").to_string_lossy().into_owned(),
            mode: OutputMode::Convert(ImageFormat::Webp),
            quality: quality as u8,
            lossless: false,
            metadata: MetadataPolicy::Strip,
            target_size: None,
            conflict: ConflictPolicy::Rename,
        },
        transform: None,
        resize: None,
        rename: None,
    };
    let done = AtomicU32::new(0);
    let out_bytes = std::sync::atomic::AtomicU64::new(0);
    let started = Instant::now();
    let summary = run(&job, threads as usize, &AtomicBool::new(false), |r| {
        if r.status == FileStatus::Done {
            done.fetch_add(1, Ordering::Relaxed);
            out_bytes.fetch_add(r.output_size.unwrap_or(0), Ordering::Relaxed);
        }
    })
    .expect("job");
    let secs = started.elapsed().as_secs_f64();
    let cpus = std::thread::available_parallelism().map_or(1, |n| n.get());
    println!(
        "{} × {w}×{h} JPEG → WebP q{quality}: {done}/{count} ok, {failed} failed, {secs:.1} s, {:.2} img/s, {:.1} MP/s, {} → {} MB, workers {}",
        count,
        f64::from(count) / secs,
        f64::from(count) * f64::from(w) * f64::from(h) / 1e6 / secs,
        input_bytes / 1_000_000,
        out_bytes.load(Ordering::Relaxed) / 1_000_000,
        if threads == 0 { cpus } else { threads as usize },
        done = done.load(Ordering::Relaxed),
        failed = summary.failed,
    );
}
