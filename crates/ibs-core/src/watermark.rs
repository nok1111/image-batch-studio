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
pub(crate) static FONT_BYTES: &[u8] = include_bytes!("../fonts/DejaVuSans.ttf");

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
        // Bound the tile: a rotated tile briefly allocates a ~2x-area canvas
        // plus its rotated copy, per worker, on top of the decoded image.
        let size = (spec.scale * min_dim).clamp(1.0, MAX_TILE_EDGE);
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
            // The expanded canvas can be larger than the rotated tile (it
            // must also fit the unrotated one); crop so edge margins land
            // exactly where `position`/`margin` say.
            tile = crop_to_content(tile);
        }
        tile
    }
}

fn render_text(font: &FontArc, text: &str, height: f32, color: [u8; 4]) -> RgbaImage {
    let mut scale = font
        .pt_to_px_scale(height)
        .unwrap_or_else(|| PxScale::from(height));
    let (mut w, mut h) = text_size(scale, font, text);
    // MAX_TILE_EDGE caps the height via `size` but not the width a long
    // string accumulates — shrink the font until the box fits, or the
    // canvas below can reach gigabytes.
    if w > MAX_TILE_EDGE as u32 {
        let shrink = MAX_TILE_EDGE / w as f32;
        scale = PxScale::from(scale.x * shrink);
        (w, h) = text_size(scale, font, text);
    }
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

/// Longest tile side in pixels. Rotating briefly doubles the allocation, so
/// this also bounds per-worker memory on huge inputs.
const MAX_TILE_EDGE: f32 = 4096.0;

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

/// Tile placement as an origin, step and column/row counts — computed lazily
/// so a huge base with a tiny tile never materializes millions of positions.
fn placement_grid(
    spec: &WatermarkSpec,
    tw: i64,
    th: i64,
    bw: i64,
    bh: i64,
) -> (i64, i64, i64, i64, u32, u32) {
    let min_dim = bw.min(bh) as f32;
    if spec.tiled {
        let step_x = (tw as f32 * (1.0 + spec.spacing)).max(1.0) as i64;
        let step_y = (th as f32 * (1.0 + spec.spacing)).max(1.0) as i64;
        // Saturating add: an extreme spacing saturates the cast to i64::MAX,
        // and wrapping the column count would silently drop every tile.
        let cols = (bw.saturating_add(step_x.saturating_sub(1)) / step_x).max(0) as u32;
        let rows = (bh.saturating_add(step_y.saturating_sub(1)) / step_y).max(0) as u32;
        return (0, 0, step_x, step_y, cols, rows);
    }
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
    (x, y, 0, 0, 1, 1)
}

/// Stamps the watermark on `img` (in place). 16-bit sources keep their
/// precision: the tile is widened to 16 bit and composited in RGBA16 space
/// instead of reducing the whole image to 8 bit.
pub fn apply(spec: &WatermarkSpec, prepared: &Prepared, img: &mut DynamicImage) {
    let is_16bit = matches!(
        img,
        DynamicImage::ImageLuma16(_)
            | DynamicImage::ImageLumaA16(_)
            | DynamicImage::ImageRgb16(_)
            | DynamicImage::ImageRgba16(_)
    );
    if is_16bit {
        let mut base = img.to_rgba16();
        let min_dim = base.width().min(base.height()) as f32;
        let tile8 = prepared.tile(spec, min_dim);
        let tile = image::ImageBuffer::<image::Rgba<u16>, Vec<u16>>::from_fn(
            tile8.width(),
            tile8.height(),
            |x, y| {
                let p = tile8.get_pixel(x, y);
                image::Rgba([
                    p[0] as u16 * 257,
                    p[1] as u16 * 257,
                    p[2] as u16 * 257,
                    p[3] as u16 * 257,
                ])
            },
        );
        let (gx, gy, sx, sy, cols, rows) = placement_grid(
            spec,
            tile.width() as i64,
            tile.height() as i64,
            base.width() as i64,
            base.height() as i64,
        );
        for row in 0..rows {
            for col in 0..cols {
                overlay16(&mut base, &tile, gx + col as i64 * sx, gy + row as i64 * sy);
            }
        }
        *img = DynamicImage::ImageRgba16(base);
        return;
    }
    let mut base = img.to_rgba8();
    let min_dim = base.width().min(base.height()) as f32;
    let tile = prepared.tile(spec, min_dim);
    let (gx, gy, sx, sy, cols, rows) = placement_grid(
        spec,
        tile.width() as i64,
        tile.height() as i64,
        base.width() as i64,
        base.height() as i64,
    );
    for row in 0..rows {
        for col in 0..cols {
            image::imageops::overlay(&mut base, &tile, gx + col as i64 * sx, gy + row as i64 * sy);
        }
    }
    *img = DynamicImage::ImageRgba8(base);
}

/// Straight-alpha source-over composite; `imageops::overlay` only blends
/// 8-bit and float pixels, so the 16-bit path does it by hand.
fn overlay16(
    base: &mut image::ImageBuffer<image::Rgba<u16>, Vec<u16>>,
    tile: &image::ImageBuffer<image::Rgba<u16>, Vec<u16>>,
    x0: i64,
    y0: i64,
) {
    let (bw, bh) = (base.width() as i64, base.height() as i64);
    for ty in 0..tile.height() as i64 {
        let by = y0 + ty;
        if by < 0 || by >= bh {
            continue;
        }
        for tx in 0..tile.width() as i64 {
            let bx = x0 + tx;
            if bx < 0 || bx >= bw {
                continue;
            }
            let s = *tile.get_pixel(tx as u32, ty as u32);
            let sa = s[3] as u32;
            if sa == 0 {
                continue;
            }
            let d = base.get_pixel_mut(bx as u32, by as u32);
            // Proper source-over for straight alpha: out_a = sa + da(1-sa),
            // out_c = (s*sa + d*da(1-sa)) / out_a — the destination's own
            // alpha must attenuate its contribution.
            let sa = sa as u64;
            let da = d[3] as u64;
            let out_a = sa * 65535 + da * (65535 - sa);
            for c in 0..3 {
                let num = s[c] as u64 * sa * 65535 + d[c] as u64 * da * (65535 - sa);
                d[c] = ((num + out_a / 2) / out_a) as u16;
            }
            d[3] = ((out_a + 32767) / 65535) as u16;
        }
    }
}
