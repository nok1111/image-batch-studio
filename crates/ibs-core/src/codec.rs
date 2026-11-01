//! Decoding with safety limits, encoding per output format (with optional
//! metadata), target-size search and verification.

use crate::{FileError, FileErrorCode, ImageFormat};
use image::codecs::{
    avif::AvifEncoder,
    bmp::BmpEncoder,
    gif::GifEncoder,
    ico::IcoEncoder,
    jpeg::JpegEncoder,
    png::{CompressionType, FilterType, PngEncoder},
    tiff::TiffEncoder,
    webp::WebPEncoder,
};
use image::metadata::Orientation;
use image::{
    DynamicImage, ImageDecoder, ImageEncoder, ImageError, ImageReader, Limits, Rgb, RgbImage,
};
use std::io::Cursor;
use std::path::Path;

/// Largest accepted width/height (guards against decompression bombs).
pub const MAX_DIMENSION: u32 = 30_000;
/// Largest decoder allocation.
pub const MAX_ALLOC: u64 = 1536 * 1024 * 1024;
pub const ICO_MAX_DIMENSION: u32 = 256;
/// rav1e speed (1 slowest … 10 fastest).
const AVIF_SPEED: u8 = 6;

fn limits() -> Limits {
    let mut l = Limits::default();
    l.max_image_width = Some(MAX_DIMENSION);
    l.max_image_height = Some(MAX_DIMENSION);
    l.max_alloc = Some(MAX_ALLOC);
    l
}

pub struct Decoded {
    /// Pixels with the EXIF orientation already applied.
    pub image: DynamicImage,
    /// Multi-frame input of which only the first frame was decoded.
    pub animated: bool,
    pub icc: Option<Vec<u8>>,
    /// EXIF with its orientation tag reset (the pixels are already upright).
    pub exif: Option<Vec<u8>>,
}

/// Decodes by content (not extension), with dimension and memory limits.
pub fn decode(path: &Path) -> Result<Decoded, FileError> {
    let mut reader = ImageReader::open(path)
        .map_err(FileError::io)?
        .with_guessed_format()
        .map_err(FileError::io)?;
    if reader.format().is_none() {
        return Err(FileError::new(
            FileErrorCode::Unsupported,
            "not a recognized image file",
        ));
    }
    if reader.format() == Some(image::ImageFormat::Avif) {
        // The `image` crate has no pure-Rust AVIF decoder.
        return decode_avif(path);
    }
    let is_gif = reader.format() == Some(image::ImageFormat::Gif);
    reader.limits(limits());
    let mut decoder = reader.into_decoder().map_err(map_decode_error)?;
    let icc = decoder.icc_profile().ok().flatten();
    let mut exif = decoder.exif_metadata().ok().flatten();
    let orientation = decoder.orientation().unwrap_or(Orientation::NoTransforms);
    if let Some(e) = exif.as_mut() {
        let _ = Orientation::remove_from_exif_chunk(e);
    }
    let mut image = DynamicImage::from_decoder(decoder).map_err(map_decode_error)?;
    image.apply_orientation(orientation);
    let animated = is_gif && is_animated_gif(path);
    Ok(Decoded {
        image,
        animated,
        icc,
        exif,
    })
}

