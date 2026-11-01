import { useRef } from "react";
import { useTranslation } from "react-i18next";
import { useVirtualizer } from "@tanstack/react-virtual";
import { X } from "lucide-react";
import { Badge } from "@/components/ui/badge";
import { formatBytes } from "@/lib/format";
import { cn } from "@/lib/utils";
import type { QueueItem } from "@/stores/queue";

const ROW_HEIGHT = 36;

const STATUS_VARIANT = {
  pending: "muted",
  done: "success",
  skipped: "warning",
  failed: "danger",
  cancelled: "muted",
} as const;

const bool = (v: boolean, t: (k: string) => string) =>
  v ? t("analyzer.yes") : "—";

/** Analyzer's read-only grid: the facts `analyze_files` gathered per file. */
export function AnalysisTable({
  items,
  running,
  onRemove,
}: {
  items: QueueItem[];
  running: boolean;
  onRemove: (i: number) => void;
}) {
  const { t } = useTranslation();
  const parentRef = useRef<HTMLDivElement>(null);
  const virtualizer = useVirtualizer({
    count: items.length,
    getScrollElement: () => parentRef.current,
    estimateSize: () => ROW_HEIGHT,
    overscan: 12,
  });

  return (
    <div className="flex h-full flex-col">
      <div className="flex items-center gap-3 border-b border-border px-4 py-2 text-[11px] font-medium uppercase tracking-wide text-text-secondary">
        <span className="min-w-0 flex-1">{t("analyzer.colName")}</span>
        <span className="w-20 shrink-0 text-right">{t("analyzer.colDims")}</span>
        <span className="w-14 shrink-0">{t("analyzer.colFormat")}</span>
        <span className="w-16 shrink-0">{t("analyzer.colColor")}</span>
        <span className="w-10 shrink-0 text-center">{t("analyzer.colAlpha")}</span>
        <span className="w-10 shrink-0 text-center">{t("analyzer.colAnim")}</span>
        <span className="w-10 shrink-0 text-center">EXIF</span>
        <span className="w-10 shrink-0 text-center">ICC</span>
        <span className="w-20 shrink-0 text-right">{t("analyzer.colSize")}</span>
        <span className="w-20 shrink-0 text-right">{t("analyzer.colEst")}</span>
        <span className="w-14 shrink-0 text-right">Δ</span>
        <span className="w-20 shrink-0" />
      </div>
      <div ref={parentRef} className="flex-1 overflow-auto" role="list">
        <div style={{ height: virtualizer.getTotalSize(), position: "relative" }}>
          {virtualizer.getVirtualItems().map((row) => {
            const item = items[row.index]!;
            const a = item.analysis;
            const saving =
              a?.estWebpBytes != null && item.size > 0
                ? (1 - a.estWebpBytes / item.size) * 100
                : null;
            return (
              <div
                key={item.path}
                role="listitem"
                className="absolute left-0 top-0 flex w-full items-center gap-3 border-b border-border/60 px-4 text-sm"
                style={{ height: ROW_HEIGHT, transform: `translateY(${row.start}px)` }}
              >
                <span className="min-w-0 flex-1 truncate text-text" title={item.path}>
                  {item.name}
                </span>
                <span className="w-20 shrink-0 text-right text-xs tabular-nums text-text-secondary">
                  {a ? `${a.width}×${a.height}` : ""}
                </span>
                <span className="w-14 shrink-0 text-xs uppercase text-text-secondary">
                  {a?.format ?? ""}
                </span>
                <span className="w-16 shrink-0 text-xs text-text-secondary">{a?.color ?? ""}</span>
                <span className="w-10 shrink-0 text-center text-xs text-text-secondary">
                  {a ? bool(a.hasAlpha, t) : ""}
                </span>
                <span className="w-10 shrink-0 text-center text-xs text-text-secondary">
                  {a ? bool(a.animated, t) : ""}
                </span>
                <span className="w-10 shrink-0 text-center text-xs text-text-secondary">
                  {a ? bool(a.hasExif, t) : ""}
                </span>
                <span className="w-10 shrink-0 text-center text-xs text-text-secondary">
                  {a ? bool(a.hasIcc, t) : ""}
                </span>
                <span className="w-20 shrink-0 text-right text-xs tabular-nums text-text-secondary">
                  {formatBytes(item.size)}
                </span>
                <span className="w-20 shrink-0 text-right text-xs tabular-nums text-text-secondary">
                  {a?.estWebpBytes != null ? formatBytes(a.estWebpBytes) : ""}
                </span>
                <span
                  className={cn(
                    "w-14 shrink-0 text-right text-xs tabular-nums",
                    saving !== null && saving >= 0 ? "text-success" : "text-warning",
                  )}
                >
                  {saving !== null ? `${saving >= 0 ? "−" : "+"}${Math.abs(saving).toFixed(0)}%` : ""}
                </span>
                <span className="flex w-20 shrink-0 items-center justify-end gap-1">
                  <Badge variant={STATUS_VARIANT[item.status]}>
                    {item.status === "pending" && running
                      ? t("workspace.status.queued")
                      : t(`workspace.status.${item.status}`)}
                  </Badge>
                  {!running && (
                    <button
                      type="button"
                      onClick={() => onRemove(row.index)}
                      className="rounded p-1 text-text-secondary hover:bg-card-hover hover:text-text"
                      aria-label={t("workspace.remove")}
                    >
                      <X className="size-3.5" />
                    </button>
                  )}
                </span>
              </div>
            );
          })}
        </div>
      </div>
    </div>
  );
}
