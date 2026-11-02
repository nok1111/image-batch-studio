import { describe, expect, it } from "vitest";
import { aggregateAnalysis } from "./analyze";
import type { FileAnalysis } from "@/lib/ipc";

const analysis = (over: Partial<FileAnalysis> = {}): FileAnalysis => ({
  width: 100,
  height: 50,
  format: "png",
  color: "rgba8",
  hasAlpha: true,
  animated: false,
  hasExif: false,
  hasIcc: false,
  estWebpBytes: 40,
  ...over,
});

describe("aggregateAnalysis", () => {
  it("sums analyzed files and estimates savings", () => {
    const stats = aggregateAnalysis([
      { size: 100, status: "done", analysis: analysis() },
      {
        size: 200,
        status: "done",
        analysis: analysis({ format: "jpeg", hasAlpha: false, animated: true, hasExif: true, estWebpBytes: 40 }),
      },
    ]);
    expect(stats.analyzed).toBe(2);
    expect(stats.failed).toBe(0);
    expect(stats.totalBytes).toBe(300);
    expect(stats.estBytes).toBe(80);
    expect(stats.savingPercent).toBeCloseTo(73.33, 1);
    expect(stats.formats).toEqual({ png: 1, jpeg: 1 });
    expect(stats.withAlpha).toBe(1);
    expect(stats.animated).toBe(1);
    expect(stats.withExif).toBe(1);
  });

  it("counts failures and skips unanalyzed files", () => {
    const stats = aggregateAnalysis([
      { size: 10, status: "failed", analysis: null },
      { size: 10, status: "pending", analysis: null },
      { size: 10, status: "done", analysis: analysis({ estWebpBytes: null }) },
    ]);
    expect(stats.analyzed).toBe(1);
    expect(stats.failed).toBe(1);
    expect(stats.savingPercent).toBeNull();
  });

  it("handles an empty queue", () => {
    const stats = aggregateAnalysis([]);
    expect(stats.analyzed).toBe(0);
    expect(stats.savingPercent).toBeNull();
    expect(stats.formats).toEqual({});
  });
});
