//! Watermark rendering through the engine.

use ibs_core::{
    run, ConflictPolicy, FileResult, FileStatus, ImageFormat, Job, MetadataPolicy, OutputMode,
    OutputSpec, WatermarkKind, WatermarkPosition, WatermarkSpec,
};
use image::{Rgba, RgbaImage};
use std::path::{Path, PathBuf};
use std::sync::atomic::AtomicBool;
use std::sync::Mutex;

fn spec() -> WatermarkSpec {
    WatermarkSpec {
        kind: WatermarkKind::Text,
        text: "IBS".into(),
        color: [255, 255, 255, 255],
        image_path: String::new(),
        position: WatermarkPosition::Center,
        custom: [0.5, 0.5],
        opacity: 1.0,
        rotation: 0.0,
        scale: 0.2,
        margin: 0.05,
        tiled: false,
        spacing: 0.5,
    }
}

fn png(path: &Path, px: Rgba<u8>, w: u32, h: u32) {
    RgbaImage::from_pixel(w, h, px).save(path).unwrap();
}

fn job(inputs: Vec<PathBuf>, out: &Path, watermark: Option<WatermarkSpec>) -> Job {
    Job {
        inputs: inputs
            .iter()
            .map(|p| p.to_string_lossy().into_owned())
            .collect(),
        output: OutputSpec {
            dir: out.to_string_lossy().into_owned(),
            mode: OutputMode::Convert(ImageFormat::Png),
            quality: 80,
            lossless: true,
            metadata: MetadataPolicy::Strip,
            target_size: None,
            conflict: ConflictPolicy::Rename,
        },
        transform: None,
        resize: None,
        watermark,
        rename: None,
    }
}

fn collect(job: Job) -> Vec<FileResult> {
    let results = Mutex::new(Vec::new());
    run(&job, 2, &AtomicBool::new(false), |r: FileResult| {
        results.lock().unwrap().push(r)
    })
    .unwrap();
    results.into_inner().unwrap()
}

fn output_pixels(r: &FileResult) -> RgbaImage {
    image::open(r.output.as_ref().unwrap()).unwrap().to_rgba8()
}

#[test]
fn text_watermark_changes_pixels_at_anchor_only() {
    let d = tempfile::tempdir().unwrap();
    let src = d.path().join("base.png");
    png(&src, Rgba([10, 10, 10, 255]), 100, 100);
    let mut wm = spec();
    wm.position = WatermarkPosition::BottomRight;
    let r = &collect(job(vec![src.clone()], &d.path().join("out"), Some(wm)))[0];
    assert_eq!(r.status, FileStatus::Done);
    let img = output_pixels(r);
    // Bottom-right region now has bright text pixels; top-left untouched.
    let bright = |x: u32, y: u32| img.get_pixel(x, y)[0] > 200;
    assert!((80..99).any(|x| (80..99).any(|y| bright(x, y))));
    assert!(!(0..40).any(|x| (0..40).any(|y| bright(x, y))));
}

#[test]
fn logo_watermark_and_opacity() {
    let d = tempfile::tempdir().unwrap();
    let src = d.path().join("base.png");
    let logo = d.path().join("logo.png");
    png(&src, Rgba([0, 0, 0, 255]), 100, 100);
    png(&logo, Rgba([200, 0, 0, 255]), 20, 20);
    let mut wm = spec();
    wm.kind = WatermarkKind::Image;
    wm.image_path = logo.to_string_lossy().into_owned();
    wm.position = WatermarkPosition::Center;
    wm.scale = 0.2; // 20 px wide on a 100 px image
    let r = &collect(job(vec![src], &d.path().join("out"), Some(wm.clone())))[0];
    assert_eq!(r.status, FileStatus::Done);
    let img = output_pixels(r);
    assert!(img.get_pixel(50, 50)[0] > 150, "logo red at centre");
    assert_eq!(img.get_pixel(5, 5)[0], 0, "corner untouched");

    // Opacity 0 leaves the image visually identical to no watermark.
    wm.opacity = 0.0;
    let r2 = &collect(job(
        vec![d.path().join("base.png")],
        &d.path().join("o2"),
        Some(wm),
    ))[0];
    assert_eq!(r2.status, FileStatus::Done);
    let img2 = output_pixels(r2);
    assert!(img2.pixels().all(|p| *p == Rgba([0, 0, 0, 255])));
}

#[test]
fn tiled_watermark_covers_multiple_regions() {
    let d = tempfile::tempdir().unwrap();
    let src = d.path().join("base.png");
    png(&src, Rgba([0, 0, 0, 255]), 100, 100);
    let mut wm = spec();
    wm.tiled = true;
    wm.scale = 0.15;
    wm.spacing = 0.2;
    let r = &collect(job(vec![src], &d.path().join("out"), Some(wm)))[0];
    assert_eq!(r.status, FileStatus::Done);
    let img = output_pixels(r);
    let changed = |cx: u32, cy: u32| {
        (cx..cx + 10).any(|x| (cy..cy + 10).any(|y| img.get_pixel(x, y)[0] > 100))
    };
    assert!(changed(0, 0) && changed(80, 80) && changed(80, 0) && changed(0, 80));
}

#[test]
fn invalid_and_incompatible_specs_fail_the_job() {
    let d = tempfile::tempdir().unwrap();
    let src = d.path().join("base.png");
    png(&src, Rgba([0, 0, 0, 255]), 8, 8);

    let mut wm = spec();
    wm.text = "   ".into();
    let j = job(vec![src.clone()], &d.path().join("a"), Some(wm));
    assert!(run(&j, 1, &AtomicBool::new(false), |_| {}).is_err());

    let mut wm = spec();
    wm.opacity = 2.0;
    let j = job(vec![src.clone()], &d.path().join("b"), Some(wm));
    assert!(run(&j, 1, &AtomicBool::new(false), |_| {}).is_err());

    let mut j = job(vec![src.clone()], &d.path().join("c"), Some(spec()));
    j.output.mode = OutputMode::CopyOriginal;
    assert!(run(&j, 1, &AtomicBool::new(false), |_| {}).is_err());

    // A missing logo file fails the whole job up front, not per file.
    let mut wm = spec();
    wm.kind = WatermarkKind::Image;
    wm.image_path = d.path().join("nope.png").to_string_lossy().into_owned();
    let j = job(vec![src], &d.path().join("d"), Some(wm));
    assert!(run(&j, 1, &AtomicBool::new(false), |_| {}).is_err());
}
