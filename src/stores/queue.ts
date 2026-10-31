import { create } from "zustand";

/** Aggregated batch progress shown in the status bar. Fed by the processing engine. */
export interface QueueStats {
  total: number;
  completed: number;
  failed: number;
  /** images per second, null when not processing */
  speed: number | null;
  /** seconds, null when unknown */
  etaSeconds: number | null;
  running: boolean;
}

export const useQueueStore = create<QueueStats>(() => ({
  total: 0,
  completed: 0,
  failed: 0,
  speed: null,
  etaSeconds: null,
  running: false,
}));
