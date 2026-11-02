import { beforeEach, describe, expect, it, vi } from "vitest";
import type { Settings } from "@/lib/ipc";

const invoke = vi.fn();
vi.mock("@tauri-apps/api/core", () => ({ invoke: (...args: unknown[]) => invoke(...args) }));

const { useSettingsStore } = await import("./settings");

const defaults: Settings = {
  theme: "dark",
  language: "system",
  defaultOutputDir: null,
  defaultFormat: "jpeg",
  defaultQuality: 85,
  concurrency: 0,
  conflictPolicy: "rename",
  onboardingDone: true,
};
const info = { name: "x", version: "0.1.0", tauriVersion: "2", os: "windows", arch: "x86_64", logicalCpus: 8 };

describe("settings store", () => {
  beforeEach(() => {
    invoke.mockReset();
    useSettingsStore.setState({ settings: null, status: "idle", error: null, saving: false });
  });

  it("loads settings and app info", async () => {
    invoke.mockImplementation((cmd: string) =>
      Promise.resolve(cmd === "get_settings" ? defaults : info),
    );
    await useSettingsStore.getState().load();
    const s = useSettingsStore.getState();
    expect(s.status).toBe("ready");
    expect(s.settings).toEqual(defaults);
    expect(s.appInfo?.logicalCpus).toBe(8);
  });

  it("sends merged settings and commits what the backend returns", async () => {
    useSettingsStore.setState({ settings: defaults, status: "ready" });
    invoke.mockImplementation((_cmd: string, args: { settings: Settings }) => Promise.resolve(args.settings));
    await useSettingsStore.getState().update({ defaultQuality: 60 });
    expect(invoke).toHaveBeenCalledWith("save_settings", { settings: { ...defaults, defaultQuality: 60 } });
    expect(useSettingsStore.getState().settings?.defaultQuality).toBe(60);
  });

  it("keeps previous settings when the backend rejects the change", async () => {
    useSettingsStore.setState({ settings: defaults, status: "ready" });
    invoke.mockRejectedValue({ code: "invalid_setting", message: "must be between 1 and 100" });
    await useSettingsStore.getState().update({ defaultQuality: 0 });
    const s = useSettingsStore.getState();
    expect(s.settings).toEqual(defaults);
    expect(s.error).toBe("must be between 1 and 100");
    expect(s.saving).toBe(false);
  });

  it("reports load errors", async () => {
    invoke.mockRejectedValue({ code: "storage", message: "disk full" });
    await useSettingsStore.getState().load();
    expect(useSettingsStore.getState().status).toBe("error");
    expect(useSettingsStore.getState().error).toBe("disk full");
  });
});
