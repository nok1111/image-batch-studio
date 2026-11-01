import { describe, expect, it } from "vitest";
import type { Settings } from "@/lib/ipc";
import { buildJob, resolveOptions, savings } from "./job";

const settings = {
  theme: "dark",
  language: "en",
  defaultOutputDir: "/out",
  defaultFormat: "webp",
  defaultQuality: 85,
  concurrency: 0,
  conflictPolicy: "rename",
} as unknown as Settings;

describe("module jobs", () => {
  it("converter follows Settings and only sends its own options", () => {
    const o = resolveOptions("converter", undefined, settings);
    const j = buildJob("converter", { ...o, targetKb: 100 }, settings, "/out", [
      "/a.png",
    ]);
    expect(j.output.mode).toEqual({ kind: "convert", format: "webp" });
    expect(j.output.quality).toBe(85);
    expect(j.output.targetSize).toBeNull();
    expect(j.resize).toBeNull();
    expect(j.rename).toBeNull();
  });

  it("compressor maps presets to quality and target size to bytes", () => {
    const o = resolveOptions(
      "compressor",
      { compressPreset: "high", targetKb: 200 },
      settings,
    );
    const j = buildJob("compressor", o, settings, "/out", []);
    expect(j.output.mode).toEqual({ kind: "sameAsSource" });
    expect(j.output.quality).toBe(60);
    expect(j.output.targetSize).toBe(204800);
    const custom = buildJob(
      "compressor",
      { ...o, compressPreset: "custom", quality: 33, format: "png" },
      settings,
      "/out",
      [],
    );
    expect(custom.output.quality).toBe(33);
    expect(custom.output.targetSize).toBeNull();
  });

  it("resizer and renamer attach their specs; renamer copies originals", () => {
    const r = buildJob(
      "resizer",
      resolveOptions("resizer", undefined, settings),
      settings,
      "/o",
      [],
    );
    expect(r.resize?.mode).toBe("fit");
    const n = buildJob(
      "renamer",
      resolveOptions("renamer", undefined, settings),
      settings,
      "/o",
      [],
    );
    expect(n.output.mode).toEqual({ kind: "copyOriginal" });
    expect(n.rename?.template).toBe("{name}_{n:03}");
    expect(n.resize).toBeNull();
  });

  it("savings use real output sizes of completed files", () => {
    expect(
      savings([
        { status: "done", size: 1000, outputSize: 400 },
        { status: "failed", size: 1000, outputSize: null },
        { status: "done", size: 1000, outputSize: 600 },
      ]),
    ).toEqual({ before: 2000, after: 1000, percent: 50 });
    expect(savings([]).percent).toBeNull();
  });
  it("metadata strips metadata and cropRotate sends its transform", () => {
    const m = buildJob(
      "metadata",
      resolveOptions("metadata", undefined, settings),
      settings,
      "/o",
      [],
    );
    expect(m.output.metadata).toBe("strip");
    expect(m.transform).toBeNull();
    const c = buildJob(
      "cropRotate",
      resolveOptions(
        "cropRotate",
        {
          transform: {
            rotate: 90,
            flipHorizontal: true,
            flipVertical: false,
            crop: { x: 10, y: 20, width: 100, height: 50 },
          },
        },
        settings,
      ),
      settings,
      "/o",
      [],
    );
    expect(c.transform).toEqual({
      rotate: 90,
      flipHorizontal: true,
      flipVertical: false,
      crop: { x: 10, y: 20, width: 100, height: 50 },
    });
    expect(c.resize).toBeNull();
  });
});
