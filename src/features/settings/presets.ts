import type { ModuleId } from "@/app/modules";
import type { ModuleOptions } from "@/features/workspace/job";
import type { ResizePreset } from "@/lib/ipc";

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
  outputDir: "string|null",
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

/** Keeps only known keys with the right primitive type; engines validate values. */
function sanitizeOptions(raw: unknown): Partial<ModuleOptions> | null {
  if (!raw || typeof raw !== "object" || Array.isArray(raw)) return null;
  const out: Partial<ModuleOptions> = {};
  const src = raw as Record<string, unknown>;
  for (const [key, kind] of Object.entries(OPTION_KEYS)) {
    const v = src[key];
    if (v !== undefined && typeOk(v, kind)) {
      (out as Record<string, unknown>)[key] = v;
    }
  }
  return Object.keys(out).length > 0 ? out : {};
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
          typeof (p as ResizePreset).spec === "object" &&
          (p as ResizePreset).spec !== null &&
          typeof (p as ResizePreset).spec.width === "number" &&
          typeof (p as ResizePreset).spec.height === "number",
      )
    : [];
  return { version: PRESETS_VERSION, moduleOptions, resizePresets };
}
