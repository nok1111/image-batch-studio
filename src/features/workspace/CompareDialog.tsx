import { useEffect, useRef, useState, type PointerEvent } from "react";
import { useTranslation } from "react-i18next";
import { X } from "lucide-react";
import { Segmented } from "@/components/ui/segmented";
import { Slider } from "@/components/ui/slider";
import { formatBytes } from "@/lib/format";
import { ipc } from "@/lib/ipc";
import { cn } from "@/lib/utils";
import type { QueueItem } from "@/stores/queue";
import { displayMime } from "./display";

const basename = (p: string) => p.split(/[\\/]/).pop() ?? p;

type Side = {
  url: string | null;
  failed: boolean;
  size: [number, number] | null;
};
const empty: Side = { url: null, failed: false, size: null };

function useImage(path: string): [Side, (s: [number, number] | null) => void] {
  const [side, setSide] = useState<Side>(empty);
  useEffect(() => {
    let url: string | null = null;
    let live = true;
    setSide(empty);
    ipc
      .loadImage(path)
      .then((buf) => {
        url = URL.createObjectURL(new Blob([buf], { type: displayMime(path) }));
        if (live) setSide({ url, failed: false, size: null });
        else URL.revokeObjectURL(url);
      })
      .catch(() => live && setSide({ url: null, failed: true, size: null }));
    return () => {
      live = false;
      if (url) URL.revokeObjectURL(url);
    };
  }, [path]);
  const loaded = (size: [number, number] | null) =>
    setSide((s) => (size ? { ...s, size } : { ...s, failed: true }));
  return [side, loaded];
}

