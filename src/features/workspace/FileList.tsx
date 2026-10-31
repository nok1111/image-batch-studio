import { useRef } from "react";
import { useTranslation } from "react-i18next";
import { useVirtualizer } from "@tanstack/react-virtual";
import { X } from "lucide-react";
import { Badge } from "@/components/ui/badge";
import { formatBytes } from "@/lib/format";
import type { PlannedOutput } from "@/lib/ipc";
import { cn } from "@/lib/utils";
import type { QueueItem } from "@/stores/queue";

const basename = (p: string) => p.split(/[\\/]/).pop() ?? p;

const ROW_HEIGHT = 40;

const STATUS_VARIANT = {
  pending: "muted",
  done: "success",
  skipped: "warning",
  failed: "danger",
  cancelled: "muted",
} as const;

/** Virtualized so batches of tens of thousands of files stay smooth. */
export function FileList({ items, running, onRemove, previews, renamed = false }: { items: QueueItem[]; running: boolean; onRemove: (i: number) => void; previews?: PlannedOutput[] | null; renamed?: boolean }) {
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
          const plan = previews?.[row.index];
          const warnings = item.warnings.map((w) => t(`workspace.warnings.${w}`)).join(" · ");
          const delta = item.outputSize !== null && item.size > 0 ? (item.outputSize / item.size - 1) * 100 : null;
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
              <span className={cn("w-14 shrink-0 text-right text-xs tabular-nums", delta !== null && delta <= 0 ? "text-success" : "text-warning")}>
                {delta !== null && item.status === "done" ? `${delta > 0 ? "+" : "−"}${Math.abs(delta).toFixed(0)}%` : ""}
              </span>
              {previews !== undefined &&
                (item.output && item.status === "done" && renamed ? (
                  <span className="w-56 shrink-0 truncate text-xs text-text" title={item.output}>
                    → {basename(item.output)}
                  </span>
                ) : (
                  <span
                    className={cn("w-56 shrink-0 truncate text-xs", plan?.renumbered || plan?.action === "skip" || plan?.action === "replace" ? "text-warning" : plan?.error ? "text-danger" : "text-text")}
                    title={plan?.target ?? (plan?.error ? `${t(`workspace.errors.${plan.error.code}`)}\n${plan.error.message}` : undefined)}
                  >
                    {plan?.target ? `→ ${basename(plan.target)}${plan.action === "skip" ? ` (${t("workspace.status.skipped")})` : ""}` : plan?.error ? t(`workspace.errors.${plan.error.code}`) : ""}
                  </span>
                ))}
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
                  title={warnings || undefined}
                >
                  {warnings}
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
