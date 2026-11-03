import type { ModuleId } from "@/app/modules";
import type {
  ImageFormat,
  Job,
  MetadataPolicy,
  OutputMode,
  RenameSpec,
  ResizeSpec,
  Settings,
  TransformSpec,
  WatermarkSpec,
  SpriteMode,
  ThumbnailSpec,
  ThumbBg,
  ThumbImage,
  ThumbLayer,
} from "@/lib/ipc";

export type FormatChoice = ImageFormat | "source";
export type CompressPreset =
  "maxQuality" | "balanced" | "high" | "web" | "custom";

/** Pipeline steps the user can combine in one job (engine order). */
export interface PipelineUse {
  transform: boolean;
  resize: boolean;
  watermark: boolean;
  rename: boolean;
}

/** Sprites module options; maps to the engine SpriteMode. */
export interface SpriteOptions {
  mode: "cut" | "pack";
  cut: {
    sizing: "rowsCols" | "tileSize";
    rows: number;
    cols: number;
    tileW: number;
    tileH: number;
    edge: "include" | "crop" | "pad";
    skipTransparent: boolean;
    dedupe: boolean;
    naming: "index" | "rowCol";
  };
  pack: {
    name: string;
    cell: "auto" | "fixed";
    cellW: number;
    cellH: number;
    padding: number;
    margin: number;
    maxW: number;
    maxH: number;
    powerOfTwo: boolean;
    order: "name" | "area" | "width" | "height";
    trim: boolean;
    dedupe: boolean;
    metadata: boolean;
  };
}

/** Social thumbnail canvas options; maps to the engine ThumbnailSpec. */
export interface ThumbTextOpt {
  text: string;
  x: number;
  y: number;
  size: number;
  color: string;
  strokeW: number;
  strokeColor: string;
  font: string;
  rotate: number;
  uppercase: boolean;
  shadowDx: number;
  shadowDy: number;
  shadowBlur: number;
  shadowColor: string;
}

/** Placement + effects shared by the source image and extra image layers. */
export interface ThumbImgFx {
  fit: "cover" | "contain" | "stretch";
  zoomX: number;
  zoomY: number;
  x: number;
  y: number;
  radius: number;
  borderW: number;
  borderColor: string;
  shadow: boolean;
  outlineW: number;
  outlineColor: string;
  opacity: number;
  flipH: boolean;
  flipV: boolean;
  brightness: number;
  contrast: number;
  hue: number;
  saturation: number;
}

export type ThumbLayerOpt =
  | { kind: "source"; img: ThumbImgFx }
  | { kind: "image"; path: string; img: ThumbImgFx }
  | { kind: "text"; text: ThumbTextOpt };

export const DEFAULT_IMG_FX: ThumbImgFx = {
  fit: "cover",
  zoomX: 100,
  zoomY: 100,
  x: 0,
  y: 0,
  radius: 0,
  borderW: 0,
  borderColor: "#ffffff",
  shadow: false,
  outlineW: 0,
  outlineColor: "#ffffff",
  opacity: 100,
  flipH: false,
  flipV: false,
  brightness: 0,
  contrast: 0,
  hue: 0,
  saturation: 0,
};

export interface ThumbnailOptions {
  presetId: string;
  width: number;
  height: number;
  bg: {
    type: "transparent" | "color" | "blur" | "gradient" | "image";
    color: string;
    from: string;
    to: string;
    angle: number;
    blurRadius: number;
    imagePath: string;
    dim: number;
  };
  bgAdjust: { brightness: number; contrast: number; hue: number; saturation: number };
  vignette: number;
  layers: ThumbLayerOpt[];
  /** Selected layer index (UI state; the engine ignores it). */
  sel: number;
}

/** Every option a tool module can set; each module uses a subset. */
export interface ModuleOptions {
  format: FormatChoice;
  quality: number;
  lossless: boolean;
  metadata: MetadataPolicy;
  /** Compressor target size in KB; null = no target. */
  targetKb: number | null;
  compressPreset: CompressPreset;
  resize: ResizeSpec;
  transform: TransformSpec;
  watermark: WatermarkSpec;
  presetId: number | null;
  rename: RenameSpec;
  sprite: SpriteOptions;
  thumbnail: ThumbnailOptions;
  outputDir: string | null;
  pipelineUse: PipelineUse;
  /** Per-step preset selections inside the Pipeline (independent of presetId). */
  pipelineResizePresetId: number | null;
  pipelineWmPresetId: number | null;
}

