import { useEffect, useMemo, useState } from "react";
import { useTranslation } from "react-i18next";
import { ipc, type PackPreview, type PackSpec } from "@/lib/ipc";
import { cn } from "@/lib/utils";
import { displayMime } from "./display";

const GAP = 24; // px between sheets in the preview
const LABEL_MIN_W = 46; // screen px before a cell gets its name
const MAX_THUMBS = 120; // beyond this, sprites render as plain rects

/** Renders the real pack layout (sheets + sprite rects) returned by the engine. */
export function SpritePackPreview({
  spec,
  inputs,
  disabled,
}: {
  spec: PackSpec;
  inputs: string[];
  disabled?: boolean;
}) {
  const { t } = useTranslation();
  const [preview, setPreview] = useState<PackPreview | null>(null);
  const [error, setError] = useState(false);
  const [urls, setUrls] = useState<Record<number, string>>({});
  const specKey = useMemo(() => JSON.stringify(spec), [spec]);
  const inputsKey = useMemo(() => inputs.join("\n"), [inputs]);

  useEffect(() => {
    if (inputs.length === 0) {
      setPreview(null);
      return;
    }
    let live = true;
    setError(false);
    const timer = setTimeout(() => {
      ipc
        .previewSpritePack(spec, inputs)
        .then((p) => live && setPreview(p))
        .catch(() => {
          if (live) {
            setPreview(null);
            setError(true);
          }
        });
    }, 250);
    return () => {
      live = false;
      clearTimeout(timer);
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [specKey, inputsKey]);

  // Thumbnails for the placed sprites (bounded).
  useEffect(() => {
    if (!preview) {
      setUrls({});
      return;
    }
    const indices = new Set<number>();
    for (const s of preview.sheets)
      for (const r of s.sprites) indices.add(r.index);
    if (indices.size > MAX_THUMBS) {
      setUrls({});
      return;
    }
    let live = true;
    const created: string[] = [];
    Promise.all(
      [...indices].map(async (i) => {
        const path = inputs[i];
        if (!path) return null;
        try {
          const buf = await ipc.loadImage(path);
          const url = URL.createObjectURL(
            new Blob([buf], { type: displayMime(path) }),
          );
          created.push(url);
          return [i, url] as const;
        } catch {
          return null;
        }
      }),
    ).then((entries) => {
      if (!live) {
        created.forEach((u) => URL.revokeObjectURL(u));
        return;
      }
      const map: Record<number, string> = {};
      for (const e of entries) if (e) map[e[0]] = e[1];
      setUrls(map);
    });
    return () => {
      live = false;
      created.forEach((u) => URL.revokeObjectURL(u));
      setUrls({});
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [preview]);

  const total = useMemo(
    () => preview?.sheets.reduce((n, s) => n + s.sprites.length, 0) ?? 0,
    [preview],
  );
  // Virtual canvas: sheets laid side by side, scaled to fit the stage.
  const canvas = useMemo(() => {
    if (!preview || preview.sheets.length === 0) return null;
    const w =
      preview.sheets.reduce((n, s) => n + s.width, 0) +
      GAP * (preview.sheets.length - 1);
    const h = Math.max(...preview.sheets.map((s) => s.height));
    return { w, h };
  }, [preview]);

  let offset = 0;

  return (
    <div className="flex w-full flex-col gap-2">
      <div
        className={cn(
          "relative w-full overflow-hidden rounded-lg border border-border bg-bg",
          disabled && "opacity-60",
        )}
        style={{ aspectRatio: "16/10" }}
        role="img"
        aria-label={t("sprite.packPreview")}
      >
        {preview && canvas ? (
          <svg
            className="absolute inset-0 h-full w-full"
            viewBox={`0 0 ${canvas.w} ${canvas.h}`}
            preserveAspectRatio="xMidYMid meet"
          >
            {preview.sheets.map((sheet, si) => {
              const x0 = offset;
              offset += sheet.width + GAP;
              return (
                <g key={si}>
                  <rect
                    x={x0}
                    y={0}
                    width={sheet.width}
                    height={sheet.height}
                    fill="rgba(34,211,238,0.04)"
                    stroke="#22d3ee"
                    strokeWidth={Math.max(1, canvas.w / 800)}
                  />
                  {sheet.sprites.map((r, i) => {
                    const big =
                      (r.w / canvas.w) * 100 >= LABEL_MIN_W / 10;
                    return (
                      <g key={i}>
                        {urls[r.index] && (
                          <image
                            href={urls[r.index]}
                            x={x0 + r.x}
                            y={r.y}
                            width={Math.max(1, r.w)}
                            height={Math.max(1, r.h)}
                            preserveAspectRatio="none"
                          />
                        )}
                        <rect
                          x={x0 + r.x}
                          y={r.y}
                          width={Math.max(1, r.w)}
                          height={Math.max(1, r.h)}
                          fill={
                            urls[r.index] ? "none" : "rgba(34,211,238,0.12)"
                          }
                          stroke="rgba(34,211,238,0.7)"
                          strokeWidth={Math.max(0.5, canvas.w / 1600)}
                        />
                        {big && (
                          <text
                            x={x0 + r.x + r.w / 2}
                            y={r.y + r.h / 2}
                            textAnchor="middle"
                            dominantBaseline="middle"
                            fill="#a5f3fc"
                            fontSize={Math.max(8, canvas.w / 90)}
                            fontFamily="monospace"
                            stroke="rgba(0,0,0,0.7)"
                            strokeWidth={canvas.w / 600}
                            paintOrder="stroke"
                          >
                            {r.name.length > 16
                              ? `${r.name.slice(0, 14)}…`
                              : r.name}
                          </text>
                        )}
                      </g>
                    );
                  })}
                </g>
              );
            })}
          </svg>
        ) : (
          <div className="absolute inset-0 flex items-center justify-center text-xs text-text-secondary">
            {error
              ? t("sprite.previewFailed")
              : inputs.length === 0
                ? t("sprite.previewEmpty")
                : t("sprite.previewing")}
          </div>
        )}
      </div>
      {preview && (
        <p className="text-[11px] text-text-secondary">
          {t("sprite.packPreviewInfo", {
            sheets: preview.sheets.length,
            w: preview.sheets.map((s) => s.width).join(", "),
            h: preview.sheets.map((s) => s.height).join(", "),
            sprites: total,
            cellW: preview.cellW,
            cellH: preview.cellH,
          })}
          {preview.failed > 0 &&
            ` · ${t("sprite.packPreviewFailed", { count: preview.failed })}`}
          {(spec.trim || spec.dedupe) && ` · ${t("sprite.packPreviewNote")}`}
        </p>
      )}
    </div>
  );
}
