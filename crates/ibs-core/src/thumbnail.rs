//! Social-media thumbnails: fixed canvas + background + ordered layers.

use crate::watermark::FONT_BYTES;
use crate::{codec, FileError, FileErrorCode};
use ab_glyph::{FontArc, PxScale};
use image::{imageops, DynamicImage, GrayImage, Luma, Rgba, RgbaImage};
use imageproc::distance_transform::Norm;
use imageproc::drawing::{draw_text_mut, text_size};
use imageproc::geometric_transformations::{rotate_about_center, Interpolation};
use imageproc::morphology::dilate;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use ts_rs::TS;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ThumbnailSpec {
    /// Canvas dimensions.
    pub width: u32,
    pub height: u32,
    pub bg: ThumbBg,
    /// Color grading applied to the rendered background.
    pub bg_adjust: ThumbAdjust,
    /// Edge darkening over the whole canvas, 0-100.
    pub vignette: u32,
    /// Rendered bottom-to-top; the source image is one layer among them.
    pub layers: Vec<ThumbLayer>,
}

/// One drawable layer.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(tag = "kind", rename_all = "camelCase")]
#[ts(export)]
pub enum ThumbLayer {
    /// The batch input image.
    Source(ThumbImage),
    /// An external image file, the same on every output.
    Image {
        path: String,
        fx: ThumbImage,
    },
    Text(ThumbText),
}

/// Placement + effects for one image layer.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ThumbImage {
    pub fit: ThumbFit,
    /// Percent per axis; 100 = the plain fit.
    pub zoom_x: u32,
    pub zoom_y: u32,
    /// Offset in canvas px, from the centered position.
    pub x: i32,
    pub y: i32,
    /// Corner radius, px.
    pub radius: u32,
    pub border_w: u32,
    pub border_color: [u8; 4],
    pub shadow: bool,
    /// Silhouette outline (the YouTube look), px.
    pub outline_w: u32,
    pub outline_color: [u8; 4],
    /// 0-100.
    pub opacity: u32,
    pub flip_h: bool,
    pub flip_v: bool,
    pub adjust: ThumbAdjust,
}

/// Color grading (-100..=100 each; hue is degrees).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ThumbAdjust {
    pub brightness: i32,
    pub contrast: i32,
    /// Degrees 0-360.
    pub hue: i32,
    pub saturation: i32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum ThumbFit {
    /// Fill the canvas cropping what overflows.
    Cover,
    /// Fit inside, showing the background around.
    Contain,
    /// Ignore aspect: stretch to the canvas.
    Stretch,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(tag = "kind", rename_all = "camelCase")]
