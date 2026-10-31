import { useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import { open } from "@tauri-apps/plugin-dialog";
import { getCurrentWebview } from "@tauri-apps/api/webview";
import { FilePlus2, FolderOpen, FolderPlus, Loader2, Play, Square, Trash2, Upload } from "lucide-react";
import { Button } from "@/components/ui/button";
import { Card } from "@/components/ui/card";
import { Select } from "@/components/ui/select";
import { Slider } from "@/components/ui/slider";
import { formatDuration } from "@/lib/format";
import type { ImageFormat } from "@/lib/ipc";
import { cn } from "@/lib/utils";
import { useQueueStore } from "@/stores/queue";
import { useSettingsStore } from "@/stores/settings";
import { FileList } from "./FileList";

const INPUT_EXTENSIONS = ["png", "jpg", "jpeg", "jpe", "jfif", "webp", "bmp", "tif", "tiff", "gif", "ico"];

/** AVIF output is enabled together with the full Converter. */
const OUTPUT_FORMATS: { value: ImageFormat; label: string }[] = [
  { value: "jpeg", label: "JPG" },
  { value: "png", label: "PNG" },
  { value: "webp", label: "WebP (lossless)" },
  { value: "bmp", label: "BMP" },
  { value: "tiff", label: "TIFF" },
  { value: "gif", label: "GIF" },
  { value: "ico", label: "ICO (≤ 256 px)" },
];

/** Shared input queue + output options + run controls used by tool modules. */
export function Workspace() {
  const { t } = useTranslation();
  const settings = useSettingsStore((s) => s.settings)!;
  const q = useQueueStore();
  const { format, quality, outputDir } = q.options ?? {
    format: settings.defaultFormat === "avif" ? "jpeg" : settings.defaultFormat,
    quality: settings.defaultQuality,
    outputDir: settings.defaultOutputDir,
  };
  const setFormat = (v: ImageFormat) => q.setOptions({ format: v, quality, outputDir });
  const setQuality = (v: number) => q.setOptions({ format, quality: v, outputDir });
  const setOutputDir = (v: string) => q.setOptions({ format, quality, outputDir: v });
  const [dragging, setDragging] = useState(false);

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
            void useQueueStore.getState().addPaths(e.payload.paths);
          }
        })
        .then((fn) => (disposed ? fn() : (unlisten = fn)));
    } catch {
      // Not running inside Tauri (e.g. unit tests).
    }
    return () => {
      disposed = true;
      unlisten?.();
    };
  }, []);

  const addFiles = async () => {
    const picked = await open({ multiple: true, filters: [{ name: t("workspace.images"), extensions: INPUT_EXTENSIONS }] });
    if (picked) await q.addPaths(Array.isArray(picked) ? picked : [picked]);
  };
  const addFolder = async () => {
    const picked = await open({ directory: true, multiple: true });
    if (picked) await q.addPaths(Array.isArray(picked) ? picked : [picked]);
  };
  const chooseOutput = async () => {
    const dir = await open({ directory: true, multiple: false, defaultPath: outputDir ?? undefined });
    if (typeof dir === "string") {
      setOutputDir(dir);
      return dir;
    }
    return null;
  };
  const start = async () => {
    const dir = outputDir ?? (await chooseOutput());
    if (!dir) return;
    await q.start({ dir, format, quality, conflict: settings.conflictPolicy });
  };

  const busy = q.running || q.scanning;
  const s = q.summary;

  return (
    <div className="flex h-full min-h-0 flex-col gap-4 px-8 pb-6">
      <div className="flex flex-wrap items-center gap-2">
        <Button variant="secondary" onClick={() => void addFiles()} disabled={busy}>
          <FilePlus2 />
          {t("workspace.addFiles")}
        </Button>
        <Button variant="secondary" onClick={() => void addFolder()} disabled={busy}>
          <FolderPlus />
          {t("workspace.addFolder")}
        </Button>
        <Button variant="ghost" onClick={q.clear} disabled={busy || q.items.length === 0}>
          <Trash2 />
          {t("workspace.clear")}
        </Button>
        <span className="ml-auto text-xs text-text-secondary" aria-live="polite">
          {q.scanning ? (
            <span className="flex items-center gap-1.5">
              <Loader2 className="size-3.5 animate-spin" />
              {t("workspace.scanning")}
            </span>
          ) : (
            q.scanNotice && (
              <>
                {t("workspace.added", { count: q.scanNotice.added })}
                {q.scanNotice.duplicates > 0 && ` · ${t("workspace.duplicates", { count: q.scanNotice.duplicates })}`}
                {q.scanNotice.unsupported > 0 && ` · ${t("workspace.unsupported", { count: q.scanNotice.unsupported })}`}
                {q.scanNotice.truncated && ` · ${t("workspace.truncated")}`}
              </>
            )
          )}
        </span>
      </div>

      <Card className={cn("relative min-h-64 flex-1 overflow-hidden p-0", dragging && "ring-2 ring-accent")}>
        {q.items.length === 0 ? (
          <div className="flex h-full flex-col items-center justify-center gap-2 p-8 text-center">
            <Upload className="size-8 text-text-secondary/60" />
            <p className="text-sm font-medium text-text">{t("workspace.dropTitle")}</p>
            <p className="text-xs text-text-secondary">{t("workspace.dropHint")}</p>
          </div>
        ) : (
          <FileList items={q.items} running={q.running} onRemove={q.removeItem} />
        )}
        {dragging && (
          <div className="pointer-events-none absolute inset-0 flex items-center justify-center bg-accent/10 text-sm font-medium text-accent">
            {t("workspace.dropActive")}
          </div>
        )}
      </Card>

      <Card className="flex flex-wrap items-end gap-6 px-5 py-4">
        <label className="flex flex-col gap-1.5 text-xs text-text-secondary">
          {t("workspace.format")}
          <Select<ImageFormat> value={format} onValueChange={setFormat} options={OUTPUT_FORMATS} aria-label={t("workspace.format")} />
        </label>
        <div className={cn("flex w-48 flex-col gap-2.5 text-xs text-text-secondary", format !== "jpeg" && "opacity-40")}>
          <span>
            {t("workspace.quality")}: <span className="tabular-nums text-text">{quality}</span>
          </span>
          <Slider value={quality} min={1} max={100} step={1} onValueChange={setQuality} disabled={format !== "jpeg"} aria-label={t("workspace.quality")} />
        </div>
        <div className="flex min-w-0 flex-1 flex-col gap-1.5 text-xs text-text-secondary">
          {t("workspace.outputFolder")}
          <button
            type="button"
            onClick={() => void chooseOutput()}
            disabled={q.running}
            className="flex h-9 min-w-0 items-center gap-2 rounded-lg border border-border bg-bg-secondary px-3 text-left text-sm text-text hover:bg-card-hover disabled:opacity-50 cursor-pointer"
          >
            <FolderOpen className="size-4 shrink-0 text-text-secondary" />
            <span className="truncate">{outputDir ?? t("workspace.noFolder")}</span>
          </button>
        </div>
        {q.running ? (
          <Button variant="danger" size="lg" onClick={() => void q.cancel()} disabled={q.cancelling}>
            {q.cancelling ? <Loader2 className="animate-spin" /> : <Square />}
            {q.cancelling ? t("workspace.cancelling") : t("workspace.cancel")}
          </Button>
        ) : (
          <Button size="lg" onClick={() => void start()} disabled={busy || q.items.length === 0}>
            <Play />
            {t("workspace.start", { count: q.items.length })}
          </Button>
        )}
      </Card>

      {(q.error || s) && (
        <p role="status" className={cn("text-sm", q.error ? "text-danger" : "text-text-secondary")}>
          {q.error
            ? t("workspace.jobError", { message: q.error })
            : t("workspace.summary", {
                done: s!.done,
                skipped: s!.skipped,
                failed: s!.failed,
                cancelled: s!.cancelled,
                time: formatDuration(s!.elapsedMs / 1000),
              })}
        </p>
      )}
    </div>
  );
}
