import { expect, it } from "vitest";
import { formatDuration } from "./format";

it("formats durations", () => {
  expect(formatDuration(0)).toBe("0:00");
  expect(formatDuration(65)).toBe("1:05");
  expect(formatDuration(3725)).toBe("1:02:05");
  expect(formatDuration(-3)).toBe("0:00");
});
