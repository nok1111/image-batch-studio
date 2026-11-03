//! Sprite tools: grid cutting (one image → many tiles) and sprite sheet
//! packing (many images → one sheet + JSON metadata).

use crate::codec::{self, EncodeOptions};
use crate::engine::JobError;
use crate::output::{Plan, Planner, TEMP_PREFIX};
use crate::{
    FileError, FileErrorCode, FileResult, FileStatus, FileWarning, ImageFormat, Job, OutputMode,
};
use image::{imageops, DynamicImage, GenericImageView, RgbaImage};
use rayon::prelude::*;
use serde::{Deserialize, Serialize};
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use ts_rs::TS;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(tag = "mode", rename_all = "camelCase")]
#[ts(export)]
pub enum SpriteMode {
    Cut(CutSpec),
    Pack(PackSpec),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct CutSpec {
    pub sizing: CutSizing,
    pub edge: CutEdge,
    /// Skip tiles whose pixels are all transparent.
    pub skip_transparent: bool,
    /// Skip tiles identical to an earlier tile of the same image.
    pub dedupe: bool,
    pub naming: TileNaming,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(tag = "kind", rename_all = "camelCase")]
#[ts(export)]
pub enum CutSizing {
    /// Fixed grid: `rows` × `cols` tiles per image.
    RowsCols { rows: u32, cols: u32 },
    /// Tiles of `width` × `height` px; the grid follows the image size.
    TileSize { width: u32, height: u32 },
}

/// What to do with the remainder when the image does not divide evenly.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum CutEdge {
    /// Edge tiles keep the leftover pixels (smaller tiles).
    Include,
    /// Leftover pixels are discarded; every tile is the same size.
    Crop,
    /// Edge tiles are padded with transparency to the full tile size.
    Pad,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum TileNaming {
    /// `{stem}_{index:03}` — index counts tiles of that image.
    Index,
    /// `{stem}_r{row}_c{col}`.
    RowCol,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct PackSpec {
    /// Output sheet base name; `_2`, `_3`… are appended for extra sheets.
    pub name: String,
    pub cell: PackCell,
    /// Pixels of space between cells.
    pub padding: u32,
    /// Pixels of space around the sheet edge.
    pub margin: u32,
    /// Max sheet size; when set, extra sprites overflow into new sheets.
    pub max_size: Option<[u32; 2]>,
    /// Round the final sheet dimensions up to a power of two.
    pub power_of_two: bool,
    pub order: PackOrder,
    /// Crop each sprite's transparent border; offsets land in the metadata.
    pub trim: bool,
    /// Identical sprites share one cell.
    pub dedupe: bool,
    /// Write a JSON metadata file next to every sheet.
    pub metadata: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(tag = "kind", rename_all = "camelCase")]
#[ts(export)]
pub enum PackCell {
    /// Cell fits the largest sprite (after trimming).
    Auto,
    Fixed {
        width: u32,
        height: u32,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum PackOrder {
    Name,
    Area,
    Width,
    Height,
}

impl SpriteMode {
    pub fn validate(&self) -> Result<(), String> {
        match self {
            SpriteMode::Cut(c) => match c.sizing {
                CutSizing::RowsCols { rows, cols } if rows == 0 || cols == 0 => {
                    Err("grid rows and columns must be at least 1".into())
                }
                CutSizing::TileSize { width, height } if width == 0 || height == 0 => {
                    Err("tile size must be at least 1 px".into())
                }
                _ => Ok(()),
            },
            SpriteMode::Pack(p) => {
                if p.name.trim().is_empty() {
                    return Err("sheet name is empty".into());
                }
                if let PackCell::Fixed { width, height } = p.cell {
                    if width == 0 || height == 0 {
                        return Err("cell size must be at least 1 px".into());
                    }
                }
                if let Some([w, h]) = p.max_size {
                    if w == 0 || h == 0 {
                        return Err("max sheet size must be at least 1 px".into());
                    }
                    if p.margin * 2 >= w || p.margin * 2 >= h {
                        return Err("margin leaves no room inside the sheet".into());
                    }
                }
                Ok(())
            }
        }
    }
}

fn format_for(job: &Job, input: &Path) -> Result<ImageFormat, FileError> {
    match job.output.mode {
        OutputMode::Convert(f) => Ok(f),
        OutputMode::SameAsSource => ImageFormat::from_input_path(input)
            .ok_or_else(|| FileError::new(FileErrorCode::Unsupported, "unknown file extension")),
        OutputMode::CopyOriginal => Ok(ImageFormat::Png),
    }
}

/// Writes encoded image `bytes` to `target` via temp file, then verifies it.
#[allow(clippy::too_many_arguments)]
fn write_image(
    bytes: &[u8],
    target: &Path,
    replace: bool,
    dir: &Path,
    format: ImageFormat,
    w: u32,
    h: u32,
    cancel: &AtomicBool,
) -> Result<Option<u64>, FileError> {
    let ext = target
        .extension()
        .map(|e| e.to_string_lossy().into_owned())
        .unwrap_or_default();
    let mut tmp = tempfile::Builder::new()
        .prefix(TEMP_PREFIX)
        .suffix(&format!(".{ext}"))
        .tempfile_in(dir)
        .map_err(FileError::io)?;
    tmp.as_file_mut().write_all(bytes).map_err(FileError::io)?;
    tmp.as_file_mut().flush().map_err(FileError::io)?;
    if cancel.load(Ordering::Relaxed) {
        return Ok(None);
    }
    codec::verify(tmp.path(), format, w, h)?;
    let persisted = if replace {
        tmp.persist(target)
    } else {
        tmp.persist_noclobber(target)
    };
    let file = persisted.map_err(|e| FileError::io(e.error))?;
    Ok(Some(file.metadata().map_err(FileError::io)?.len()))
}

/// Writes raw `bytes` (e.g. JSON metadata) to `target` via temp file.
fn write_raw(
    bytes: &[u8],
    target: &Path,
    replace: bool,
    dir: &Path,
    cancel: &AtomicBool,
) -> Result<Option<u64>, FileError> {
    let mut tmp = tempfile::Builder::new()
        .prefix(TEMP_PREFIX)
        .tempfile_in(dir)
        .map_err(FileError::io)?;
    tmp.as_file_mut().write_all(bytes).map_err(FileError::io)?;
    tmp.as_file_mut().flush().map_err(FileError::io)?;
    if cancel.load(Ordering::Relaxed) {
        return Ok(None);
    }
    let persisted = if replace {
        tmp.persist(target)
    } else {
        tmp.persist_noclobber(target)
    };
    let file = persisted.map_err(|e| FileError::io(e.error))?;
    Ok(Some(file.metadata().map_err(FileError::io)?.len()))
}

// ---------------------------------------------------------------- cut

fn tile_stem(base: &str, naming: TileNaming, index: u32, row: u32, col: u32) -> String {
    match naming {
        TileNaming::Index => format!("{base}_{index:03}"),
        TileNaming::RowCol => format!("{base}_r{row}_c{col}"),
    }
}

fn is_transparent(img: &RgbaImage) -> bool {
    img.pixels().all(|p| p.0[3] == 0)
}

fn pixel_hash(img: &RgbaImage) -> u64 {
    let mut h = DefaultHasher::new();
    img.width().hash(&mut h);
    img.height().hash(&mut h);
    img.as_raw().hash(&mut h);
    h.finish()
}

fn encode_opts<'a>(job: &'a Job, format: ImageFormat) -> EncodeOptions<'a> {
    EncodeOptions {
        format,
        quality: job.output.quality,
        lossless: job.output.lossless,
        icc: None,
        exif: None,
    }
}

fn cut_one(
    index: u32,
    input: &Path,
    spec: &CutSpec,
    job: &Job,
    dir: &Path,
    planner: &Planner,
    cancel: &AtomicBool,
) -> FileResult {
    let mk = |status: FileStatus| FileResult {
        index,
        status,
        output: None,
        output_size: None,
        error: None,
        warnings: Vec::new(),
    };
    let inner = (|| -> Result<FileResult, FileError> {
        let format = format_for(job, input)?;
        let decoded = codec::decode(input)?;
        let mut warnings = Vec::new();
        if decoded.animated {
            warnings.push(FileWarning::FirstFrameOnly);
        }
        let (img_w, img_h) = decoded.image.dimensions();
        let (tile_w, tile_h, cols, rows) = match spec.sizing {
            CutSizing::RowsCols { rows, cols } => {
                let (tw, th) = match spec.edge {
                    CutEdge::Crop => (img_w / cols, img_h / rows),
                    _ => (img_w.div_ceil(cols), img_h.div_ceil(rows)),
                };
                (tw, th, cols, rows)
            }
            CutSizing::TileSize { width, height } => {
                (width, height, img_w.div_ceil(width), img_h.div_ceil(height))
            }
        };
        if tile_w == 0 || tile_h == 0 {
            return Err(FileError::new(
                FileErrorCode::TooLarge,
                "grid is finer than the image",
            ));
        }
        codec::check_encodable(format, tile_w, tile_h)?;
        let opts = encode_opts(job, format);
        let stem = input
            .file_stem()
            .map(|s| s.to_string_lossy().into_owned())
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| "image".into());
        let rgba = decoded.image.to_rgba8();
        let mut seen = std::collections::HashSet::new();
        let mut first_output: Option<String> = None;
        let mut total_size: u64 = 0;
        let mut wrote_any = false;
        for row in 0..rows {
            for col in 0..cols {
                if cancel.load(Ordering::Relaxed) {
                    return Ok(mk(FileStatus::Cancelled));
                }
                let (x, y) = (col * tile_w, row * tile_h);
                if x >= img_w || y >= img_h {
                    continue;
                }
                let (cw, ch) = ((img_w - x).min(tile_w), (img_h - y).min(tile_h));
                let mut tile = imageops::crop_imm(&rgba, x, y, cw, ch).to_image();
                if spec.edge == CutEdge::Pad && (cw != tile_w || ch != tile_h) {
                    let mut canvas = RgbaImage::new(tile_w, tile_h);
                    imageops::overlay(&mut canvas, &tile, 0, 0);
                    tile = canvas;
                }
                if spec.skip_transparent && is_transparent(&tile) {
                    continue;
                }
                if spec.dedupe && !seen.insert(pixel_hash(&tile)) {
                    continue;
                }
                let n = row * cols + col;
                let name = tile_stem(&stem, spec.naming, n, row, col);
                let (target, replace) = match planner.plan_named(&name, format.extension())? {
                    Plan::Write { target, replace } => (target, replace),
                    Plan::Skip { target } => {
                        if first_output.is_none() {
                            first_output = Some(target.to_string_lossy().into_owned());
                        }
                        continue;
                    }
                };
                let img = DynamicImage::ImageRgba8(tile);
                let encoded = codec::encode(&img, &opts)?;
                let size = write_image(
                    &encoded.bytes,
                    &target,
                    replace,
                    dir,
                    format,
                    img.width(),
                    img.height(),
                    cancel,
                )?;
                let Some(size) = size else {
                    return Ok(mk(FileStatus::Cancelled));
                };
                wrote_any = true;
                total_size += size;
                if first_output.is_none() {
                    first_output = Some(target.to_string_lossy().into_owned());
                }
            }
        }
        Ok(FileResult {
            status: if wrote_any {
                FileStatus::Done
            } else {
                FileStatus::Skipped
            },
            output: first_output,
            output_size: wrote_any.then_some(total_size),
            warnings,
            ..mk(FileStatus::Failed)
        })
    })();
    inner.unwrap_or_else(|e| FileResult {
        error: Some(e),
        ..mk(FileStatus::Failed)
    })
}

// ---------------------------------------------------------------- pack

struct Sprite {
    index: u32,
    name: String,
    img: RgbaImage,
    /// (x, y) where the trim crop started inside the original image.
    offset: (u32, u32),
    source_w: u32,
    source_h: u32,
}

fn alpha_bbox(img: &RgbaImage) -> Option<(u32, u32, u32, u32)> {
    let (w, h) = img.dimensions();
    let (mut x0, mut y0, mut x1, mut y1) = (w, h, 0u32, 0u32);
    for (x, y, p) in img.enumerate_pixels() {
        if p.0[3] > 0 {
            x0 = x0.min(x);
            y0 = y0.min(y);
            x1 = x1.max(x + 1);
            y1 = y1.max(y + 1);
        }
    }
    (x1 > x0 && y1 > y0).then_some((x0, y0, x1, y1))
}

struct Placement {
    /// Index into the `dims` slice passed to `layout`.
    sprite: usize,
    x: u32,
    y: u32,
}

struct Sheet {
    placements: Vec<Placement>,
    width: u32,
    height: u32,
    cell_w: u32,
    cell_h: u32,
}

fn cell_dims(dims: &[(u32, u32)], spec: &PackSpec) -> (u32, u32) {
    match spec.cell {
        PackCell::Fixed { width, height } => (width, height),
        PackCell::Auto => (
            dims.iter().map(|d| d.0).max().unwrap_or(1),
            dims.iter().map(|d| d.1).max().unwrap_or(1),
        ),
    }
}

/// Row-major layout of `dims` into sheets; `dims` is already sorted.
fn layout(dims: &[(u32, u32)], spec: &PackSpec) -> Result<Vec<Sheet>, FileError> {
    let (cw, ch) = cell_dims(dims, spec);
    let (max_w, max_h) = spec
        .max_size
        .map(|m| {
            (
                m[0].saturating_sub(spec.margin * 2),
                m[1].saturating_sub(spec.margin * 2),
            )
        })
        .unzip();
    let cols = match max_w {
        Some(m) => ((m + spec.padding) / (cw + spec.padding)).max(1),
        None => (dims.len() as f64).sqrt().ceil().max(1.0) as u32,
    };
    let rows_fit = match max_h {
        Some(m) => ((m + spec.padding) / (ch + spec.padding)).max(1),
        None => (dims.len() as u32).div_ceil(cols),
    };
    let per_sheet = (cols * rows_fit).max(1) as usize;
    let mut sheets = Vec::new();
    for (off, chunk) in dims.chunks(per_sheet).enumerate() {
        let mut placements = Vec::with_capacity(chunk.len());
        let (mut cols_used, mut rows_used) = (0u32, 0u32);
        for (i, _) in chunk.iter().enumerate() {
            let (col, row) = ((i as u32) % cols, (i as u32) / cols);
            placements.push(Placement {
                sprite: off * per_sheet + i,
                x: spec.margin + col * (cw + spec.padding),
                y: spec.margin + row * (ch + spec.padding),
            });
            cols_used = cols_used.max(col + 1);
            rows_used = rows_used.max(row + 1);
        }
        let (mut w, mut h) = (
            spec.margin * 2 + cols_used * cw + cols_used.saturating_sub(1) * spec.padding,
            spec.margin * 2 + rows_used * ch + rows_used.saturating_sub(1) * spec.padding,
        );
        if let (Some(mw), Some(mh)) = (spec.max_size.map(|m| m[0]), spec.max_size.map(|m| m[1])) {
            w = mw;
            h = mh;
        }
        if spec.power_of_two {
            w = w.next_power_of_two();
            h = h.next_power_of_two();
        }
        sheets.push(Sheet {
            placements,
            width: w,
            height: h,
            cell_w: cw,
            cell_h: ch,
        });
    }
    Ok(sheets)
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct MetaSprite<'a> {
    name: &'a str,
    x: u32,
    y: u32,
    w: u32,
    h: u32,
    offset_x: u32,
    offset_y: u32,
    source_w: u32,
    source_h: u32,
    duplicated: bool,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct MetaSheet<'a> {
    image: &'a str,
    width: u32,
    height: u32,
    sprites: Vec<MetaSprite<'a>>,
}

fn cancelled_all(inputs: &[PathBuf], on_result: &dyn Fn(FileResult)) {
    for i in 0..inputs.len() {
        on_result(FileResult {
            index: i as u32,
            status: FileStatus::Cancelled,
            output: None,
            output_size: None,
            error: None,
            warnings: Vec::new(),
        });
    }
}

fn pack(
    inputs: &[PathBuf],
    spec: &PackSpec,
    job: &Job,
    dir: &Path,
    planner: &Planner,
    cancel: &AtomicBool,
    on_result: &dyn Fn(FileResult),
) {
    // Decode in parallel, keep input order, per-input failure isolation.
    let decoded: Vec<Result<codec::Decoded, FileError>> =
        inputs.par_iter().map(|p| codec::decode(p)).collect();
    let mut sprites: Vec<Sprite> = Vec::new();
    let mut failed: Vec<(u32, FileError)> = Vec::new();
    for (i, r) in decoded.into_iter().enumerate() {
        match r {
            Ok(d) => {
                let rgba = d.image.to_rgba8();
                let (img, offset) = if spec.trim {
                    match alpha_bbox(&rgba) {
                        Some((x0, y0, x1, y1)) => (
                            imageops::crop_imm(&rgba, x0, y0, x1 - x0, y1 - y0).to_image(),
                            (x0, y0),
                        ),
                        None => (rgba, (0, 0)),
                    }
                } else {
                    (rgba, (0, 0))
                };
                sprites.push(Sprite {
                    index: i as u32,
                    name: inputs[i]
                        .file_name()
                        .map(|s| s.to_string_lossy().into_owned())
                        .unwrap_or_else(|| format!("{i}")),
                    source_w: d.image.width(),
                    source_h: d.image.height(),
                    offset,
                    img,
                });
            }
            Err(e) => failed.push((i as u32, e)),
        }
    }
    if cancel.load(Ordering::Relaxed) {
        cancelled_all(inputs, on_result);
        return;
    }
    let order = spec.order;
    sprites.sort_by(|a, b| {
        let ord = match order {
            PackOrder::Name => a.name.to_lowercase().cmp(&b.name.to_lowercase()),
            PackOrder::Area => {
                (b.img.width() * b.img.height()).cmp(&(a.img.width() * a.img.height()))
            }
            PackOrder::Width => b.img.width().cmp(&a.img.width()),
            PackOrder::Height => b.img.height().cmp(&a.img.height()),
        };
        ord.then(a.index.cmp(&b.index))
    });
    // Identical sprites point at the first copy.
    let mut alias: Vec<Option<usize>> = vec![None; sprites.len()];
    if spec.dedupe {
        let mut seen = std::collections::HashMap::new();
        for (i, s) in sprites.iter().enumerate() {
            let h = pixel_hash(&s.img);
            match seen.get(&h) {
                Some(&first) => alias[i] = Some(first),
                None => {
                    seen.insert(h, i);
                }
            }
        }
    }
    let drawn: Vec<&Sprite> = sprites
        .iter()
        .enumerate()
        .filter(|(i, _)| !spec.dedupe || alias[*i].is_none())
        .map(|(_, s)| s)
        .collect();
    let dims: Vec<(u32, u32)> = drawn
        .iter()
        .map(|s| (s.img.width(), s.img.height()))
        .collect();
    let sheets = match layout(&dims, spec) {
        Ok(s) => s,
        Err(e) => {
            for i in 0..inputs.len() {
                on_result(FileResult {
                    index: i as u32,
                    status: FileStatus::Failed,
                    output: None,
                    output_size: None,
                    error: Some(e.clone()),
                    warnings: Vec::new(),
                });
            }
            return;
        }
    };
    let format = format_for(job, &inputs[0]).unwrap_or(ImageFormat::Png);
    let multi = sheets.len() > 1;
    // sprite index (in `sprites`) → (sheet path, sheet size) once written.
    let mut output_of: Vec<Option<(PathBuf, u64)>> = vec![None; sprites.len()];
    for (si, sheet) in sheets.iter().enumerate() {
        if cancel.load(Ordering::Relaxed) {
            cancelled_all(inputs, on_result);
            return;
        }
        let mut canvas = RgbaImage::new(sheet.width, sheet.height);
        let mut metas: Vec<(usize, MetaSprite)> = Vec::new();
        for p in &sheet.placements {
            let s = drawn[p.sprite];
            let (dx, dy) = (
                (sheet.cell_w.saturating_sub(s.img.width())) / 2,
                (sheet.cell_h.saturating_sub(s.img.height())) / 2,
            );
            imageops::overlay(&mut canvas, &s.img, (p.x + dx) as i64, (p.y + dy) as i64);
            let sprite_idx = sprites.iter().position(|o| std::ptr::eq(o, s)).unwrap();
            metas.push((
                sprite_idx,
                MetaSprite {
                    name: &s.name,
                    x: p.x + dx,
                    y: p.y + dy,
                    w: s.img.width(),
                    h: s.img.height(),
                    offset_x: s.offset.0,
                    offset_y: s.offset.1,
                    source_w: s.source_w,
                    source_h: s.source_h,
                    duplicated: false,
                },
            ));
        }
        // Aliased sprites appear in metadata pointing at their twin's cell.
        if spec.dedupe {
            for (i, a) in alias.iter().enumerate() {
                let Some(first) = a else { continue };
                if let Some((_, twin)) = metas.iter().find(|(si2, _)| *si2 == *first) {
                    let (x, y, w, h) = (twin.x, twin.y, twin.w, twin.h);
                    let s = &sprites[i];
                    metas.push((
                        i,
                        MetaSprite {
                            name: &s.name,
                            x,
                            y,
                            w,
                            h,
                            offset_x: s.offset.0,
                            offset_y: s.offset.1,
                            source_w: s.source_w,
                            source_h: s.source_h,
                            duplicated: true,
                        },
                    ));
                }
            }
        }
        let stem = if multi {
            format!("{}_{}", spec.name, si + 1)
        } else {
            spec.name.clone()
        };
        let ext = format.extension();
        let sprite_idxs: Vec<usize> = metas.iter().map(|(i, _)| *i).collect();
        let write = (|| -> Result<(PathBuf, u64), FileError> {
            let (target, replace) = match planner.plan_named(&stem, ext)? {
                Plan::Write { target, replace } => (target, replace),
                Plan::Skip { target } => (target, false),
            };
            codec::check_encodable(format, sheet.width, sheet.height)?;
            let img = DynamicImage::ImageRgba8(canvas);
            let encoded = codec::encode(&img, &encode_opts(job, format))?;
            let size = write_image(
                &encoded.bytes,
                &target,
                replace,
                dir,
                format,
                sheet.width,
                sheet.height,
                cancel,
            )?
            .ok_or_else(|| FileError::new(FileErrorCode::Internal, "cancelled"))?;
            if spec.metadata {
                let meta = MetaSheet {
                    image: target
                        .file_name()
                        .map(|s| s.to_string_lossy().into_owned())
                        .unwrap_or_default()
                        .leak(),
                    width: sheet.width,
                    height: sheet.height,
                    sprites: metas.into_iter().map(|(_, m)| m).collect(),
                };
                let json = serde_json::to_string_pretty(&meta)
                    .map_err(|e| FileError::new(FileErrorCode::Internal, e.to_string()))?;
                let (jpath, jreplace) = match planner.plan_named(&stem, "json")? {
                    Plan::Write { target, replace } => (target, replace),
                    Plan::Skip { target } => (target, false),
                };
                write_raw(json.as_bytes(), &jpath, jreplace, dir, cancel)?;
            }
            Ok((target, size))
        })();
        match write {
            Ok((t, size)) => {
                for i in &sprite_idxs {
                    output_of[*i] = Some((t.clone(), size));
                }
            }
            Err(e) => {
                for i in &sprite_idxs {
                    let s = &sprites[*i];
                    on_result(FileResult {
                        index: s.index,
                        status: FileStatus::Failed,
                        output: None,
                        output_size: None,
                        error: Some(e.clone()),
                        warnings: Vec::new(),
                    });
                    output_of[*i] = Some((PathBuf::new(), 0));
                }
            }
        }
    }
    for (fi, e) in failed {
        on_result(FileResult {
            index: fi,
            status: FileStatus::Failed,
            output: None,
            output_size: None,
            error: Some(e),
            warnings: Vec::new(),
        });
    }
    for (i, s) in sprites.iter().enumerate() {
        let out = match alias[i] {
            Some(first) => output_of[first].clone(),
            None => output_of[i].clone(),
        };
        if let Some((t, size)) = out {
            if t.as_os_str().is_empty() {
                continue; // already reported as failed
            }
            on_result(FileResult {
                index: s.index,
                status: FileStatus::Done,
                output: Some(t.to_string_lossy().into_owned()),
                output_size: Some(size),
                error: None,
                warnings: Vec::new(),
            });
        }
    }
}

/// What the sheet preview needs: rect of each sprite inside each sheet.
#[derive(Debug, Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct PackRect {
    /// Index into the `inputs` slice, for image loading on the UI side.
    pub index: u32,
    pub name: String,
    pub x: u32,
    pub y: u32,
    pub w: u32,
    pub h: u32,
}

