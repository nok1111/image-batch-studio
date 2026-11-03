import { useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import type { TFunction } from "i18next";
import { Link, useNavigate } from "react-router-dom";
import {
  ArrowRight,
  Clock,
  Files,
  HardDriveDownload,
  Images,
  Repeat2,
  Trash2,
  Upload,
} from "lucide-react";
import { getCurrentWebview } from "@tauri-apps/api/webview";
import { moduleById, type ModuleId } from "@/app/modules";
import { PageHeader } from "@/components/layout/PageHeader";
import { Card, CardContent } from "@/components/ui/card";
import { Button } from "@/components/ui/button";
import { ipc } from "@/lib/ipc";
import { cn } from "@/lib/utils";
import { useQueueStore } from "@/stores/queue";
import type { ModuleOptions } from "@/features/workspace/job";

interface BatchRow {
  id: number;
  at: number;
  module: string;
  files: number;
  ok: number;
  failed: number;
  bytesIn: number;
  bytesOut: number;
  outputDir: string;
  meta: string;
}

interface BatchStats {
  batches: number;
  files: number;
  ok: number;
  failed: number;
  bytesIn: number;
  bytesOut: number;
  topModule: string | null;
}

interface BatchMeta {
  module?: string;
  options?: Partial<ModuleOptions>;
  inputs?: string[];
  output?: string;
}

const fmtBytes = (n: number) =>
  n >= 1 << 20 ? `${(n / (1 << 20)).toFixed(1)} MB` : `${(n / 1024).toFixed(0)} KB`;

const fmtWhen = (at: number, t: TFunction) => {
  const days = Math.floor((Date.now() / 1000 - at) / 86400);
  if (days <= 0) return t("dash.today");
  if (days === 1) return t("dash.yesterday");
  return t("dash.daysAgo", { n: days });
};

export function DashboardPage() {
  const { t } = useTranslation();
  const navigate = useNavigate();
  const [data, setData] = useState<{ batches: BatchRow[]; stats: BatchStats } | null>(null);
  const [dragging, setDragging] = useState(false);

  const reload = () => ipc.dashboardData().then(setData).catch(() => {});
  useEffect(() => {
    reload();
  }, []);

  // Drag & drop anywhere on the dashboard starts a batch.
  useEffect(() => {
    let unlisten: (() => void) | undefined;
    let disposed = false;
    try {
      void getCurrentWebview()
        .onDragDropEvent((e) => {
          if (e.payload.type === "enter" || e.payload.type === "over") setDragging(true);
          else if (e.payload.type === "leave") setDragging(false);
          else if (e.payload.type === "drop") {
            setDragging(false);
            void useQueueStore.getState().addPaths(e.payload.paths).then(() => navigate("/converter"));
          }
        })
        .then((fn) => (disposed ? fn() : (unlisten = fn)));
    } catch {
      // Outside Tauri (tests/dev browser).
    }
    return () => {
      disposed = true;
      unlisten?.();
    };
  }, [navigate]);

  const batches = data?.batches ?? [];
  const stats = data?.stats;
  const last = batches[0];
  const topModule = stats?.topModule ? moduleById(stats.topModule as ModuleId) : null;
  const saved = Math.max(0, (stats?.bytesIn ?? 0) - (stats?.bytesOut ?? 0));

  const repeat = (b: BatchRow) => {
    let meta: BatchMeta = {};
    try {
      meta = JSON.parse(b.meta || "{}") as BatchMeta;
    } catch {
      /* corrupt meta */
    }
    const moduleId = meta.module || b.module;
    const q = useQueueStore.getState();
    const m = moduleById(moduleId as ModuleId);
    if (m) {
      q.setOptions(m.id, { ...meta.options, outputDir: meta.output } as Partial<ModuleOptions>);
    }
    const p = meta.inputs?.length ? q.addPaths(meta.inputs) : Promise.resolve();
    void p.finally(() => {
      if (m) navigate(m.path);
    });
  };

  const remove = (id: number) => void ipc.deleteBatch(id).then(reload).catch(() => {});

  const moduleName = (id: string) => {
    const m = moduleById(id as ModuleId);
    return m ? t(`nav.${m.id}`) : id;
  };

  const cards = [
      { icon: Files, label: t("dash.statFiles"), value: String(stats?.files ?? 0) },
      {
        icon: HardDriveDownload,
        label: t("dash.statSaved"),
        value: fmtBytes(saved),
      },
      {
        icon: Images,
        label: t("dash.statSuccess"),
        value:
          stats && stats.files > 0
            ? `${Math.round((stats.ok / stats.files) * 100)}%`
            : "—",
      },
      {
        icon: Clock,
        label: t("dash.statTopModule"),
        value: topModule ? moduleName(topModule.id) : "—",
      },
  ];

  return (
    <div className="flex flex-col gap-6">
      <PageHeader title={t("dash.title")} description={t("dash.subtitle")} />

      {/* Drop zone: the fastest way to start a batch. */}
      <div
        className={cn(
          "flex flex-col items-center justify-center gap-2 rounded-xl border-2 border-dashed py-10 text-center transition-colors",
          dragging ? "border-accent bg-accent/10" : "border-border bg-bg-card/40",
        )}
      >
        <Upload className="size-8 text-text-secondary" />
        <p className="text-sm text-text">{t("dash.dropTitle")}</p>
        <p className="text-xs text-text-secondary">{t("dash.dropHint")}</p>
      </div>

      {/* Continue where you left off. */}
      {last && (
        <Card className="border-accent/40">
          <CardContent className="flex items-center gap-4 p-4">
            <div className="min-w-0 flex-1">
              <p className="text-[11px] font-medium uppercase tracking-wide text-text-secondary">
                {t("dash.lastBatch")}
              </p>
              <p className="truncate text-sm font-medium text-text">
                {moduleName(last.module)} · {last.files} {t("dash.files")} ·{" "}
                {fmtWhen(last.at, t)}
              </p>
              <p className="truncate text-[11px] text-text-secondary">{last.outputDir}</p>
            </div>
            <Button size="sm" onClick={() => repeat(last)}>
              <Repeat2 className="mr-1 size-3.5" />
              {t("dash.repeat")}
            </Button>
          </CardContent>
        </Card>
      )}

      {/* Stats. */}
      <div className="grid grid-cols-2 gap-3 md:grid-cols-4">
        {cards.map(({ icon: Icon, label, value }) => (
          <Card key={label}>
            <CardContent className="flex flex-col gap-1 p-4">
              <Icon className="size-4 text-accent" />
              <span className="text-lg font-semibold text-text">{value}</span>
              <span className="text-[11px] text-text-secondary">{label}</span>
            </CardContent>
          </Card>
        ))}
      </div>

      {/* Recent batches. */}
      <section>
        <h2 className="mb-2 text-sm font-medium text-text">{t("dash.recent")}</h2>
        {batches.length === 0 ? (
          <p className="rounded-lg border border-border p-6 text-center text-xs text-text-secondary">
            {t("dash.recentEmpty")}
          </p>
        ) : (
          <div className="flex flex-col gap-1.5">
            {batches.map((b) => (
              <div
                key={b.id}
                className="group flex items-center gap-3 rounded-lg border border-border px-3 py-2"
              >
                <span className="w-36 truncate text-sm text-text">{moduleName(b.module)}</span>
                <span className="text-[11px] text-text-secondary">
                  {b.files} {t("dash.files")} · {b.ok} ✓
                  {b.failed > 0 && <span className="text-red-400"> · {b.failed} ✕</span>}
                </span>
                <span className="min-w-0 flex-1 truncate text-[11px] text-text-secondary">
                  {fmtWhen(b.at, t)}
                </span>
                <button
                  type="button"
                  onClick={() => repeat(b)}
                  className="flex items-center gap-1 rounded px-2 py-1 text-[11px] text-accent opacity-0 transition-opacity hover:bg-accent/10 group-hover:opacity-100"
                >
                  <Repeat2 className="size-3" />
                  {t("dash.repeat")}
                </button>
                <button
                  type="button"
                  onClick={() => remove(b.id)}
                  className="rounded p-1 text-text-secondary opacity-0 transition-opacity hover:text-red-400 group-hover:opacity-100"
                  title={t("dash.delete")}
                >
                  <Trash2 className="size-3.5" />
                </button>
              </div>
            ))}
          </div>
        )}
      </section>

      {batches.length === 0 && (
        <p className="text-[11px] text-text-secondary">
          {t("dash.tip")}{" "}
          <Link to="/converter" className="text-accent hover:underline">
            {t("nav.converter")} <ArrowRight className="inline size-3" />
          </Link>
        </p>
      )}
    </div>
  );
}