/// Decodes an AVIF with zenavif (pure-Rust rav1d), applying `irot`/`imir`.
fn decode_avif(path: &Path) -> Result<Decoded, FileError> {
    let data = std::fs::read(path).map_err(FileError::io)?;
    // `probe` can't read grid (tiled) AVIFs, so treat it as advisory: reject
    // oversized files early when it works, and check again after decoding.
    let probe = zenavif::detect::probe(&data).ok();
    let animated = probe.as_ref().is_some_and(|p| p.has_animation);
    if let Some(p) = &probe {
        if p.width > MAX_DIMENSION || p.height > MAX_DIMENSION {
            return Err(FileError::new(
                FileErrorCode::TooLarge,
                format!(
                    "{}×{} exceeds the {MAX_DIMENSION}px limit",
                    p.width, p.height
                ),
            ));
        }
    }
    let config = zenavif::DecoderConfig::new()
        .prefer_8bit(true)
        .frame_size_limit(MAX_ALLOC as u32 / 4);
    let mut decoder = zenavif::ManagedAvifDecoder::new(&data, &config).map_err(avif_error)?;
    // The parser keeps its own copy; ours is dead weight.
    drop(data);
    // Container-level dimensions (covers grid AVIFs detect::probe can't read),
    // checked before any pixel allocation.
    let info_dims = decoder.probe_info().map_err(avif_error)?;
    if info_dims.width > MAX_DIMENSION || info_dims.height > MAX_DIMENSION {
        return Err(FileError::new(
            FileErrorCode::TooLarge,
            format!(
                "{}×{} exceeds the {MAX_DIMENSION}px limit",
                info_dims.width, info_dims.height
            ),
        ));
    }
    let (buf, info) = decoder
        .decode_full(&zenavif::Unstoppable)
        .map_err(avif_error)?;
    let (w, h) = (buf.width(), buf.height());
    if w > MAX_DIMENSION || h > MAX_DIMENSION {
        return Err(FileError::new(
            FileErrorCode::TooLarge,
            format!("{w}×{h} exceeds the {MAX_DIMENSION}px limit"),
        ));
    }
    let bytes = buf.as_slice().contiguous_bytes().into_owned();
    let desc = buf.descriptor();
    let mut image = if desc.layout_compatible(zenpixels::PixelDescriptor::RGBA8) {
        DynamicImage::ImageRgba8(
            image::RgbaImage::from_raw(w, h, bytes).ok_or_else(avif_decode_err)?,
        )
    } else if desc.layout_compatible(zenpixels::PixelDescriptor::RGB8) {
        DynamicImage::ImageRgb8(RgbImage::from_raw(w, h, bytes).ok_or_else(avif_decode_err)?)
    } else {
        return Err(FileError::new(
            FileErrorCode::Unsupported,
            format!("unsupported AVIF pixel format: {desc:?}"),
        ));
    };
    // `irot` angles are counter-clockwise while image's rotate* are clockwise.
    if let Some(rot) = info.rotation {
        image = match rot.angle {
            90 => image.rotate270(),
            180 => image.rotate180(),
            270 => image.rotate90(),
            _ => image,
        };
    }
    if let Some(mirror) = info.mirror {
        image = match mirror.axis {
            0 => image.fliph(),
            _ => image.flipv(),
        };
    }
    let mut exif = info.exif;
    if let Some(chunk) = exif.as_deref_mut() {
        // Orientation parsing expects the TIFF blob, without the "Exif\0\0" prefix.
        let skip = if chunk.starts_with(b"Exif\0\0") { 6 } else { 0 };
        if let Some(orientation) = Orientation::remove_from_exif_chunk(&mut chunk[skip..]) {
            image.apply_orientation(orientation);
        }
    }
    Ok(Decoded {
        image,
        animated,
        icc: info.icc_profile,
        exif,
    })
}

fn avif_error(e: impl std::fmt::Display) -> FileError {
    FileError::new(FileErrorCode::Decode, e.to_string())
}

fn avif_decode_err() -> FileError {
    FileError::new(FileErrorCode::Decode, "malformed AVIF pixel buffer")
}

fn is_animated_gif(path: &Path) -> bool {
    use image::AnimationDecoder;
    let Ok(file) = std::fs::File::open(path) else {
        return false;
    };
    let Ok(mut decoder) = image::codecs::gif::GifDecoder::new(std::io::BufReader::new(file)) else {
        return false;
    };
    if decoder.set_limits(limits()).is_err() {
        return false;
    }
    decoder.into_frames().take(2).filter(Result::is_ok).count() > 1
}

fn map_decode_error(e: ImageError) -> FileError {
    let code = match &e {
        ImageError::Limits(_) => FileErrorCode::TooLarge,
        ImageError::Unsupported(_) => FileErrorCode::Unsupported,
        ImageError::IoError(io) if io.kind() != std::io::ErrorKind::UnexpectedEof => {
            FileErrorCode::Io
        }
        _ => FileErrorCode::Decode,
    };
    FileError::new(code, e.to_string())
}

fn encode_error(e: ImageError) -> FileError {
    match e {
        ImageError::IoError(io) => FileError::io(io),
        other => FileError::new(FileErrorCode::Encode, other.to_string()),
    }
}