/** Before/after viewer: the result is drawn over the original and clipped at a draggable divider. */
export function CompareDialog({
  item,
  onClose,
  originalFit = "cover",
}: {
  item: QueueItem;
  onClose: () => void;
  /** How the original fills the result's frame: "cover" matches proportional resizes and Fill crops, "fill" matches Exact. */
  originalFit?: "cover" | "fill";
}) {
  const { t } = useTranslation();
  const output = item.output!;
  const [before, setBeforeSize] = useImage(item.path);
  const [after, setAfterSize] = useImage(output);
  const [split, setSplit] = useState(50);
  const [zoom, setZoom] = useState<"fit" | "actual">("fit");
  const stage = useRef<HTMLDivElement>(null);
  const viewport = useRef<HTMLDivElement>(null);
  const [room, setRoom] = useState<[number, number]>([0, 0]);
  const dragging = useRef(false);

  useEffect(() => {
    const el = viewport.current;
    if (!el) return;
    const ro = new ResizeObserver(([e]) => {
      if (e) setRoom([e.contentRect.width, e.contentRect.height]);
    });
    ro.observe(el);
    return () => ro.disconnect();
  }, []);

  useEffect(() => {
    const onKey = (e: KeyboardEvent) => e.key === "Escape" && onClose();
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [onClose]);

  const move = (e: PointerEvent) => {
    const r = stage.current?.getBoundingClientRect();
    if (!r || r.width === 0) return;
    setSplit(
      Math.min(100, Math.max(0, ((e.clientX - r.left) / r.width) * 100)),
    );
  };

  // Both layers share the result's frame, so 100 % shows the result's real pixels.
  const frame = after.size ?? before.size;
  const scale =
    frame && zoom === "fit"
      ? Math.min(1, room[0] / frame[0], room[1] / frame[1])
      : 1;
  const delta =
    item.outputSize !== null && item.size > 0
      ? (item.outputSize / item.size - 1) * 100
      : null;
  const dims = (s: [number, number] | null) => (s ? `${s[0]}×${s[1]}` : "");
  const img = (
    side: Side,
    onSize: (s: [number, number] | null) => void,
    alt: string,
    fit: string,
    clip?: string,
  ) =>
    side.url && (
      <img
        src={side.url}
        alt={alt}
        draggable={false}
        onLoad={(e) =>
          onSize([e.currentTarget.naturalWidth, e.currentTarget.naturalHeight])
        }
        onError={() => onSize(null)}
        className={cn("absolute inset-0 h-full w-full select-none", fit)}
        style={clip ? { clipPath: clip } : undefined}
      />
    );

  return (
    <div
      className="fixed inset-0 z-50 flex items-center justify-center bg-black/70 p-6"
      role="dialog"
      aria-modal="true"
      aria-label={t("compare.title")}
      onClick={(e) => e.target === e.currentTarget && onClose()}
    >
      <div className="flex h-full max-h-[900px] w-full max-w-[1400px] flex-col overflow-hidden rounded-xl border border-border bg-bg-secondary shadow-card">
        <div className="flex items-center gap-4 border-b border-border px-4 py-3 text-sm">
          <div className="min-w-0 flex-1">
            <div className="truncate font-medium text-text" title={item.path}>
              {item.name} → {basename(output)}
            </div>
            <div className="text-xs tabular-nums text-text-secondary">
              {t("compare.original")}: {formatBytes(item.size)}{" "}
              {dims(before.size)} · {t("compare.result")}:{" "}
              {item.outputSize !== null ? formatBytes(item.outputSize) : ""}{" "}
              {dims(after.size)}
              {delta !== null && (
                <span
                  className={cn(
                    "ml-2",
                    delta <= 0 ? "text-success" : "text-warning",
                  )}
                >
                  {delta > 0 ? "+" : "−"}
                  {Math.abs(delta).toFixed(1)}%
                </span>
              )}
            </div>
          </div>
          <Segmented
            value={zoom}
            onValueChange={setZoom}
            aria-label={t("compare.title")}
            options={[
              { value: "fit", label: t("compare.fit") },
              { value: "actual", label: t("compare.actual") },
            ]}
          />
          <button
            type="button"
            onClick={onClose}
            aria-label={t("compare.close")}
            className="rounded p-1.5 text-text-secondary hover:bg-card hover:text-text cursor-pointer"
          >
            <X className="size-4" />
          </button>
        </div>
        <div
          ref={viewport}
          className="relative flex min-h-0 flex-1 overflow-auto bg-bg"
        >
          <div
            ref={stage}
            className="relative m-auto shrink-0 cursor-ew-resize touch-none"
            style={
              frame
                ? { width: frame[0] * scale, height: frame[1] * scale }
                : { width: "100%", height: "100%" }
            }
            onPointerDown={(e) => {
              dragging.current = true;
              e.currentTarget.setPointerCapture(e.pointerId);
              move(e);
            }}
            onPointerMove={(e) => dragging.current && move(e)}
            onPointerUp={() => (dragging.current = false)}
          >
            {img(
              before,
              setBeforeSize,
              t("compare.original"),
              originalFit === "fill" ? "object-fill" : "object-cover",
            )}
            {img(
              after,
              setAfterSize,
              t("compare.result"),
              "object-fill",
              `inset(0 0 0 ${split}%)`,
            )}
            <div
              className="pointer-events-none absolute inset-y-0 w-0.5 bg-accent"
              style={{ left: `${split}%` }}
            />
          </div>
          <span className="pointer-events-none absolute left-3 top-3 rounded bg-black/60 px-2 py-0.5 text-xs text-white">
            {t("compare.original")}
          </span>
          <span className="pointer-events-none absolute right-3 top-3 rounded bg-black/60 px-2 py-0.5 text-xs text-white">
            {t("compare.result")}
          </span>
          {(before.failed || after.failed || !before.url || !after.url) && (
            <div className="pointer-events-none absolute inset-0 flex items-center justify-center text-sm text-text-secondary">
              {before.failed || after.failed
                ? t("compare.unavailable")
                : t("compare.loading")}
            </div>
          )}
        </div>
        <div className="flex items-center gap-4 border-t border-border px-4 py-3">
          <span className="text-xs text-text-secondary">
            {t("compare.hint")}
          </span>
          <div className="flex-1">
            <Slider
              value={split}
              min={0}
              max={100}
              step={1}
              onValueChange={setSplit}
              aria-label={t("compare.split")}
            />
          </div>
        </div>
      </div>
    </div>
  );
}
