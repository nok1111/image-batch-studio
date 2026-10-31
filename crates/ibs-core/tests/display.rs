use ibs_core::codec::display_bytes;
use image::{ImageFormat, Rgba, RgbaImage};

#[test]
fn native_formats_are_returned_untouched_and_tiff_becomes_png() {
    let dir = tempfile::tempdir().unwrap();
    let img = RgbaImage::from_pixel(7, 5, Rgba([10, 20, 30, 128]));

    let png = dir.path().join("a.png");
    img.save_with_format(&png, ImageFormat::Png).unwrap();
    assert_eq!(display_bytes(&png).unwrap(), std::fs::read(&png).unwrap());

    let tif = dir.path().join("a.tiff");
    img.save_with_format(&tif, ImageFormat::Tiff).unwrap();
    let shown = display_bytes(&tif).unwrap();
    let back = image::load_from_memory_with_format(&shown, ImageFormat::Png).unwrap();
    assert_eq!(back.to_rgba8(), img);

    let txt = dir.path().join("a.txt");
    std::fs::write(&txt, b"hello").unwrap();
    assert!(display_bytes(&txt).is_err());
    assert!(display_bytes(&dir.path().join("missing.png")).is_err());
}
