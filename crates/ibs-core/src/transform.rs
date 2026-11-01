use crate::error::{FileError, FileErrorCode};
use image::DynamicImage;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// Rotation + flip + rectangular crop applied to every input, before resize.
/// The same spec is applied to every file; a crop that lies entirely outside an
/// image makes that file fail with [`FileErrorCode::CropOutside`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct TransformSpec {
    /// Clockwise rotation: 0, 90, 180 or 270 degrees.
    pub rotate: u16,
    /// Mirror left-right.
    #[serde(default)]
    pub flip_horizontal: bool,
    /// Mirror top-bottom.
    #[serde(default)]
    pub flip_vertical: bool,
    /// Crop rectangle in source pixels; `None` keeps the whole image.
    pub crop: Option<CropSpec>,
}

/// A crop rectangle in source pixels. Clamped to each image's bounds.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct CropSpec {
    #[ts(type = "number")]
    pub x: u32,
    #[ts(type = "number")]
    pub y: u32,
    #[ts(type = "number")]
    pub width: u32,
    #[ts(type = "number")]
    pub height: u32,
}

impl TransformSpec {
    /// Default spec: the identity (no transform). Serialized options may set any
    /// subset of these fields.
    pub fn identity() -> Self {
        Self {
            rotate: 0,
            flip_horizontal: false,
            flip_vertical: false,
            crop: None,
        }
    }

    pub fn validate(&self) -> Result<(), String> {
        if !matches!(self.rotate, 0 | 90 | 180 | 270) {
            return Err(format!(
                "rotate must be 0, 90, 180 or 270, got {}",
                self.rotate
            ));
        }
        if let Some(c) = self.crop {
            if c.width == 0 || c.height == 0 {
                return Err("crop area must be at least 1×1".into());
            }
        }
        Ok(())
    }

    /// Dimensions after this transform on a `w`×`h` image, without decoding it.
    /// `Err` means the crop would not intersect the image at all.
    pub fn dims(&self, w: u32, h: u32) -> Result<(u32, u32), FileError> {
        let (w, h) = match self.crop {
            Some(c) => crop_rect(c, w, h)?,
            None => (w, h),
        };
        Ok(if matches!(self.rotate, 90 | 270) {
            (h, w)
        } else {
            (w, h)
        })
    }

    /// Applies the crop, then rotation and flips, so a crop matches what the
    /// user draws on the unrotated image.
    pub fn apply(&self, img: &DynamicImage) -> Result<DynamicImage, FileError> {
        let mut out = match self.crop {
            Some(c) => {
                let (w, h) = crop_rect(c, img.width(), img.height())?;
                img.crop_imm(c.x, c.y, w, h)
            }
            None => img.clone(),
        };
        out = match self.rotate {
            90 => out.rotate90(),
            180 => out.rotate180(),
            270 => out.rotate270(),
            _ => out,
        };
        if self.flip_horizontal {
            out = out.fliph();
        }
        if self.flip_vertical {
            out = out.flipv();
        }
        Ok(out)
    }
}

/// The part of `c` inside a `w`×`h` image, as (width, height).
fn crop_rect(c: CropSpec, w: u32, h: u32) -> Result<(u32, u32), FileError> {
    if c.x >= w || c.y >= h {
        return Err(FileError::new(
            FileErrorCode::CropOutside,
            format!("crop {c:?} does not intersect a {w}×{h} image"),
        ));
    }
    Ok((c.width.min(w - c.x).max(1), c.height.min(h - c.y).max(1)))
}