#[derive(Debug, Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct PackSheetPreview {
    pub width: u32,
    pub height: u32,
    pub sprites: Vec<PackRect>,
}

#[derive(Debug, Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct PackPreview {
    pub sheets: Vec<PackSheetPreview>,
    /// Inputs whose dimensions could not be read.
    pub failed: u32,
    /// Cell size the layout used (auto = largest sprite).
    pub cell_w: u32,
    pub cell_h: u32,
}

/// Computes the sheet layout without pixels: header dims only.
/// Ignores `trim` (needs pixels) and `dedupe` (needs pixels).
pub fn preview_pack(inputs: &[String], spec: &PackSpec) -> Result<PackPreview, JobError> {
    SpriteMode::Pack(spec.clone())
        .validate()
        .map_err(JobError::InvalidOptions)?;
    let paths: Vec<PathBuf> = inputs.iter().map(PathBuf::from).collect();
    let dims: Vec<Result<(u32, u32), FileError>> =
        paths.par_iter().map(|p| codec::dimensions(p)).collect();
    let mut sprites: Vec<(u32, String, u32, u32)> = Vec::new();
    let mut failed = 0u32;
    for (i, r) in dims.into_iter().enumerate() {
        match r {
            Ok((w, h)) => sprites.push((
                i as u32,
                paths[i]
                    .file_name()
                    .map(|s| s.to_string_lossy().into_owned())
                    .unwrap_or_else(|| format!("{i}")),
                w,
                h,
            )),
            Err(_) => failed += 1,
        }
    }
    let order = spec.order;
    sprites.sort_by(|a, b| {
        let ord = match order {
            PackOrder::Name => a.1.to_lowercase().cmp(&b.1.to_lowercase()),
            PackOrder::Area => (b.2 * b.3).cmp(&(a.2 * a.3)),
            PackOrder::Width => b.2.cmp(&a.2),
            PackOrder::Height => b.3.cmp(&a.3),
        };
        ord.then(a.1.cmp(&b.1))
    });
    let dims: Vec<(u32, u32)> = sprites.iter().map(|s| (s.2, s.3)).collect();
    let (cw, ch) = cell_dims(&dims, spec);
    let sheets = layout(&dims, spec).map_err(|e| JobError::InvalidOptions(e.to_string()))?;
    Ok(PackPreview {
        sheets: sheets
            .iter()
            .map(|sh| PackSheetPreview {
                width: sh.width,
                height: sh.height,
                sprites: sh
                    .placements
                    .iter()
                    .map(|p| {
                        let (sw, shh) = dims[p.sprite];
                        PackRect {
                            index: sprites[p.sprite].0,
                            name: sprites[p.sprite].1.clone(),
                            x: p.x + (sh.cell_w.saturating_sub(sw)) / 2,
                            y: p.y + (sh.cell_h.saturating_sub(shh)) / 2,
                            w: sw,
                            h: shh,
                        }
                    })
                    .collect(),
            })
            .collect(),
        failed,
        cell_w: cw,
        cell_h: ch,
    })
}

