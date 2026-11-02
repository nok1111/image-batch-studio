import type { ModuleId } from "@/app/modules";
import type { ModuleOptions } from "@/features/workspace/job";
import type { ResizePreset, ResizeSpec } from "@/lib/ipc";

export const PRESETS_VERSION = 1;

export interface PresetsFile {
  version: number;
  moduleOptions: Partial<Record<ModuleId, Partial<ModuleOptions>>>;
  resizePresets: ResizePreset[];
}

const MODULES: ModuleId[] = [
  "converter",
  "compressor",
  "resizer",
  "watermark",
  "renamer",
  "metadata",
  "cropRotate",
  "analyzer",
  "pipeline",
];

const OPTION_KEYS: Record<
  keyof ModuleOptions,
  "number" | "string" | "boolean" | "object" | "number|null" | "string|null"
> = {
  format: "string",
  quality: "number",
  lossless: "boolean",
  metadata: "string",
  targetKb: "number|null",
  compressPreset: "string",
  resize: "object",
  transform: "object",
  presetId: "number|null",
  rename: "object",
  watermark: "object",
  pipelineUse: "object",
  pipelineResizePresetId: "number|null",
  pipelineWmPresetId: "number|null",
  outputDir: "string|null",
};

const RESIZE_MODES = new Set(["fit", "fill", "exact", "width", "height", "percent"]);
const ROTATIONS = new Set([0, 90, 180, 270]);

const num = (v: unknown) => typeof v === "number" && Number.isFinite(v);

function isResizeSpec(v: unknown): v is ResizeSpec {
  const s = v as Partial<ResizeSpec> | null;
  return (
    !!s &&
    typeof s === "object" &&
    typeof s.mode === "string" &&
    RESIZE_MODES.has(s.mode) &&
    num(s.width) &&
    num(s.height) &&
    num(s.percent) &&
    typeof s.allowUpscale === "boolean"
  );
}

function isCropSpec(v: unknown): boolean {
  const s = v as Record<string, unknown> | null;
  return (
    !!s && typeof s === "object" && num(s.x) && num(s.y) && num(s.width) && num(s.height)
  );
}

function isTransformSpec(v: unknown): boolean {
  const s = v as Record<string, unknown> | null;
  return (
    !!s &&
    typeof s === "object" &&
    typeof s.rotate === "number" &&
    ROTATIONS.has(s.rotate) &&
    typeof s.flipHorizontal === "boolean" &&
    typeof s.flipVertical === "boolean" &&
    (s.crop === null || isCropSpec(s.crop))
  );
}

function isRenameSpec(v: unknown): boolean {
  const s = v as Record<string, unknown> | null;
  return (
    !!s &&
    typeof s === "object" &&
    typeof s.template === "string" &&
    typeof s.prefix === "string" &&
    typeof s.suffix === "string" &&
    typeof s.find === "string" &&
    typeof s.replace === "string" &&
    num(s.start)
  );
}

const PIPELINE_STEPS = new Set(["transform", "resize", "watermark", "rename"]);

function isPipelineUse(v: unknown): boolean {
  const s = v as Record<string, unknown> | null;
  return (
    !!s &&
    typeof s === "object" &&
    !Array.isArray(s) &&
    Object.keys(s).every((k) => PIPELINE_STEPS.has(k)) &&
    Object.values(s).every((x) => typeof x === "boolean")
  );
}

/** Spec-valued options need full shape validation, not just "is an object". */
const OBJECT_VALIDATORS: Partial<
  Record<keyof ModuleOptions, (v: unknown) => boolean>
> = {
  resize: isResizeSpec,
  transform: isTransformSpec,
  rename: isRenameSpec,
  pipelineUse: isPipelineUse,
};

function typeOk(v: unknown, kind: string): boolean {
  switch (kind) {
    case "number|null":
      return v === null || typeof v === "number";
    case "string|null":
      return v === null || typeof v === "string";
    case "object":
      return typeof v === "object" && v !== null && !Array.isArray(v);
    default:
      return typeof v === kind;
  }
}

/** Keeps only known keys with the right shape; engines validate values. */
function sanitizeOptions(raw: unknown): Partial<ModuleOptions> | null {
  if (!raw || typeof raw !== "object" || Array.isArray(raw)) return null;
  const out: Partial<ModuleOptions> = {};
  const src = raw as Record<string, unknown>;
  for (const [key, kind] of Object.entries(OPTION_KEYS)) {
    const v = src[key];
    if (v === undefined) continue;
    const validator = OBJECT_VALIDATORS[key as keyof ModuleOptions];
    if (validator ? validator(v) : typeOk(v, kind)) {
      (out as Record<string, unknown>)[key] = v;
    }
  }
  return Object.keys(out).length > 0 ? out : {};
}

/** Two resize specs that configure the same result. */
export function sameResizeSpec(a: ResizeSpec, b: ResizeSpec): boolean {
  return (
    a.mode === b.mode &&
    a.width === b.width &&
    a.height === b.height &&
    a.percent === b.percent &&
    a.allowUpscale === b.allowUpscale
  );
}

/** Builds the portable presets file from live state. */
export function buildPresetsFile(
  moduleOptions: PresetsFile["moduleOptions"],
  resizePresets: ResizePreset[],
): PresetsFile {
  return { version: PRESETS_VERSION, moduleOptions, resizePresets };
}

/** Parses and sanitizes an imported file. Throws on wrong shape or version. */
export function parsePresetsFile(text: string): PresetsFile {
  const data = JSON.parse(text) as Record<string, unknown>;
  if (!data || typeof data !== "object" || data.version !== PRESETS_VERSION) {
    throw new Error("unsupported presets file");
  }
  const moduleOptions: PresetsFile["moduleOptions"] = {};
  if (data.moduleOptions && typeof data.moduleOptions === "object") {
    for (const [mod, raw] of Object.entries(
      data.moduleOptions as Record<string, unknown>,
    )) {
      if (!MODULES.includes(mod as ModuleId)) continue;
      const clean = sanitizeOptions(raw);
      if (clean) moduleOptions[mod as ModuleId] = clean;
    }
  }
  const resizePresets = Array.isArray(data.resizePresets)
    ? data.resizePresets.filter(
        (p): p is ResizePreset =>
          !!p &&
          typeof p === "object" &&
          typeof (p as ResizePreset).name === "string" &&
          isResizeSpec((p as ResizePreset).spec),
      )
    : [];
  return { version: PRESETS_VERSION, moduleOptions, resizePresets };
}