export const COMPRESS_QUALITY: Record<
  Exclude<CompressPreset, "custom">,
  number
> = {
  maxQuality: 92,
  balanced: 80,
  high: 60,
  web: 72,
};

export const DEFAULT_RESIZE: ResizeSpec = {
  mode: "fit",
  width: 1920,
  height: 1080,
  percent: 50,
  allowUpscale: false,
};
export const DEFAULT_WATERMARK: WatermarkSpec = {
  kind: "text",
  text: "",
  color: [255, 255, 255, 255],
  imagePath: "",
  position: "bottomRight",
  custom: [0.5, 0.5],
  opacity: 0.6,
  rotation: 0,
  scale: 0.12,
  margin: 0.04,
  tiled: false,
  spacing: 0.5,
};
export interface ThumbPreset {
  id: string;
  label: string;
  w: number;
  h: number;
}

/** Presets used by the biggest social platforms. */
export const THUMB_PRESETS: ThumbPreset[] = [
  { id: "yt-thumb", label: "YouTube · Thumbnail", w: 1280, h: 720 },
  { id: "yt-banner", label: "YouTube · Banner", w: 2560, h: 1440 },
  { id: "yt-shorts", label: "YouTube Shorts / TikTok", w: 1080, h: 1920 },
  { id: "ig-square", label: "Instagram · Post cuadrado", w: 1080, h: 1080 },
  { id: "ig-portrait", label: "Instagram · Post vertical", w: 1080, h: 1350 },
  { id: "ig-landscape", label: "Instagram · Post horizontal", w: 1080, h: 566 },
  { id: "ig-story", label: "Instagram Story / Reels", w: 1080, h: 1920 },
  { id: "fb-post", label: "Facebook · Post", w: 1200, h: 630 },
  { id: "fb-cover", label: "Facebook · Portada", w: 851, h: 315 },
  { id: "x-post", label: "X / Twitter · Post", w: 1600, h: 900 },
  { id: "x-header", label: "X / Twitter · Header", w: 1500, h: 500 },
  { id: "li-post", label: "LinkedIn · Post", w: 1200, h: 627 },
  { id: "li-banner", label: "LinkedIn · Banner", w: 1584, h: 396 },
  { id: "pin", label: "Pinterest · Pin", w: 1000, h: 1500 },
  { id: "twitch-thumb", label: "Twitch · Thumbnail", w: 1280, h: 720 },
  { id: "og", label: "Open Graph · Link preview", w: 1200, h: 630 },
];

const SANS = "sans-serif";
const SERIF = "serif";
const MONO = "monospace";
const CURS = "cursive";
const FANT = "fantasy";

