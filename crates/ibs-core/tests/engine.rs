//! End-to-end engine tests on generated fixtures (no binary fixtures in git).

use ibs_core::output::TEMP_PREFIX;
use ibs_core::{
    run, ConflictPolicy, FileErrorCode, FileResult, FileStatus, ImageFormat, Job, MetadataPolicy,
    OutputMode, OutputSpec,
};
use image::{Rgba, RgbaImage};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;
use std::time::{Duration, Instant};

fn write_png(path: &Path, w: u32, h: u32) {
    let img = RgbaImage::from_fn(w, h, |x, y| {
        Rgba([(x % 256) as u8, (y % 256) as u8, 90, 200])
    });
    img.save(path).unwrap();
}

fn crc32(bytes: &[u8]) -> u32 {
    let mut crc = 0xFFFF_FFFFu32;
    for &b in bytes {
        crc ^= u32::from(b);
        for _ in 0..8 {
            crc = if crc & 1 != 0 {
                (crc >> 1) ^ 0xEDB8_8320
            } else {
                crc >> 1
            };
        }
    }
    !crc
}

/// A valid PNG header announcing 60000×60000 RGBA (≈14 GB decoded).
fn write_png_bomb(path: &Path) {
    let mut ihdr = b"IHDR".to_vec();
    ihdr.extend(60_000u32.to_be_bytes());
    ihdr.extend(60_000u32.to_be_bytes());
    ihdr.extend([8, 6, 0, 0, 0]);
    let mut png = b"\x89PNG\r\n\x1a\n".to_vec();
    png.extend(13u32.to_be_bytes());
    png.extend(&ihdr);
    png.extend(crc32(&ihdr).to_be_bytes());
    png.extend(0u32.to_be_bytes());
    png.extend(b"IEND");
    png.extend(crc32(b"IEND").to_be_bytes());
    std::fs::write(path, png).unwrap();
}

fn job(inputs: &[PathBuf], out: &Path, format: ImageFormat, conflict: ConflictPolicy) -> Job {
    Job {
        inputs: inputs
            .iter()
            .map(|p| p.to_string_lossy().into_owned())
            .collect(),
        output: OutputSpec {
            dir: out.to_string_lossy().into_owned(),
            mode: OutputMode::Convert(format),
            quality: 80,
            lossless: true,
            metadata: MetadataPolicy::Strip,
            target_size: None,
            conflict,
        },
        transform: None,
        resize: None,
        watermark: None,
        rename: None,
    }
}

fn run_collect(job: &Job, cancel: &AtomicBool) -> Vec<FileResult> {
    let results = Mutex::new(Vec::new());
    run(job, 4, cancel, |r| results.lock().unwrap().push(r)).unwrap();
    let mut v = results.into_inner().unwrap();
    v.sort_by_key(|r| r.index);
    v
}

fn no_temp_files(dir: &Path) -> bool {
    std::fs::read_dir(dir)
        .unwrap()
        .flatten()
        .all(|e| !e.file_name().to_string_lossy().starts_with(TEMP_PREFIX))
}

#[test]
fn converts_every_supported_output_format_with_verified_dimensions() {
    let d = tempfile::tempdir().unwrap();
    let src = d.path().join("in.png");
    write_png(&src, 64, 48);
    for format in [
        ImageFormat::Png,
        ImageFormat::Jpeg,
        ImageFormat::Webp,
        ImageFormat::Bmp,
        ImageFormat::Tiff,
        ImageFormat::Gif,
        ImageFormat::Ico,
    ] {
        let out = d.path().join(format!("out-{format:?}"));
        let r = run_collect(
            &job(
                std::slice::from_ref(&src),
                &out,
                format,
                ConflictPolicy::Rename,
            ),
            &AtomicBool::new(false),
        );
        assert_eq!(
            r[0].status,
            FileStatus::Done,
            "{format:?}: {:?}",
            r[0].error
        );
        let path = PathBuf::from(r[0].output.as_ref().unwrap());
        assert_eq!(path.extension().unwrap(), format.extension());
        assert_eq!(
            image::image_dimensions(&path).unwrap(),
            (64, 48),
            "{format:?}"
        );
        assert_eq!(
            r[0].output_size,
            Some(std::fs::metadata(&path).unwrap().len())
        );
        assert!(no_temp_files(&out));
    }
}

