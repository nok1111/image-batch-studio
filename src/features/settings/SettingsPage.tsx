import { useEffect, useState, type ReactNode } from "react";
import { useTranslation } from "react-i18next";
import { open } from "@tauri-apps/plugin-dialog";
import { Check, Download, FolderOpen, Info, Lock, RotateCcw, Upload, X } from "lucide-react";
import { PageHeader } from "@/components/layout/PageHeader";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { Select } from "@/components/ui/select";
import { Segmented } from "@/components/ui/segmented";
import { Slider } from "@/components/ui/slider";
import { LOCALE_NATIVE_NAMES, SUPPORTED_LOCALES } from "@/i18n";
import { BUY_URL } from "@/lib/ipc";
import type { ConflictPolicy, ImageFormat, Language, Theme } from "@/lib/ipc";
import { useLicenseStore } from "@/stores/license";
import { errorMessage, ipc } from "@/lib/ipc";
import { useQueueStore } from "@/stores/queue";
import { useSettingsStore } from "@/stores/settings";
import { buildPresetsFile, parsePresetsFile, sameResizeSpec } from "./presets";

const FORMATS: { value: ImageFormat; label: string }[] = [
  { value: "jpeg", label: "JPG" },
  { value: "png", label: "PNG" },
  { value: "webp", label: "WebP" },
  { value: "avif", label: "AVIF" },
  { value: "bmp", label: "BMP" },
  { value: "tiff", label: "TIFF" },
  { value: "gif", label: "GIF" },
  { value: "ico", label: "ICO" },
];

const MAX_CONCURRENCY = 64;

