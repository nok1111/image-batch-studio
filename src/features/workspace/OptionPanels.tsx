import { useEffect, useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import { Save, Trash2 } from "lucide-react";
import type { ModuleId } from "@/app/modules";
import { Button } from "@/components/ui/button";
import { Segmented } from "@/components/ui/segmented";
import { Select } from "@/components/ui/select";
import { Slider } from "@/components/ui/slider";
import {
  errorMessage,
  ipc,
  type MetadataPolicy,
  type PlannedOutput,
  type ResizeMode,
  type ResizePreset,
  type WatermarkKind,
  type WatermarkPosition,
  type WatermarkPreset,
  type WatermarkSpec,
} from "@/lib/ipc";
import { cn } from "@/lib/utils";
import {
  fileStem,
  renderRenameStem,
  todayUtc,
  validateTemplate,
} from "./rename";
import { CropEditor } from "./CropEditor";
import { SpriteGridPreview } from "./SpriteGridPreview";
import { SpritePackPreview } from "./SpritePackPreview";
import { ThumbnailEditor } from "./ThumbnailEditor";
import { WatermarkPreview } from "./WatermarkPreview";
import { aggregateAnalysis } from "./analyze";
import { formatBytes } from "@/lib/format";
import type { QueueItem } from "@/stores/queue";
import {
  COMPRESS_QUALITY,
  effectiveQuality,
  supportsTargetSize,
  spriteSpec,
  THUMB_FONTS,
  THUMB_PRESETS,
  DEFAULT_IMG_FX,
  newThumbText,
  type ThumbImgFx,
  type ThumbTextOpt,
  usesQuality,
  type CompressPreset,
  type FormatChoice,
  type ModuleOptions,
} from "./job";

export interface PanelProps {
  module: ModuleId;
  o: ModuleOptions;
  set: (patch: Partial<ModuleOptions>) => void;
  disabled: boolean;
  /** Queued files (used by Crop & Rotate to pick the reference image). */
  items?: QueueItem[];
  /** Engine-planned output names (used by the Renamer panel's live example). */
  previews?: PlannedOutput[] | null;
}

const FORMAT_LABELS: Record<Exclude<FormatChoice, "source">, string> = {
  jpeg: "JPG",
  png: "PNG",
  webp: "WebP",
  avif: "AVIF",
  bmp: "BMP",
  tiff: "TIFF",
  gif: "GIF",
  ico: "ICO (≤ 256 px)",
};

const fieldClass = "flex flex-col gap-1.5 text-xs text-text-secondary";
const inputClass =
  "h-9 rounded-lg border border-border bg-bg-secondary px-3 text-sm text-text outline-none focus:ring-2 focus:ring-accent disabled:opacity-50";

function NumberField({
  label,
  value,
  onChange,
  min,
  max,
  disabled,
  className,
}: {
  label: string;
  value: number | null;
  onChange: (v: number | null) => void;
  min: number;
  max: number;
  disabled?: boolean;
  className?: string;
}) {
  return (
    <label className={cn(fieldClass, className)}>
      {label}
      <input
        type="number"
        inputMode="numeric"
        min={min}
        max={max}
        value={value ?? ""}
        disabled={disabled}
        onChange={(e) => {
          if (e.target.value === "") return onChange(null);
          const n = Math.round(Number(e.target.value));
          if (Number.isFinite(n)) onChange(Math.min(max, Math.max(min, n)));
        }}
        className={cn(inputClass, "w-28 tabular-nums")}
      />
    </label>
  );
}

function TextField({
  label,
  value,
  onChange,
  disabled,
  className,
  placeholder,
}: {
  label: string;
  value: string;
  onChange: (v: string) => void;
  disabled?: boolean;
  className?: string;
  placeholder?: string;
}) {
  return (
    <label className={cn(fieldClass, className)}>
      {label}
      <input
        type="text"
        value={value}
        placeholder={placeholder}
        disabled={disabled}
        onChange={(e) => onChange(e.target.value)}
        className={inputClass}
        spellCheck={false}
      />
    </label>
  );
}

function SliderField({
  label,
  value,
  onChange,
  min,
  max,
  disabled,
}: {
  label: string;
  value: number;
  onChange: (v: number) => void;
  min: number;
  max: number;
  disabled?: boolean;
}) {
  return (
    <label className={cn(fieldClass, "min-w-36")}>
      <span className="flex items-center justify-between gap-2">
        {label}
        <span className="font-mono text-[10px] text-text-secondary">{value}</span>
      </span>
      <input
        type="range"
        min={min}
        max={max}
        value={value}
        disabled={disabled}
        onChange={(e) => onChange(Number(e.target.value))}
        className="h-8 w-full cursor-pointer accent-accent"
      />
    </label>
  );
}

function Check({
  label,
  checked,
  onChange,
  disabled,
}: {
  label: string;
  checked: boolean;
  onChange: (v: boolean) => void;
  disabled?: boolean;
}) {
  return (
    <label
      className={cn(
        "flex h-9 items-center gap-2 text-xs text-text",
        disabled && "opacity-40",
      )}
    >
      <input
        type="checkbox"
        checked={checked}
        disabled={disabled}
        onChange={(e) => onChange(e.target.checked)}
        className="size-4 accent-[var(--color-accent)]"
      />
      {label}
    </label>
  );
}

function FormatField({
  o,
  set,
  disabled,
  formats,
  allowSource,
}: PanelProps & {
  formats: Exclude<FormatChoice, "source">[];
  allowSource: boolean;
}) {
  const { t } = useTranslation();
  const options = [
    ...(allowSource
      ? [{ value: "source" as FormatChoice, label: t("options.sameAsSource") }]
      : []),
    ...formats.map((f) => ({
      value: f as FormatChoice,
      label: FORMAT_LABELS[f],
    })),
  ];
  return (
    <label className={fieldClass}>
      {t("workspace.format")}
      <Select<FormatChoice>
        value={o.format}
        onValueChange={(format) => !disabled && set({ format })}
        options={options}
        aria-label={t("workspace.format")}
      />
    </label>
  );
}

function QualityField({ module, o, set, disabled }: PanelProps) {
  const { t } = useTranslation();
  const active =
    usesQuality(o.format, o.lossless) &&
    !(module === "compressor" && o.compressPreset !== "custom");
  const q = effectiveQuality(module, o);
  return (
    <div
      className={cn(
        "flex w-44 flex-col gap-2.5 text-xs text-text-secondary",
        !active && "opacity-40",
      )}
    >
      <span>
        {t("workspace.quality")}:{" "}
        <span className="tabular-nums text-text">{q}</span>
      </span>
      <Slider
        value={q}
        min={1}
        max={100}
        step={1}
        onValueChange={(quality) => set({ quality })}
        disabled={!active || disabled}
        aria-label={t("workspace.quality")}
      />
    </div>
  );
}

function CommonEncoding(props: PanelProps) {
  const { t } = useTranslation();
  const { o, set, disabled } = props;
  const webpLike = o.format === "webp" || o.format === "source";
  return (
    <>
      <QualityField {...props} />
      <Check
        label={t("options.lossless")}
        checked={o.lossless}
        onChange={(lossless) => set({ lossless })}
        disabled={disabled || !webpLike}
      />
      <label className={fieldClass}>
        {t("options.metadata")}
        <Segmented<MetadataPolicy>
          value={o.metadata}
          onValueChange={(metadata) => !disabled && set({ metadata })}
          options={[
            { value: "strip", label: t("options.metadataStrip") },
            { value: "keep", label: t("options.metadataKeep") },
          ]}
          aria-label={t("options.metadata")}
        />
      </label>
    </>
  );
}

const ALL_FORMATS: Exclude<FormatChoice, "source">[] = [
  "jpeg",
  "png",
  "webp",
  "avif",
  "bmp",
  "tiff",
  "gif",
  "ico",
];

export function ConverterPanel(props: PanelProps) {
  return (
    <>
      <FormatField {...props} formats={ALL_FORMATS} allowSource={false} />
      <CommonEncoding {...props} />
    </>
  );
}

export function CompressorPanel(props: PanelProps) {
  const { t } = useTranslation();
  const { o, set, disabled } = props;
  const presets: CompressPreset[] = [
    "maxQuality",
    "balanced",
    "high",
    "web",
    "custom",
  ];
  const targetOk = supportsTargetSize(o.format, o.lossless);
  return (
    <>
      <label className={fieldClass}>
        {t("options.preset")}
        <Segmented<CompressPreset>
          value={o.compressPreset}
          onValueChange={(compressPreset) =>
            !disabled &&
            set(
              compressPreset === "custom"
                ? { compressPreset, quality: effectiveQuality("compressor", o) }
                : { compressPreset },
            )
          }
          options={presets.map((p) => ({
            value: p,
            label:
              p === "custom"
                ? t("options.presets.custom")
                : `${t(`options.presets.${p}`)} · ${COMPRESS_QUALITY[p]}`,
          }))}
          aria-label={t("options.preset")}
        />
      </label>
      <FormatField
        {...props}
        formats={["jpeg", "webp", "avif", "png"]}
        allowSource
      />
      <QualityField {...props} />
      <Check
        label={t("options.lossless")}
        checked={o.lossless}
        onChange={(lossless) => set({ lossless })}
        disabled={disabled || !(o.format === "webp" || o.format === "source")}
      />
      <div
        title={
          targetOk ? t("options.targetHint") : t("options.targetUnavailable")
        }
      >
        <NumberField
          label={t("options.targetSize")}
          value={targetOk ? o.targetKb : null}
          onChange={(targetKb) => set({ targetKb })}
          min={1}
          max={1_000_000}
          disabled={disabled || !targetOk}
        />
      </div>
    </>
  );
}

const RESIZE_MODES: ResizeMode[] = [
  "fit",
  "fill",
  "exact",
  "width",
  "height",
  "percent",
];

export function ResizerPanel(props: PanelProps) {
  const { t } = useTranslation();
  const { o, set, disabled } = props;
  const r = o.resize;
  const [presets, setPresets] = useState<ResizePreset[]>([]);
  const [name, setName] = useState("");
  const [error, setError] = useState<string | null>(null);
  useEffect(() => {
    ipc
      .listResizePresets()
      .then(setPresets, (e: unknown) => setError(errorMessage(e)));
  }, []);
  const selected = presets.find((p) => p.id === o.presetId) ?? null;
  const setSpec = (patch: Partial<ModuleOptions["resize"]>) =>
    set({ resize: { ...r, ...patch } });
  const save = async (id: number | null) => {
    try {
      const saved = await ipc.saveResizePreset({
        id,
        name: id === null ? name : (selected?.name ?? name),
        spec: r,
      });
      setPresets((ps) =>
        id === null
          ? [...ps, saved]
          : ps.map((p) => (p.id === saved.id ? saved : p)),
      );
      set({ presetId: saved.id });
      setName("");
      setError(null);
    } catch (e) {
      setError(errorMessage(e));
    }
  };
  const remove = async () => {
    if (selected?.id == null) return;
    try {
      await ipc.deleteResizePreset(selected.id);
      setPresets((ps) => ps.filter((p) => p.id !== selected.id));
      set({ presetId: null });
    } catch (e) {
      setError(errorMessage(e));
    }
  };
  const needsW = r.mode !== "height" && r.mode !== "percent";
  const needsH = r.mode !== "width" && r.mode !== "percent";
  const dirty =
    selected !== null && JSON.stringify(selected.spec) !== JSON.stringify(r);
  return (
    <>
      <label className={fieldClass}>
        {t("options.platformPreset")}
        <Select<string>
          value={o.presetId === null ? "custom" : String(o.presetId)}
          onValueChange={(v) => {
            if (disabled) return;
            const p = presets.find((x) => String(x.id) === v);
            set(p ? { presetId: p.id, resize: p.spec } : { presetId: null });
          }}
          options={[
            { value: "custom", label: t("options.customSize") },
            ...presets.map((p) => ({ value: String(p.id), label: p.name })),
          ]}
          aria-label={t("options.platformPreset")}
        />
      </label>
      <label className={fieldClass}>
        {t("options.resizeMode")}
        <Select<ResizeMode>
          value={r.mode}
          onValueChange={(mode) => !disabled && setSpec({ mode })}
          options={RESIZE_MODES.map((m) => ({
            value: m,
            label: t(`options.modes.${m}`),
          }))}
          aria-label={t("options.resizeMode")}
        />
      </label>
      {needsW && (
        <NumberField
          label={t("options.width")}
          value={r.width}
          onChange={(v) => setSpec({ width: v ?? 1 })}
          min={1}
          max={30000}
          disabled={disabled}
        />
      )}
      {needsH && (
        <NumberField
          label={t("options.height")}
          value={r.height}
          onChange={(v) => setSpec({ height: v ?? 1 })}
          min={1}
          max={30000}
          disabled={disabled}
        />
      )}
      {r.mode === "percent" && (
        <NumberField
          label={t("options.percent")}
          value={r.percent}
          onChange={(v) => setSpec({ percent: v ?? 1 })}
          min={1}
          max={1000}
          disabled={disabled}
        />
      )}
      <Check
        label={t("options.allowUpscale")}
        checked={r.allowUpscale}
        onChange={(allowUpscale) => setSpec({ allowUpscale })}
        disabled={disabled}
      />
      <FormatField {...props} formats={ALL_FORMATS} allowSource />
      <QualityField {...props} />
      <div className="flex items-end gap-2">
        <TextField
          label={t("options.presetName")}
          value={name}
          onChange={setName}
          disabled={disabled}
          className="w-40"
        />
        <Button
          variant="secondary"
          onClick={() => void save(null)}
          disabled={disabled || name.trim() === ""}
        >
          <Save />
          {t("options.savePreset")}
        </Button>
        {selected && (
          <>
            <Button
              variant="secondary"
              onClick={() => void save(selected.id)}
              disabled={disabled || !dirty}
            >
              {t("options.updatePreset")}
            </Button>
            <Button
              variant="ghost"
              onClick={() => void remove()}
              disabled={disabled}
              aria-label={t("options.deletePreset")}
            >
              <Trash2 />
            </Button>
          </>
        )}
      </div>
      {error && <p className="w-full text-xs text-danger">{error}</p>}
    </>
  );
}

const RENAME_TOKENS = ["{name}", "{n}", "{n:03}", "{date}"] as const;

const TOKEN_HINT_KEYS = {
  "{name}": "renamer.tokenName",
  "{n}": "renamer.tokenCounter",
  "{n:03}": "renamer.tokenCounterPad",
  "{date}": "renamer.tokenDate",
} as const;

function RenameStep({ n, label }: { n: number; label: string }) {
  return (
    <span className="flex items-center gap-1.5 text-xs font-medium text-text">
      <span className="inline-flex size-4 items-center justify-center rounded-full bg-accent/15 text-[10px] font-semibold text-accent">
        {n}
      </span>
      {label}
    </span>
  );
}

export function RenamerPanel({ o, set, disabled, items, previews }: PanelProps) {
  const { t } = useTranslation();
  const r = o.rename;
  const setR = (patch: Partial<ModuleOptions["rename"]>) =>
    set({ rename: { ...r, ...patch } });
  const tplRef = useRef<HTMLInputElement>(null);
  const files = items ?? [];

  const insertToken = (tok: string) => {
    const el = tplRef.current;
    const cur = r.template;
    const start = el?.selectionStart ?? cur.length;
    const end = el?.selectionEnd ?? start;
    setR({ template: cur.slice(0, start) + tok + cur.slice(end) });
    requestAnimationFrame(() => {
      el?.focus();
      el?.setSelectionRange(start + tok.length, start + tok.length);
    });
  };

  const tplError = validateTemplate(r.template);
  const ext = (name: string) => /\.[^.]+$/.exec(name)?.[0] ?? "";
  const baseName = (p: string) => p.split(/[\\/]/).pop() ?? p;
  const example = (i: number) => {
    const item = files[i];
    const from = item?.name ?? "IMG_2041.jpg";
    const target = previews?.[i]?.target;
    const to =
      target !== undefined && target !== null
        ? baseName(target)
        : renderRenameStem(r, fileStem(from), i, todayUtc()) + ext(from);
    return { from, to };
  };
  const exampleIndices = files.length > 1 ? [0, files.length - 1] : [0];

  return (
    <div className="flex w-full min-w-72 max-w-3xl flex-col gap-4">
      <div className="flex flex-col gap-1.5">
        <RenameStep n={1} label={t("renamer.stepClean")} />
        <div className="flex items-end gap-2">
          <TextField
            label={t("options.find")}
            value={r.find}
            onChange={(find) => setR({ find })}
            disabled={disabled}
            className="w-40"
          />
          <span aria-hidden className="mb-2.5 text-text-secondary">
            →
          </span>
          <TextField
            label={t("options.replace")}
            value={r.replace}
            onChange={(replace) => setR({ replace })}
            disabled={disabled}
            className="w-40"
          />
        </div>
        <p className="text-[11px] text-text-secondary">
          {t("renamer.stepCleanHint")}
        </p>
      </div>

      <div className="flex flex-col gap-1.5">
        <RenameStep n={2} label={t("renamer.stepTemplate")} />
        <label className={cn(fieldClass, "w-full max-w-md")}>
          {t("options.template")}
          <input
            ref={tplRef}
            type="text"
            value={r.template}
            placeholder="{name}_{n:03}_{date}"
            disabled={disabled}
            onChange={(e) => setR({ template: e.target.value })}
            className={cn(inputClass, "font-mono")}
          />
        </label>
        <div className="flex flex-wrap items-center gap-1.5">
          <span className="text-[11px] text-text-secondary">
            {t("renamer.tokensLabel")}
          </span>
          {RENAME_TOKENS.map((tok) => (
            <button
              key={tok}
              type="button"
              disabled={disabled}
              title={t(TOKEN_HINT_KEYS[tok])}
              onClick={() => insertToken(tok)}
              className="rounded-md border border-border bg-card px-2 py-0.5 font-mono text-[11px] text-text hover:bg-card-hover disabled:opacity-50 cursor-pointer"
            >
              {tok}
            </button>
          ))}
        </div>
      </div>

      <div className="flex flex-col gap-1.5">
        <RenameStep n={3} label={t("renamer.stepFinish")} />
        <div className="flex flex-wrap items-end gap-x-6 gap-y-3">
          <TextField
            label={t("options.prefix")}
            value={r.prefix}
            onChange={(prefix) => setR({ prefix })}
            disabled={disabled}
            className="w-32"
          />
          <TextField
            label={t("options.suffix")}
            value={r.suffix}
            onChange={(suffix) => setR({ suffix })}
            disabled={disabled}
            className="w-32"
          />
          <NumberField
            label={t("options.start")}
            value={r.start}
            onChange={(v) => setR({ start: v ?? 0 })}
            min={0}
            max={999_999_999}
            disabled={disabled}
          />
        </div>
      </div>

      <div className="flex flex-col gap-1 rounded-lg border border-border bg-bg-secondary/60 px-3 py-2">
        <span className="text-[11px] font-medium uppercase tracking-wide text-text-secondary">
          {t("renamer.exampleTitle")}
        </span>
        {tplError ? (
          <p className="text-xs text-danger">
            {t("options.previewError", { message: tplError })}
          </p>
        ) : (
          exampleIndices.map((i) => {
            const ex = example(i);
            return (
              <p
                key={i}
                className="truncate font-mono text-xs text-text"
                title={`${ex.from} → ${ex.to}`}
              >
                {files[i] === undefined
                  ? t("renamer.exampleGeneric", { name: ex.from })
                  : ex.from}
                <span className="text-text-secondary"> → </span>
                <span className="font-medium text-accent">{ex.to}</span>
              </p>
            );
          })
        )}
        <p className="text-[11px] text-text-secondary">
          {t("renamer.exampleNote")}
        </p>
      </div>
    </div>
  );
}

export function MetadataPanel(props: PanelProps) {
  const { t } = useTranslation();
  return (
    <>
      <FormatField {...props} formats={ALL_FORMATS} allowSource />
      <p className="max-w-72 text-xs leading-5 text-text-secondary">
        {t("meta.info")}
      </p>
    </>
  );
}

const ROTATIONS = [0, 90, 180, 270] as const;

export function CropRotatePanel({ o, set, disabled, items }: PanelProps) {
  const { t } = useTranslation();
  const tr = o.transform;
  const files = items ?? [];
  const [ref, setRef] = useState<string | null>(null);
  const [nat, setNat] = useState<[number, number] | null>(null);
  const refPath =
    files.find((i) => i.path === ref)?.path ?? files[0]?.path ?? null;
  const patch = (p: Partial<typeof tr>) => set({ transform: { ...tr, ...p } });
  const natDims = nat ?? null;
  const full = natDims
    ? { x: 0, y: 0, width: natDims[0], height: natDims[1] }
    : null;
  const crop = tr.crop ?? full;
  const result = (() => {
    if (!natDims) return null;
    const r = tr.rotate === 90 || tr.rotate === 270;
    return r ? [crop!.height, crop!.width] : [crop!.width, crop!.height];
  })();
  const setCropField =
    (key: "x" | "y" | "width" | "height") => (v: number | null) => {
      if (!crop || !natDims || v === null) return;
      const c = { ...crop, [key]: v };
      c.x = Math.min(c.x, natDims[0] - 1);
      c.y = Math.min(c.y, natDims[1] - 1);
      c.width = Math.max(1, Math.min(c.width, natDims[0] - c.x));
      c.height = Math.max(1, Math.min(c.height, natDims[1] - c.y));
      patch({ crop: c });
    };
  return (
    <>
      <FormatField
        module="cropRotate"
        o={o}
        set={set}
        disabled={disabled}
        formats={["jpeg", "png", "webp", "avif", "bmp", "tiff", "ico"]}
        allowSource
      />
      <label className={fieldClass}>
        {t("cropRotate.rotation")}
        <Segmented
          value={String(tr.rotate)}
          onValueChange={(rotate) =>
            !disabled && patch({ rotate: Number(rotate) })
          }
          options={ROTATIONS.map((r) => ({ value: String(r), label: `${r}°` }))}
          aria-label={t("cropRotate.rotation")}
        />
      </label>
      <div className="flex flex-col gap-0.5">
        <Check
          label={t("cropRotate.flipH")}
          checked={tr.flipHorizontal}
          onChange={(flipHorizontal) => patch({ flipHorizontal })}
          disabled={disabled}
        />
        <Check
          label={t("cropRotate.flipV")}
          checked={tr.flipVertical}
          onChange={(flipVertical) => patch({ flipVertical })}
          disabled={disabled}
        />
      </div>
      <div className="flex flex-col gap-2">
        <label className={fieldClass}>
          {t("cropRotate.reference")}
          <Select<string>
            value={refPath ?? ""}
            onValueChange={setRef}
            options={files.map((i) => ({ value: i.path, label: i.name }))}
            aria-label={t("cropRotate.reference")}
            disabled={disabled || files.length === 0}
          />
        </label>
        {refPath ? (
          <CropEditor
            path={refPath}
            crop={tr.crop}
            disabled={disabled}
            onChange={(c) => patch({ crop: c })}
            onSize={(s) => setNat(s)}
          />
        ) : (
          <p className="text-xs text-text-secondary">
            {t("cropRotate.noFiles")}
          </p>
        )}
        {crop && natDims && (
          <div className="flex flex-wrap items-end gap-3">
            <NumberField
              label="X"
              value={crop.x}
              min={0}
              max={natDims[0] - 1}
              onChange={setCropField("x")}
              disabled={disabled}
              className="w-24"
            />
            <NumberField
              label="Y"
              value={crop.y}
              min={0}
              max={natDims[1] - 1}
              onChange={setCropField("y")}
              disabled={disabled}
              className="w-24"
            />
            <NumberField
              label={t("options.width")}
              value={crop.width}
              min={1}
              max={natDims[0]}
              onChange={setCropField("width")}
              disabled={disabled}
              className="w-24"
            />
            <NumberField
              label={t("options.height")}
              value={crop.height}
              min={1}
              max={natDims[1]}
              onChange={setCropField("height")}
              disabled={disabled}
              className="w-24"
            />
            <Button
              type="button"
              variant="secondary"
              size="sm"
              disabled={disabled}
              onClick={() => patch({ crop: full })}
            >
              {t("cropRotate.resetCrop")}
            </Button>
            <span className="text-xs tabular-nums text-text-secondary">
              {natDims[0]}×{natDims[1]} →{" "}
              {result ? `${result[0]}×${result[1]}` : ""}
            </span>
          </div>
        )}
      </div>
    </>
  );
}

const POSITIONS: WatermarkPosition[] = [
  "topLeft",
  "topCenter",
  "topRight",
  "middleLeft",
  "center",
  "middleRight",
  "bottomLeft",
  "bottomCenter",
  "bottomRight",
];

function PctSlider({
  label,
  value,
  onChange,
  min,
  max,
  disabled,
}: {
  label: string;
  value: number;
  onChange: (v: number) => void;
  min: number;
  max: number;
  disabled?: boolean;
}) {
  return (
    <div className={cn("flex w-44 flex-col gap-2.5 text-xs text-text-secondary", disabled && "opacity-40")}>
      <span>
        {label}: <span className="tabular-nums text-text">{Math.round(value * 100)}%</span>
      </span>
      <Slider
        value={value * 100}
        min={min}
        max={max}
        step={1}
        onValueChange={(v) => onChange(v / 100)}
        disabled={disabled}
        aria-label={label}
      />
    </div>
  );
}

export function WatermarkPanel({ o, set, disabled, items }: PanelProps) {
  const { t } = useTranslation();
  const w = o.watermark;
  const files = items ?? [];
  const [ref, setRef] = useState<string | null>(null);
  const [presets, setPresets] = useState<WatermarkPreset[]>([]);
  const [name, setName] = useState("");
  const [error, setError] = useState<string | null>(null);
  useEffect(() => {
    ipc
      .listWatermarkPresets()
      .then(setPresets, (e: unknown) => setError(errorMessage(e)));
  }, []);
  const refPath = files.find((i) => i.path === ref)?.path ?? files[0]?.path ?? null;
  const patch = (p: Partial<WatermarkSpec>) =>
    set({ watermark: { ...w, ...p } });
  const selected = presets.find((p) => p.id === o.presetId) ?? null;
  const save = async (id: number | null) => {
    try {
      const saved = await ipc.saveWatermarkPreset({
        id,
        name: id === null ? name : (selected?.name ?? name),
        spec: w,
      });
      setPresets((ps) =>
        id === null ? [...ps, saved] : ps.map((p) => (p.id === saved.id ? saved : p)),
      );
      set({ presetId: saved.id });
      setName("");
      setError(null);
    } catch (e) {
      setError(errorMessage(e));
    }
  };
  const remove = async () => {
    if (selected?.id == null) return;
    try {
      await ipc.deleteWatermarkPreset(selected.id);
      setPresets((ps) => ps.filter((p) => p.id !== selected.id));
      set({ presetId: null });
    } catch (e) {
      setError(errorMessage(e));
    }
  };
  const pickLogo = async () => {
    try {
      const path = await ipc.pickWatermarkImage();
      if (path) patch({ imagePath: path });
    } catch (e) {
      setError(errorMessage(e));
    }
  };
  const dirty =
    selected !== null && JSON.stringify(selected.spec) !== JSON.stringify(w);
  return (
    <>
      <label className={fieldClass}>
        {t("options.platformPreset")}
        <Select<string>
          value={o.presetId === null ? "custom" : String(o.presetId)}
          onValueChange={(v) => {
            if (disabled) return;
            const p = presets.find((x) => String(x.id) === v);
            set(p ? { presetId: p.id, watermark: p.spec } : { presetId: null });
          }}
          options={[
            { value: "custom", label: t("options.customSize") },
            ...presets.map((p) => ({ value: String(p.id), label: p.name })),
          ]}
          aria-label={t("options.platformPreset")}
        />
      </label>
      <label className={fieldClass}>
        {t("wm.kind")}
        <Segmented<WatermarkKind>
          value={w.kind}
          onValueChange={(kind) => !disabled && patch({ kind })}
          options={[
            { value: "text", label: t("wm.kindText") },
            { value: "image", label: t("wm.kindImage") },
          ]}
          aria-label={t("wm.kind")}
        />
      </label>
      {w.kind === "text" ? (
        <>
          <TextField
            label={t("wm.text")}
            value={w.text}
            onChange={(text) => patch({ text })}
            disabled={disabled}
            className="w-56"
            placeholder={t("wm.textPlaceholder")}
          />
          <label className={fieldClass}>
            {t("wm.color")}
            <input
              type="color"
              disabled={disabled}
              value={`#${w.color.slice(0, 3).map((c) => c.toString(16).padStart(2, "0")).join("")}`}
              onChange={(e) => {
                const h = e.target.value;
                patch({
                  color: [
                    parseInt(h.slice(1, 3), 16),
                    parseInt(h.slice(3, 5), 16),
                    parseInt(h.slice(5, 7), 16),
                    255,
                  ],
                });
              }}
              className="h-9 w-16 cursor-pointer rounded-lg border border-border bg-bg-secondary"
            />
          </label>
        </>
      ) : (
        <div className="flex items-end gap-2">
          <Button variant="secondary" onClick={() => void pickLogo()} disabled={disabled}>
            {t("wm.chooseImage")}
          </Button>
          <span className="max-w-52 truncate text-xs text-text-secondary">
            {w.imagePath ? w.imagePath.split(/[\\/]/).pop() : t("wm.noImage")}
          </span>
        </div>
      )}
      <PctSlider
        label={t("wm.scale")}
        value={w.scale}
        onChange={(scale) => patch({ scale })}
        min={1}
        max={100}
        disabled={disabled}
      />
      <PctSlider
        label={t("wm.opacity")}
        value={w.opacity}
        onChange={(opacity) => patch({ opacity })}
        min={0}
        max={100}
        disabled={disabled}
      />
      <PctSlider
        label={t("wm.margin")}
        value={w.margin}
        onChange={(margin) => patch({ margin })}
        min={0}
        max={50}
        disabled={disabled || w.tiled || w.position === "custom"}
      />
      <div className={cn("flex w-44 flex-col gap-2.5 text-xs text-text-secondary", disabled && "opacity-40")}>
        <span>
          {t("wm.rotation")}: <span className="tabular-nums text-text">{Math.round(w.rotation)}°</span>
        </span>
        <Slider
          value={w.rotation}
          min={0}
          max={360}
          step={1}
          onValueChange={(rotation) => patch({ rotation })}
          disabled={disabled}
          aria-label={t("wm.rotation")}
        />
      </div>
      <div className="flex flex-col gap-2">
        <label className={fieldClass}>
          {t("wm.position")}
          <div className="grid w-24 grid-cols-3 gap-1">
            {POSITIONS.map((p) => (
              <button
                key={p}
                type="button"
                disabled={disabled || w.tiled}
                onClick={() => patch({ position: p })}
                aria-label={p}
                className={cn(
                  "size-7 rounded-md border border-border bg-bg-secondary",
                  w.position === p && !w.tiled && "border-accent bg-accent/20",
                )}
              />
            ))}
            <button
              type="button"
              disabled={disabled || w.tiled}
              onClick={() => patch({ position: "custom" })}
              aria-label="custom"
              title={t("wm.custom")}
              className={cn(
                "size-7 rounded-md border border-border bg-bg-secondary text-[10px]",
                w.position === "custom" && !w.tiled && "border-accent bg-accent/20",
              )}
            >
              ✛
            </button>
          </div>
        </label>
        {w.position === "custom" && !w.tiled && (
          <div className="flex items-end gap-3">
            <NumberField
              label={`${t("wm.customX")} %`}
              value={Math.round(w.custom[0] * 100)}
              min={0}
              max={100}
              onChange={(v) => patch({ custom: [(v ?? 50) / 100, w.custom[1]] })}
              disabled={disabled}
            />
            <NumberField
              label={`${t("wm.customY")} %`}
              value={Math.round(w.custom[1] * 100)}
              min={0}
              max={100}
              onChange={(v) => patch({ custom: [w.custom[0], (v ?? 50) / 100] })}
              disabled={disabled}
            />
          </div>
        )}
      </div>
      <div className="flex flex-col gap-0.5">
        <Check
          label={t("wm.tiled")}
          checked={w.tiled}
          onChange={(tiled) => patch({ tiled })}
          disabled={disabled}
        />
        {w.tiled && (
          <PctSlider
            label={t("wm.spacing")}
            value={w.spacing}
            onChange={(spacing) => patch({ spacing })}
            min={0}
            max={200}
            disabled={disabled}
          />
        )}
      </div>
      <FormatField module="watermark" o={o} set={set} disabled={disabled} formats={ALL_FORMATS} allowSource />
      <QualityField module="watermark" o={o} set={set} disabled={disabled} />
      <div className="flex items-end gap-2">
        <TextField
          label={t("options.presetName")}
          value={name}
          onChange={setName}
          disabled={disabled}
          className="w-40"
        />
        <Button variant="secondary" onClick={() => void save(null)} disabled={disabled || name.trim() === ""}>
          <Save />
          {t("options.savePreset")}
        </Button>
        {selected && (
          <>
            <Button variant="secondary" onClick={() => void save(selected.id)} disabled={disabled || !dirty}>
              {t("options.updatePreset")}
            </Button>
            <Button
              variant="ghost"
              onClick={() => void remove()}
              disabled={disabled}
              aria-label={t("options.deletePreset")}
            >
              <Trash2 />
            </Button>
          </>
        )}
      </div>
      {error && <p className="w-full text-xs text-danger">{error}</p>}
      <div className="flex w-full flex-col gap-2">
        <label className={fieldClass}>
          {t("wm.preview")}
          <Select<string>
            value={refPath ?? ""}
            onValueChange={setRef}
            options={files.map((i) => ({ value: i.path, label: i.name }))}
            aria-label={t("wm.preview")}
            disabled={disabled || files.length === 0}
          />
        </label>
        {refPath ? (
          <WatermarkPreview path={refPath} spec={w} />
        ) : (
          <p className="text-xs text-text-secondary">{t("cropRotate.noFiles")}</p>
        )}
      </div>
    </>
  );
}

/** Sprites: grid cutting and sprite sheet packing share one module. */
export function SpritesPanel({ o, set, disabled, items }: PanelProps) {
  const { t } = useTranslation();
  const sp = o.sprite;
  const files = items ?? [];
  const [ref, setRef] = useState<string | null>(null);
  const refPath =
    files.find((i) => i.path === ref)?.path ?? files[0]?.path ?? null;
  const setS = (patch: Partial<ModuleOptions["sprite"]>) =>
    set({ sprite: { ...sp, ...patch } });
  const setCut = (patch: Partial<ModuleOptions["sprite"]["cut"]>) =>
    setS({ cut: { ...sp.cut, ...patch } });
  const setPack = (patch: Partial<ModuleOptions["sprite"]["pack"]>) =>
    setS({ pack: { ...sp.pack, ...patch } });
  return (
    <div className="flex w-full max-w-4xl flex-col gap-4">
      <div className="flex flex-col gap-1.5">
        <label className={fieldClass}>
          {t("sprite.mode")}
          <Segmented
            value={sp.mode}
            onValueChange={(mode) =>
              !disabled && setS({ mode: mode as ModuleOptions["sprite"]["mode"] })
            }
            options={[
              { value: "cut", label: t("sprite.cutMode") },
              { value: "pack", label: t("sprite.packMode") },
            ]}
          />
        </label>
        <p className="text-[11px] text-text-secondary">
          {sp.mode === "cut" ? t("sprite.cutHint") : t("sprite.packHint")}
        </p>
      </div>

      {sp.mode === "cut" ? (
        <>
          <div className="flex flex-wrap items-end gap-x-6 gap-y-3">
            <label className={fieldClass}>
              {t("sprite.cutHow")}
              <Segmented
                value={sp.cut.sizing}
                onValueChange={(sizing) =>
                  !disabled &&
                  setCut({ sizing: sizing as ModuleOptions["sprite"]["cut"]["sizing"] })
                }
                options={[
                  { value: "rowsCols", label: t("sprite.rowsCols") },
                  { value: "tileSize", label: t("sprite.tileSize") },
                ]}
              />
            </label>
            {sp.cut.sizing === "rowsCols" ? (
              <>
                <NumberField label={t("sprite.cols")} value={sp.cut.cols} onChange={(v) => setCut({ cols: v ?? 1 })} min={1} max={999} disabled={disabled} />
                <NumberField label={t("sprite.rows")} value={sp.cut.rows} onChange={(v) => setCut({ rows: v ?? 1 })} min={1} max={999} disabled={disabled} />
              </>
            ) : (
              <>
                <NumberField label={t("sprite.tileW")} value={sp.cut.tileW} onChange={(v) => setCut({ tileW: v ?? 1 })} min={1} max={30000} disabled={disabled} />
                <NumberField label={t("sprite.tileH")} value={sp.cut.tileH} onChange={(v) => setCut({ tileH: v ?? 1 })} min={1} max={30000} disabled={disabled} />
              </>
            )}
            <label className={fieldClass}>
              {t("sprite.edge")}
              <Segmented
                value={sp.cut.edge}
                onValueChange={(edge) =>
                  !disabled &&
                  setCut({ edge: edge as ModuleOptions["sprite"]["cut"]["edge"] })
                }
                options={[
                  { value: "include", label: t("sprite.edgeInclude") },
                  { value: "crop", label: t("sprite.edgeCrop") },
                  { value: "pad", label: t("sprite.edgePad") },
                ]}
              />
            </label>
            <label className={fieldClass}>
              {t("sprite.naming")}
              <Segmented
                value={sp.cut.naming}
                onValueChange={(naming) =>
                  !disabled &&
                  setCut({ naming: naming as ModuleOptions["sprite"]["cut"]["naming"] })
                }
                options={[
                  { value: "index", label: t("sprite.nameIndex") },
                  { value: "rowCol", label: t("sprite.nameRowCol") },
                ]}
              />
            </label>
          </div>
          <div className="flex flex-wrap gap-x-6 gap-y-2">
            <Check label={t("sprite.skipTransparent")} checked={sp.cut.skipTransparent} onChange={(skipTransparent) => setCut({ skipTransparent })} disabled={disabled} />
            <Check label={t("sprite.dedupe")} checked={sp.cut.dedupe} onChange={(dedupe) => setCut({ dedupe })} disabled={disabled} />
          </div>
          <div className="flex flex-col gap-2">
            <label className={fieldClass}>
              {t("sprite.reference")}
              <Select<string>
                value={refPath ?? ""}
                onValueChange={setRef}
                options={files.map((i) => ({ value: i.path, label: i.name }))}
                aria-label={t("sprite.reference")}
                disabled={disabled || files.length === 0}
              />
            </label>
            {refPath ? (
              <SpriteGridPreview path={refPath} cut={sp.cut} disabled={disabled} />
            ) : (
              <p className="text-[11px] text-text-secondary">
                {t("sprite.previewEmpty")}
              </p>
            )}
          </div>
        </>
      ) : (
        <>
          <div className="flex flex-wrap items-end gap-x-6 gap-y-3">
            <TextField label={t("sprite.sheetName")} value={sp.pack.name} onChange={(name) => setPack({ name })} disabled={disabled} className="w-40" />
            <label className={fieldClass}>
              {t("sprite.cell")}
              <Segmented
                value={sp.pack.cell}
                onValueChange={(cell) =>
                  !disabled &&
                  setPack({ cell: cell as ModuleOptions["sprite"]["pack"]["cell"] })
                }
                options={[
                  { value: "auto", label: t("sprite.cellAuto") },
                  { value: "fixed", label: t("sprite.cellFixed") },
                ]}
              />
            </label>
            {sp.pack.cell === "fixed" && (
              <>
                <NumberField label={t("sprite.cellW")} value={sp.pack.cellW} onChange={(v) => setPack({ cellW: v ?? 1 })} min={1} max={16384} disabled={disabled} />
                <NumberField label={t("sprite.cellH")} value={sp.pack.cellH} onChange={(v) => setPack({ cellH: v ?? 1 })} min={1} max={16384} disabled={disabled} />
              </>
            )}
            <NumberField label={t("sprite.padding")} value={sp.pack.padding} onChange={(v) => setPack({ padding: v ?? 0 })} min={0} max={512} disabled={disabled} />
            <NumberField label={t("sprite.margin")} value={sp.pack.margin} onChange={(v) => setPack({ margin: v ?? 0 })} min={0} max={512} disabled={disabled} />
            <label className={fieldClass}>
              {t("sprite.order")}
              <Segmented
                value={sp.pack.order}
                onValueChange={(order) =>
                  !disabled &&
                  setPack({ order: order as ModuleOptions["sprite"]["pack"]["order"] })
                }
                options={[
                  { value: "name", label: t("sprite.orderName") },
                  { value: "area", label: t("sprite.orderArea") },
                  { value: "width", label: t("sprite.orderWidth") },
                  { value: "height", label: t("sprite.orderHeight") },
                ]}
              />
            </label>
          </div>
          <div className="flex flex-wrap items-end gap-x-6 gap-y-3">
            <NumberField label={t("sprite.maxW")} value={sp.pack.maxW} onChange={(v) => setPack({ maxW: v ?? 0 })} min={0} max={16384} disabled={disabled} />
            <NumberField label={t("sprite.maxH")} value={sp.pack.maxH} onChange={(v) => setPack({ maxH: v ?? 0 })} min={0} max={16384} disabled={disabled} />
          </div>
          <div className="flex flex-wrap gap-x-6 gap-y-2">
            <Check label={t("sprite.pot")} checked={sp.pack.powerOfTwo} onChange={(powerOfTwo) => setPack({ powerOfTwo })} disabled={disabled} />
            <Check label={t("sprite.trim")} checked={sp.pack.trim} onChange={(trim) => setPack({ trim })} disabled={disabled} />
            <Check label={t("sprite.packDedupe")} checked={sp.pack.dedupe} onChange={(dedupe) => setPack({ dedupe })} disabled={disabled} />
            <Check label={t("sprite.metadata")} checked={sp.pack.metadata} onChange={(metadata) => setPack({ metadata })} disabled={disabled} />
          </div>
          {(() => {
            const spec = spriteSpec(o);
            return spec.mode === "pack" ? (
              <SpritePackPreview
                spec={spec}
                inputs={files.map((f) => f.path)}
                disabled={disabled}
              />
            ) : null;
          })()}
        </>
      )}
      <FormatField module="sprites" o={o} set={set} disabled={disabled} formats={["png", "jpeg", "webp", "avif", "bmp", "tiff", "gif", "ico"]} allowSource />
      <QualityField module="sprites" o={o} set={set} disabled={disabled} />
    </div>
  );
}

/** Thumbnails: fixed social canvas, background, and an ordered layer stack. */
export function ThumbnailsPanel({ o, set, disabled, items }: PanelProps) {
  const { t } = useTranslation();
  const th = o.thumbnail;
  const files = items ?? [];
  const layers = th.layers ?? [];
  const sel = Math.min(th.sel ?? 0, Math.max(0, layers.length - 1));
  const layer = layers[sel];
  const [ref, setRef] = useState<string | null>(null);
  const refPath =
    files.find((i) => i.path === ref)?.path ?? files[0]?.path ?? null;
  const setThumb = (patch: Partial<ModuleOptions["thumbnail"]>) =>
    set({ thumbnail: { ...th, ...patch } });
  const setBg = (patch: Partial<ModuleOptions["thumbnail"]["bg"]>) =>
    setThumb({ bg: { ...th.bg, ...patch } });
  const bgAdjust = th.bgAdjust ?? { brightness: 0, contrast: 0, hue: 0, saturation: 0 };
  const setBgAdj = (patch: Partial<ModuleOptions["thumbnail"]["bgAdjust"]>) =>
    setThumb({ bgAdjust: { ...bgAdjust, ...patch } });
  const setLayers = (ls: ModuleOptions["thumbnail"]["layers"]) =>
    setThumb({ layers: ls, sel: Math.min(sel, Math.max(0, ls.length - 1)) });
  const setLayer = (
    i: number,
    patch: Partial<ModuleOptions["thumbnail"]["layers"][number]>,
  ) => setLayers(layers.map((l, j) => (j === i ? ({ ...l, ...patch } as typeof l) : l)));
  const setFx = (patch: Partial<ThumbImgFx>) => {
    if (!layer || layer.kind === "text") return;
    setLayer(sel, { img: { ...layer.img, ...patch } } as never);
  };
  const setText = (patch: Partial<ThumbTextOpt>) => {
    if (!layer || layer.kind !== "text") return;
    setLayer(sel, { text: { ...layer.text, ...patch } } as never);
  };
  const moveLayer = (i: number, dir: -1 | 1) => {
    const j = i + dir;
    if (j < 0 || j >= layers.length) return;
    const ls = [...layers];
    const [m] = ls.splice(i, 1);
    ls.splice(j, 0, m!);
    setThumb({ layers: ls, sel: j });
  };
  const addText = () =>
    setThumb({
      layers: [
        ...layers,
        { kind: "text", text: newThumbText(th.width, th.height, t("thumb.defaultText")) },
      ],
      sel: layers.length,
    });
  const addImage = async () => {
    const p = await ipc.pickImage();
    if (!p) return;
    setThumb({
      layers: [
        ...layers,
        {
          kind: "image",
          path: p,
          img: {
            ...DEFAULT_IMG_FX,
            fit: "contain" as const,
            zoomX: 40,
            zoomY: 40,
            y: Math.round(th.height / 4),
          },
        },
      ],
      sel: layers.length,
    });
  };
  const removeLayer = (i: number) => setLayers(layers.filter((_, j) => j !== i));
  const layerLabel = (l: ModuleOptions["thumbnail"]["layers"][number], i: number) =>
    l.kind === "source"
      ? t("thumb.layerSource")
      : l.kind === "image"
        ? (l.path.split(/[\\/]/).pop() ?? t("thumb.layerImage"))
        : (l.text.text || `${t("thumb.layerText")} ${i + 1}`);
  const presetOptions = [
    ...THUMB_PRESETS.map((p) => ({
      value: p.id,
      label: `${p.label} · ${p.w}×${p.h}`,
    })),
    { value: "custom", label: t("thumb.custom") },
  ];
  const fx = layer && layer.kind !== "text" ? layer.img : null;
  return (
    <div className="flex w-full flex-col gap-4">
      <div className="flex flex-wrap items-end gap-x-6 gap-y-3">
        <label className={fieldClass}>
          {t("thumb.preset")}
          <Select<string>
            value={th.presetId}
            onValueChange={(id) => {
              const p = THUMB_PRESETS.find((p) => p.id === id);
              setThumb(
                p ? { presetId: id, width: p.w, height: p.h } : { presetId: id },
              );
            }}
            options={presetOptions}
            aria-label={t("thumb.preset")}
            disabled={disabled}
          />
        </label>
        {th.presetId === "custom" && (
          <>
            <NumberField label={t("thumb.width")} value={th.width} onChange={(v) => setThumb({ width: v ?? 1 })} min={1} max={16384} disabled={disabled} />
            <NumberField label={t("thumb.height")} value={th.height} onChange={(v) => setThumb({ height: v ?? 1 })} min={1} max={16384} disabled={disabled} />
          </>
        )}
      </div>

      <div className="grid grid-cols-1 items-start gap-3 xl:grid-cols-[minmax(0,1fr)_340px]">
        <div className="flex flex-col gap-2">
          <label className={fieldClass}>
            {t("sprite.reference")}
            <Select<string>
              value={refPath ?? ""}
              onValueChange={setRef}
              options={files.map((i) => ({ value: i.path, label: i.name }))}
              aria-label={t("sprite.reference")}
              disabled={disabled || files.length === 0}
            />
          </label>
          {refPath ? (
            <ThumbnailEditor path={refPath} o={o} setThumb={setThumb} disabled={disabled} />
          ) : (
            <p className="text-[11px] text-text-secondary">{t("sprite.previewEmpty")}</p>
          )}
        </div>

        <div className="flex min-h-0 flex-col gap-3 xl:max-h-[80vh] xl:overflow-y-auto xl:pr-1">
        {/* Layer stack: top of the list is drawn last (on top). */}
        <div className="flex flex-col gap-1.5 self-start rounded-lg border border-border p-2">
          <div className="flex items-center justify-between gap-1">
            <span className="text-xs font-medium text-text">{t("thumb.layers")}</span>
            <div className="flex gap-1">
              <Button size="sm" variant="secondary" onClick={addText} disabled={disabled} title={t("thumb.addText")}>
                +T
              </Button>
              <Button size="sm" variant="secondary" onClick={() => void addImage()} disabled={disabled} title={t("thumb.addImage")}>
                +IMG
              </Button>
            </div>
          </div>
          <div className="flex max-h-72 flex-col gap-1 overflow-y-auto">
            {[...layers].reverse().map((l, ri) => {
              const i = layers.length - 1 - ri;
              return (
                <div
                  key={i}
                  className={cn(
                    "flex items-center gap-1 rounded-md border px-1.5 py-1",
                    sel === i ? "border-accent bg-accent/10" : "border-border",
                  )}
                >
                  <button
                    type="button"
                    onClick={() => setThumb({ sel: i })}
                    className="min-w-0 flex-1 truncate text-left text-[11px] text-text"
                  >
                    {layerLabel(l, i)}
                  </button>
                  <button type="button" disabled={disabled || i === layers.length - 1} onClick={() => moveLayer(i, 1)} className="px-0.5 text-text-secondary hover:text-text disabled:opacity-30" title={t("thumb.layerUp")}>
                    ▲
                  </button>
                  <button type="button" disabled={disabled || i === 0} onClick={() => moveLayer(i, -1)} className="px-0.5 text-text-secondary hover:text-text disabled:opacity-30" title={t("thumb.layerDown")}>
                    ▼
                  </button>
                  <button type="button" disabled={disabled || layers.length <= 1} onClick={() => removeLayer(i)} className="px-0.5 text-red-400 hover:text-red-300 disabled:opacity-30" title={t("thumb.layerRemove")}>
                    ✕
                  </button>
                </div>
              );
            })}
            {layers.length === 0 && (
              <p className="py-2 text-center text-[11px] text-text-secondary">{t("thumb.layersEmpty")}</p>
            )}
          </div>
        </div>
      {/* Options of the selected layer. */}
      {fx && layer && (
        <div className="flex flex-col gap-3 rounded-lg border border-border p-3">
          <span className="text-xs font-medium text-text">
            {t("thumb.layerOptions", { name: layerLabel(layer, sel) })}
          </span>
          <div className="flex flex-wrap items-end gap-x-6 gap-y-3">
            <label className={fieldClass}>
              {t("thumb.fit")}
              <Segmented
                value={fx.fit}
                onValueChange={(fit) => !disabled && setFx({ fit: fit as ThumbImgFx["fit"] })}
                options={[
                  { value: "cover", label: t("thumb.cover") },
                  { value: "contain", label: t("thumb.contain") },
                  { value: "stretch", label: t("thumb.stretch") },
                ]}
              />
            </label>
            <NumberField label={t("thumb.radius")} value={fx.radius} onChange={(v) => setFx({ radius: v ?? 0 })} min={0} max={2000} disabled={disabled} />
            <NumberField label={t("thumb.borderW")} value={fx.borderW} onChange={(v) => setFx({ borderW: v ?? 0 })} min={0} max={200} disabled={disabled} />
            {fx.borderW > 0 && (
              <label className={fieldClass}>
                {t("thumb.borderColor")}
                <input type="color" value={fx.borderColor} onChange={(e) => setFx({ borderColor: e.target.value })} disabled={disabled} className="h-8 w-12 cursor-pointer rounded border border-border bg-transparent" />
              </label>
            )}
            <NumberField label={t("thumb.outlineW")} value={fx.outlineW} onChange={(v) => setFx({ outlineW: v ?? 0 })} min={0} max={200} disabled={disabled} />
            {fx.outlineW > 0 && (
              <label className={fieldClass}>
                {t("thumb.outlineColor")}
                <input type="color" value={fx.outlineColor} onChange={(e) => setFx({ outlineColor: e.target.value })} disabled={disabled} className="h-8 w-12 cursor-pointer rounded border border-border bg-transparent" />
              </label>
            )}
            <Check label={t("thumb.shadow")} checked={fx.shadow} onChange={(shadow) => setFx({ shadow })} disabled={disabled} />
            <Check label={t("thumb.flipH")} checked={fx.flipH} onChange={(flipH) => setFx({ flipH })} disabled={disabled} />
            <Check label={t("thumb.flipV")} checked={fx.flipV} onChange={(flipV) => setFx({ flipV })} disabled={disabled} />
          </div>
          <div className="flex flex-wrap items-end gap-x-6 gap-y-3">
            <SliderField label={t("thumb.brightness")} value={fx.brightness} onChange={(brightness) => setFx({ brightness })} min={-100} max={100} disabled={disabled} />
            <SliderField label={t("thumb.contrast")} value={fx.contrast} onChange={(contrast) => setFx({ contrast })} min={-100} max={100} disabled={disabled} />
            <SliderField label={t("thumb.hue")} value={fx.hue} onChange={(hue) => setFx({ hue })} min={0} max={360} disabled={disabled} />
            <SliderField label={t("thumb.saturation")} value={fx.saturation} onChange={(saturation) => setFx({ saturation })} min={-100} max={100} disabled={disabled} />
            <SliderField label={t("thumb.opacity")} value={fx.opacity} onChange={(opacity) => setFx({ opacity })} min={10} max={100} disabled={disabled} />
          </div>
        </div>
      )}
      {layer?.kind === "text" && (
        <div className="flex flex-wrap items-end gap-x-4 gap-y-2 rounded-lg border border-border p-3">
          <TextField label={t("thumb.text")} value={layer.text.text} onChange={(text) => setText({ text })} disabled={disabled} className="w-48" />
          <label className={fieldClass}>
            {t("thumb.font")}
            <Select<string>
              value={layer.text.font}
              onValueChange={(font) => setText({ font })}
              options={THUMB_FONTS.map((f) => ({
                value: f.id,
                label: f.label,
                style: { fontFamily: f.css },
              }))}
              aria-label={t("thumb.font")}
              disabled={disabled}
            />
          </label>
          <NumberField label={t("thumb.textSize")} value={layer.text.size} onChange={(v) => setText({ size: v ?? 8 })} min={4} max={4096} disabled={disabled} />
          <label className={fieldClass}>
            {t("thumb.textColor")}
            <input type="color" value={layer.text.color} onChange={(e) => setText({ color: e.target.value })} disabled={disabled} className="h-8 w-12 cursor-pointer rounded border border-border bg-transparent" />
          </label>
          <NumberField label={t("thumb.strokeW")} value={layer.text.strokeW} onChange={(v) => setText({ strokeW: v ?? 0 })} min={0} max={100} disabled={disabled} />
          {layer.text.strokeW > 0 && (
            <label className={fieldClass}>
              {t("thumb.strokeColor")}
              <input type="color" value={layer.text.strokeColor} onChange={(e) => setText({ strokeColor: e.target.value })} disabled={disabled} className="h-8 w-12 cursor-pointer rounded border border-border bg-transparent" />
            </label>
          )}
          <NumberField label={t("thumb.rotate")} value={layer.text.rotate} onChange={(v) => setText({ rotate: v ?? 0 })} min={-180} max={180} disabled={disabled} />
          <Check label={t("thumb.uppercase")} checked={layer.text.uppercase} onChange={(uppercase) => setText({ uppercase })} disabled={disabled} />
          <NumberField label={t("thumb.shadowDx")} value={layer.text.shadowDx} onChange={(v) => setText({ shadowDx: v ?? 0 })} min={-200} max={200} disabled={disabled} />
          <NumberField label={t("thumb.shadowDy")} value={layer.text.shadowDy} onChange={(v) => setText({ shadowDy: v ?? 0 })} min={-200} max={200} disabled={disabled} />
          <NumberField label={t("thumb.shadowBlur")} value={layer.text.shadowBlur} onChange={(v) => setText({ shadowBlur: v ?? 0 })} min={0} max={100} disabled={disabled} />
          {(layer.text.shadowDx !== 0 || layer.text.shadowDy !== 0 || layer.text.shadowBlur > 0) && (
            <label className={fieldClass}>
              {t("thumb.shadowColor")}
              <input type="color" value={layer.text.shadowColor} onChange={(e) => setText({ shadowColor: e.target.value })} disabled={disabled} className="h-8 w-12 cursor-pointer rounded border border-border bg-transparent" />
            </label>
          )}
        </div>
      )}

      <div className="flex flex-wrap items-end gap-x-6 gap-y-3">
        <label className={fieldClass}>
          {t("thumb.bg")}
          <Segmented
            value={th.bg.type}
            onValueChange={(type) =>
              !disabled &&
              setBg({ type: type as ModuleOptions["thumbnail"]["bg"]["type"] })
            }
            options={[
              { value: "blur", label: t("thumb.bgBlur") },
              { value: "color", label: t("thumb.bgColor") },
              { value: "gradient", label: t("thumb.bgGradient") },
              { value: "image", label: t("thumb.bgImage") },
              { value: "transparent", label: t("thumb.bgTransparent") },
            ]}
          />
        </label>
        {th.bg.type === "image" && (
          <>
            <Button
              size="sm"
              variant="secondary"
              disabled={disabled}
              onClick={async () => {
                const p = await ipc.pickImage();
                if (p) setBg({ imagePath: p });
              }}
            >
              {th.bg.imagePath ? t("thumb.bgImageChange") : t("thumb.bgImagePick")}
            </Button>
            {th.bg.imagePath && (
              <span className="max-w-48 truncate self-center text-[11px] text-text-secondary" title={th.bg.imagePath}>
                {th.bg.imagePath.split(/[\\/]/).pop()}
              </span>
            )}
            <NumberField label={t("thumb.bgBlurRadius")} value={th.bg.blurRadius} onChange={(v) => setBg({ blurRadius: v ?? 0 })} min={0} max={200} disabled={disabled} />
            <NumberField label={t("thumb.bgDim")} value={th.bg.dim} onChange={(v) => setBg({ dim: v ?? 0 })} min={0} max={100} disabled={disabled} />
          </>
        )}
        {th.bg.type === "color" && (
          <label className={fieldClass}>
            {t("thumb.bgColor")}
            <input type="color" value={th.bg.color} onChange={(e) => setBg({ color: e.target.value })} disabled={disabled} className="h-8 w-12 cursor-pointer rounded border border-border bg-transparent" />
          </label>
        )}
        {th.bg.type === "gradient" && (
          <>
            <label className={fieldClass}>
              {t("thumb.bgFrom")}
              <input type="color" value={th.bg.from} onChange={(e) => setBg({ from: e.target.value })} disabled={disabled} className="h-8 w-12 cursor-pointer rounded border border-border bg-transparent" />
            </label>
            <label className={fieldClass}>
              {t("thumb.bgTo")}
              <input type="color" value={th.bg.to} onChange={(e) => setBg({ to: e.target.value })} disabled={disabled} className="h-8 w-12 cursor-pointer rounded border border-border bg-transparent" />
            </label>
            <NumberField label={t("thumb.bgAngle")} value={th.bg.angle} onChange={(v) => setBg({ angle: v ?? 0 })} min={0} max={360} disabled={disabled} />
          </>
        )}
        {th.bg.type === "blur" && (
          <NumberField label={t("thumb.bgBlurRadius")} value={th.bg.blurRadius} onChange={(v) => setBg({ blurRadius: v ?? 0 })} min={1} max={200} disabled={disabled} />
        )}
      </div>
      <div className="flex flex-wrap items-end gap-x-6 gap-y-3">
        <SliderField label={t("thumb.bgBrightness")} value={bgAdjust.brightness} onChange={(brightness) => setBgAdj({ brightness })} min={-100} max={100} disabled={disabled} />
        <SliderField label={t("thumb.bgContrast")} value={bgAdjust.contrast} onChange={(contrast) => setBgAdj({ contrast })} min={-100} max={100} disabled={disabled} />
        <SliderField label={t("thumb.bgHue")} value={bgAdjust.hue} onChange={(hue) => setBgAdj({ hue })} min={0} max={360} disabled={disabled} />
        <SliderField label={t("thumb.bgSaturation")} value={bgAdjust.saturation} onChange={(saturation) => setBgAdj({ saturation })} min={-100} max={100} disabled={disabled} />
        <SliderField label={t("thumb.vignette")} value={th.vignette} onChange={(vignette) => setThumb({ vignette })} min={0} max={100} disabled={disabled} />
      </div>
        </div>
      </div>

      <FormatField module="thumbnails" o={o} set={set} disabled={disabled} formats={["png", "jpeg", "webp", "avif", "bmp", "tiff", "gif", "ico"]} allowSource />
      <QualityField module="thumbnails" o={o} set={set} disabled={disabled} />
    </div>
  );
}

/** Pipeline: the user stacks crop/rotate, resize, watermark and rename in one job. */
export function PipelinePanel(props: PanelProps) {
  const { t } = useTranslation();
  const { o, set, disabled } = props;
  // Each embedded panel would otherwise read/write the shared presetId;
  // scope it per step so resize and watermark presets can't collide.
  const scopePreset = (
    field: "pipelineResizePresetId" | "pipelineWmPresetId",
  ): PanelProps => ({
    ...props,
    o: { ...o, presetId: o[field] },
    set: (patch) => {
      const { presetId, ...rest } = patch;
      set({
        ...rest,
        ...(presetId !== undefined ? { [field]: presetId } : {}),
      });
    },
  });
  const section = (
    key: "transform" | "resize" | "watermark" | "rename",
    content: React.ReactNode,
  ) => (
    <div className="flex basis-full flex-wrap items-center gap-x-6 gap-y-3 rounded-lg border border-border px-3 py-2">
      <Check
        label={t(`pipe.${key}`)}
        checked={o.pipelineUse[key]}
        onChange={(v) =>
          set({ pipelineUse: { ...o.pipelineUse, [key]: v } })
        }
        disabled={disabled}
      />
      {o.pipelineUse[key] && (
        <div className="flex basis-full flex-wrap items-end gap-x-6 gap-y-3">
          {content}
        </div>
      )}
    </div>
  );
  return (
    <>
      <FormatField {...props} formats={ALL_FORMATS} allowSource />
      <CommonEncoding {...props} />
      {section("transform", <CropRotatePanel {...props} />)}
      {section("resize", <ResizerPanel {...scopePreset("pipelineResizePresetId")} />)}
      {section("watermark", <WatermarkPanel {...scopePreset("pipelineWmPresetId")} />)}
      {section("rename", <RenamerPanel {...props} />)}
    </>
  );
}

function Stat({ label, value }: { label: string; value: string }) {
  return (
    <div className="flex min-w-24 flex-col gap-0.5">
      <span className="text-[11px] uppercase tracking-wide text-text-secondary">
        {label}
      </span>
      <span className="text-sm font-medium tabular-nums text-text">{value}</span>
    </div>
  );
}

export function AnalyzerPanel({ items }: PanelProps) {
  const { t } = useTranslation();
  const files = items ?? [];
  const stats = aggregateAnalysis(files);
  const formats = Object.entries(stats.formats)
    .map(([f, n]) => `${f.toUpperCase()} ${n}`)
    .join(" · ");
  return (
    <div className="flex w-full flex-wrap items-center gap-x-8 gap-y-3">
      <Stat
        label={t("analyzer.statAnalyzed")}
        value={`${stats.analyzed}/${files.length}`}
      />
      <Stat
        label={t("analyzer.statSize")}
        value={formatBytes(stats.totalBytes)}
      />
      <Stat
        label={t("analyzer.statEst")}
        value={stats.analyzed > 0 ? formatBytes(stats.estBytes) : "—"}
      />
      <Stat
        label={t("analyzer.statSaving")}
        value={
          stats.savingPercent !== null
            ? `${stats.savingPercent >= 0 ? "−" : "+"}${Math.abs(stats.savingPercent).toFixed(1)}%`
            : "—"
        }
      />
      <Stat label={t("analyzer.statAlpha")} value={String(stats.withAlpha)} />
      <Stat
        label={t("analyzer.statAnimated")}
        value={String(stats.animated)}
      />
      <Stat label={t("analyzer.statExif")} value={String(stats.withExif)} />
      {formats && (
        <span className="text-xs text-text-secondary">{formats}</span>
      )}
      <p className="w-full text-xs text-text-secondary">{t("analyzer.hint")}</p>
    </div>
  );
}
