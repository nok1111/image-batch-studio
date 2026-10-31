import { describe, expect, it } from "vitest";
import en from "./locales/en.json";
import es from "./locales/es.json";
import ptBR from "./locales/pt-BR.json";
import { resolveLocale } from "./index";

type Tree = { [k: string]: string | Tree };

function flatten(tree: Tree, prefix = ""): Record<string, string> {
  return Object.entries(tree).reduce<Record<string, string>>((acc, [k, v]) => {
    const key = prefix ? `${prefix}.${k}` : k;
    if (typeof v === "string") acc[key] = v;
    else Object.assign(acc, flatten(v, key));
    return acc;
  }, {});
}

const placeholders = (s: string) => [...s.matchAll(/{{\s*(\w+)\s*}}/g)].map((m) => m[1]).sort();

describe("locales", () => {
  const base = flatten(en);
  for (const [name, locale] of [
    ["es", es],
    ["pt-BR", ptBR],
  ] as const) {
    const flat = flatten(locale);
    it(`${name} has exactly the same keys as en`, () => {
      expect(Object.keys(flat).sort()).toEqual(Object.keys(base).sort());
    });
    it(`${name} keeps the same interpolation variables`, () => {
      for (const [key, value] of Object.entries(base)) {
        expect(placeholders(flat[key] ?? ""), key).toEqual(placeholders(value));
      }
    });
    it(`${name} has no empty strings`, () => {
      for (const [key, value] of Object.entries(flat)) expect(value.trim(), key).not.toBe("");
    });
  }
});

describe("resolveLocale", () => {
  it("uses explicit preference", () => {
    expect(resolveLocale("es", ["en-US"])).toBe("es");
  });
  it("maps system languages", () => {
    expect(resolveLocale("system", ["pt-PT"])).toBe("pt-BR");
    expect(resolveLocale("system", ["es-MX", "en"])).toBe("es");
    expect(resolveLocale("system", ["fr-FR", "en-GB"])).toBe("en");
  });
  it("falls back to English", () => {
    expect(resolveLocale("system", ["de-DE"])).toBe("en");
    expect(resolveLocale("system", [])).toBe("en");
  });
});
