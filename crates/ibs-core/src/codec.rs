//! Decoding with safety limits, encoding per output format, and verification.

use crate::{FileError, FileErrorCode, ImageFormat};
use image::codecs::{
    bmp::BmpEncoder, gif::GifEncoder, ico::IcoEncoder, jpeg::JpegEncoder, png::PngEncoder,
    tiff::TiffEncoder, webp::WebPEncoder,
};
use image::{DynamicImage, ImageError, ImageReader, Limits, Rgb, RgbImage};
use std::io::{Seek, Write};
use std::path::Path;

/// Largest accepted width/height (guards against decompression bombs).
pub const MAX_DIMENSION: u32 = 30_000;
/// Largest decoder allocation.
pub const MAX_ALLOC: u64 = 1536 * 1024 * 1024;
pub const ICO_MAX_DIMENSION: u32 = 256;

fn limits() -> Limits {
    let mut l = Limits::default();
    l.max_image_width = Some(MAX_DIMENSION);
    l.max_image_height = Some(MAX_DIMENSION);
    l.max_alloc = Some(MAX_ALLOC);
    l
}

pub struct Decoded {
    pub image: DynamicImage,
    /// Multi-frame input of which only the first frame was decoded.
    pub animated: bool,
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
    let is_gif = reader.format() == Some(image::ImageFormat::Gif);
    reader.limits(limits());
    let image = reader.decode().map_err(map_decode_error)?;
    let animated = is_gif && is_animated_gif(path);
    Ok(Decoded { image, animated })
}

fn is_animated_gif(path: &Path) -> bool {
    use image::{AnimationDecoder, ImageDecoder};
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

pub fn encode<W: Write + Seek>(
    img: &DynamicImage,
    format: ImageFormat,
    quality: u8,
    w: &mut W,
) -> Result<(), FileError> {
    check_encodable(format, img.width(), img.height())?;
    let result = match format {
        ImageFormat::Png => img.write_with_encoder(PngEncoder::new(w)),
        ImageFormat::Jpeg => DynamicImage::ImageRgb8(flatten_on_white(img))
            .write_with_encoder(JpegEncoder::new_with_quality(w, quality.clamp(1, 100))),
        ImageFormat::Webp => to_8bit(img).write_with_encoder(WebPEncoder::new_lossless(w)),
        ImageFormat::Bmp => to_8bit(img).write_with_encoder(BmpEncoder::new(w)),
        ImageFormat::Tiff => to_8bit(img).write_with_encoder(TiffEncoder::new(w)),
        ImageFormat::Gif => {
            let frame = image::Frame::new(img.to_rgba8());
            GifEncoder::new(w).encode_frame(frame)
        }
        ImageFormat::Ico => {
            DynamicImage::ImageRgba8(img.to_rgba8()).write_with_encoder(IcoEncoder::new(w))
        }
        ImageFormat::Avif => unreachable!("rejected by check_encodable"),
    };
    result.map_err(encode_error)
}

/// A written file counts as successful only if it is non-empty and decodes
/// back to the expected dimensions.
pub fn verify(path: &Path, width: u32, height: u32) -> Result<(), FileError> {
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
    if (img.width(), img.height()) != (width, height) {
        return Err(FileError::new(
            FileErrorCode::Verify,
            format!(
                "output is {}×{}, expected {width}×{height}",
                img.width(),
                img.height()
            ),
        ));
    }
    Ok(())
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
    fn ico_and_avif_limits() {
        assert!(check_encodable(ImageFormat::Ico, 256, 256).is_ok());
        assert_eq!(
            check_encodable(ImageFormat::Ico, 257, 10).unwrap_err().code,
            FileErrorCode::OutputUnsupported
        );
        assert_eq!(
            check_encodable(ImageFormat::Avif, 1, 1).unwrap_err().code,
            FileErrorCode::OutputUnsupported
        );
    }
}