/** Font choices for text layers; the engine resolves them on the host system. */
export const THUMB_FONTS: { id: string; label: string; css: string }[] = [
  { id: "dejavu", label: "DejaVu Sans (bundled)", css: SANS },
  // — Poster / display (the thumbnail look) —
  { id: "impact", label: "Impact", css: `Impact, 'Arial Black', ${SANS}` },
  { id: "arialblack", label: "Arial Black", css: `'Arial Black', ${SANS}` },
  { id: "cooperblack", label: "Cooper Black", css: `'Cooper Black', ${SERIF}` },
  { id: "franklinheavy", label: "Franklin Gothic Heavy", css: `'Franklin Gothic Heavy', 'Arial Black', ${SANS}` },
  { id: "gillsansultra", label: "Gill Sans Ultra Bold", css: `'Gill Sans Ultra Bold', ${SANS}` },
  { id: "segoeblack", label: "Segoe UI Black", css: `'Segoe UI Black', 'Segoe UI', ${SANS}` },
  { id: "rockwellxb", label: "Rockwell Extra Bold", css: `'Rockwell Extra Bold', Rockwell, ${SERIF}` },
  { id: "rockwell", label: "Rockwell", css: `Rockwell, ${SERIF}` },
  { id: "stencil", label: "Stencil", css: `Stencil, ${FANT}` },
  { id: "showcard", label: "Showcard Gothic", css: `'Showcard Gothic', ${FANT}` },
  { id: "broadway", label: "Broadway", css: `Broadway, ${FANT}` },
  { id: "jokerman", label: "Jokerman", css: `Jokerman, ${FANT}` },
  { id: "ravie", label: "Ravie", css: `Ravie, ${FANT}` },
  { id: "playbill", label: "Playbill", css: `Playbill, ${FANT}` },
  { id: "snapitc", label: "Snap ITC", css: `'Snap ITC', ${FANT}` },
  { id: "vinerhand", label: "Viner Hand ITC", css: `'Viner Hand ITC', ${CURS}` },
  { id: "curlz", label: "Curlz MT", css: `'Curlz MT', ${CURS}` },
  { id: "mistral", label: "Mistral", css: `Mistral, ${CURS}` },
  { id: "corsiva", label: "Monotype Corsiva", css: `'Monotype Corsiva', ${CURS}` },
  { id: "oldenglish", label: "Old English Text", css: `'Old English Text MT', ${SERIF}` },
  { id: "widelatin", label: "Wide Latin", css: `'Wide Latin', ${SERIF}` },
  { id: "algerian", label: "Algerian", css: `Algerian, ${FANT}` },
  { id: "erasbold", label: "Eras Bold ITC", css: `'Eras Bold ITC', ${SANS}` },
  { id: "magneto", label: "Magneto", css: `Magneto, ${FANT}` },
  { id: "britannic", label: "Britannic Bold", css: `'Britannic Bold', ${SANS}` },
  { id: "castellar", label: "Castellar", css: `Castellar, ${SERIF}` },
  { id: "colonna", label: "Colonna MT", css: `'Colonna MT', ${SERIF}` },
  { id: "harrington", label: "Harrington", css: `Harrington, ${SERIF}` },
  { id: "elephant", label: "Elephant", css: `Elephant, ${SERIF}` },
  { id: "matura", label: "Matura MT Script", css: `'Matura MT Script Capitals', ${CURS}` },
  // — Casual / handwriting —
  { id: "comic", label: "Comic Sans", css: `'Comic Sans MS', ${CURS}` },
  { id: "comicbold", label: "Comic Sans Bold", css: `'Comic Sans MS', ${CURS}` },
  { id: "inkfree", label: "Ink Free", css: `'Ink Free', ${CURS}` },
  { id: "segoeprint", label: "Segoe Print", css: `'Segoe Print', ${CURS}` },
  { id: "segoescript", label: "Segoe Script", css: `'Segoe Script', ${CURS}` },
  { id: "bradley", label: "Bradley Hand", css: `'Bradley Hand ITC', ${CURS}` },
  { id: "freestyle", label: "Freestyle Script", css: `'Freestyle Script', ${CURS}` },
  { id: "kristen", label: "Kristen ITC", css: `'Kristen ITC', ${CURS}` },
  { id: "mvboli", label: "MV Boli", css: `'MV Boli', ${CURS}` },
  { id: "juice", label: "Juice ITC", css: `'Juice ITC', ${CURS}` },
  { id: "pristina", label: "Pristina", css: `Pristina, ${CURS}` },
  { id: "rage", label: "Rage Italic", css: `'Rage Italic', ${CURS}` },
  { id: "tempus", label: "Tempus Sans ITC", css: `'Tempus Sans ITC', ${CURS}` },
  { id: "blackadder", label: "Blackadder ITC", css: `'Blackadder ITC', ${CURS}` },
  { id: "edwardian", label: "Edwardian Script", css: `'Edwardian Script ITC', ${CURS}` },
  // — Sans —
  { id: "arial", label: "Arial", css: `Arial, ${SANS}` },
  { id: "arialbold", label: "Arial Bold", css: `Arial, ${SANS}` },
  { id: "verdana", label: "Verdana", css: `Verdana, ${SANS}` },
  { id: "verdanabold", label: "Verdana Bold", css: `Verdana, ${SANS}` },
  { id: "tahoma", label: "Tahoma", css: `Tahoma, ${SANS}` },
  { id: "tahomabold", label: "Tahoma Bold", css: `Tahoma, ${SANS}` },
  { id: "calibri", label: "Calibri", css: `Calibri, ${SANS}` },
  { id: "calibribold", label: "Calibri Bold", css: `Calibri, ${SANS}` },
  { id: "candara", label: "Candara", css: `Candara, ${SANS}` },
  { id: "candarabold", label: "Candara Bold", css: `Candara, ${SANS}` },
  { id: "corbel", label: "Corbel", css: `Corbel, ${SANS}` },
  { id: "corbelbold", label: "Corbel Bold", css: `Corbel, ${SANS}` },
  { id: "trebuchet", label: "Trebuchet", css: `'Trebuchet MS', ${SANS}` },
  { id: "trebuchetbold", label: "Trebuchet Bold", css: `'Trebuchet MS', ${SANS}` },
  { id: "centurygothic", label: "Century Gothic", css: `'Century Gothic', ${SANS}` },
  { id: "centurygothicbold", label: "Century Gothic Bold", css: `'Century Gothic', ${SANS}` },
  // — Serif —
  { id: "georgia", label: "Georgia", css: `Georgia, ${SERIF}` },
  { id: "georgiabold", label: "Georgia Bold", css: `Georgia, ${SERIF}` },
  { id: "times", label: "Times", css: `'Times New Roman', ${SERIF}` },
  { id: "palatino", label: "Palatino", css: `'Palatino Linotype', ${SERIF}` },
  { id: "garamond", label: "Garamond", css: `Garamond, ${SERIF}` },
  { id: "bookman", label: "Bookman Old Style", css: `'Bookman Old Style', ${SERIF}` },
  { id: "cambria", label: "Cambria", css: `Cambria, ${SERIF}` },
  { id: "constantia", label: "Constantia", css: `Constantia, ${SERIF}` },
  // — Monospace / tech —
  { id: "courier", label: "Courier", css: `'Courier New', ${MONO}` },
  { id: "courierbold", label: "Courier Bold", css: `'Courier New', ${MONO}` },
  { id: "consolas", label: "Consolas", css: `Consolas, ${MONO}` },
  { id: "lucidaconsole", label: "Lucida Console", css: `'Lucida Console', ${MONO}` },
];

