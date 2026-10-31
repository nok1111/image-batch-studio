import { useTranslation } from "react-i18next";
import { CircleCheck, CircleX, Files, FolderOpen, Gauge, Timer } from "lucide-react";
import { useQueueStore } from "@/stores/queue";
import { useSettingsStore } from "@/stores/settings";
import { formatDuration } from "@/lib/format";
import { ipc } from "@/lib/ipc";

export function StatusBar() {
  const { t } = useTranslation();
  const q = useQueueStore();
  const outputDir = useSettingsStore((s) => s.settings?.defaultOutputDir ?? null);

  return (
    <footer className="flex h-9 shrink-0 items-center gap-5 border-t border-border bg-bg-secondary px-4 text-xs text-text-secondary">
      <span className="flex items-center gap-1.5">
        <span className={q.running ? "size-2 rounded-full bg-accent-2" : "size-2 rounded-full bg-text-secondary/50"} />
        {t("status.ready")}
      </span>
      <Stat icon={Files} label={t("status.total")} value={q.total} />
      <Stat icon={CircleCheck} label={t("status.completed")} value={q.completed} />
      <Stat icon={CircleX} label={t("status.failed")} value={q.failed} />
      <span className="flex items-center gap-1.5">
        <Gauge className="size-3.5" />
        {q.speed === null ? "—" : t("status.speed", { value: q.speed.toFixed(1) })}
      </span>
      <span className="flex items-center gap-1.5">
        <Timer className="size-3.5" />
        {t("status.eta")}: {q.etaSeconds === null ? "—" : formatDuration(q.etaSeconds)}
      </span>
      <button
        type="button"
        disabled={!outputDir}
        onClick={() => void ipc.openOutputDir().catch(() => undefined)}
        title={outputDir ?? t("status.noOutput")}
        className="ml-auto flex max-w-80 items-center gap-1.5 truncate rounded px-2 py-1 hover:bg-card hover:text-text disabled:cursor-default disabled:opacity-50 disabled:hover:bg-transparent cursor-pointer"
      >
        <FolderOpen className="size-3.5 shrink-0" />
        <span className="truncate">{outputDir ? t("status.openOutput") : t("status.noOutput")}</span>
      </button>
    </footer>
  );
}

function Stat({ icon: Icon, label, value }: { icon: typeof Files; label: string; value: number }) {
  return (
    <span className="flex items-center gap-1.5">
      <Icon className="size-3.5" />
      {label}: <span className="tabular-nums text-text">{value.toLocaleString()}</span>
    </span>
  );
}
