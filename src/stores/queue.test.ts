import { beforeEach, describe, expect, it, vi } from "vitest";
import type { JobEvent, ScanResult } from "@/lib/ipc";

const invoke = vi.fn();
vi.mock("@tauri-apps/api/core", () => ({
  invoke: (...args: unknown[]) => invoke(...args),
  Channel: class {
    onmessage: (e: unknown) => void = () => undefined;
  },
}));

const { useQueueStore } = await import("./queue");

const scan = (names: string[], extra: Partial<ScanResult> = {}): ScanResult => ({
  files: names.map((n) => ({ path: `/in/${n}`, name: n, size: 100, format: "png" })),
  unsupported: 0,
  truncated: false,
  ...extra,
});

const job = {
  output: { dir: "/out", mode: { kind: "convert", format: "jpeg" }, quality: 80, lossless: false, metadata: "strip", targetSize: null, conflict: "rename" },
  resize: null,
  rename: null,
} as const;

describe("queue store", () => {
  beforeEach(() => {
    invoke.mockReset();
    useQueueStore.getState().clear();
    useQueueStore.setState({ running: false, lastOutputDir: null });
    useQueueStore.getState().clear();
  });

  it("keeps output options when the queue is cleared", () => {
    useQueueStore.setState({ options: {} });
    useQueueStore.getState().setOptions("converter", { format: "webp" });
    useQueueStore.getState().setOptions("converter", { quality: 50 });
    useQueueStore.getState().setOptions("compressor", { quality: 30 });
    useQueueStore.getState().clear();
    // outputDir stays unset so it keeps following the Settings default.
    expect(useQueueStore.getState().options).toEqual({ converter: { format: "webp", quality: 50 }, compressor: { quality: 30 } });
  });

  it("adds scanned files and drops duplicates", async () => {
    invoke.mockResolvedValueOnce(scan(["a.png", "b.png"], { unsupported: 2 }));
    await useQueueStore.getState().addPaths(["/in"]);
    invoke.mockResolvedValueOnce(scan(["b.png", "c.png"]));
    await useQueueStore.getState().addPaths(["/in"]);
    const s = useQueueStore.getState();
    expect(s.items.map((i) => i.name)).toEqual(["a.png", "b.png", "c.png"]);
    expect(s.total).toBe(3);
    expect(s.bytesTotal).toBe(300);
    expect(s.scanNotice).toEqual({ added: 1, duplicates: 1, unsupported: 0, truncated: false });
  });

  it("starts a job with every queued path and applies progress", async () => {
    invoke.mockResolvedValueOnce(scan(["a.png", "b.png", "c.png", "d.png"]));
    await useQueueStore.getState().addPaths(["/in"]);
    invoke.mockResolvedValueOnce(undefined);
    await useQueueStore.getState().start(job);
    const [cmd, args] = invoke.mock.calls[invoke.mock.calls.length - 1]!;
    expect(cmd).toBe("start_job");
    expect((args as { job: { inputs: string[] } }).job.inputs).toEqual(["/in/a.png", "/in/b.png", "/in/c.png", "/in/d.png"]);
    expect(useQueueStore.getState().running).toBe(true);

    const startedAt = useQueueStore.getState().startedAt!;
    const progress: JobEvent = {
      type: "progress",
      results: [
        { index: 0, status: "done", output: "/out/a.jpg", outputSize: 50, error: null, warnings: ["firstFrameOnly"] },
        { index: 2, status: "failed", output: null, outputSize: null, error: { code: "decode", message: "bad" }, warnings: [] },
      ],
    };
    useQueueStore.getState().applyEvent(progress, startedAt + 2000);
    const s = useQueueStore.getState();
    expect(s.completed).toBe(1);
    expect(s.failed).toBe(1);
    expect(s.items[2]?.status).toBe("failed");
    expect(s.items[0]?.warnings).toEqual(["firstFrameOnly"]);
    expect(s.lastOutputDir).toBe("/out");
    expect(s.items[1]?.status).toBe("pending");
    expect(s.speed).toBeCloseTo(1);
    // 200 of 400 bytes in 2 s → 2 s remaining
    expect(s.etaSeconds).toBeCloseTo(2);

    useQueueStore.getState().applyEvent({
      type: "finished",
      summary: { total: 4, done: 1, skipped: 0, failed: 1, cancelled: 2, elapsedMs: 2000 },
    });
    expect(useQueueStore.getState().running).toBe(false);
    expect(useQueueStore.getState().summary?.cancelled).toBe(2);
  });

  it("does not change the queue while a batch is running", async () => {
    invoke.mockResolvedValueOnce(scan(["a.png"]));
    await useQueueStore.getState().addPaths(["/in"]);
    invoke.mockResolvedValueOnce(undefined);
    await useQueueStore.getState().start(job);
    useQueueStore.getState().clear();
    useQueueStore.getState().removeItem(0);
    await useQueueStore.getState().addPaths(["/other"]);
    expect(useQueueStore.getState().items).toHaveLength(1);
  });

  it("surfaces a rejected start and stays idle", async () => {
    invoke.mockResolvedValueOnce(scan(["a.png"]));
    await useQueueStore.getState().addPaths(["/in"]);
    invoke.mockRejectedValueOnce({ code: "busy", message: "a batch is already running" });
    await useQueueStore.getState().start(job);
    expect(useQueueStore.getState().running).toBe(false);
    expect(useQueueStore.getState().error).toBe("a batch is already running");
    expect(useQueueStore.getState().lastOutputDir).toBeNull();
  });
});
