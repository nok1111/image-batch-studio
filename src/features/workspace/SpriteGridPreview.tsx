import { useEffect, useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import { ipc } from "@/lib/ipc";
import { cn } from "@/lib/utils";
import { displayMime } from "./display";
import type { ModuleOptions } from "./job";

interface Grid {
  cols: number;
  rows: number;
  tileW: number;
  tileH: number;
  /** Remainder pixels dropped/padded on the right and bottom. */
  remW: number;
  remH: number;
}

function computeGrid(
  cut: ModuleOptions["sprite"]["cut"],
  w: number,
  h: number,
): Grid | null {
  if (w <= 0 || h <= 0) return null;
  if (cut.sizing === "rowsCols") {
    const cols = Math.max(1, Math.floor(cut.cols));
    const rows = Math.max(1, Math.floor(cut.rows));
    const tileW = cut.edge === "crop" ? Math.floor(w / cols) : Math.ceil(w / cols);
    const tileH = cut.edge === "crop" ? Math.floor(h / rows) : Math.ceil(h / rows);
    if (tileW <= 0 || tileH <= 0) return null;
    return {
      cols,
      rows,
      tileW,
      tileH,
      remW: w - cols * tileW,
      remH: h - rows * tileH,
    };
  }
  const tileW = Math.max(1, Math.floor(cut.tileW));
  const tileH = Math.max(1, Math.floor(cut.tileH));
  return {
    cols: Math.ceil(w / tileW),
    rows: Math.ceil(h / tileH),
    tileW,
    tileH,
    remW: w % tileW === 0 ? 0 : w - Math.floor(w / tileW) * tileW,
    remH: h % tileH === 0 ? 0 : h - Math.floor(h / tileH) * tileH,
  };
}

/** Shows the source image with the cutting grid overlaid. */
export function SpriteGridPreview({
  path,
  cut,
  disabled,
}: {
  path: string;
  cut: ModuleOptions["sprite"]["cut"];
  disabled?: boolean;
}) {
  const { t } = useTranslation();
  const stage = useRef<HTMLDivElement>(null);
  const [nat, setNat] = useState<[number, number] | null>(null);
  const [url, setUrl] = useState<string | null>(null);
  const [failed, setFailed] = useState(false);

  useEffect(() => {
    let objectUrl: string | null = null;
    let live = true;
    setUrl(null);
    setNat(null);
    setFailed(false);
    ipc
      .loadImage(path)
      .then((buf) => {
        objectUrl = URL.createObjectURL(
          new Blob([buf], { type: displayMime(path) }),
        );
        if (live) setUrl(objectUrl);
        else URL.revokeObjectURL(objectUrl);
      })
      .catch(() => live && setFailed(true));
    return () => {
      live = false;
      if (objectUrl) URL.revokeObjectURL(objectUrl);
    };
  }, [path]);

  const grid = nat ? computeGrid(cut, nat[0], nat[1]) : null;

  const box = () => {
    const el = stage.current;
    if (!el || !nat || nat[0] === 0 || nat[1] === 0)
      return { x: 0, y: 0, w: 0, h: 0, s: 1 };
    const s = Math.min(el.clientWidth / nat[0], el.clientHeight / nat[1]);
    return {
      x: (el.clientWidth - nat[0] * s) / 2,
      y: (el.clientHeight - nat[1] * s) / 2,
      w: nat[0] * s,
      h: nat[1] * s,
      s,
    };
  };
  const b = box();
  const tiles = grid ? grid.cols * grid.rows : 0;

  return (
    <div className="flex w-full flex-col gap-2">
      <div
        ref={stage}
        className={cn(
          "relative w-full overflow-hidden rounded-lg border border-border bg-bg select-none",
          disabled && "opacity-60",
        )}
        style={{ aspectRatio: "16/10" }}
        role="img"
        aria-label={t("sprite.preview")}
      >
        {url && (
          <img
            src={url}
            alt=""
            draggable={false}
            onLoad={(e) =>
              setNat([
                e.currentTarget.naturalWidth,
                e.currentTarget.naturalHeight,
              ])
            }
            onError={() => setFailed(true)}
            className="absolute h-full w-full object-contain"
          />
        )}
        {url && grid && nat && b.w > 0 && (
          <svg
            className="pointer-events-none absolute"
            style={{ left: b.x, top: b.y, width: b.w, height: b.h }}
            viewBox={`0 0 ${nat[0]} ${nat[1]}`}
            preserveAspectRatio="none"
          >
            {/* discarded remainder (crop edge) */}
            {cut.edge === "crop" && (
              <>
                {grid.remW < 0 || grid.tileW * grid.cols < nat[0] ? (
                  <rect
                    x={grid.tileW * grid.cols}
                    y={0}
                    width={Math.max(0, nat[0] - grid.tileW * grid.cols)}
                    height={nat[1]}
                    fill="rgba(0,0,0,0.55)"
                  />
                ) : null}
                {grid.tileH * grid.rows < nat[1] ? (
                  <rect
                    x={0}
                    y={grid.tileH * grid.rows}
                    width={grid.tileW * grid.cols}
                    height={Math.max(0, nat[1] - grid.tileH * grid.rows)}
                    fill="rgba(0,0,0,0.55)"
                  />
                ) : null}
              </>
            )}
            {/* vertical lines */}
            {Array.from({ length: grid.cols - 1 }, (_, i) => (
              <line
                key={`v${i}`}
                x1={(i + 1) * grid.tileW}
                y1={0}
                x2={(i + 1) * grid.tileW}
                y2={nat[1]}
                stroke="#22d3ee"
                strokeWidth={Math.max(1, nat[0] / 400)}
                strokeDasharray={`${Math.max(4, nat[1] / 60)} ${Math.max(3, nat[1] / 90)}`}
                opacity={0.9}
              />
            ))}
            {/* horizontal lines */}
            {Array.from({ length: grid.rows - 1 }, (_, i) => (
              <line
                key={`h${i}`}
                x1={0}
                y1={(i + 1) * grid.tileH}
                x2={nat[0]}
                y2={(i + 1) * grid.tileH}
                stroke="#22d3ee"
                strokeWidth={Math.max(1, nat[0] / 400)}
                strokeDasharray={`${Math.max(4, nat[0] / 60)} ${Math.max(3, nat[0] / 90)}`}
                opacity={0.9}
              />
            ))}
            {/* padded area (pad edge) */}
            {cut.edge === "pad" && (
              <>
                {grid.tileW * grid.cols > nat[0] && (
                  <rect
                    x={nat[0]}
                    y={0}
                    width={grid.tileW * grid.cols - nat[0]}
                    height={nat[1]}
                    fill="none"
                  />
                )}
                {grid.tileW * grid.cols > nat[0] && (
                  <rect
                    x={(grid.cols - 1) * grid.tileW}
                    y={0}
                    width={grid.tileW}
                    height={nat[1]}
                    fill="none"
                    stroke="#f59e0b"
                    strokeWidth={Math.max(1, nat[0] / 500)}
                  />
                )}
                {grid.tileH * grid.rows > nat[1] && (
                  <rect
                    x={0}
                    y={(grid.rows - 1) * grid.tileH}
                    width={Math.min(nat[0], grid.tileW * grid.cols)}
                    height={grid.tileH}
                    fill="none"
                    stroke="#f59e0b"
                    strokeWidth={Math.max(1, nat[0] / 500)}
                  />
                )}
              </>
            )}
          </svg>
        )}
        {failed && (
          <div className="absolute inset-0 flex items-center justify-center text-xs text-text-secondary">
            {t("sprite.previewFailed")}
          </div>
        )}
      </div>
      {grid && nat && (
        <p className="text-[11px] text-text-secondary">
          {t("sprite.previewInfo", {
            tiles,
            cols: grid.cols,
            rows: grid.rows,
            w: Math.min(grid.tileW, nat[0]),
            h: Math.min(grid.tileH, nat[1]),
          })}
          {cut.edge === "crop" &&
            (grid.tileW * grid.cols < nat[0] ||
              grid.tileH * grid.rows < nat[1]) &&
            ` · ${t("sprite.previewCropNote")}`}
          {cut.edge === "pad" &&
            (grid.tileW * grid.cols > nat[0] ||
              grid.tileH * grid.rows > nat[1]) &&
            ` · ${t("sprite.previewPadNote")}`}
        </p>
      )}
    </div>
  );
}
