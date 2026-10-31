import { create } from "zustand";
import { errorMessage, ipc, type AppInfo, type Settings } from "@/lib/ipc";

type Status = "idle" | "loading" | "ready" | "error";

interface SettingsState {
  settings: Settings | null;
  appInfo: AppInfo | null;
  status: Status;
  error: string | null;
  saving: boolean;
  lastSavedAt: number | null;
  load: () => Promise<void>;
  update: (patch: Partial<Settings>) => Promise<void>;
  reset: () => Promise<void>;
}

export const useSettingsStore = create<SettingsState>((set, get) => ({
  settings: null,
  appInfo: null,
  status: "idle",
  error: null,
  saving: false,
  lastSavedAt: null,

  load: async () => {
    set({ status: "loading", error: null });
    try {
      const [settings, appInfo] = await Promise.all([ipc.getSettings(), ipc.appInfo()]);
      set({ settings, appInfo, status: "ready" });
    } catch (e) {
      set({ status: "error", error: errorMessage(e) });
    }
  },

  // Persist first, then commit: the UI only shows what the backend accepted.
  update: async (patch) => {
    const current = get().settings;
    if (!current) return;
    set({ saving: true, error: null });
    try {
      const saved = await ipc.saveSettings({ ...current, ...patch });
      set({ settings: saved, saving: false, lastSavedAt: Date.now() });
    } catch (e) {
      set({ saving: false, error: errorMessage(e) });
    }
  },

  reset: async () => {
    set({ saving: true, error: null });
    try {
      const settings = await ipc.resetSettings();
      set({ settings, saving: false, lastSavedAt: Date.now() });
    } catch (e) {
      set({ saving: false, error: errorMessage(e) });
    }
  },
}));
