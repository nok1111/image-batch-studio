import { useEffect, useState } from "react";
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
  type ResizeMode,
  type ResizePreset,
  type WatermarkKind,
  type WatermarkPosition,
  type WatermarkPreset,
  type WatermarkSpec,
} from "@/lib/ipc";
import { cn } from "@/lib/utils";
import { CropEditor } from "./CropEditor";
import { WatermarkPreview } from "./WatermarkPreview";
import type { QueueItem } from "@/stores/queue";
import {
  COMPRESS_QUALITY,
  effectiveQuality,
  supportsTargetSize,
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

export function RenamerPanel({ o, set, disabled }: PanelProps) {
  const { t } = useTranslation();
  const r = o.rename;
  const setR = (patch: Partial<ModuleOptions["rename"]>) =>
    set({ rename: { ...r, ...patch } });
  return (
    <>
      <TextField
        label={t("options.template")}
        value={r.template}
        onChange={(template) => setR({ template })}
        disabled={disabled}
        className="w-56"
        placeholder="{name}_{n:03}_{date}"
      />
      <TextField
        label={t("options.prefix")}
        value={r.prefix}
        onChange={(prefix) => setR({ prefix })}
        disabled={disabled}
        className="w-28"
      />
      <TextField
        label={t("options.suffix")}
        value={r.suffix}
        onChange={(suffix) => setR({ suffix })}
        disabled={disabled}
        className="w-28"
      />
      <TextField
        label={t("options.find")}
        value={r.find}
        onChange={(find) => setR({ find })}
        disabled={disabled}
        className="w-28"
      />
      <TextField
        label={t("options.replace")}
        value={r.replace}
        onChange={(replace) => setR({ replace })}
        disabled={disabled}
        className="w-28"
      />
      <NumberField
        label={t("options.start")}
        value={r.start}
        onChange={(v) => setR({ start: v ?? 0 })}
        min={0}
        max={999_999_999}
        disabled={disabled}
      />
      <p className="w-full text-xs text-text-secondary">
        {t("options.templateHint")}
      </p>
    </>
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
