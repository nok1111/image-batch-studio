import { useRef } from "react";
import { useTranslation } from "react-i18next";
import { useVirtualizer } from "@tanstack/react-virtual";
import { X } from "lucide-react";
import { Badge } from "@/components/ui/badge";
import { formatBytes } from "@/lib/format";
import type { QueueItem } from "@/stores/queue";

const ROW_HEIGHT = 40;

const STATUS_VARIANT = {
  pending: "muted",
  done: "success",
  skipped: "warning",
  failed: "danger",
  cancelled: "muted",
} as const;

/** Virtualized so batches of tens of thousands of files stay smooth. */
export function FileList({ items, running, onRemove }: { items: QueueItem[]; running: boolean; onRemove: (i: number) => void }) {
  const { t } = useTranslation();
  const parentRef = useRef<HTMLDivElement>(null);
  const virtualizer = useVirtualizer({
    count: items.length,
    getScrollElement: () => parentRef.current,
    estimateSize: () => ROW_HEIGHT,
    overscan: 12,
  });

  return (
    <div ref={parentRef} className="h-full overflow-auto" role="list" aria-label={t("workspace.fileList")}>
      <div style={{ height: virtualizer.getTotalSize(), position: "relative" }}>
        {virtualizer.getVirtualItems().map((row) => {
          const item = items[row.index]!;
          const statusLabel =
            item.status === "pending" && running ? t("workspace.status.queued") : t(`workspace.status.${item.status}`);
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
              <span className="w-20 shrink-0 text-right text-xs tabular-nums text-text-secondary">{formatBytes(item.size)}</span>
              <span className="w-20 shrink-0 text-right text-xs tabular-nums text-text-secondary">
                {item.outputSize !== null ? formatBytes(item.outputSize) : ""}
              </span>
              <span className="flex w-28 shrink-0 justify-end" title={item.output ?? undefined}>
                <Badge variant={STATUS_VARIANT[item.status]}>{statusLabel}</Badge>
              </span>
              {item.error ? (
                <span className="w-64 shrink-0 truncate text-xs text-danger" title={`${t(`workspace.errors.${item.error.code}`)}\n${item.error.message}`}>
                  {t(`workspace.errors.${item.error.code}`)}
                </span>
              ) : (
                <span
                  className="w-64 shrink-0 truncate text-xs text-warning"
                  title={item.warning ? t(`workspace.warnings.${item.warning}`) : undefined}
                >
                  {item.warning ? t(`workspace.warnings.${item.warning}`) : ""}
                </span>
              )}
              <button
                type="button"
                disabled={running}
                onClick={() => onRemove(row.index)}
                aria-label={t("workspace.remove", { name: item.name })}
                className="shrink-0 rounded p-1 text-text-secondary hover:bg-card hover:text-text disabled:invisible cursor-pointer"
              >
                <X className="size-3.5" />
              </button>
            </div>
          );
        })}
      </div>
    </div>
  );
}