export const DEFAULT_THUMBNAIL: ThumbnailOptions = {
  presetId: "yt-thumb",
  width: 1280,
  height: 720,
  bg: {
    type: "blur",
    color: "#111318",
    from: "#1e293b",
    to: "#0f172a",
    angle: 135,
    blurRadius: 24,
    imagePath: "",
    dim: 0,
  },
  bgAdjust: { brightness: 0, contrast: 0, hue: 0, saturation: 0 },
  vignette: 0,
  layers: [{ kind: "source", img: { ...DEFAULT_IMG_FX } }],
  sel: 0,
};


const hexToRgba = (hex: string): [number, number, number, number] => {
  const m = /^#?([0-9a-f]{6})([0-9a-f]{2})?$/i.exec(hex.trim());
  const v = m ? parseInt(m[1] ?? "000000", 16) : 0;
  const a = m?.[2] ? parseInt(m[2], 16) : 255;
  return [(v >> 16) & 255, (v >> 8) & 255, v & 255, a];
};

function thumbFx(f: ThumbImgFx): ThumbImage {
  return {
    fit: f.fit,
    zoomX: Math.round(f.zoomX),
    zoomY: Math.round(f.zoomY),
    x: Math.round(f.x),
    y: Math.round(f.y),
    radius: Math.round(f.radius),
    borderW: Math.round(f.borderW),
    borderColor: hexToRgba(f.borderColor),
    shadow: f.shadow,
    outlineW: Math.round(f.outlineW),
    outlineColor: hexToRgba(f.outlineColor),
    opacity: Math.round(f.opacity),
    flipH: f.flipH,
    flipV: f.flipV,
    adjust: {
      brightness: Math.round(f.brightness),
      contrast: Math.round(f.contrast),
      hue: Math.round(f.hue),
      saturation: Math.round(f.saturation),
    },
  };
}