/// Fails early when `format` cannot be produced from an image of this size.
pub fn check_encodable(format: ImageFormat, width: u32, height: u32) -> Result<(), FileError> {
    if !format.can_encode() {
        return Err(FileError::new(
            FileErrorCode::OutputUnsupported,
            format!("{format:?} output is not available in this version"),
        ));
    }
    if format == ImageFormat::Ico && (width > ICO_MAX_DIMENSION || height > ICO_MAX_DIMENSION) {
        return Err(FileError::new(
            FileErrorCode::OutputUnsupported,
            format!("ICO supports up to {ICO_MAX_DIMENSION}×{ICO_MAX_DIMENSION} pixels"),
        ));
    }
    Ok(())
}

/// 8-bit RGB or RGBA depending on whether the source has alpha.
fn to_8bit(img: &DynamicImage) -> DynamicImage {
    match img {
        DynamicImage::ImageRgb8(_) | DynamicImage::ImageRgba8(_) => img.clone(),
        _ if img.color().has_alpha() => DynamicImage::ImageRgba8(img.to_rgba8()),
        _ => DynamicImage::ImageRgb8(img.to_rgb8()),
    }
}

/// Composites transparency onto white, for formats without alpha.
fn flatten_on_white(img: &DynamicImage) -> RgbImage {
    if !img.color().has_alpha() {
        return img.to_rgb8();
    }
    let rgba = img.to_rgba8();
    RgbImage::from_fn(rgba.width(), rgba.height(), |x, y| {
        let p = rgba.get_pixel(x, y).0;
        let a = u16::from(p[3]);
        let blend = |c: u8| ((u16::from(c) * a + 255 * (255 - a) + 127) / 255) as u8;
        Rgb([blend(p[0]), blend(p[1]), blend(p[2])])
    })
}

#[derive(Debug, Clone, Copy)]
pub struct EncodeOptions<'a> {
    pub format: ImageFormat,
    pub quality: u8,
    /// WebP only.
    pub lossless: bool,
    pub icc: Option<&'a [u8]>,
    pub exif: Option<&'a [u8]>,
}

pub struct Encoded {
    pub bytes: Vec<u8>,
    /// Some requested metadata could not be stored in this format.
    pub metadata_dropped: bool,
}

/// (ICC, EXIF) support per output format.
fn metadata_support(format: ImageFormat, lossless: bool) -> (bool, bool) {
    match format {
        ImageFormat::Png | ImageFormat::Jpeg => (true, true),
        ImageFormat::Webp => {
            let _ = lossless;
            (true, true)
        }
        ImageFormat::Avif => (false, true),
        ImageFormat::Tiff => (true, false),
        ImageFormat::Bmp | ImageFormat::Gif | ImageFormat::Ico => (false, false),
    }
}

fn with_metadata<E: ImageEncoder>(mut e: E, icc: Option<&[u8]>, exif: Option<&[u8]>) -> E {
    if let Some(icc) = icc {
        let _ = e.set_icc_profile(icc.to_vec());
    }
    if let Some(exif) = exif {
        let _ = e.set_exif_metadata(exif.to_vec());
    }
    e
}

pub fn encode(img: &DynamicImage, o: &EncodeOptions) -> Result<Encoded, FileError> {
    check_encodable(o.format, img.width(), img.height())?;
    let (icc_ok, exif_ok) = metadata_support(o.format, o.lossless);
    let icc = o.icc.filter(|_| icc_ok);
    let exif = o.exif.filter(|_| exif_ok);
    let metadata_dropped = (o.icc.is_some() && !icc_ok) || (o.exif.is_some() && !exif_ok);
    let q = o.quality.clamp(1, 100);
    let mut buf = Cursor::new(Vec::new());
    let w = &mut buf;
    let result = match o.format {
        ImageFormat::Png => img.write_with_encoder(with_metadata(
            PngEncoder::new_with_quality(w, CompressionType::Best, FilterType::Adaptive),
            icc,
            exif,
        )),
        ImageFormat::Jpeg => DynamicImage::ImageRgb8(flatten_on_white(img)).write_with_encoder(
            with_metadata(JpegEncoder::new_with_quality(w, q), icc, exif),
        ),
        ImageFormat::Webp if o.lossless => {
            to_8bit(img).write_with_encoder(with_metadata(WebPEncoder::new_lossless(w), icc, exif))
        }
        ImageFormat::Webp => {
            let bytes = encode_webp_lossy(img, q, icc, exif)?;
            return Ok(Encoded {
                bytes,
                metadata_dropped,
            });
        }
        ImageFormat::Avif => to_8bit(img).write_with_encoder(with_metadata(
            AvifEncoder::new_with_speed_quality(w, AVIF_SPEED, q).with_num_threads(Some(1)),
            None,
            exif,
        )),
        ImageFormat::Bmp => to_8bit(img).write_with_encoder(BmpEncoder::new(w)),
        ImageFormat::Tiff => {
            to_8bit(img).write_with_encoder(with_metadata(TiffEncoder::new(w), icc, None))
        }
        ImageFormat::Gif => {
            let frame = image::Frame::new(img.to_rgba8());
            GifEncoder::new(w).encode_frame(frame)
        }
        ImageFormat::Ico => {
            DynamicImage::ImageRgba8(img.to_rgba8()).write_with_encoder(IcoEncoder::new(w))
        }
    };
    result.map_err(encode_error)?;
    Ok(Encoded {
        bytes: buf.into_inner(),
        metadata_dropped,
    })
}

