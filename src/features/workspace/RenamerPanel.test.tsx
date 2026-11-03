import { beforeEach, describe, expect, it, vi } from "vitest";
import { fireEvent, render, screen } from "@testing-library/react";
import i18n from "@/i18n";
import { resolveOptions } from "./job";
import type { QueueItem } from "@/stores/queue";
import type { Settings } from "@/lib/ipc";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn(async () => null) }));
vi.mock("@tauri-apps/plugin-dialog", () => ({ open: vi.fn(async () => null) }));

const { RenamerPanel } = await import("./OptionPanels");

const settings: Settings = {
  theme: "dark",
  language: "system",
  defaultOutputDir: null,
  defaultFormat: "jpeg",
  defaultQuality: 85,
  concurrency: 0,
  conflictPolicy: "rename",
  onboardingDone: true,
};

const item = (name: string): QueueItem => ({
  path: `C:/in/${name}`,
  name,
  size: 100,
  status: "pending",
  output: null,
  outputSize: null,
  error: null,
  warnings: [],
  analysis: null,
});

const options = () => resolveOptions("renamer", undefined, settings);

describe("RenamerPanel", () => {
  beforeEach(async () => {
    await i18n.changeLanguage("en");
  });

  it("shows a live example computed from the first queued file", () => {
    const o = options();
    o.rename.template = "{name}_{n:03}";
    render(
      <RenamerPanel
        module="renamer"
        o={o}
        set={() => {}}
        disabled={false}
        items={[item("IMG_2041.jpg")]}
      />,
    );
    expect(screen.getByText("Result preview")).toBeInTheDocument();
    expect(screen.getByText("IMG_2041_001.jpg")).toBeInTheDocument();
  });

  it("inserts a token at the caret when a chip is clicked", () => {
    const o = options();
    o.rename.template = "{name}";
    const set = vi.fn();
    render(
      <RenamerPanel
        module="renamer"
        o={o}
        set={set}
        disabled={false}
        items={[]}
      />,
    );
    fireEvent.click(screen.getByRole("button", { name: "{date}" }));
    expect(set).toHaveBeenCalledWith({
      rename: { ...o.rename, template: "{name}{date}" },
    });
  });

  it("shows the template error instead of the example", () => {
    const o = options();
    o.rename.template = "{bogus";
    render(
      <RenamerPanel
        module="renamer"
        o={o}
        set={() => {}}
        disabled={false}
        items={[item("a.png")]}
      />,
    );
    expect(screen.getByText(/Cannot preview/)).toBeInTheDocument();
    expect(screen.queryByText("a_001.png")).not.toBeInTheDocument();
  });
});