#[ts(export)]
pub enum ThumbBg {
    Transparent,
    Color {
        color: [u8; 4],
    },
    /// The source image itself, cover-fit and blurred.
    Blur {
        radius: u32,
    },
    Gradient {
        from: [u8; 4],
        to: [u8; 4],
        angle: u32,
    },
    /// An external image; `dim` 0-100 darkens it for contrast.
    Image {
        path: String,
        blur: u32,
        dim: u32,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ThumbText {
    pub text: String,
    /// Center of the text block, canvas px.
    pub x: i32,
    pub y: i32,
    /// Cap height, canvas px.
    pub size: u32,
    pub color: [u8; 4],
    pub stroke_w: u32,
    pub stroke_color: [u8; 4],
    /// Font key: "dejavu" or a system font name.
    pub font: String,
    /// Rotation in degrees.
    pub rotate: i32,
    pub uppercase: bool,
    pub shadow_dx: i32,
    pub shadow_dy: i32,
    pub shadow_blur: u32,
    pub shadow_color: [u8; 4],
}

impl ThumbnailSpec {
    pub fn validate(&self) -> Result<(), String> {
        if self.width == 0 || self.height == 0 || self.width > 16384 || self.height > 16384 {
            return Err("canvas size must be between 1 and 16384 px".into());
        }
        for (i, l) in self.layers.iter().enumerate() {
            match l {
                ThumbLayer::Source(_) | ThumbLayer::Image { .. } => {
                    let fx = match l {
                        ThumbLayer::Source(fx) => fx,
                        ThumbLayer::Image { fx, .. } => fx,
                        _ => unreachable!(),
                    };
                    if fx.zoom_x == 0 || fx.zoom_y == 0 {
                        return Err("layer zoom must be at least 1%".into());
                    }
                }
                ThumbLayer::Text(t) => {
                    if t.text.trim().is_empty() || t.size == 0 || t.size > 4096 {
                        return Err(format!("text layer {i} needs content and size 1..4096"));
                    }
                }
            }
        }
        Ok(())
    }
}

// ---------------------------------------------------------------- helpers

fn rounded_mask(w: u32, h: u32, r: u32) -> RgbaImage {
    let mut m = RgbaImage::from_pixel(w, h, Rgba([0, 0, 0, 0]));
    let r = (r.min(w / 2).min(h / 2)) as i64;
    for y in 0..h as i64 {
        for x in 0..w as i64 {
            let inside = if r <= 0 {
                true
            } else {
                let cx = if x < r {
                    r
                } else if x >= w as i64 - r {
                    w as i64 - r - 1
                } else {
                    x
                };
                let cy = if y < r {
                    r
                } else if y >= h as i64 - r {
                    h as i64 - r - 1
                } else {
                    y
                };
                (x - cx) * (x - cx) + (y - cy) * (y - cy) <= r * r
            };
            if inside {
                m.put_pixel(x as u32, y as u32, Rgba([255, 255, 255, 255]));
            }
        }
    }
    imageops::blur(&m, 0.7)
}

fn gradient(w: u32, h: u32, from: [u8; 4], to: [u8; 4], angle: u32) -> RgbaImage {
    let rad = (angle as f32).to_radians();
    let (dx, dy) = (rad.cos(), rad.sin());
    let (fw, fh) = (w as f32, h as f32);
    let corners = [(0.0, 0.0), (fw, 0.0), (0.0, fh), (fw, fh)];
    let mut tmin = f32::MAX;
    let mut tmax = f32::MIN;
    for (cx, cy) in corners {
        let t = cx * dx + cy * dy;
        tmin = tmin.min(t);
        tmax = tmax.max(t);
    }
    let span = (tmax - tmin).max(f32::EPSILON);
    RgbaImage::from_fn(w, h, |x, y| {
        let t = ((x as f32 * dx + y as f32 * dy) - tmin) / span;
        Rgba([
            (from[0] as f32 + (to[0] as f32 - from[0] as f32) * t).round() as u8,
            (from[1] as f32 + (to[1] as f32 - from[1] as f32) * t).round() as u8,
            (from[2] as f32 + (to[2] as f32 - from[2] as f32) * t).round() as u8,
            (from[3] as f32 + (to[3] as f32 - from[3] as f32) * t).round() as u8,
        ])
    })
}

fn cover_scaled(img: &RgbaImage, cw: u32, ch: u32) -> RgbaImage {
    let (iw, ih) = img.dimensions();
    let k = (cw as f32 / iw as f32).max(ch as f32 / ih as f32);
    let (bw, bh) = (
        (iw as f32 * k).round().max(1.0) as u32,
        (ih as f32 * k).round().max(1.0) as u32,
    );
    let scaled = imageops::resize(img, bw, bh, imageops::FilterType::Triangle);
    let x = ((bw as i64 - cw as i64) / 2).max(0) as u32;
    let y = ((bh as i64 - ch as i64) / 2).max(0) as u32;
    imageops::crop_imm(&scaled, x, y, cw.min(bw), ch.min(bh)).to_image()
}

/// (x, y, w, h) of a placed image layer on the canvas.
fn placement(dims: (u32, u32), fx: &ThumbImage, cw: u32, ch: u32) -> (i64, i64, u32, u32) {
    let (iw, ih) = dims;
    let (zx, zy) = (
        fx.zoom_x.max(1) as f32 / 100.0,
        fx.zoom_y.max(1) as f32 / 100.0,
    );
    let (nw, nh) = match fx.fit {
        ThumbFit::Stretch => (cw as f32 * zx, ch as f32 * zy),
        ThumbFit::Cover => {
            let k = (cw as f32 / iw as f32).max(ch as f32 / ih as f32);
            (iw as f32 * k * zx, ih as f32 * k * zy)
        }
        ThumbFit::Contain => {
            let k = (cw as f32 / iw as f32).min(ch as f32 / ih as f32);
            (iw as f32 * k * zx, ih as f32 * k * zy)
        }
    };
    let nw = nw.max(1.0).round() as u32;
    let nh = nh.max(1.0).round() as u32;
    (
        (cw as i64 - nw as i64) / 2 + fx.x as i64,
        (ch as i64 - nh as i64) / 2 + fx.y as i64,
        nw,
        nh,
    )
}

fn grade(img: RgbaImage, a: &ThumbAdjust) -> RgbaImage {
    let mut out = img;
    if a.hue.rem_euclid(360) != 0 {
        out = imageops::huerotate(&out, a.hue);
    }
    if a.saturation != 0 {
        let f = 1.0 + a.saturation as f32 / 100.0;
        for p in out.pixels_mut() {
            let luma = 0.2126 * p[0] as f32 + 0.7152 * p[1] as f32 + 0.0722 * p[2] as f32;
            for c in 0..3 {
                p[c] = (luma + (p[c] as f32 - luma) * f).clamp(0.0, 255.0) as u8;
            }
        }
    }
    if a.brightness != 0 {
        out = imageops::brighten(&out, (a.brightness as f32 * 2.55).round() as i32);
    }
    if a.contrast != 0 {
        out = imageops::contrast(&out, a.contrast as f32);
    }
    out
}

fn alpha_mask(img: &RgbaImage) -> GrayImage {
    GrayImage::from_fn(img.width(), img.height(), |x, y| {
        Luma([img.get_pixel(x, y)[3]])
    })
}

fn alpha_bbox(img: &RgbaImage) -> Option<(u32, u32, u32, u32)> {
    let (w, h) = img.dimensions();
    let (mut x0, mut y0, mut x1, mut y1) = (w, h, 0u32, 0u32);
    for (x, y, p) in img.enumerate_pixels() {
        if p[3] > 0 {
            x0 = x0.min(x);
            y0 = y0.min(y);
            x1 = x1.max(x + 1);
            y1 = y1.max(y + 1);
        }
    }
    (x1 > x0 && y1 > y0).then_some((x0, y0, x1, y1))
}

fn crop_to_content(img: RgbaImage) -> RgbaImage {
    match alpha_bbox(&img) {
        Some((x0, y0, x1, y1)) => imageops::crop_imm(&img, x0, y0, x1 - x0, y1 - y0).to_image(),
        None => img,
    }
}

/// Candidate file names per font key (covers case/naming variants per OS).
fn font_files(name: &str) -> Option<&'static [&'static str]> {
    Some(match name {
        "impact" => &["impact.ttf", "Impact.ttf"],
        "arialblack" => &["ariblk.ttf", "ARIBLK.TTF"],
        "cooperblack" => &["COOPBL.TTF", "coopbl.ttf"],
        "franklinheavy" => &["FRAHV.TTF", "frahv.ttf"],
        "gillsansultra" => &["GILSANUB.TTF", "gilsanub.ttf"],
        "stencil" => &["STENCIL.TTF", "stencil.ttf"],
        "showcard" => &["SHOWG.TTF", "showg.ttf"],
        "broadway" => &["BROADW.TTF", "broadw.ttf"],
        "jokerman" => &["JOKERMAN.TTF", "jokerman.ttf"],
        "ravie" => &["RAVIE.TTF", "ravie.ttf"],
        "playbill" => &["PLAYBILL.TTF", "playbill.ttf"],
        "snapitc" => &["SNAP____.TTF", "snap____.ttf"],
        "vinerhand" => &["VINERITC.TTF", "vineritc.ttf"],
        "curlz" => &["CURLZ___.TTF", "curlz___.ttf"],
        "mistral" => &["MISTRAL.TTF", "mistral.ttf"],
        "corsiva" => &["MTCORSVA.TTF", "mtcorsva.ttf"],
        "oldenglish" => &["OLDENGL.TTF", "oldengl.ttf"],
        "widelatin" => &["LATINWD.TTF", "latinwd.ttf"],
        "algerian" => &["ALGER.TTF", "alger.ttf"],
        "erasbold" => &["ERASBD.TTF", "erasbd.ttf", "ERASDEMI.TTF"],
        "magneto" => &["MAGNETOB.TTF", "magnetob.ttf"],
        "britannic" => &["BRITANIC.TTF", "britanic.ttf"],
        "castellar" => &["CASTELAR.TTF", "castelar.ttf"],
        "colonna" => &["COLONNA.TTF", "colonna.ttf"],
        "harrington" => &["HARNGTON.TTF", "harngton.ttf"],
        "elephant" => &["ELEPHNT.TTF", "elephnt.ttf"],
        "matura" => &["MATURASC.TTF", "maturasc.ttf"],
        "segoeblack" => &["seguibl.ttf", "SEGUIBL.TTF"],
        "rockwell" => &["ROCK.TTF", "rock.ttf"],
        "rockwellxb" => &["ROCKEB.TTF", "rockeb.ttf"],
        "pristina" => &["PRISTINA.TTF", "pristina.ttf"],
        "rage" => &["RAGE.TTF", "rage.ttf"],
        "tempus" => &["TEMPUSITC.TTF", "tempusitc.ttf"],
        "segoeprint" => &["segoepr.ttf", "segoeprb.ttf"],
        "segoescript" => &["segoesc.ttf", "segoescb.ttf"],
        "inkfree" => &["Inkfree.ttf", "inkfree.ttf"],
        "bradley" => &["bradhitc.ttf", "BRADHITC.TTF"],
        "freestyle" => &["FREESCPT.TTF", "freescpt.ttf"],
        "kristen" => &["KRISTENITC.TTF", "kristenitc.ttf"],
        "mvboli" => &["mvboli.ttf", "MVBOLI.TTF"],
        "blackadder" => &["ITCBLKAD.TTF", "itcblkad.ttf"],
        "edwardian" => &["ITCEDSCR.TTF", "itcedscr.ttf"],
        "juice" => &["JUICE___.TTF", "juice___.ttf"],
        "arial" => &["arial.ttf", "Arial.ttf"],
        "arialbold" => &["arialbd.ttf", "ARIALBD.TTF"],
        "verdana" => &["verdana.ttf", "Verdana.ttf"],
        "verdanabold" => &["verdanab.ttf", "VERDANAB.TTF"],
        "tahoma" => &["tahoma.ttf", "Tahoma.ttf"],
        "tahomabold" => &["tahomabd.ttf", "TAHOMABD.TTF"],
        "calibri" => &["calibri.ttf", "calibrib.ttf"],
        "calibribold" => &["calibrib.ttf", "calibri.ttf"],
        "candara" => &["Candara.ttf", "Candarab.ttf", "candara.ttf"],
        "candarabold" => &["Candarab.ttf", "Candara.ttf"],
        "corbel" => &["corbel.ttf", "corbelb.ttf"],
        "corbelbold" => &["corbelb.ttf", "corbel.ttf"],
        "constantia" => &["constan.ttf", "constanb.ttf"],
        "cambria" => &["cambria.ttc", "cambriab.ttf"],
        "georgia" => &["georgia.ttf", "georgiab.ttf"],
        "georgiabold" => &["georgiab.ttf", "georgia.ttf"],
        "centurygothic" => &["GOTHIC.TTF", "GOTHICB.TTF", "gothic.ttf"],
        "centurygothicbold" => &["GOTHICB.TTF", "GOTHIC.TTF"],
        "bookman" => &["BOOKOS.TTF", "BOOKOSB.TTF", "bookos.ttf"],
        "garamond" => &["GARA.TTF", "GARABD.TTF", "gara.ttf"],
        "palatino" => &["pala.ttf", "palab.ttf"],
        "trebuchet" => &["trebuc.ttf", "trebucbd.ttf"],
        "trebuchetbold" => &["trebucbd.ttf", "trebuc.ttf"],
        "courier" => &["cour.ttf", "courbd.ttf"],
        "courierbold" => &["courbd.ttf", "cour.ttf"],
        "consolas" => &["consola.ttf", "consolab.ttf"],
        "lucidaconsole" => &["lucon.ttf", "LUCON.TTF"],
        "comic" => &["comic.ttf", "comicbd.ttf"],
        "comicbold" => &["comicbd.ttf", "comic.ttf"],
        _ => return None,
    })
}

fn system_font_bytes(name: &str) -> Option<Vec<u8>> {
    let files = font_files(name)?;
    #[cfg(windows)]
    let dirs: Vec<PathBuf> = {
        let mut v = vec![PathBuf::from("C:\\Windows\\Fonts")];
        if let Ok(la) = std::env::var("LOCALAPPDATA") {
            v.push(PathBuf::from(la).join("Microsoft\\Windows\\Fonts"));
        }
        v
    };
    #[cfg(target_os = "macos")]
    let dirs: Vec<PathBuf> = vec![
        PathBuf::from("/System/Library/Fonts"),
        PathBuf::from("/Library/Fonts"),
    ];
    #[cfg(all(unix, not(target_os = "macos")))]
    let dirs: Vec<PathBuf> = vec![
        PathBuf::from("/usr/share/fonts"),
        PathBuf::from("/usr/share/fonts/truetype/msttcorefonts"),
        PathBuf::from("/usr/local/share/fonts"),
    ];
    for d in &dirs {
        for f in files {
            if let Ok(bytes) = std::fs::read(d.join(f)) {
                return Some(bytes);
            }
        }
    }
    None
}

fn font_arc(key: &str) -> Result<FontArc, FileError> {
    let fallback = || {
        FontArc::try_from_slice(FONT_BYTES)
            .map_err(|_| FileError::new(FileErrorCode::Internal, "bundled font failed"))
    };
    if key.is_empty() || key == "dejavu" {
        return fallback();
    }
    match system_font_bytes(key).and_then(|b| FontArc::try_from_vec(b).ok()) {
        Some(f) => Ok(f),
        None => fallback(),
    }
}

fn text_tile(font: &FontArc, t: &ThumbText) -> RgbaImage {
    let text = if t.uppercase {
        t.text.to_uppercase()
    } else {
        t.text.clone()
    };
    let scale = PxScale::from(t.size as f32);
    let (w, h) = text_size(scale, font, &text);
    let pad = t.stroke_w * 2 + 4;
    let mut tile = RgbaImage::from_pixel(w + pad * 2, h + pad * 2, Rgba([0, 0, 0, 0]));
    let (ox, oy) = (pad as i32, pad as i32);
    if t.stroke_w > 0 {
        let s = t.stroke_w as i32;
        for (dx, dy) in [
            (-s, 0),
            (s, 0),
            (0, -s),
            (0, s),
            (-s, -s),
            (s, -s),
            (-s, s),
            (s, s),
        ] {
            draw_text_mut(
                &mut tile,
                Rgba(t.stroke_color),
                ox + dx,
                oy + dy,
                scale,
                font,
                &text,
            );
        }
    }
    draw_text_mut(&mut tile, Rgba(t.color), ox, oy, scale, font, &text);
    crop_to_content(tile)
}

fn text_layer(font: &FontArc, t: &ThumbText) -> (RgbaImage, i64, i64) {
    let mut tile = text_tile(font, t);
    if t.rotate != 0 {
        let rad = (t.rotate as f32).to_radians();
        let (sin, cos) = (rad.sin().abs(), rad.cos().abs());
        let (tw, th) = (tile.width() as f32, tile.height() as f32);
        let ew = ((tw * cos + th * sin).ceil() as u32).max(tile.width());
        let eh = ((tw * sin + th * cos).ceil() as u32).max(tile.height());
        let mut expanded = RgbaImage::from_pixel(ew, eh, Rgba([0, 0, 0, 0]));
        imageops::overlay(
            &mut expanded,
            &tile,
            ((ew - tile.width()) / 2) as i64,
            ((eh - tile.height()) / 2) as i64,
        );
        tile = crop_to_content(rotate_about_center(
            &expanded,
            rad,
            Interpolation::Bilinear,
            Rgba([0, 0, 0, 0]),
        ));
    }
    (
        tile.clone(),
        t.x as i64 - tile.width() as i64 / 2,
        t.y as i64 - tile.height() as i64 / 2,
    )
}

/// Draws one image layer (source or external) onto the canvas.
fn image_layer(canvas: &mut RgbaImage, rgba: &RgbaImage, fx: &ThumbImage, cw: u32, ch: u32) {
    let (px, py, nw, nh) = placement(rgba.dimensions(), fx, cw, ch);
    let mut placed = imageops::resize(rgba, nw, nh, imageops::FilterType::Lanczos3);
    if fx.flip_h {
        placed = imageops::flip_horizontal(&placed);
    }
    if fx.flip_v {
        placed = imageops::flip_vertical(&placed);
    }
    placed = grade(placed, &fx.adjust);
    if fx.radius > 0 {
        let mask = rounded_mask(nw, nh, fx.radius);
        for (p, m) in placed.pixels_mut().zip(mask.pixels()) {
            p[3] = ((p[3] as u32 * m[3] as u32) / 255) as u8;
        }
    }
    if fx.opacity < 100 {
        let f = fx.opacity.min(100);
        for p in placed.pixels_mut() {
            p[3] = (p[3] as u32 * f / 100) as u8;
        }
    }
    if fx.shadow {
        let mut sil = RgbaImage::new(nw, nh);
        for (p, s) in placed.pixels().zip(sil.pixels_mut()) {
            if p[3] > 0 {
                *s = Rgba([0, 0, 0, 160]);
            }
        }
        let sil = imageops::blur(&sil, (nw.max(nh) as f32 / 80.0).max(2.0));
        let off = (ch as i64 / 60).max(4);
        imageops::overlay(canvas, &sil, px, py + off);
    }
    if fx.outline_w > 0 {
        let mask = alpha_mask(&placed);
        let grown = dilate(&mask, Norm::LInf, fx.outline_w.max(1) as u8);
        let mut ring = RgbaImage::new(nw, nh);
        for (x, y, m) in grown.enumerate_pixels() {
            if m[0] > 0 && mask.get_pixel(x, y)[0] == 0 {
                ring.put_pixel(x, y, Rgba(fx.outline_color));
            }
        }
        imageops::overlay(canvas, &ring, px, py);
    }
    if fx.border_w > 0 {
        let b = fx.border_w;
        let mut ring = RgbaImage::new(nw + 2 * b, nh + 2 * b);
        let mask = rounded_mask(nw + 2 * b, nh + 2 * b, fx.radius + b);
        for (p, m) in ring.pixels_mut().zip(mask.pixels()) {
            if m[3] > 0 {
                *p = Rgba(fx.border_color);
            }
        }
        imageops::overlay(canvas, &ring, px - b as i64, py - b as i64);
    }
    imageops::overlay(canvas, &placed, px, py);
}

/// Composes the thumbnail canvas for one decoded image.
pub fn apply(img: &DynamicImage, spec: &ThumbnailSpec) -> Result<DynamicImage, FileError> {
    let (cw, ch) = (spec.width, spec.height);
    let mut canvas = match &spec.bg {
        ThumbBg::Transparent => RgbaImage::new(cw, ch),
        ThumbBg::Color { color } => RgbaImage::from_pixel(cw, ch, Rgba(*color)),
        ThumbBg::Gradient { from, to, angle } => gradient(cw, ch, *from, *to, *angle),
        ThumbBg::Blur { radius } => {
            let cover = cover_scaled(&img.to_rgba8(), cw, ch);
            imageops::blur(&cover, *radius as f32 / 4.0)
        }
        ThumbBg::Image { path, blur, dim } => {
            let bg_img = codec::decode(&PathBuf::from(path))
                .map_err(|e| FileError::new(e.code, format!("background image: {}", e.message)))?;
            let mut cover = cover_scaled(&bg_img.image.to_rgba8(), cw, ch);
            if *blur > 0 {
                cover = imageops::blur(&cover, *blur as f32 / 4.0);
            }
            if *dim > 0 {
                let a = (*dim as f32 / 100.0 * 255.0).round() as u8;
                imageops::overlay(
                    &mut cover,
                    &RgbaImage::from_pixel(cw, ch, Rgba([0, 0, 0, a])),
                    0,
                    0,
                );
            }
            cover
        }
    };
    canvas = grade(canvas, &spec.bg_adjust);

    let mut fonts: HashMap<String, FontArc> = HashMap::new();
    for layer in &spec.layers {
        match layer {
            ThumbLayer::Source(fx) => image_layer(&mut canvas, &img.to_rgba8(), fx, cw, ch),
            ThumbLayer::Image { path, fx } => {
                let d = codec::decode(&PathBuf::from(path))
                    .map_err(|e| FileError::new(e.code, format!("layer image: {}", e.message)))?;
                image_layer(&mut canvas, &d.image.to_rgba8(), fx, cw, ch);
            }
            ThumbLayer::Text(t) => {
                if !fonts.contains_key(&t.font) {
                    fonts.insert(t.font.clone(), font_arc(&t.font)?);
                }
                let font = &fonts[&t.font];
                let (tile, tx, ty) = text_layer(font, t);
                if t.shadow_dx != 0 || t.shadow_dy != 0 || t.shadow_blur > 0 {
                    let mut sh = tile.clone();
                    for p in sh.pixels_mut() {
                        if p[3] > 0 {
                            *p = Rgba([
                                t.shadow_color[0],
                                t.shadow_color[1],
                                t.shadow_color[2],
                                ((p[3] as u32 * t.shadow_color[3] as u32) / 255) as u8,
                            ]);
                        }
                    }
                    if t.shadow_blur > 0 {
                        sh = imageops::blur(&sh, t.shadow_blur as f32 / 3.0);
                    }
                    imageops::overlay(
                        &mut canvas,
                        &sh,
                        tx + t.shadow_dx as i64,
                        ty + t.shadow_dy as i64,
                    );
                }
                imageops::overlay(&mut canvas, &tile, tx, ty);
            }
        }
    }

    if spec.vignette > 0 {
        let strength = spec.vignette.min(100) as f32 / 100.0;
        let (fx, fy) = (cw as f32 / 2.0, ch as f32 / 2.0);
        let max_d = (fx * fx + fy * fy).sqrt();
        for (x, y, p) in canvas.enumerate_pixels_mut() {
            let d = (((x as f32 - fx).powi(2) + (y as f32 - fy).powi(2)).sqrt() / max_d)
                .clamp(0.0, 1.0);
            let dark = strength * d * d;
            for c in 0..3 {
                p[c] = (p[c] as f32 * (1.0 - dark)).round() as u8;
            }
        }
    }
    Ok(DynamicImage::ImageRgba8(canvas))
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::DynamicImage;

    fn fx() -> ThumbImage {
        ThumbImage {
            fit: ThumbFit::Cover,
            zoom_x: 100,
            zoom_y: 100,
            x: 0,
            y: 0,
            radius: 0,
            border_w: 0,
            border_color: [255, 255, 255, 255],
            shadow: false,
            outline_w: 0,
            outline_color: [255, 255, 0, 255],
            opacity: 100,
            flip_h: false,
            flip_v: false,
            adjust: ThumbAdjust::default(),
        }
    }

    fn spec() -> ThumbnailSpec {
        ThumbnailSpec {
            width: 1280,
            height: 720,
            bg: ThumbBg::Transparent,
            bg_adjust: ThumbAdjust::default(),
            vignette: 0,
            layers: vec![ThumbLayer::Source(fx())],
        }
    }

    #[test]
    fn produces_canvas_size() {
        let img = DynamicImage::new_rgba8(800, 600);
        let out = apply(&img, &spec()).unwrap();
        assert_eq!((out.width(), out.height()), (1280, 720));
    }

    #[test]
    fn validates() {
        let mut s = spec();
        s.width = 0;
        assert!(s.validate().is_err());
        s.width = 1280;
        s.layers.push(ThumbLayer::Text(ThumbText {
            text: "".into(),
            x: 0,
            y: 0,
            size: 32,
            color: [255, 255, 255, 255],
            stroke_w: 0,
            stroke_color: [0, 0, 0, 255],
            font: "dejavu".into(),
            rotate: 0,
            uppercase: false,
            shadow_dx: 0,
            shadow_dy: 0,
            shadow_blur: 0,
            shadow_color: [0, 0, 0, 255],
        }));
        assert!(s.validate().is_err());
    }

    #[test]
    fn layer_order_is_respected() {
        // Two source layers at different positions both land on the canvas.
        let mut s = spec();
        let mut top = fx();
        top.x = 100;
        s.layers.push(ThumbLayer::Source(top));
        let img = DynamicImage::new_rgba8(80, 60);
        assert!(apply(&img, &s).is_ok());
    }

    #[test]
    fn gradient_fills_corners() {
        let g = gradient(4, 4, [0, 0, 0, 255], [255, 255, 255, 255], 0);
        assert!(g.get_pixel(0, 0)[0] < g.get_pixel(3, 0)[0]);
    }
}
