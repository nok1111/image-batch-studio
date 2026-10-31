import { describe, expect, it } from "vitest";
import { formatDuration } from "./format";

it("formats durations", () => {
  expect(formatDuration(0)).toBe("0:00");
  expect(formatDuration(65)).toBe("1:05");
  expect(formatDuration(3725)).toBe("1:02:05");
  expect(formatDuration(-3)).toBe("0:00");
});

describe("formatBytes", () => {
  it("scales units", async () => {
    const { formatBytes } = await import("./format");
    expect(formatBytes(0)).toBe("0 B");
    expect(formatBytes(1536)).toBe("1.5 KB");
    expect(formatBytes(50 * 1024 * 1024)).toBe("50 MB");
  });
});
