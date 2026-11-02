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