fn encode_webp_lossy(
    img: &DynamicImage,
    quality: u8,
    icc: Option<&[u8]>,
    exif: Option<&[u8]>,
) -> Result<Vec<u8>, FileError> {
    let img = to_8bit(img);
    let (w, h) = (img.width(), img.height());
    let encoder = match &img {
        DynamicImage::ImageRgba8(p) => webp::Encoder::from_rgba(p.as_raw(), w, h),
        DynamicImage::ImageRgb8(p) => webp::Encoder::from_rgb(p.as_raw(), w, h),
        _ => unreachable!("to_8bit returns RGB8 or RGBA8"),
    };
    let encoded = encoder
        .encode_advanced(&{
            let mut c = webp::WebPConfig::new()
                .map_err(|_| FileError::new(FileErrorCode::Encode, "WebP config failed"))?;
            c.quality = f32::from(quality);
            c.method = 4;
            c
        })
        .map_err(|e| FileError::new(FileErrorCode::Encode, format!("WebP: {e:?}")))?;
    if icc.is_none() && exif.is_none() {
        return Ok(encoded.to_vec());
    }
    webp_mux(&encoded, w, h, icc, exif)
        .ok_or_else(|| FileError::new(FileErrorCode::Encode, "malformed WebP from encoder"))
}

/// Rewraps a simple WebP as extended (VP8X) with ICCP/EXIF chunks.
fn webp_mux(
    data: &[u8],
    width: u32,
    height: u32,
    icc: Option<&[u8]>,
    exif: Option<&[u8]>,
) -> Option<Vec<u8>> {
    if data.len() < 12 || &data[0..4] != b"RIFF" || &data[8..12] != b"WEBP" {
        return None;
    }
    let mut image_chunks = Vec::new();
    let mut alpha = false;
    let mut pos = 12;
    while pos + 8 <= data.len() {
        let fourcc = &data[pos..pos + 4];
        let len = u32::from_le_bytes(data[pos + 4..pos + 8].try_into().ok()?) as usize;
        let end = pos.checked_add(8 + len + (len & 1))?.min(data.len());
        match fourcc {
            b"VP8X" => alpha |= data.get(pos + 8).is_some_and(|f| f & 0x10 != 0),
            b"ALPH" => {
                alpha = true;
                image_chunks.extend_from_slice(&data[pos..end]);
            }
            _ => image_chunks.extend_from_slice(&data[pos..end]),
        }
        pos = end;
    }
    let chunk = |out: &mut Vec<u8>, fourcc: &[u8], body: &[u8]| {
        out.extend_from_slice(fourcc);
        out.extend_from_slice(&(body.len() as u32).to_le_bytes());
        out.extend_from_slice(body);
        if body.len() % 2 == 1 {
            out.push(0);
        }
    };
    let mut flags = 0u8;
    if icc.is_some() {
        flags |= 0x20;
    }
    if alpha {
        flags |= 0x10;
    }
    if exif.is_some() {
        flags |= 0x08;
    }
    let mut vp8x = vec![flags, 0, 0, 0];
    vp8x.extend_from_slice(&(width - 1).to_le_bytes()[..3]);
    vp8x.extend_from_slice(&(height - 1).to_le_bytes()[..3]);
    let mut body = b"WEBP".to_vec();
    chunk(&mut body, b"VP8X", &vp8x);
    if let Some(icc) = icc {
        chunk(&mut body, b"ICCP", icc);
    }
    body.extend_from_slice(&image_chunks);
    if let Some(exif) = exif {
        chunk(&mut body, b"EXIF", exif);
    }
    let mut out = b"RIFF".to_vec();
    out.extend_from_slice(&(body.len() as u32).to_le_bytes());
    out.extend_from_slice(&body);
    Some(out)
}

