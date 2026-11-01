//! Text and logo watermarks rendered over each processed image.

use crate::{codec, FileError, FileErrorCode};
use ab_glyph::{Font, FontArc, PxScale, ScaleFont};
use image::{imageops::FilterType, DynamicImage, Rgba, RgbaImage};
use imageproc::drawing::{draw_text_mut, text_size};
use imageproc::geometric_transformations::{rotate_about_center, Interpolation};
use serde::{Deserialize, Serialize};
use std::path::Path;
use ts_rs::TS;

/// Bundled fallback font (DejaVu Sans, Bitstream Vera license — see
/// `fonts/DejaVuSans-LICENSE.txt`).
static FONT_BYTES: &[u8] = include_bytes!("../fonts/DejaVuSans.ttf");

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum WatermarkKind {
    Text,
    /// A logo read from `WatermarkSpec::image_path`.
    Image,
}

/// Anchor grid; `Custom` places the watermark centre at `custom` fractions of
/// the image size.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum WatermarkPosition {
    TopLeft,
    TopCenter,
    TopRight,
    MiddleLeft,
    Center,
    MiddleRight,
    BottomLeft,
    BottomCenter,
    BottomRight,
    Custom,
}

/// Note: `Eq` is asserted manually (f32 fields) so `Job` can derive `Eq`;
/// `validate` rejects non-finite values, so NaN never reaches the engine.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct WatermarkSpec {
    pub kind: WatermarkKind,
    /// kind `text`: the rendered string (empty = invalid).
    pub text: String,
    /// Text colour RGBA, 0–255 per channel.
    #[ts(type = "[number, number, number, number]")]
    pub color: [u8; 4],
    /// kind `image`: path to the logo file, decoded once per job.
    pub image_path: String,
    pub position: WatermarkPosition,
    /// `position` custom: watermark centre as a 0–1 fraction of the image.
    #[ts(type = "[number, number]")]
    pub custom: [f32; 2],
    /// Extra alpha multiplier, 0–1.
    pub opacity: f32,
    /// Clockwise degrees applied to the tile.
    pub rotation: f32,
    /// Tile size as a fraction of the image's smaller side (0 < s ≤ 1): text
    /// em height for `text`, longest side for `image`.
    pub scale: f32,
    /// Edge margin as a fraction of the smaller side (0–0.5).
    pub margin: f32,
    /// Repeat across the whole image; ignores `position`/`margin`/`custom`.
    pub tiled: bool,
    /// Gap between tiles as a fraction of tile size (≥ 0).
    pub spacing: f32,
}

impl Eq for WatermarkSpec {}

impl WatermarkSpec {
    pub fn validate(&self) -> Result<(), String> {
        let bad = |reason: &str| format!("watermark: {reason}");
        match self.kind {
            WatermarkKind::Text => {
                if self.text.trim().is_empty() {
                    return Err(bad("text is empty"));
                }
                if self.text.chars().count() > 512 {
                    return Err(bad("text longer than 512 characters"));
                }
            }
            WatermarkKind::Image => {
                if self.image_path.is_empty() {
                    return Err(bad("image path is empty"));
                }
            }
        }
        if !(self.scale > 0.0 && self.scale <= 1.0) {
            return Err(bad("scale must be in (0, 1]"));
        }
        if !(0.0..=1.0).contains(&self.opacity) {
            return Err(bad("opacity must be in [0, 1]"));
        }
        if !self.rotation.is_finite() {
            return Err(bad("rotation must be finite"));
        }
        if !(0.0..=0.5).contains(&self.margin) {
            return Err(bad("margin must be in [0, 0.5]"));
        }
        if !(self.spacing >= 0.0 && self.spacing.is_finite()) {
            return Err(bad("spacing must be a non-negative number"));
        }
        if self.custom.iter().any(|v| !v.is_finite()) {
            return Err(bad("custom position must be finite"));
        }
        Ok(())
    }
}

/// Watermark material decoded once per job.
pub enum Prepared {
    Text(FontArc),
    Logo(RgbaImage),
}

impl Prepared {
    pub fn load(spec: &WatermarkSpec) -> Result<Self, FileError> {
        match spec.kind {
            WatermarkKind::Text => FontArc::try_from_slice(FONT_BYTES)
                .map(Prepared::Text)
                .map_err(|_| {
                    FileError::new(FileErrorCode::Internal, "bundled font failed to load")
                }),
            WatermarkKind::Image => codec::decode(Path::new(&spec.image_path))
                .map(|d| Prepared::Logo(d.image.to_rgba8()))
                .map_err(|e| FileError::new(e.code, format!("watermark image: {}", e.message))),
        }
    }