export const newThumbText = (w: number, h: number, text: string): ThumbTextOpt => ({
  text,
  x: w / 2,
  y: h / 2,
  size: Math.round(h / 8),
  color: "#ffffff",
  strokeW: Math.max(1, Math.round(h / 180)),
  strokeColor: "#000000",
  font: "dejavu",
  rotate: 0,
  uppercase: false,
  shadowDx: 0,
  shadowDy: 0,
  shadowBlur: 0,
  shadowColor: "#000000",
});

function thumbnailSpec(t: ThumbnailOptions): ThumbnailSpec {
  const bg: ThumbBg =
    t.bg.type === "color"
      ? { kind: "color", color: hexToRgba(t.bg.color) }
      : t.bg.type === "gradient"
        ? { kind: "gradient", from: hexToRgba(t.bg.from), to: hexToRgba(t.bg.to), angle: t.bg.angle }
        : t.bg.type === "blur"
          ? { kind: "blur", radius: t.bg.blurRadius }
          : t.bg.type === "image"
            ? { kind: "image", path: t.bg.imagePath, blur: t.bg.blurRadius, dim: t.bg.dim }
            : { kind: "transparent" };
  const layers: ThumbLayer[] = (t.layers ?? []).flatMap((l) => {
    if (l.kind === "source") return [{ kind: "source", ...thumbFx(l.img) }];
    if (l.kind === "image")
      return l.path ? [{ kind: "image", path: l.path, fx: thumbFx(l.img) }] : [];
    return l.text.text.trim() === ""
      ? []
      : [
          {
            kind: "text",
            text: l.text.text,
            x: Math.round(l.text.x),
            y: Math.round(l.text.y),
            size: Math.round(l.text.size),
            color: hexToRgba(l.text.color),
            strokeW: Math.round(l.text.strokeW),
            strokeColor: hexToRgba(l.text.strokeColor),
            font: l.text.font,
            rotate: Math.round(l.text.rotate),
            uppercase: l.text.uppercase,
            shadowDx: Math.round(l.text.shadowDx),
            shadowDy: Math.round(l.text.shadowDy),
            shadowBlur: Math.round(l.text.shadowBlur),
            shadowColor: hexToRgba(l.text.shadowColor),
          } as ThumbLayer,
        ];
  });
  return {
    width: Math.round(t.width),
    height: Math.round(t.height),
    bg,
    bgAdjust: {
      brightness: Math.round(t.bgAdjust?.brightness ?? 0),
      contrast: Math.round(t.bgAdjust?.contrast ?? 0),
      hue: Math.round(t.bgAdjust?.hue ?? 0),
      saturation: Math.round(t.bgAdjust?.saturation ?? 0),
    },
    vignette: Math.round(t.vignette ?? 0),
    layers,
  };
}

export const DEFAULT_SPRITE: SpriteOptions = {
  mode: "cut",
  cut: {
    sizing: "rowsCols",
    rows: 4,
    cols: 4,
    tileW: 256,
    tileH: 256,
    edge: "include",
    skipTransparent: false,
    dedupe: false,
    naming: "index",
  },
  pack: {
    name: "sheet",
    cell: "auto",
    cellW: 256,
    cellH: 256,
    padding: 2,
    margin: 0,
    maxW: 0,
    maxH: 0,
    powerOfTwo: false,
    order: "name",
    trim: false,
    dedupe: false,
    metadata: true,
  },
};

export function spriteSpec(o: ModuleOptions): SpriteMode {
  const c = o.sprite.cut;
  const p = o.sprite.pack;
  return o.sprite.mode === "cut"
    ? {
        mode: "cut",
        sizing:
          c.sizing === "rowsCols"
            ? { kind: "rowsCols", rows: c.rows, cols: c.cols }
            : { kind: "tileSize", width: c.tileW, height: c.tileH },
        edge: c.edge,
        skipTransparent: c.skipTransparent,
        dedupe: c.dedupe,
        naming: c.naming,
      }
    : {
        mode: "pack",
        name: p.name.trim() === "" ? "sheet" : p.name.trim(),
        cell:
          p.cell === "fixed"
            ? { kind: "fixed", width: p.cellW, height: p.cellH }
            : { kind: "auto" },
        padding: p.padding,
        margin: p.margin,
        maxSize: p.maxW > 0 && p.maxH > 0 ? [p.maxW, p.maxH] : null,
        powerOfTwo: p.powerOfTwo,
        order: p.order,
        trim: p.trim,
        dedupe: p.dedupe,
        metadata: p.metadata,
      };
}

