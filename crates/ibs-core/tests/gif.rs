//! Byte-level GIF metadata stripping: animation must survive a strip.

use ibs_core::{
    gif, run, ConflictPolicy, FileResult, FileStatus, ImageFormat, Job, MetadataPolicy, OutputMode,
    OutputSpec,
};
use image::codecs::gif::{GifDecoder, GifEncoder};
use image::AnimationDecoder;
use image::{Delay, Frame, Rgba, RgbaImage};
use std::path::Path;
use std::sync::atomic::AtomicBool;
use std::sync::Mutex;

fn animated_gif(path: &Path, frames: u8) {
    let mut enc = GifEncoder::new(std::fs::File::create(path).unwrap());
    enc.set_repeat(image::codecs::gif::Repeat::Infinite)
        .unwrap();
    enc.encode_frames((0..frames).map(|i| {
        Frame::from_parts(
            RgbaImage::from_pixel(8, 8, Rgba([i * 60, 10, 20, 255])),
            0,
            0,
            Delay::from_numer_denom_ms(100, 1),
        )
    }))
    .unwrap();
}

/// Inserts comment / XMP / ICC extension blocks just before the trailer.
fn add_metadata_blocks(bytes: &mut Vec<u8>) {
    assert_eq!(bytes.last(), Some(&0x3B));
    bytes.pop();
    bytes.extend([0x21, 0xFE, 0x05]);
    bytes.extend(b"HELLO");
    bytes.push(0x00);
    bytes.extend([0x21, 0xFF, 0x0B]);
    bytes.extend(b"XMP DataXMP");
    bytes.extend([0x03]);
    bytes.extend(b"abc");
    bytes.push(0x00);
    bytes.extend([0x21, 0xFF, 0x0B]);
    bytes.extend(b"ICCRGBG1011");
    bytes.extend([0x03]);
    bytes.extend(b"xyz");
    bytes.push(0x00);
    bytes.push(0x3B);
}

fn frame_count(bytes: &[u8]) -> usize {
    GifDecoder::new(std::io::Cursor::new(bytes))
        .unwrap()
        .into_frames()
        .count()
}

#[test]
fn strip_drops_metadata_blocks_and_keeps_animation_and_loop() {
    let d = tempfile::tempdir().unwrap();
    let src = d.path().join("anim.gif");
    animated_gif(&src, 3);
    let mut bytes = std::fs::read(&src).unwrap();
    add_metadata_blocks(&mut bytes);

    let out = gif::strip_metadata(&bytes).unwrap();
    assert!(!out.windows(5).any(|w| w == b"HELLO"));
    assert!(!out.windows(8).any(|w| w == b"XMP Data"));
    assert!(!out.windows(7).any(|w| w == b"ICCRGBG"));
    // NETSCAPE loop extension survived.
    assert!(out.windows(8).any(|w| w == b"NETSCAPE"));
    assert_eq!(out.last(), Some(&0x3B));
    assert_eq!(frame_count(&out), 3);
}

#[test]
fn gce_bound_to_a_dropped_text_block_is_removed_with_it() {
    let d = tempfile::tempdir().unwrap();
    let src = d.path().join("anim.gif");
    animated_gif(&src, 3);
    let mut bytes = std::fs::read(&src).unwrap();
    // A GCE (huge 10 s delay) bound to a plain-text block, before the trailer.
    assert_eq!(bytes.last(), Some(&0x3B));
    bytes.pop();
    bytes.extend([0x21, 0xF9, 0x04, 0x04, 0xE8, 0x03, 0x00, 0x00]);
    bytes.extend([0x21, 0x01, 0x0C]);
    bytes.extend([0x00; 12]); // fixed-size plain-text header
    bytes.extend([0x03]);
    bytes.extend(b"txt");
    bytes.push(0x00);
    bytes.push(0x3B);
    let gces = |b: &[u8]| b.windows(3).filter(|w| w == b"\x21\xF9\x04").count();
    let before = gces(&bytes);
    let out = gif::strip_metadata(&bytes).unwrap();
    assert_eq!(gces(&out), before - 1, "the text block's GCE must go too");
    assert_eq!(frame_count(&out), 3);
}

#[test]
fn gif_without_metadata_is_unchanged() {
    let d = tempfile::tempdir().unwrap();
    let src = d.path().join("clean.gif");
    animated_gif(&src, 2);
    let bytes = std::fs::read(&src).unwrap();
    let out = gif::strip_metadata(&bytes).unwrap();
    assert_eq!(out, bytes);
}

