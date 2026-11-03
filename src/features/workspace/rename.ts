import type { RenameSpec } from "@/lib/ipc/bindings/RenameSpec";

/**
 * TypeScript port of `RenameSpec::render`/`sanitize` (crates/ibs-core/src/naming.rs).
 * Used for the live filename example in the Renamer panel — the engine still
 * produces the authoritative names via `previewJob`.
 */

type Token =
  | { k: "text"; s: string }
  | { k: "name" }
  | { k: "date" }
  | { k: "n"; w: number };

const MAX_PADDING = 9;

const RESERVED = new Set([
  "CON",
  "PRN",
  "AUX",
  "NUL",
  "COM1",
  "COM2",
  "COM3",
  "COM4",
  "COM5",
  "COM6",
  "COM7",
  "COM8",
  "COM9",
  "LPT1",
  "LPT2",
  "LPT3",
  "LPT4",
  "LPT5",
  "LPT6",
  "LPT7",
  "LPT8",
  "LPT9",
]);

/** Returns the parsed tokens, or the engine-style error string. */
export function parseTemplate(template: string): Token[] | string {
  const out: Token[] = [];
  let rest = template;
  for (;;) {
    const open = rest.indexOf("{");
    if (open === -1) break;
    if (open > 0) out.push({ k: "text", s: rest.slice(0, open) });
    const close = rest.indexOf("}", open);
    if (close === -1)
      return `unclosed \`{\` in template \`${template}\``;
    const tag = rest.slice(open + 1, close);
    if (tag === "name") out.push({ k: "name" });
    else if (tag === "date") out.push({ k: "date" });
    else if (tag === "n") out.push({ k: "n", w: 0 });
    else {
      const m = /^n:(\d{1,2})$/.exec(tag);
      if (!m) return `unknown token \`{${tag}}\``;
      out.push({ k: "n", w: Math.min(Number(m[1]), MAX_PADDING) });
    }
    rest = rest.slice(close + 1);
  }
  if (rest !== "") out.push({ k: "text", s: rest });
  return out;
}

/** Mirrors `RenameSpec::validate`; returns null when the template is usable. */
export function validateTemplate(template: string): string | null {
  if (template.trim() === "") return "rename template is empty";
  const parsed = parseTemplate(template);
  return typeof parsed === "string" ? parsed : null;
}

/** Makes a stem valid on Windows, macOS and Linux (same rules as the engine). */
export function sanitizeStem(stem: string): string {
  let s = "";
  for (const c of stem) {
    const code = c.codePointAt(0)!;
    const isControl = code < 0x20 || (code >= 0x7f && code <= 0x9f);
    s += isControl || '<>:"/\\|?*'.includes(c) ? "_" : c;
  }
  while (s.endsWith(".") || s.endsWith(" ")) s = s.slice(0, -1);
  s = s.replace(/^\s+/, "");
  if (s === "") return "image";
  const base = s.split(".")[0]!.toUpperCase();
  return RESERVED.has(base) ? `_${s}` : s;
}

/**
 * File stem (without extension) for the input at `index`.
 * `date` is the file modified date (YYYY-MM-DD, UTC) substituted into `{date}`.
 */
export function renderRenameStem(
  spec: RenameSpec,
  stem: string,
  index: number,
  date: string,
): string {
  const tokens = parseTemplate(spec.template);
  if (typeof tokens === "string") return sanitizeStem(stem);
  const name =
    spec.find === "" ? stem : stem.split(spec.find).join(spec.replace);
  const n = spec.start + index;
  let out = spec.prefix;
  for (const t of tokens) {
    if (t.k === "text") out += t.s;
    else if (t.k === "name") out += name;
    else if (t.k === "n") out += String(n).padStart(t.w, "0");
    else out += date;
  }
  out += spec.suffix;
  return sanitizeStem(out);
}

/** Strips the last extension ("a.b.png" → "a.b"), like Rust's `file_stem`. */
export function fileStem(name: string): string {
  const dot = name.lastIndexOf(".");
  return dot > 0 ? name.slice(0, dot) : name;
}

/** Today's date in UTC, for the `{date}` token in the example preview. */
export function todayUtc(): string {
  return new Date().toISOString().slice(0, 10);
}
