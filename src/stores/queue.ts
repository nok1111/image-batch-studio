import { create } from "zustand";
import { errorMessage, ipc, type FileError, type ImageFormat, type FileStatus, type FileWarning, type JobEvent, type JobSummary, type OutputSpec } from "@/lib/ipc";

export type ItemStatus = "pending" | FileStatus;

export interface QueueItem {
  path: string;
  name: string;
  size: number;
  status: ItemStatus;
  output: string | null;
  outputSize: number | null;
  error: FileError | null;
  warning: FileWarning | null;
}

/** Aggregated batch progress shown in the status bar. */
export interface QueueStats {
  total: number;
  /** done + skipped */
  completed: number;
  failed: number;
  /** images per second, null when not processing */
  speed: number | null;
  /** seconds, null when unknown */
  etaSeconds: number | null;
  running: boolean;
}

/** Output choices of the workspace, kept while navigating between pages. */
export interface OutputOptions {
  format: ImageFormat;
  quality: number;
  outputDir: string | null;
}

export interface ScanNotice {
  added: number;
  duplicates: number;
  unsupported: number;
  truncated: boolean;
}

interface QueueState extends QueueStats {
  items: QueueItem[];
  scanning: boolean;
  scanNotice: ScanNotice | null;
  summary: JobSummary | null;
  error: string | null;
  startedAt: number | null;
  cancelling: boolean;
  bytesTotal: number;
  bytesDone: number;
  /** Output folder of the latest batch, for the status bar. */
  lastOutputDir: string | null;
  options: OutputOptions | null;
  setOptions: (options: OutputOptions) => void;
  addPaths: (paths: string[]) => Promise<void>;
  removeItem: (index: number) => void;
  clear: () => void;
  start: (output: OutputSpec) => Promise<void>;
  cancel: () => Promise<void>;
  applyEvent: (event: JobEvent, now?: number) => void;
}

const idle = { completed: 0, failed: 0, speed: null, etaSeconds: null, running: false, summary: null, startedAt: null, cancelling: false, bytesDone: 0 };

export const useQueueStore = create<QueueState>((set, get) => ({
  ...idle,
  total: 0,
  items: [],
  scanning: false,
  scanNotice: null,
  error: null,
  bytesTotal: 0,
  lastOutputDir: null,
  options: null,

  setOptions: (options) => set({ options }),

  addPaths: async (paths) => {
    if (paths.length === 0 || get().running) return;
    set({ scanning: true, error: null });
    try {
      const scan = await ipc.scanInputs(paths);
      const known = new Set(get().items.map((i) => i.path));
      const fresh: QueueItem[] = [];
      for (const f of scan.files) {
        if (known.has(f.path)) continue;
        known.add(f.path);
        fresh.push({ path: f.path, name: f.name, size: f.size, status: "pending", output: null, outputSize: null, error: null, warning: null });
      }
      const items = [...get().items, ...fresh];
      set({
        items,
        total: items.length,
        bytesTotal: items.reduce((n, i) => n + i.size, 0),
        scanning: false,
        scanNotice: {
          added: fresh.length,
          duplicates: scan.files.length - fresh.length,
          unsupported: scan.unsupported,
          truncated: scan.truncated,
        },
      });
    } catch (e) {
      set({ scanning: false, error: errorMessage(e) });
    }
  },

  removeItem: (index) => {
    if (get().running) return;
    const items = get().items.filter((_, i) => i !== index);
    set({ ...idle, items, total: items.length, bytesTotal: items.reduce((n, i) => n + i.size, 0) });
  },

  clear: () => {
    if (get().running) return;
    set({ ...idle, items: [], total: 0, bytesTotal: 0, scanNotice: null, error: null });
  },

  start: async (output) => {
    const { items, running } = get();
    if (running || items.length === 0) return;
    set({
      ...idle,
      running: true,
      startedAt: Date.now(),
      error: null,
      items: items.map((i) => ({ ...i, status: "pending", output: null, outputSize: null, error: null, warning: null })),
    });
    try {
      await ipc.startJob({ inputs: items.map((i) => i.path), output }, (ev) => get().applyEvent(ev));
      set({ lastOutputDir: output.dir });
    } catch (e) {
      set({ running: false, startedAt: null, error: errorMessage(e) });
    }
  },

  cancel: async () => {
    if (!get().running) return;
    set({ cancelling: true });
    try {
      await ipc.cancelJob();
    } catch (e) {
      set({ cancelling: false, error: errorMessage(e) });
    }
  },

  applyEvent: (event, now = Date.now()) => {
    const s = get();
    if (event.type === "aborted") {
      set({ running: false, cancelling: false, speed: null, etaSeconds: null, error: event.message });
      return;
    }
    if (event.type === "finished") {
      set({ running: false, cancelling: false, speed: null, etaSeconds: null, summary: event.summary });
      return;
    }
    const items = s.items.slice();
    let { completed, failed, bytesDone } = s;
    for (const r of event.results) {
      const item = items[r.index];
      if (!item) continue;
      items[r.index] = { ...item, status: r.status, output: r.output, outputSize: r.outputSize, error: r.error, warning: r.warning };
      if (r.status === "done" || r.status === "skipped") completed++;
      if (r.status === "failed") failed++;
      bytesDone += item.size;
    }
    const elapsed = s.startedAt ? (now - s.startedAt) / 1000 : 0;
    const processed = completed + failed;
    const speed = elapsed > 0 ? processed / elapsed : null;
    const bytesPerSecond = elapsed > 0 ? bytesDone / elapsed : 0;
    const etaSeconds = bytesPerSecond > 0 ? Math.max(0, s.bytesTotal - bytesDone) / bytesPerSecond : null;
    set({ items, completed, failed, bytesDone, speed, etaSeconds });
  },
}));