#[test]
fn bad_files_fail_individually_and_the_batch_completes() {
    let d = tempfile::tempdir().unwrap();
    let good1 = d.path().join("good1.png");
    let good2 = d.path().join("good2.png");
    write_png(&good1, 32, 32);
    write_png(&good2, 32, 32);
    let truncated = d.path().join("truncated.png");
    let bytes = std::fs::read(&good1).unwrap();
    std::fs::write(&truncated, &bytes[..bytes.len() / 2]).unwrap();
    let text = d.path().join("notes.jpg");
    std::fs::write(&text, "this is not an image").unwrap();
    let empty = d.path().join("empty.png");
    std::fs::write(&empty, b"").unwrap();
    let bomb = d.path().join("bomb.png");
    write_png_bomb(&bomb);
    let missing = d.path().join("deleted.png");
    let big_ico_src = d.path().join("big.png");
    write_png(&big_ico_src, 300, 10);

    let inputs = [good1, truncated, text, empty, bomb, missing, good2];
    let out = d.path().join("out");
    let r = run_collect(
        &job(&inputs, &out, ImageFormat::Jpeg, ConflictPolicy::Rename),
        &AtomicBool::new(false),
    );
    let code = |i: usize| r[i].error.as_ref().map(|e| e.code);
    assert_eq!(r[0].status, FileStatus::Done);
    assert_eq!(r[6].status, FileStatus::Done);
    assert_eq!(code(1), Some(FileErrorCode::Decode));
    assert!(matches!(
        code(2),
        Some(FileErrorCode::Unsupported | FileErrorCode::Decode)
    ));
    assert!(matches!(
        code(3),
        Some(FileErrorCode::Unsupported | FileErrorCode::Decode)
    ));
    assert_eq!(code(4), Some(FileErrorCode::TooLarge));
    assert_eq!(code(5), Some(FileErrorCode::Io));
    assert!(no_temp_files(&out));

    let r = run_collect(
        &job(
            &[big_ico_src],
            &out,
            ImageFormat::Ico,
            ConflictPolicy::Rename,
        ),
        &AtomicBool::new(false),
    );
    assert_eq!(
        r[0].error.as_ref().map(|e| e.code),
        Some(FileErrorCode::OutputUnsupported)
    );
}

#[test]
fn sources_are_never_modified_whatever_the_policy() {
    let d = tempfile::tempdir().unwrap();
    let src = d.path().join("photo.png");
    write_png(&src, 20, 20);
    let original = std::fs::read(&src).unwrap();
    for policy in [
        ConflictPolicy::Overwrite,
        ConflictPolicy::Skip,
        ConflictPolicy::Rename,
    ] {
        // Output folder == source folder and same format → same target name.
        let r = run_collect(
            &job(
                std::slice::from_ref(&src),
                d.path(),
                ImageFormat::Png,
                policy,
            ),
            &AtomicBool::new(false),
        );
        match policy {
            ConflictPolicy::Overwrite => assert_eq!(
                r[0].error.as_ref().unwrap().code,
                FileErrorCode::WouldOverwriteSource
            ),
            ConflictPolicy::Skip => assert_eq!(r[0].status, FileStatus::Skipped),
            ConflictPolicy::Rename => {
                assert_eq!(r[0].status, FileStatus::Done);
                assert_ne!(PathBuf::from(r[0].output.as_ref().unwrap()), src);
            }
        }
        assert_eq!(std::fs::read(&src).unwrap(), original, "{policy:?}");
    }
}

