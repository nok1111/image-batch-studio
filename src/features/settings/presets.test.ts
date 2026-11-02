import { describe, expect, it } from "vitest";
import {
  buildPresetsFile,
  parsePresetsFile,
  PRESETS_VERSION,
} from "./presets";

const spec = {
  mode: "fit" as const,
  width: 1920,
  height: 1080,
  percent: 50,
  allowUpscale: false,
};

describe("presets file", () => {
  it("round-trips module options and resize presets", () => {
    const file = buildPresetsFile(
      {
        converter: { format: "webp", quality: 81 },
        resizer: { presetId: 3, resize: spec },
      },
      [{ id: 3, name: "Web", spec }],
    );
    const parsed = parsePresetsFile(JSON.stringify(file));
    expect(parsed.moduleOptions.converter?.format).toBe("webp");
    expect(parsed.moduleOptions.converter?.quality).toBe(81);
    expect(parsed.moduleOptions.resizer?.resize).toEqual(spec);
    expect(parsed.resizePresets).toHaveLength(1);
    expect(parsed.resizePresets[0]!.name).toBe("Web");
  });

  it("rejects bad JSON, wrong versions and unknown modules/keys", () => {
    expect(() => parsePresetsFile("not json")).toThrow();
    expect(() =>
      parsePresetsFile(JSON.stringify({ version: 99 })),
    ).toThrow("unsupported");
    const parsed = parsePresetsFile(
      JSON.stringify({
        version: PRESETS_VERSION,
        moduleOptions: {
          notAModule: { format: "png" },
          converter: { format: "png", evil: 1, quality: "high" },
          resizer: { resize: {}, transform: { rotate: 45 } },
        },
        resizePresets: [
          { name: "no spec" },
          null,
          { name: "partial spec", spec: { width: 10 } },
          { name: "Ok", spec },
        ],
      }),
    );
    expect(parsed.moduleOptions["notAModule" as never]).toBeUndefined();
    // Wrong-typed fields are dropped, valid ones kept.
    expect(parsed.moduleOptions.converter).toEqual({ format: "png" });
    // Malformed spec objects are dropped wholesale, not shallow-merged.
    expect(parsed.moduleOptions.resizer).toEqual({});
    expect(parsed.resizePresets.map((p) => p.name)).toEqual(["Ok"]);
  });

  it("accepts only strict pipelineUse step flags", () => {
    const parsed = parsePresetsFile(
      JSON.stringify({
        version: PRESETS_VERSION,
        moduleOptions: {
          pipeline: {
            pipelineUse: { resize: "false", rename: true, bogus: true },
          },
        },
      }),
    );
    // Non-boolean values and unknown steps reject the whole object.
    expect(parsed.moduleOptions.pipeline).toEqual({});
    const ok = parsePresetsFile(
      JSON.stringify({
        version: PRESETS_VERSION,
        moduleOptions: {
          pipeline: { pipelineUse: { resize: true, watermark: false } },
        },
      }),
    );
    expect(ok.moduleOptions.pipeline).toEqual({
      pipelineUse: { resize: true, watermark: false },
    });
  });

  it("tolerates missing sections", () => {
    const parsed = parsePresetsFile(JSON.stringify({ version: PRESETS_VERSION }));
    expect(parsed.moduleOptions).toEqual({});
    expect(parsed.resizePresets).toEqual([]);
  });
});