export const DEFAULT_RENAME: RenameSpec = {
  template: "{name}_{n:03}",
  prefix: "",
  suffix: "",
  find: "",
  replace: "",
  start: 1,
};

/** Overrides chosen in the UI on top of per-module defaults that follow Settings. */
export function resolveOptions(
  module: ModuleId,
  overrides: Partial<ModuleOptions> | undefined,
  settings: Settings,
): ModuleOptions {
  const defaults: ModuleOptions = {
    format: module === "converter" ? settings.defaultFormat : "source",
    quality: settings.defaultQuality,
    lossless: false,
    metadata: "strip",
    targetKb: null,
    compressPreset: "balanced",
    resize: DEFAULT_RESIZE,
    transform: {
      rotate: 0,
      flipHorizontal: false,
      flipVertical: false,
      crop: null,
    },
    presetId: null,
    watermark: DEFAULT_WATERMARK,
    rename: DEFAULT_RENAME,
    sprite: DEFAULT_SPRITE,
    thumbnail: DEFAULT_THUMBNAIL,
    outputDir: settings.defaultOutputDir,
    pipelineUse: {
      transform: false,
      resize: false,
      watermark: false,
      rename: false,
    },
    pipelineResizePresetId: null,
    pipelineWmPresetId: null,
  };
  return { ...defaults, ...overrides };
}

/** Lossy encoders honour quality; "source" may be lossy depending on each file. */
export function usesQuality(format: FormatChoice, lossless: boolean): boolean {
  return (
    format === "jpeg" ||
    format === "avif" ||
    format === "source" ||
    (format === "webp" && !lossless)
  );
}

export function supportsTargetSize(
  format: FormatChoice,
  lossless: boolean,
): boolean {
  return (
    format === "jpeg" || format === "source" || (format === "webp" && !lossless)
  );
}

export function effectiveQuality(module: ModuleId, o: ModuleOptions): number {
  return module === "compressor" && o.compressPreset !== "custom"
    ? COMPRESS_QUALITY[o.compressPreset]
    : o.quality;
}

export function outputMode(module: ModuleId, o: ModuleOptions): OutputMode {
  if (module === "renamer") return { kind: "copyOriginal" };
  return o.format === "source"
    ? { kind: "sameAsSource" }
    : { kind: "convert", format: o.format };
}

export function buildJob(
  module: ModuleId,
  o: ModuleOptions,
  settings: Settings,
  dir: string,
  inputs: string[],
): Job {
  const target =
    module === "compressor" &&
    o.targetKb !== null &&
    o.targetKb > 0 &&
    supportsTargetSize(o.format, o.lossless)
      ? Math.round(o.targetKb * 1024)
      : null;
  return {
    inputs,
    output: {
      dir,
      mode: outputMode(module, o),
      quality: effectiveQuality(module, o),
      lossless: o.lossless,
      metadata: o.metadata,
      targetSize: target,
      conflict: settings.conflictPolicy,
    },
    resize:
      module === "resizer" || (module === "pipeline" && o.pipelineUse.resize)
        ? o.resize
        : null,
    transform:
      module === "cropRotate" ||
      (module === "pipeline" && o.pipelineUse.transform)
        ? o.transform
        : null,
    watermark:
      module === "watermark" || (module === "pipeline" && o.pipelineUse.watermark)
        ? o.watermark
        : null,
    rename:
      module === "renamer" || (module === "pipeline" && o.pipelineUse.rename)
        ? o.rename
        : null,
    sprite: module === "sprites" ? spriteSpec(o) : null,
    thumbnail: module === "thumbnails" ? thumbnailSpec(o.thumbnail) : null,
  };
}

/** Bytes before/after over completed files; percent > 0 means smaller. */
export function savings(
  items: { status: string; size: number; outputSize: number | null }[],
) {
  let before = 0;
  let after = 0;
  for (const i of items) {
    if (i.status !== "done" || i.outputSize === null) continue;
    before += i.size;
    after += i.outputSize;
  }
  return {
    before,
    after,
    percent: before > 0 ? (1 - after / before) * 100 : null,
  };
}