#[test]
fn existing_outputs_follow_the_conflict_policy() {
    let d = tempfile::tempdir().unwrap();
    let src = d.path().join("a.png");
    write_png(&src, 10, 10);
    let out = d.path().join("out");
    std::fs::create_dir(&out).unwrap();
    let existing = out.join("a.bmp");
    std::fs::write(&existing, b"previous").unwrap();
    let go = |p| {
        run_collect(
            &job(std::slice::from_ref(&src), &out, ImageFormat::Bmp, p),
            &AtomicBool::new(false),
        )
    };

    let r = go(ConflictPolicy::Skip);
    assert_eq!(r[0].status, FileStatus::Skipped);
    assert_eq!(std::fs::read(&existing).unwrap(), b"previous");

    let r = go(ConflictPolicy::Rename);
    assert_eq!(
        PathBuf::from(r[0].output.as_ref().unwrap()),
        out.join("a (1).bmp")
    );
    assert_eq!(std::fs::read(&existing).unwrap(), b"previous");

    let r = go(ConflictPolicy::Overwrite);
    assert_eq!(
        PathBuf::from(r[0].output.as_ref().unwrap()),
        out.join("a.bmp")
    );
    assert_eq!(image::image_dimensions(&existing).unwrap(), (10, 10));
}

#[test]
fn same_stem_inputs_get_distinct_outputs() {
    let d = tempfile::tempdir().unwrap();
    let a = d.path().join("x.png");
    let b = d.path().join("x.bmp");
    write_png(&a, 8, 8);
    image::open(&a).unwrap().save(&b).unwrap();
    let out = d.path().join("out");
    let r = run_collect(
        &job(&[a, b], &out, ImageFormat::Jpeg, ConflictPolicy::Overwrite),
        &AtomicBool::new(false),
    );
    assert!(r.iter().all(|r| r.status == FileStatus::Done));
    assert_ne!(r[0].output, r[1].output);
}

#[test]
fn cancellation_stops_quickly_and_leaves_no_temp_files() {
    let d = tempfile::tempdir().unwrap();
    let src_dir = d.path().join("in");
    std::fs::create_dir(&src_dir).unwrap();
    let inputs: Vec<PathBuf> = (0..300)
        .map(|i| {
            let p = src_dir.join(format!("{i:03}.png"));
            write_png(&p, 256, 256);
            p
        })
        .collect();
    let out = d.path().join("out");
    let cancel = AtomicBool::new(false);
    let seen = Mutex::new(Vec::new());
    let cancelled_at = Mutex::new(None::<Instant>);
    let j = job(&inputs, &out, ImageFormat::Png, ConflictPolicy::Rename);
    let summary = run(&j, 2, &cancel, |r| {
        let mut s = seen.lock().unwrap();
        s.push(r);
        if s.len() == 5 {
            cancel.store(true, Ordering::Relaxed);
            *cancelled_at.lock().unwrap() = Some(Instant::now());
        }
    })
    .unwrap();
    let stop_latency = cancelled_at.lock().unwrap().unwrap().elapsed();
    assert!(stop_latency < Duration::from_secs(1), "{stop_latency:?}");
    assert!(summary.cancelled > 0);
    assert_eq!(
        summary.done + summary.cancelled + summary.failed + summary.skipped,
        300
    );
    assert_eq!(seen.lock().unwrap().len(), 300);
    assert!(no_temp_files(&out));
    let written = std::fs::read_dir(&out).unwrap().count() as u32;
    assert_eq!(written, summary.done);
}

#[test]
fn thousand_image_batch_completes() {
    let d = tempfile::tempdir().unwrap();
    let src_dir = d.path().join("in");
    std::fs::create_dir(&src_dir).unwrap();
    let inputs: Vec<PathBuf> = (0..1000)
        .map(|i| {
            let p = src_dir.join(format!("{i:04}.png"));
            write_png(&p, 16, 16);
            p
        })
        .collect();
    let out = d.path().join("out");
    let summary = run(
        &job(&inputs, &out, ImageFormat::Jpeg, ConflictPolicy::Rename),
        0,
        &AtomicBool::new(false),
        |_| {},
    )
    .unwrap();
    assert_eq!(summary.done, 1000);
    assert_eq!(std::fs::read_dir(&out).unwrap().count(), 1000);
}

