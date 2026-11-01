import { create } from "zustand";
import { errorMessage, ipc, type LicenseStatus } from "@/lib/ipc";

interface LicenseState {
  status: LicenseStatus | null;
  error: string | null;
  busy: boolean;
  load: () => Promise<void>;
  activate: (key: string) => Promise<boolean>;
  deactivate: () => Promise<void>;
}

export const useLicenseStore = create<LicenseState>((set) => ({
  status: null,
  error: null,
  busy: false,
  load: async () => {
    try {
      set({ status: await ipc.licenseStatus() });
    } catch (e) {
      set({ error: errorMessage(e) });
    }
  },
  activate: async (key) => {
    set({ busy: true, error: null });
    try {
      set({ status: await ipc.licenseActivate(key), busy: false });
      return true;
    } catch (e) {
      set({ busy: false, error: errorMessage(e) });
      return false;
    }
  },
  deactivate: async () => {
    set({ busy: true, error: null });
    try {
      set({ status: await ipc.licenseDeactivate(), busy: false });
    } catch (e) {
      set({ busy: false, error: errorMessage(e) });
    }
  },
}));