/// Runs a sprite job: grid cutting or sheet packing.
pub fn execute<F>(
    job: &Job,
    spec: &SpriteMode,
    dir: &Path,
    inputs: &[PathBuf],
    cancel: &AtomicBool,
    on_result: F,
) -> Result<crate::engine::JobSummary, JobError>
where
    F: Fn(FileResult) + Sync,
{
    let started = std::time::Instant::now();
    let format = match job.output.mode {
        OutputMode::Convert(f) => f,
        _ => ImageFormat::Png,
    };
    let planner = Planner::new(dir, format, job.output.conflict, inputs);
    let counts: [std::sync::atomic::AtomicU32; 4] = Default::default();
    let emit = |r: FileResult| {
        let slot = match r.status {
            FileStatus::Done => 0,
            FileStatus::Skipped => 1,
            FileStatus::Failed => 2,
            FileStatus::Cancelled => 3,
        };
        counts[slot].fetch_add(1, Ordering::Relaxed);
        on_result(r);
    };
    match spec {
        SpriteMode::Cut(c) => {
            let results: Vec<FileResult> = inputs
                .par_iter()
                .enumerate()
                .map(|(i, input)| {
                    if cancel.load(Ordering::Relaxed) {
                        FileResult {
                            index: i as u32,
                            status: FileStatus::Cancelled,
                            output: None,
                            output_size: None,
                            error: None,
                            warnings: Vec::new(),
                        }
                    } else {
                        cut_one(i as u32, input, c, job, dir, &planner, cancel)
                    }
                })
                .collect();
            for r in results {
                emit(r);
            }
        }
        SpriteMode::Pack(p) => pack(inputs, p, job, dir, &planner, cancel, &emit),
    }
    let [done, skipped, failed, cancelled] = counts.map(|a| a.into_inner());
    Ok(crate::engine::JobSummary {
        total: inputs.len() as u32,
        done,
        skipped,
        failed,
        cancelled,
        elapsed_ms: started.elapsed().as_millis() as u64,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pack_spec(cell: PackCell, max_size: Option<[u32; 2]>) -> PackSpec {
        PackSpec {
            name: "s".into(),
            cell,
            padding: 0,
            margin: 0,
            max_size,
            power_of_two: false,
            order: PackOrder::Name,
            trim: false,
            dedupe: false,
            metadata: false,
        }
    }

    #[test]
    fn validates_specs() {
        let mut cut = CutSpec {
            sizing: CutSizing::RowsCols { rows: 0, cols: 2 },
            edge: CutEdge::Include,
            skip_transparent: false,
            dedupe: false,
            naming: TileNaming::Index,
        };
        assert!(SpriteMode::Cut(cut.clone()).validate().is_err());
        cut.sizing = CutSizing::RowsCols { rows: 4, cols: 4 };
        assert!(SpriteMode::Cut(cut).validate().is_ok());
        assert!(SpriteMode::Pack(pack_spec(PackCell::Auto, None))
            .validate()
            .is_ok());
    }

    #[test]
    fn packs_on_a_grid() {
        let mut spec = pack_spec(
            PackCell::Fixed {
                width: 10,
                height: 10,
            },
            None,
        );
        spec.padding = 2;
        spec.margin = 1;
        let dims: Vec<(u32, u32)> = (0..5).map(|_| (4, 4)).collect();
        let sheets = layout(&dims, &spec).unwrap();
        assert_eq!(sheets.len(), 1);
        let s = &sheets[0];
        // ceil(sqrt(5)) = 3 cols → 3×2 grid.
        assert_eq!(s.width, 2 + 3 * 10 + 2 * 2);
        assert_eq!(s.height, 2 + 2 * 10 + 2);
        assert_eq!(s.placements.len(), 5);
    }

    #[test]
    fn multipacks_when_full() {
        let spec = pack_spec(
            PackCell::Fixed {
                width: 10,
                height: 10,
            },
            Some([32, 22]),
        );
        let dims: Vec<(u32, u32)> = (0..7).map(|_| (10, 10)).collect();
        let sheets = layout(&dims, &spec).unwrap();
        // cols=3, rows=2 → 6 per sheet → 2 sheets.
        assert_eq!(sheets.len(), 2);
        assert_eq!(sheets[0].placements.len(), 6);
        assert_eq!(sheets[1].placements.len(), 1);
    }
}
