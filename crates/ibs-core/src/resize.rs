//! Resize modes with exact, documented output dimensions.

use crate::codec::MAX_DIMENSION;
use image::{imageops::FilterType, DynamicImage};
use serde::{Deserialize, Serialize};
use ts_rs::TS;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum ResizeMode {
    /// Exactly `width`×`height` (may distort).
    Exact,
    /// `percent` of the original size.
    Percent,
    /// `width`, height keeps the aspect ratio.
    Width,
    /// `height`, width keeps the aspect ratio.
    Height,
    /// Largest size that fits inside `width`×`height`.
    Fit,
    /// Covers `width`×`height`, then center-crops the overflow.
    Fill,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ResizeSpec {
    pub mode: ResizeMode,
    pub width: u32,
    pub height: u32,
    pub percent: u32,
    /// When false, images are never made larger than the original.
    pub allow_upscale: bool,
}

/// Scaled size plus the final crop (equal to the scaled size except for `Fill`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Geometry {
    pub scaled: (u32, u32),
    pub output: (u32, u32),
}

fn scale(v: u32, f: f64) -> u32 {
    ((f64::from(v) * f).round() as u32).clamp(1, MAX_DIMENSION)
}

impl ResizeSpec {
    pub fn validate(&self) -> Result<(), String> {
        let dim_ok = |v: u32| (1..=MAX_DIMENSION).contains(&v);
        let ok = match self.mode {
            ResizeMode::Percent => (1..=1000).contains(&self.percent),
            ResizeMode::Width => dim_ok(self.width),
            ResizeMode::Height => dim_ok(self.height),
            ResizeMode::Exact | ResizeMode::Fit | ResizeMode::Fill => {
                dim_ok(self.width) && dim_ok(self.height)
            }
        };
        if ok {
            Ok(())
        } else {
            Err(format!("invalid resize settings: {self:?}"))
        }
    }

    pub fn geometry(&self, w: u32, h: u32) -> Geometry {
        let (bw, bh) = (f64::from(self.width), f64::from(self.height));
        let (sw, sh) = (f64::from(w), f64::from(h));
        let limit = |f: f64| if self.allow_upscale { f } else { f.min(1.0) };
        let uniform = |f: f64| {
            let f = limit(f);
            let s = (scale(w, f), scale(h, f));
            Geometry {
                scaled: s,
                output: s,
            }
        };
        match self.mode {
            ResizeMode::Exact => {
                let s = if !self.allow_upscale && (self.width > w || self.height > h) {
                    (w, h)
                } else {
                    (self.width, self.height)
                };
                Geometry {
                    scaled: s,
                    output: s,
                }
            }
            ResizeMode::Percent => uniform(f64::from(self.percent) / 100.0),
            ResizeMode::Width => uniform(bw / sw),
            ResizeMode::Height => uniform(bh / sh),
            ResizeMode::Fit => uniform((bw / sw).min(bh / sh)),
            ResizeMode::Fill => {
                let f = limit((bw / sw).max(bh / sh));
                let s = (scale(w, f), scale(h, f));
                Geometry {
                    scaled: s,
                    output: (s.0.min(self.width), s.1.min(self.height)),
                }
            }
        }
    }

    pub fn apply(&self, img: &DynamicImage) -> DynamicImage {
        let g = self.geometry(img.width(), img.height());
        let mut out = if g.scaled == (img.width(), img.height()) {
            img.clone()
        } else {
            img.resize_exact(g.scaled.0, g.scaled.1, FilterType::Lanczos3)
        };
        if g.output != g.scaled {
            let x = (g.scaled.0 - g.output.0) / 2;
            let y = (g.scaled.1 - g.output.1) / 2;
            out = out.crop_imm(x, y, g.output.0, g.output.1);
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn spec(mode: ResizeMode, width: u32, height: u32, up: bool) -> ResizeSpec {
        ResizeSpec {
            mode,
            width,
            height,
            percent: 50,
            allow_upscale: up,
        }
    }

    #[test]
    fn every_mode_produces_the_expected_size() {
        use ResizeMode::*;
        let cases = [
            (spec(Exact, 300, 300, true), (1000, 500), (300, 300)),
            (spec(Percent, 0, 0, true), (1000, 501), (500, 251)),
            (spec(Width, 400, 0, true), (1000, 500), (400, 200)),
            (spec(Height, 0, 100, true), (1000, 500), (200, 100)),
            (spec(Fit, 400, 400, true), (1000, 500), (400, 200)),
            (spec(Fit, 400, 400, true), (500, 1000), (200, 400)),
            (spec(Fill, 400, 400, true), (1000, 500), (400, 400)),
            (spec(Fill, 1080, 1350, true), (4000, 3000), (1080, 1350)),
            // No upscaling: small images keep their size (Fill crops instead).
            (spec(Fit, 4000, 4000, false), (1000, 500), (1000, 500)),
            (spec(Width, 4000, 0, false), (1000, 500), (1000, 500)),
            (spec(Exact, 2000, 100, false), (1000, 500), (1000, 500)),
            (spec(Fill, 800, 800, false), (1000, 500), (800, 500)),
            (spec(Fit, 4000, 4000, true), (1000, 500), (4000, 2000)),
            (spec(Width, 1, 0, true), (1000, 10), (1, 1)),
        ];
        for (s, (w, h), expected) in cases {
            assert_eq!(s.geometry(w, h).output, expected, "{s:?} on {w}×{h}");
            let img = DynamicImage::new_rgb8(w, h);
            let out = s.apply(&img);
            assert_eq!((out.width(), out.height()), expected, "apply {s:?}");
        }
    }

    #[test]
    fn rejects_out_of_range_settings() {
        assert!(spec(ResizeMode::Fit, 0, 10, true).validate().is_err());
        assert!(spec(ResizeMode::Width, 10, 0, true).validate().is_ok());
        assert!(ResizeSpec {
            percent: 0,
            ..spec(ResizeMode::Percent, 0, 0, true)
        }
        .validate()
        .is_err());
    }
}