    /// The watermark tile at this base image's scale, already rotated and with
    /// `opacity` applied.
    fn tile(&self, spec: &WatermarkSpec, min_dim: f32) -> RgbaImage {
        let size = (spec.scale * min_dim).max(1.0);
        let mut tile = match self {
            Prepared::Text(font) => render_text(font, &spec.text, size, spec.color),
            Prepared::Logo(rgba) => {
                // Fit the longest side to `size` — a pathological aspect
                // ratio (e.g. a 1x4000 logo) otherwise explodes the other
                // dimension and can exhaust memory across the worker pool.
                let longest = rgba.width().max(rgba.height()) as f32;
                let k = size / longest;
                let w = (rgba.width() as f32 * k).round().max(1.0) as u32;
                let h = (rgba.height() as f32 * k).round().max(1.0) as u32;
                image::imageops::resize(rgba, w, h, FilterType::Triangle)
            }
        };
        for p in tile.pixels_mut() {
            p[3] = (p[3] as f32 * spec.opacity.clamp(0.0, 1.0)).round() as u8;
        }
        let rad = spec.rotation.rem_euclid(360.0).to_radians();
        if rad > f32::EPSILON {
            // rotate_about_center keeps the image size, so expand the canvas
            // to the rotated bounding box first or the corners get clipped.
            let (sin, cos) = (rad.sin().abs(), rad.cos().abs());
            let tw = tile.width() as f32;
            let th = tile.height() as f32;
            let ew = ((tw * cos + th * sin).ceil() as u32).max(tile.width());
            let eh = ((tw * sin + th * cos).ceil() as u32).max(tile.height());
            let mut expanded = RgbaImage::from_pixel(ew, eh, Rgba([0, 0, 0, 0]));
            image::imageops::overlay(
                &mut expanded,
                &tile,
                ((ew - tile.width()) / 2) as i64,
                ((eh - tile.height()) / 2) as i64,
            );
            tile = rotate_about_center(&expanded, rad, Interpolation::Bilinear, Rgba([0, 0, 0, 0]));
        }
        tile
    }
}

fn render_text(font: &FontArc, text: &str, height: f32, color: [u8; 4]) -> RgbaImage {
    let scale = font
        .pt_to_px_scale(height)
        .unwrap_or_else(|| PxScale::from(height));
    let (w, h) = text_size(scale, font, text);
    // Oversize canvas: glyph ink can extend past the logical box.
    let mut canvas = RgbaImage::from_pixel(w + h, h * 2, Rgba([0, 0, 0, 0]));
    let scaled = font.as_scaled(scale);
    draw_text_mut(
        &mut canvas,
        Rgba(color),
        0,
        (h as f32 - scaled.ascent()).max(0.0) as i32,
        scale,
        font,
        text,
    );
    crop_to_content(canvas)
}

/// Tight alpha bounding box; empty input collapses to 1×1 transparent.
fn crop_to_content(img: RgbaImage) -> RgbaImage {
    let (w, h) = (img.width(), img.height());
    let mut bbox = [w, h, 0u32, 0u32];
    for y in 0..h {
        for x in 0..w {
            if img.get_pixel(x, y)[3] != 0 {
                bbox[0] = bbox[0].min(x);
                bbox[1] = bbox[1].min(y);
                bbox[2] = bbox[2].max(x);
                bbox[3] = bbox[3].max(y);
            }
        }
    }
    if bbox[2] < bbox[0] {
        return RgbaImage::from_pixel(1, 1, Rgba([0, 0, 0, 0]));
    }
    image::imageops::crop_imm(
        &img,
        bbox[0],
        bbox[1],
        bbox[2] - bbox[0] + 1,
        bbox[3] - bbox[1] + 1,
    )
    .to_image()
}

/// Stamps the watermark on `img` (in place).
pub fn apply(spec: &WatermarkSpec, prepared: &Prepared, img: &mut DynamicImage) {
    let mut base = img.to_rgba8();
    let min_dim = base.width().min(base.height()) as f32;
    let tile = prepared.tile(spec, min_dim);
    let (tw, th) = (tile.width() as i64, tile.height() as i64);
    let (bw, bh) = (base.width() as i64, base.height() as i64);
    if spec.tiled {
        let step_x = (tw as f32 * (1.0 + spec.spacing)).max(1.0) as i64;
        let step_y = (th as f32 * (1.0 + spec.spacing)).max(1.0) as i64;
        let mut y = 0;
        while y < bh {
            let mut x = 0;
            while x < bw {
                image::imageops::overlay(&mut base, &tile, x, y);
                x += step_x;
            }
            y += step_y;
        }
    } else {
        let m = (spec.margin * min_dim) as i64;
        use WatermarkPosition::*;
        let (x, y) = match spec.position {
            TopLeft => (m, m),
            TopCenter => ((bw - tw) / 2, m),
            TopRight => (bw - tw - m, m),
            MiddleLeft => (m, (bh - th) / 2),
            Center => ((bw - tw) / 2, (bh - th) / 2),
            MiddleRight => (bw - tw - m, (bh - th) / 2),
            BottomLeft => (m, bh - th - m),
            BottomCenter => ((bw - tw) / 2, bh - th - m),
            BottomRight => (bw - tw - m, bh - th - m),
            Custom => (
                (spec.custom[0] * bw as f32) as i64 - tw / 2,
                (spec.custom[1] * bh as f32) as i64 - th / 2,
            ),
        };
        image::imageops::overlay(&mut base, &tile, x, y);
    }
    *img = DynamicImage::ImageRgba8(base);
}
