import { describe, expect, it } from "vitest";
import {
  fileStem,
  renderRenameStem,
  sanitizeStem,
  validateTemplate,
} from "./rename";
import type { RenameSpec } from "@/lib/ipc/bindings/RenameSpec";

const spec = (template: string): RenameSpec => ({
  template,
  prefix: "",
  suffix: "",
  find: "",
  replace: "",
  start: 1,
});

// Mirrors the engine tests in crates/ibs-core/src/naming.rs.
describe("renderRenameStem", () => {
  it("renders tokens", () => {
    const s: RenameSpec = {
      ...spec("{name}_{n:03}_{date}"),
      prefix: "web_",
      suffix: "_final",
      find: "IMG",
      replace: "photo",
      start: 7,
    };
    expect(renderRenameStem(s, "IMG_42", 2, "2023-11-14")).toBe(
      "web_photo_42_009_2023-11-14_final",
    );
    expect(renderRenameStem(spec("{n}"), "x", 0, "")).toBe("1");
    expect(renderRenameStem(spec("a{date}b"), "x", 0, "")).toBe("ab");
  });

  it("rejects bad templates", () => {
    expect(validateTemplate("{nme}")).not.toBeNull();
    expect(validateTemplate("{name")).not.toBeNull();
    expect(validateTemplate("  ")).not.toBeNull();
    expect(validateTemplate("{n:123}")).not.toBeNull();
    expect(validateTemplate("{name}-{n:4}")).toBeNull();
  });

  it("falls back to the sanitized stem on a bad template", () => {
    expect(renderRenameStem(spec("{bogus"), "My File", 0, "")).toBe("My File");
  });
});

describe("sanitizeStem", () => {
  it("sanitizes for every OS", () => {
    expect(sanitizeStem("a/b\\c:d*?")).toBe("a_b_c_d__");
    expect(sanitizeStem("name. . ")).toBe("name");
    expect(sanitizeStem("con")).toBe("_con");
    expect(sanitizeStem("")).toBe("image");
    expect(sanitizeStem("...")).toBe("image");
  });
});

describe("fileStem", () => {
  it("strips the last extension", () => {
    expect(fileStem("a.png")).toBe("a");
    expect(fileStem("a.b.png")).toBe("a.b");
    expect(fileStem("noext")).toBe("noext");
  });
});