pub struct Sized {
    pub encoded: Encoded,
    /// Even quality 1 is larger than the target.
    pub target_missed: bool,
}

/// Highest quality in `1..=o.quality` whose output is at most `target` bytes.
pub fn encode_to_size(
    img: &DynamicImage,
    o: &EncodeOptions,
    target: u64,
) -> Result<Sized, FileError> {
    let first = encode(img, o)?;
    if first.bytes.len() as u64 <= target {
        return Ok(Sized {
            encoded: first,
            target_missed: false,
        });
    }
    let (mut lo, mut hi) = (1u8, o.quality.saturating_sub(1).max(1));
    let mut best: Option<Encoded> = None;
    while lo <= hi {
        let mid = lo + (hi - lo) / 2;
        let e = encode(img, &EncodeOptions { quality: mid, ..*o })?;
        if e.bytes.len() as u64 <= target {
            best = Some(e);
            lo = mid + 1;
        } else if mid == 1 {
            break;
        } else {
            hi = mid - 1;
        }
    }
    match best {
        Some(encoded) => Ok(Sized {
            encoded,
            target_missed: false,
        }),
        None => Ok(Sized {
            encoded: encode(img, &EncodeOptions { quality: 1, ..*o })?,
            target_missed: true,
        }),
    }
}

/// A written file counts as successful only if it is non-empty and decodes
/// back to the expected dimensions.
pub fn verify(path: &Path, _format: ImageFormat, width: u32, height: u32) -> Result<(), FileError> {
    let len = std::fs::metadata(path).map_err(FileError::io)?.len();
    if len == 0 {
        return Err(FileError::new(
            FileErrorCode::Verify,
            "output file is empty",
        ));
    }
    let img = decode(path)
        .map_err(|e| {
            FileError::new(
                FileErrorCode::Verify,
                format!("output does not decode: {e}"),
            )
        })?
        .image;
    let dims = (img.width(), img.height());
    if dims != (width, height) {
        return Err(FileError::new(
            FileErrorCode::Verify,
            format!("output is {}×{}, expected {width}×{height}", dims.0, dims.1),
        ));
    }
    Ok(())
}

/// Reads width/height from the `ispe` property of an AVIF (`ftyp` = avif).
pub fn avif_dimensions(data: &[u8]) -> Option<(u32, u32)> {
    if data.len() < 12 || &data[4..8] != b"ftyp" || &data[8..12] != b"avif" {
        return None;
    }
    let at = data.windows(4).position(|w| w == b"ispe")?;
    let field = |o: usize| -> Option<u32> {
        Some(u32::from_be_bytes(
            data.get(at + o..at + o + 4)?.try_into().ok()?,
        ))
    };
    // fourcc, version/flags, width, height
    Some((field(8)?, field(12)?))
}

/// Largest file [`display_bytes`] will load for an on-screen comparison.
pub const MAX_DISPLAY_BYTES: u64 = 256 * 1024 * 1024;