#[test]
fn malformed_input_fails_instead_of_looping() {
    assert!(gif::strip_metadata(b"not a gif").is_err());
    assert!(gif::strip_metadata(b"GIF89a\x00").is_err());
    // Truncated mid-extension.
    assert!(gif::strip_metadata(b"GIF89a\x01\x00\x01\x00\x00\x00\x00\x21\xFE\x05HE").is_err());
}

#[test]
fn metadata_strip_job_keeps_all_gif_frames() {
    let d = tempfile::tempdir().unwrap();
    let src = d.path().join("anim.gif");
    animated_gif(&src, 3);
    let mut bytes = std::fs::read(&src).unwrap();
    add_metadata_blocks(&mut bytes);
    std::fs::write(&src, &bytes).unwrap();
    let out = d.path().join("out");
    let job = Job {
        inputs: vec![src.to_string_lossy().into_owned()],
        output: OutputSpec {
            dir: out.to_string_lossy().into_owned(),
            mode: OutputMode::SameAsSource,
            quality: 80,
            lossless: true,
            metadata: MetadataPolicy::Strip,
            target_size: None,
            conflict: ConflictPolicy::Rename,
        },
        transform: None,
        resize: None,
        watermark: None,
        rename: None,
        sprite: None,
        thumbnail: None,
    };
    let results = Mutex::new(Vec::new());
    run(&job, 2, &AtomicBool::new(false), |r: FileResult| {
        results.lock().unwrap().push(r)
    })
    .unwrap();
    let r = &results.lock().unwrap()[0];
    assert_eq!(r.status, FileStatus::Done);
    // No more "first frame only": the whole animation is preserved.
    assert!(r.warnings.is_empty());
    let produced = std::fs::read(r.output.as_ref().unwrap()).unwrap();
    assert_eq!(frame_count(&produced), 3);
    assert!(!produced.windows(5).any(|w| w == b"HELLO"));
    assert_eq!(gif::screen_size(&produced), Some((8, 8)));
    // The source file is untouched.
    assert_eq!(std::fs::read(&src).unwrap(), bytes);
}

#[test]
fn gif_strip_with_target_size_warns_instead_of_silently_ignoring() {
    let d = tempfile::tempdir().unwrap();
    let src = d.path().join("anim.gif");
    animated_gif(&src, 2);
    let out = d.path().join("out");
    let job = Job {
        inputs: vec![src.to_string_lossy().into_owned()],
        output: OutputSpec {
            dir: out.to_string_lossy().into_owned(),
            mode: OutputMode::SameAsSource,
            quality: 80,
            lossless: true,
            metadata: MetadataPolicy::Strip,
            target_size: Some(10_000),
            conflict: ConflictPolicy::Rename,
        },
        transform: None,
        resize: None,
        watermark: None,
        rename: None,
        sprite: None,
        thumbnail: None,
    };
    let results = Mutex::new(Vec::new());
    run(&job, 2, &AtomicBool::new(false), |r: FileResult| {
        results.lock().unwrap().push(r)
    })
    .unwrap();
    let r = &results.lock().unwrap()[0];
    assert_eq!(r.status, FileStatus::Done);
    assert_eq!(
        r.warnings,
        vec![ibs_core::FileWarning::TargetSizeUnsupported]
    );
}

#[test]
fn converting_an_animated_gif_still_uses_first_frame() {
    // Any real re-encode (format change) keeps the old behaviour.
    let d = tempfile::tempdir().unwrap();
    let src = d.path().join("anim.gif");
    animated_gif(&src, 3);
    let out = d.path().join("out");
    let job = Job {
        inputs: vec![src.to_string_lossy().into_owned()],
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
        watermark: None,
        rename: None,
        sprite: None,
        thumbnail: None,
    };
    let results = Mutex::new(Vec::new());
    run(&job, 2, &AtomicBool::new(false), |r: FileResult| {
        results.lock().unwrap().push(r)
    })
    .unwrap();
    let r = &results.lock().unwrap()[0];
    assert_eq!(r.status, FileStatus::Done);
    assert_eq!(
        r.warnings,
        vec![ibs_core::FileWarning::FirstFrameOnly],
        "re-encode to a still format keeps warning"
    );
}
