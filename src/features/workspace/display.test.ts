import { describe, expect, it } from "vitest";
import { displayMime } from "./display";

describe("displayMime", () => {
  it("maps formats to the bytes load_image returns", () => {
    expect(displayMime("C:\\a\\b.JPG")).toBe("image/jpeg");
    expect(displayMime("/x/y.webp")).toBe("image/webp");
    expect(displayMime("/x/y.avif")).toBe("image/avif");
    expect(displayMime("/x/y.tiff")).toBe("image/png");
    expect(displayMime("/x/y.ico")).toBe("image/png");
    expect(displayMime("/x/y.gif")).toBe("image/png");
  });
});