/// A written file counts as successful only if it is non-empty and decodes
/// back to the expected dimensions.
pub fn display_bytes(path: &Path) -> Result<Vec<u8>, FileError> {
    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .map(str::to_ascii_lowercase)
        .unwrap_or_default();
    let native = matches!(
        ext.as_str(),
        "png" | "jpg" | "jpeg" | "jpe" | "jfif" | "webp" | "bmp" | "avif"
    );
    let reencode = matches!(ext.as_str(), "tif" | "tiff" | "ico" | "gif");
    if !native && !reencode {
        return Err(FileError::new(
            FileErrorCode::Unsupported,
            format!("not an image file: {}", path.display()),
        ));
    }
    let len = std::fs::metadata(path).map_err(FileError::io)?.len();
    if len > MAX_DISPLAY_BYTES {
        return Err(FileError::new(
            FileErrorCode::TooLarge,
            format!("{} is too large to preview", path.display()),
        ));
    }
    if native {
        return std::fs::read(path).map_err(FileError::io);
    }
    let decoded = decode(path)?;
    let mut out = std::io::Cursor::new(Vec::new());
    decoded
        .image
        .write_to(&mut out, image::ImageFormat::Png)
        .map_err(|e| FileError::new(FileErrorCode::Encode, e.to_string()))?;
    Ok(out.into_inner())
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::{Rgba, RgbaImage};

    #[test]
    fn flatten_composites_transparency_on_white() {
        let mut img = RgbaImage::new(2, 1);
        img.put_pixel(0, 0, Rgba([0, 0, 0, 0]));
        img.put_pixel(1, 0, Rgba([10, 20, 30, 255]));
        let flat = flatten_on_white(&DynamicImage::ImageRgba8(img));
        assert_eq!(flat.get_pixel(0, 0).0, [255, 255, 255]);
        assert_eq!(flat.get_pixel(1, 0).0, [10, 20, 30]);
    }

    #[test]
    fn ico_limit() {
        assert!(check_encodable(ImageFormat::Ico, 256, 256).is_ok());
        assert_eq!(
            check_encodable(ImageFormat::Ico, 257, 10).unwrap_err().code,
            FileErrorCode::OutputUnsupported
        );
    }

    #[test]
    fn lossy_webp_mux_keeps_metadata_and_decodes() {
        let img = DynamicImage::ImageRgba8(RgbaImage::from_fn(33, 17, |x, y| {
            Rgba([x as u8 * 7, y as u8 * 9, 100, if x < 5 { 0 } else { 255 }])
        }));
        let exif = b"Exif\0\0MM\0\x2a\0\0\0\x08\0\0".to_vec();
        let o = EncodeOptions {
            format: ImageFormat::Webp,
            quality: 80,
            lossless: false,
            icc: None,
            exif: Some(&exif),
        };
        let out = encode(&img, &o).unwrap().bytes;
        assert_eq!(&out[12..16], b"VP8X");
        let mut dec = image::codecs::webp::WebPDecoder::new(Cursor::new(&out)).unwrap();
        assert!(dec.exif_metadata().unwrap().is_some());
        let back = DynamicImage::from_decoder(dec).unwrap();
        assert_eq!((back.width(), back.height()), (33, 17));
        assert!(back.color().has_alpha());
    }

    #[test]
    fn avif_input_decodes() {
        let img = DynamicImage::ImageRgba8(RgbaImage::from_fn(37, 23, |x, y| {
            Rgba([x as u8, y as u8, 200, if x < 10 { 128 } else { 255 }])
        }));
        let o = EncodeOptions {
            format: ImageFormat::Avif,
            quality: 80,
            lossless: false,
            icc: None,
            exif: None,
        };
        let bytes = encode(&img, &o).unwrap().bytes;
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("in.avif");
        std::fs::write(&path, &bytes).unwrap();
        let dec = decode(&path).unwrap();
        assert_eq!((dec.image.width(), dec.image.height()), (37, 23));
        assert!(!dec.animated);
        assert!(dec.image.color().has_alpha());
    }

    #[test]
    fn avif_exif_orientation_is_applied() {
        let img = DynamicImage::new_rgb8(40, 24);
        // EXIF orientation 6 (rotate 90° CW), big-endian TIFF.
        let exif =
            b"Exif\0\0MM\0\x2a\0\0\0\x08\0\x01\x01\x12\0\x03\0\0\0\x01\0\x06\0\0\0\0\0\0".to_vec();
        let o = EncodeOptions {
            format: ImageFormat::Avif,
            quality: 80,
            lossless: false,
            icc: None,
            exif: Some(&exif),
        };
        let bytes = encode(&img, &o).unwrap().bytes;
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("rot.avif");
        std::fs::write(&path, &bytes).unwrap();
        let dec = decode(&path).unwrap();
        assert_eq!((dec.image.width(), dec.image.height()), (24, 40));
        // The tag was reset in the returned EXIF, like other decoders.
        if let Some(chunk) = dec.exif {
            assert_eq!(Orientation::from_exif_chunk(&chunk), None);
        }
    }

    #[test]
    fn avif_header_dimensions() {
        let img = DynamicImage::new_rgb8(40, 24);
        let o = EncodeOptions {
            format: ImageFormat::Avif,
            quality: 60,
            lossless: false,
            icc: None,
            exif: None,
        };
        let out = encode(&img, &o).unwrap().bytes;
        assert_eq!(avif_dimensions(&out), Some((40, 24)));
        assert_eq!(avif_dimensions(b"not an avif file at all"), None);
    }
}