#[test]
fn scan_of_ten_thousand_files_is_fast() {
    let d = tempfile::tempdir().unwrap();
    for i in 0..10_000 {
        let sub = d.path().join(format!("d{}", i % 20));
        if i < 20 {
            std::fs::create_dir(&sub).unwrap();
        }
        std::fs::write(sub.join(format!("{i}.jpg")), b"").unwrap();
    }
    let t = Instant::now();
    let r = ibs_core::scan(&[d.path().to_path_buf()], &AtomicBool::new(false));
    assert_eq!(r.files.len(), 10_000);
    assert!(t.elapsed() < Duration::from_secs(5), "{:?}", t.elapsed());
}

#[test]
fn invalid_jobs_are_rejected_before_starting() {
    let d = tempfile::tempdir().unwrap();
    let src = d.path().join("a.png");
    write_png(&src, 4, 4);
    let cancel = AtomicBool::new(false);
    let mut j = job(
        std::slice::from_ref(&src),
        d.path(),
        ImageFormat::Png,
        ConflictPolicy::Rename,
    );
    j.output.target_size = Some(1000);
    assert!(run(&j, 1, &cancel, |_| {}).is_err());
    j.output.target_size = None;
    j.output.mode = OutputMode::Convert(ImageFormat::Png);
    j.output.dir = "relative/dir".into();
    assert!(run(&j, 1, &cancel, |_| {}).is_err());
    j.output.dir = d.path().to_string_lossy().into_owned();
    j.output.quality = 0;
    assert!(run(&j, 1, &cancel, |_| {}).is_err());
    j.inputs.clear();
    j.output.quality = 50;
    assert!(run(&j, 1, &cancel, |_| {}).is_err());
}

#[test]
fn animated_gif_is_flagged_as_first_frame_only() {
    use image::codecs::gif::GifEncoder;
    use image::{Delay, Frame};
    let d = tempfile::tempdir().unwrap();
    let anim = d.path().join("anim.gif");
    let still = d.path().join("still.gif");
    {
        let mut enc = GifEncoder::new(std::fs::File::create(&anim).unwrap());
        let frames = (0..3u8).map(|i| {
            Frame::from_parts(
                RgbaImage::from_pixel(8, 8, Rgba([i * 80, 0, 0, 255])),
                0,
                0,
                Delay::from_numer_denom_ms(100, 1),
            )
        });
        enc.encode_frames(frames).unwrap();
    }
    image::DynamicImage::ImageRgba8(RgbaImage::from_pixel(8, 8, Rgba([1, 2, 3, 255])))
        .save(&still)
        .unwrap();
    let out = d.path().join("out");
    let r = run_collect(
        &job(
            &[anim, still],
            &out,
            ImageFormat::Png,
            ConflictPolicy::Rename,
        ),
        &AtomicBool::new(false),
    );
    assert_eq!(r[0].status, FileStatus::Done);
    assert_eq!(r[0].warnings, vec![ibs_core::FileWarning::FirstFrameOnly]);
    assert!(r[1].warnings.is_empty());
}

#[cfg(unix)]
#[test]
fn scan_accepts_file_symlinks_but_not_folder_symlinks() {
    let d = tempfile::tempdir().unwrap();
    let real = d.path().join("real");
    std::fs::create_dir(&real).unwrap();
    write_png(&real.join("a.png"), 4, 4);
    let picked = d.path().join("picked");
    std::fs::create_dir(&picked).unwrap();
    std::os::unix::fs::symlink(real.join("a.png"), picked.join("link.png")).unwrap();
    std::os::unix::fs::symlink(&real, picked.join("linked-dir")).unwrap();
    let cancel = AtomicBool::new(false);

    let r = ibs_core::scan(&[picked.join("link.png")], &cancel);
    assert_eq!(r.files.len(), 1);
    assert!(r.files[0].size > 0);

    let r = ibs_core::scan(std::slice::from_ref(&picked), &cancel);
    assert_eq!(
        r.files.iter().map(|f| f.name.as_str()).collect::<Vec<_>>(),
        ["link.png"]
    );

    let r = ibs_core::scan(&[picked, real], &cancel);
    assert_eq!(r.files.len(), 1, "link and target are the same file");
}
