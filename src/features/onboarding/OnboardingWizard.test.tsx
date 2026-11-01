import { beforeEach, describe, expect, it, vi } from "vitest";
import { fireEvent, render, screen } from "@testing-library/react";
import i18n from "@/i18n";
import type { Settings } from "@/lib/ipc";

const invoke = vi.fn();
vi.mock("@tauri-apps/api/core", () => ({ invoke: (...args: unknown[]) => invoke(...args) }));
vi.mock("@tauri-apps/plugin-dialog", () => ({ open: vi.fn(async () => null) }));

const { useSettingsStore } = await import("@/stores/settings");
const { OnboardingWizard } = await import("./OnboardingWizard");

const settings: Settings = {
  theme: "dark",
  language: "system",
  defaultOutputDir: null,
  defaultFormat: "jpeg",
  defaultQuality: 85,
  concurrency: 0,
  conflictPolicy: "rename",
  onboardingDone: false,
};

describe("OnboardingWizard", () => {
  beforeEach(async () => {
    await i18n.changeLanguage("en");
    invoke.mockReset();
    invoke.mockImplementation(async (cmd: string, args: { settings: Settings }) => {
      if (cmd === "save_settings") return args.settings;
      return null;
    });
    useSettingsStore.setState({ settings, status: "ready", error: null, saving: false });
  });

  it("walks the three steps and persists onboardingDone on finish", async () => {
    render(<OnboardingWizard />);
    expect(screen.getByRole("heading", { name: "Welcome to Image Batch Studio" })).toBeInTheDocument();

    fireEvent.click(screen.getByRole("button", { name: /Next/ }));
    expect(screen.getByText("Make it yours")).toBeInTheDocument();

    fireEvent.click(screen.getByRole("button", { name: /Next/ }));
    expect(screen.getByText("Nine tools, one workspace")).toBeInTheDocument();

    fireEvent.click(screen.getByRole("button", { name: /Start working/ }));
    expect(invoke).toHaveBeenCalledWith(
      "save_settings",
      expect.objectContaining({ settings: expect.objectContaining({ onboardingDone: true }) }),
    );
  });

  it("marks onboarding done when skipped", async () => {
    render(<OnboardingWizard />);
    fireEvent.click(screen.getByRole("button", { name: "Skip" }));
    expect(invoke).toHaveBeenCalledWith(
      "save_settings",
      expect.objectContaining({ settings: expect.objectContaining({ onboardingDone: true }) }),
    );
  });
});