export function SettingsPage() {
  const { t } = useTranslation();
  const { settings, appInfo, update, reset, saving, error, lastSavedAt } = useSettingsStore();
  const [quality, setQuality] = useState(settings?.defaultQuality ?? 85);
  const [concurrency, setConcurrency] = useState(settings?.concurrency ?? 0);
  const [presetsMsg, setPresetsMsg] = useState<{ ok: boolean; text: string } | null>(null);

  useEffect(() => {
    if (settings) {
      setQuality(settings.defaultQuality);
      setConcurrency(settings.concurrency);
    }
  }, [settings]);

  if (!settings) return null;

  const cpus = appInfo?.logicalCpus ?? 1;
  const concurrencyMax = Math.min(MAX_CONCURRENCY, Math.max(cpus * 2, 4));

  const pickOutputDir = async () => {
    const dir = await open({ directory: true, multiple: false, defaultPath: settings.defaultOutputDir ?? undefined });
    if (typeof dir === "string") await update({ defaultOutputDir: dir });
  };

  const exportPresets = async () => {
    setPresetsMsg(null);
    try {
      const resizePresets = await ipc.listResizePresets();
      const file = buildPresetsFile(useQueueStore.getState().options, resizePresets);
      // The command opens the save dialog itself; null = user cancelled.
      const path = await ipc.exportPresets(JSON.stringify(file, null, 2));
      if (path === null) return;
      setPresetsMsg({ ok: true, text: t("settings.presetsExported") });
    } catch (e) {
      setPresetsMsg({ ok: false, text: errorMessage(e) });
    }
  };

  const importPresets = async () => {
    setPresetsMsg(null);
    try {
      // The command opens the picker itself; null = user cancelled.
      const contents = await ipc.importPresets();
      if (contents === null) return;
      const file = parsePresetsFile(contents);
      const q = useQueueStore.getState();
      // Save resize presets first, skipping entries already present (name +
      // identical spec, e.g. re-importing the seeded presets), and remember
      // exported id -> new id so option presetIds can be remapped.
      const existing = await ipc.listResizePresets();
      const idMap = new Map<number, number>();
      let saved = 0;
      let failed = 0;
      for (const preset of file.resizePresets) {
        const dupe = existing.find(
          (p) => p.name === preset.name && sameResizeSpec(p.spec, preset.spec),
        );
        if (dupe) {
          if (preset.id !== null && dupe.id !== null) idMap.set(preset.id, dupe.id);
          continue;
        }
        try {
          const inserted = await ipc.saveResizePreset({ ...preset, id: null });
          if (preset.id !== null && inserted.id !== null) idMap.set(preset.id, inserted.id);
          existing.push(inserted);
          saved++;
        } catch {
          failed++;
        }
      }
      let modules = 0;
      for (const [mod, opts] of Object.entries(file.moduleOptions)) {
        if (!opts) continue;
        const patch = { ...opts };
        if (mod === "resizer" && patch.presetId != null) {
          // Exported preset ids refer to the source machine; remap to the id
          // the preset got here, or clear it ("custom") when it wasn't imported.
          patch.presetId = idMap.get(patch.presetId) ?? null;
        }
        q.setOptions(mod as Parameters<typeof q.setOptions>[0], patch);
        modules++;
      }
      setPresetsMsg({
        ok: failed === 0,
        text:
          failed === 0
            ? t("settings.presetsImported", { modules, count: saved })
            : t("settings.presetsImportPartial", { modules, count: saved, failed }),
      });
    } catch (e) {
      setPresetsMsg({
        ok: false,
        text: e instanceof SyntaxError || (e instanceof Error && e.message === "unsupported presets file")
          ? t("settings.presetsInvalid")
          : errorMessage(e),
      });
    }
  };

  return (
    <div className="pb-8">
      <PageHeader
        title={t("settings.title")}
        description={t("settings.subtitle")}
        actions={
          <div className="flex items-center gap-3">
            {lastSavedAt && !saving && !error && (
              <span className="flex items-center gap-1 text-xs text-success">
                <Check className="size-3.5" />
                {t("settings.saved")}
              </span>
            )}
            <Button variant="secondary" onClick={() => void reset()} disabled={saving}>
              <RotateCcw />
              {t("settings.reset")}
            </Button>
          </div>
        }
      />
      <div className="flex max-w-3xl flex-col gap-4 px-8">
        {error && (
          <div role="alert" className="rounded-lg border border-danger/40 bg-danger/10 px-4 py-3 text-sm text-danger">
            {t("settings.saveError", { message: error })}
          </div>
        )}

        <Section title={t("settings.appearance")}>
          <Row label={t("settings.theme")}>
            <Segmented<Theme>
              aria-label={t("settings.theme")}
              value={settings.theme}
              onValueChange={(theme) => void update({ theme })}
              options={[
                { value: "dark", label: t("settings.themeDark") },
                { value: "light", label: t("settings.themeLight") },
              ]}
            />
          </Row>
          <Row label={t("settings.language")}>
            <Select<Language>
              aria-label={t("settings.language")}
              value={settings.language}
              onValueChange={(language) => void update({ language })}
              options={[
                { value: "system", label: t("settings.languageSystem") },
                ...SUPPORTED_LOCALES.map((l) => ({ value: l, label: LOCALE_NATIVE_NAMES[l] })),
              ]}
            />
          </Row>
        </Section>

        <Section title={t("settings.processing")}>
          <Row label={t("settings.outputDir")} hint={t("settings.outputDirHint")} stacked>
            <div className="flex items-center gap-2">
              <div
                className="flex h-9 min-w-0 flex-1 items-center truncate rounded-lg border border-border bg-bg-secondary px-3 text-sm"
                title={settings.defaultOutputDir ?? undefined}
              >
                {settings.defaultOutputDir ?? (
                  <span className="text-text-secondary">{t("settings.outputDirNone")}</span>
                )}
              </div>
              <Button variant="secondary" onClick={() => void pickOutputDir()}>
                <FolderOpen />
                {t("settings.choose")}
              </Button>
              {settings.defaultOutputDir && (
                <Button variant="ghost" size="icon" aria-label={t("settings.clear")} onClick={() => void update({ defaultOutputDir: null })}>
                  <X />
                </Button>
              )}
            </div>
          </Row>
          <Row label={t("settings.format")}>
            <Select<ImageFormat>
              aria-label={t("settings.format")}
              value={settings.defaultFormat}
              onValueChange={(defaultFormat) => void update({ defaultFormat })}
              options={FORMATS}
            />
          </Row>
          <Row label={t("settings.quality")}>
            <div className="flex w-72 items-center gap-3">
              <Slider
                aria-label={t("settings.quality")}
                min={1}
                max={100}
                value={quality}
                onValueChange={setQuality}
                onValueCommit={(defaultQuality) => void update({ defaultQuality })}
              />
              <span className="w-8 text-right text-sm tabular-nums text-text">{quality}</span>
            </div>
          </Row>
          <Row label={t("settings.concurrency")} hint={t("settings.concurrencyHint")}>
            <div className="flex w-72 items-center gap-3">
              <Slider
                aria-label={t("settings.concurrency")}
                min={0}
                max={concurrencyMax}
                value={concurrency}
                onValueChange={setConcurrency}
                onValueCommit={(c) => void update({ concurrency: c })}
              />
              <span className="w-36 whitespace-nowrap text-right text-xs text-text">
                {concurrency === 0
                  ? t("settings.concurrencyAuto", { count: cpus })
                  : t("settings.threads", { count: concurrency })}
              </span>
            </div>
          </Row>
          <Row label={t("settings.conflict")} hint={t("settings.conflictHint")}>
            <Select<ConflictPolicy>
              aria-label={t("settings.conflict")}
              className="min-w-64"
              value={settings.conflictPolicy}
              onValueChange={(conflictPolicy) => void update({ conflictPolicy })}
              options={[
                { value: "rename", label: t("settings.conflictRename") },
                { value: "skip", label: t("settings.conflictSkip") },
                { value: "overwrite", label: t("settings.conflictOverwrite") },
              ]}
            />
          </Row>
        </Section>

        <Section title={t("settings.presets")}>
          <Row label={t("settings.presets")} hint={t("settings.presetsHint")}>
            <div className="flex items-center gap-2">
              <Button variant="secondary" onClick={() => void exportPresets()}>
                <Download />
                {t("settings.presetsExport")}
              </Button>
              <Button variant="secondary" onClick={() => void importPresets()}>
                <Upload />
                {t("settings.presetsImport")}
              </Button>
            </div>
          </Row>
          {presetsMsg && (
            <p className={presetsMsg.ok ? "pb-3 text-xs text-success" : "pb-3 text-xs text-danger"}>
              {presetsMsg.text}
            </p>
          )}
        </Section>

        <Section title={t("settings.license")}>
          <LicenseSection />
        </Section>

        <Section title={t("settings.about")}>
          <dl className="grid grid-cols-[10rem_1fr] gap-y-2 text-sm">
            <dt className="text-text-secondary">{t("settings.version")}</dt>
            <dd className="text-text">{appInfo?.version ?? "—"}</dd>
            <dt className="text-text-secondary">{t("settings.runtime")}</dt>
            <dd className="text-text">Tauri {appInfo?.tauriVersion ?? "—"}</dd>
            <dt className="text-text-secondary">{t("settings.system")}</dt>
            <dd className="text-text">
              {appInfo ? `${appInfo.os} · ${appInfo.arch} · ${appInfo.logicalCpus} CPU` : "—"}
            </dd>
          </dl>
          <p className="mt-4 flex items-center gap-2 text-xs text-text-secondary">
            <Info className="size-3.5" />
            {t("dashboard.privacy")}
          </p>
        </Section>
      </div>
    </div>
  );
}

