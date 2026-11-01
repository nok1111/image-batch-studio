use ibs_core::*;
use image::codecs::gif::GifEncoder;
use image::{Delay, Frame, ImageFormat as F, RgbImage, Rgba, RgbaImage};
use std::path::PathBuf;
use std::sync::atomic::AtomicBool;
use std::sync::Mutex;

fn collect(paths: &[PathBuf]) -> Vec<AnalysisResult> {
    let out = Mutex::new(Vec::new());
    analyze(paths, 4, &AtomicBool::new(false), |r| {
        out.lock().unwrap().push(r)
    })
    .unwrap();
    let mut v = out.into_inner().unwrap();
    v.sort_by_key(|r| r.index);
    v
}

#[test]
fn reports_dimensions_format_color_alpha_and_estimate() {
    let dir = tempfile::tempdir().unwrap();
    let png = dir.path().join("a.png");
    RgbaImage::from_pixel(24, 12, Rgba([1, 2, 3, 200]))
        .save_with_format(&png, F::Png)
        .unwrap();
    let results = collect(&[png.clone()]);
    assert_eq!(results.len(), 1);
    let r = &results[0];
    assert_eq!(r.status, FileStatus::Done);
    assert_eq!(r.bytes, std::fs::metadata(&png).unwrap().len());
    let a = r.analysis.as_ref().unwrap();
    assert_eq!((a.width, a.height), (24, 12));
    assert_eq!(a.format, "png");
    assert_eq!(a.color, "rgba8");
    assert!(a.has_alpha);
    assert!(!a.animated);
    assert!(!a.has_exif);
    assert!(a.est_webp_bytes.unwrap() > 0);
}

#[test]
fn detects_exif_and_animated_gif() {
    let dir = tempfile::tempdir().unwrap();
    // JPEG with a fake EXIF APP1 segment.
    let jpg = dir.path().join("photo.jpg");
    let mut data = Vec::new();
    RgbImage::from_pixel(10, 10, image::Rgb([1, 2, 3]))
        .write_to(&mut std::io::Cursor::new(&mut data), F::Jpeg)
        .unwrap();
    let exif = b"Exif\0\0FAKE-EXIF-BLOB";
    let seg_len = (exif.len() + 2) as u16;
    std::fs::write(
        &jpg,
        [
            &data[..2],
            &[0xFF, 0xE1, (seg_len >> 8) as u8, seg_len as u8],
            exif,
            &data[2..],
        ]
        .concat(),
    )
    .unwrap();

    let anim = dir.path().join("anim.gif");
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

    let results = collect(&[jpg, anim]);
    let jpeg = results[0].analysis.as_ref().unwrap();
    assert_eq!(jpeg.format, "jpeg");
    assert_eq!(jpeg.color, "rgb8");
    assert!(!jpeg.has_alpha);
    assert!(jpeg.has_exif);
    let gif = results[1].analysis.as_ref().unwrap();
    assert_eq!(gif.format, "gif");
    assert!(gif.animated);
}

#[test]
fn missing_or_unsupported_files_fail_only_themselves() {
    let dir = tempfile::tempdir().unwrap();
    let ok = dir.path().join("ok.png");
    RgbaImage::from_pixel(4, 4, Rgba([0, 0, 0, 255]))
        .save_with_format(&ok, F::Png)
        .unwrap();
    let missing = dir.path().join("missing.png");
    let results = collect(&[missing, ok]);
    assert_eq!(results[0].status, FileStatus::Failed);
    assert!(results[0].analysis.is_none());
    assert_eq!(results[1].status, FileStatus::Done);
}

#[test]
fn cancelling_marks_unprocessed_files() {
    let dir = tempfile::tempdir().unwrap();
    let paths: Vec<PathBuf> = (0..5)
        .map(|i| {
            let p = dir.path().join(format!("{i}.png"));
            RgbaImage::from_pixel(4, 4, Rgba([i, 0, 0, 255]))
                .save_with_format(&p, F::Png)
                .unwrap();
            p
        })
        .collect();
    let cancel = AtomicBool::new(true); // already cancelled
    let out = Mutex::new(Vec::new());
    analyze(&paths, 2, &cancel, |r| out.lock().unwrap().push(r)).unwrap();
    let results = out.into_inner().unwrap();
    assert!(results.iter().all(|r| r.status == FileStatus::Cancelled));
}
