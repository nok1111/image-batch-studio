use ibs_core::*;
use image::{ImageFormat as F, RgbImage, Rgba, RgbaImage};
use std::path::PathBuf;
use std::sync::atomic::AtomicBool;

fn make(path: &PathBuf, w: u32, h: u32, color: [u8; 4]) {
    RgbaImage::from_pixel(w, h, Rgba(color))
        .save_with_format(path, F::Png)
        .unwrap();
}

fn job(dir: &std::path::Path, inputs: Vec<PathBuf>, transform: Option<TransformSpec>) -> Job {
    Job {
        inputs: inputs
            .iter()
            .map(|p| p.to_string_lossy().into_owned())
            .collect(),
        output: OutputSpec {
            dir: dir.join("out").to_string_lossy().into_owned(),
            mode: OutputMode::SameAsSource,
            quality: 90,
            lossless: false,
            metadata: MetadataPolicy::Strip,
            target_size: None,
            conflict: ConflictPolicy::Rename,
        },
        transform,
        resize: None,
        rename: None,
    }
}

fn run(job: &Job) -> Vec<FileResult> {
    let out = std::sync::Mutex::new(Vec::new());
    ibs_core::run(job, 4, &AtomicBool::new(false), |r| {
        out.lock().unwrap().push(r)
    })
    .unwrap();
    let mut out = out.into_inner().unwrap();
    out.sort_by_key(|r| r.index);
    out
}

fn dims(path: &std::path::Path) -> (u32, u32) {
    let d = image::ImageReader::open(path).unwrap().decode().unwrap();
    (d.width(), d.height())
}

#[test]
fn crop_clamped_and_rotation_swaps_dimensions() {
    let dir = tempfile::tempdir().unwrap();
    let a = dir.path().join("a.png");
    make(&a, 400, 300, [255, 0, 0, 255]);
    let spec = TransformSpec {
        rotate: 90,
        flip_horizontal: false,
        flip_vertical: false,
        crop: Some(CropSpec {
            x: 300,
            y: 0,
            width: 400, // clamped to 100
            height: 200,
        }),
    };
    let results = run(&job(dir.path(), vec![a], Some(spec)));
    assert_eq!(results[0].status, FileStatus::Done);
    let out = results[0].output.as_ref().unwrap();
    // crop 300..400 → 100×200, rotated 90° → 200×100
    assert_eq!(dims(&PathBuf::from(out)), (200, 100));
}

#[test]
fn flips_mirror_pixels() {
    let dir = tempfile::tempdir().unwrap();
    let a = dir.path().join("a.png");
    let mut img = RgbaImage::from_pixel(2, 1, Rgba([0, 0, 0, 255]));
    img.put_pixel(0, 0, Rgba([9, 9, 9, 255]));
    img.save_with_format(&a, F::Png).unwrap();
    let spec = TransformSpec {
        rotate: 0,
        flip_horizontal: true,
        flip_vertical: false,
        crop: None,
    };
    let results = run(&job(dir.path(), vec![a], Some(spec)));
    let out = image::ImageReader::open(PathBuf::from(results[0].output.as_ref().unwrap()))
        .unwrap()
        .decode()
        .unwrap()
        .to_rgba8();
    assert_eq!(*out.get_pixel(1, 0), Rgba([9, 9, 9, 255]));
}

#[test]
fn crop_fully_outside_fails_only_that_file() {
    let dir = tempfile::tempdir().unwrap();
    let a = dir.path().join("a.png");
    let b = dir.path().join("b.png");
    make(&a, 100, 100, [255, 0, 0, 255]);
    make(&b, 300, 300, [0, 0, 255, 255]);
    let spec = TransformSpec {
        rotate: 0,
        flip_horizontal: false,
        flip_vertical: false,
        crop: Some(CropSpec {
            x: 200,
            y: 0,
            width: 50,
            height: 50,
        }),
    };
    let results = run(&job(dir.path(), vec![a, b], Some(spec)));
    assert_eq!(results[0].status, FileStatus::Failed);
    assert_eq!(
        results[0].error.as_ref().unwrap().code,
        FileErrorCode::CropOutside
    );
    assert_eq!(results[1].status, FileStatus::Done);
    assert_eq!(
        dims(&PathBuf::from(results[1].output.as_ref().unwrap())),
        (50, 50)
    );
}

#[test]
fn metadata_cleaner_strips_exif_and_keeps_pixels() {
    let dir = tempfile::tempdir().unwrap();
    let src = dir.path().join("photo.jpg");
    // JPEG with a fake EXIF APP1 segment.
    let mut data = Vec::new();
    RgbImage::from_pixel(40, 30, image::Rgb([1, 2, 3]))
        .write_to(&mut std::io::Cursor::new(&mut data), F::Jpeg)
        .unwrap();
    let exif = b"Exif\0\0FAKE-EXIF-BLOB";
    let seg_len = (exif.len() + 2) as u16;
    let with_exif = [
        &data[..2],
        &[0xFF, 0xE1, (seg_len >> 8) as u8, seg_len as u8],
        exif,
        &data[2..],
    ]
    .concat();
    std::fs::write(&src, with_exif).unwrap();
    assert!(std::fs::read(&src).unwrap().len() > data.len());

    let results = run(&job(dir.path(), vec![src], None));
    assert_eq!(results[0].status, FileStatus::Done);
    let out = std::fs::read(results[0].output.as_ref().unwrap()).unwrap();
    assert!(!out.windows(14).any(|w| w == b"FAKE-EXIF-BLOB"));
    let back = image::ImageReader::open(PathBuf::from(results[0].output.as_ref().unwrap()))
        .unwrap()
        .decode()
        .unwrap();
    assert_eq!((back.width(), back.height()), (40, 30));
}