function LicenseSection() {
  const { t } = useTranslation();
  const lic = useLicenseStore();
  const [key, setKey] = useState("");
  const status = lic.status;
  return (
    <div className="flex flex-col gap-3 py-4 first:pt-0 last:pb-0">
      <p className="flex items-center gap-2 text-sm text-text">
        <Lock className="size-4 text-text-secondary" />
        {status === null && "…"}
        {status?.state === "trial" &&
          t("license.trialStatus", { count: status.daysLeft })}
        {status?.state === "licensed" &&
          t("license.licensedStatus", { key: status.maskedKey ?? "" })}
        {status?.state === "expired" && (
          <span className="text-danger">{t("license.expiredStatus")}</span>
        )}
      </p>
      {status?.state === "licensed" && status.graceDaysLeft !== null && (
        <p className="text-xs text-text-secondary">
          {t("license.graceNote", { count: status.graceDaysLeft })}
        </p>
      )}
      {status?.state !== "licensed" && (
        <div className="flex items-center gap-2">
          <input
            type="text"
            value={key}
            onChange={(e) => setKey(e.target.value)}
            placeholder={t("license.keyPlaceholder")}
            aria-label={t("license.keyPlaceholder")}
            spellCheck={false}
            className="h-9 min-w-0 flex-1 rounded-lg border border-border bg-bg-secondary px-3 font-mono text-sm text-text placeholder:text-text-secondary/60 focus:border-accent focus:outline-none"
          />
          <Button
            onClick={() => void lic.activate(key).then((ok) => ok && setKey(""))}
            disabled={lic.busy || key.trim().length < 8}
          >
            {t("license.activate")}
          </Button>
        </div>
      )}
      {status?.state === "licensed" && (
        <div className="flex items-center gap-2">
          <Button
            variant="secondary"
            onClick={() => void lic.deactivate()}
            disabled={lic.busy}
          >
            {t("license.deactivate")}
          </Button>
        </div>
      )}
      {!status?.providerReady && (
        <p className="text-xs text-text-secondary">{t("license.providerPending")}</p>
      )}
      {lic.error && (
        <p role="alert" className="text-sm text-danger">
          {t("license.error", { message: lic.error })}
        </p>
      )}
      <p className="text-xs text-text-secondary">
        <a
          href={BUY_URL}
          target="_blank"
          rel="noreferrer"
          className="text-accent hover:underline"
        >
          {t("license.buy")}
        </a>
      </p>
    </div>
  );
}

function Section({ title, children }: { title: string; children: ReactNode }) {
  return (
    <Card>
      <CardHeader>
        <CardTitle>{title}</CardTitle>
      </CardHeader>
      <CardContent className="flex flex-col divide-y divide-border">{children}</CardContent>
    </Card>
  );
}

function Row({ label, hint, children, stacked }: { label: string; hint?: string; children: ReactNode; stacked?: boolean }) {
  return (
    <div className={stacked ? "flex flex-col gap-3 py-4 first:pt-0 last:pb-0" : "flex items-center justify-between gap-6 py-4 first:pt-0 last:pb-0"}>
      <div>
        <div className="text-sm text-text">{label}</div>
        {hint && <div className="mt-0.5 max-w-md text-xs text-text-secondary">{hint}</div>}
      </div>
      {children}
    </div>
  );
}
