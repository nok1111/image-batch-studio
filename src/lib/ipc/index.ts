import { Channel, invoke } from "@tauri-apps/api/core";
import type { Settings } from "./bindings/Settings";
import type { Job } from "./bindings/Job";
import type { JobEvent } from "./bindings/JobEvent";
import type { AnalyzeEvent } from "./bindings/AnalyzeEvent";
import type { ScanResult } from "./bindings/ScanResult";
import type { PlannedOutput } from "./bindings/PlannedOutput";
import type { ResizePreset } from "./bindings/ResizePreset";
import type { WatermarkPreset } from "./bindings/WatermarkPreset";

export type { Settings } from "./bindings/Settings";
export type { Theme } from "./bindings/Theme";
export type { Language } from "./bindings/Language";
export type { ImageFormat } from "./bindings/ImageFormat";
export type { ConflictPolicy } from "./bindings/ConflictPolicy";
export type { FileError } from "./bindings/FileError";
export type { FileErrorCode } from "./bindings/FileErrorCode";
export type { FileResult } from "./bindings/FileResult";
export type { FileStatus } from "./bindings/FileStatus";
export type { FileWarning } from "./bindings/FileWarning";
export type { InputFile } from "./bindings/InputFile";
export type { Job } from "./bindings/Job";
export type { CropSpec } from "./bindings/CropSpec";
export type { TransformSpec } from "./bindings/TransformSpec";
export type { JobEvent } from "./bindings/JobEvent";
export type { JobSummary } from "./bindings/JobSummary";
export type { OutputSpec } from "./bindings/OutputSpec";
export type { ScanResult } from "./bindings/ScanResult";
export type { MetadataPolicy } from "./bindings/MetadataPolicy";
export type { OutputMode } from "./bindings/OutputMode";
export type { PlannedAction } from "./bindings/PlannedAction";
export type { PlannedOutput } from "./bindings/PlannedOutput";
export type { RenameSpec } from "./bindings/RenameSpec";
export type { ResizeMode } from "./bindings/ResizeMode";
export type { ResizePreset } from "./bindings/ResizePreset";
export type { ResizeSpec } from "./bindings/ResizeSpec";
export type { WatermarkKind } from "./bindings/WatermarkKind";
export type { WatermarkPosition } from "./bindings/WatermarkPosition";
export type { WatermarkPreset } from "./bindings/WatermarkPreset";
export type { WatermarkSpec } from "./bindings/WatermarkSpec";
export type { AnalysisResult } from "./bindings/AnalysisResult";
export type { AnalyzeEvent } from "./bindings/AnalyzeEvent";
export type { FileAnalysis } from "./bindings/FileAnalysis";

/** Shape of errors returned by Rust commands (see src-tauri/src/error.rs). */
export interface LicenseStatus {
  /** "trial" | "licensed" | "expired" */
  state: string;
  daysLeft: number;
  graceDaysLeft: number | null;
  maskedKey: string | null;
  email: string | null;
  providerReady: boolean;
}

/** Public checkout URL; shown as "Buy a license" when configured. */
export const BUY_URL = "https://nok1111.lemonsqueezy.com/checkout";

export interface CommandError {
  code: string;
  message: string;
}

export interface AppInfo {
  name: string;
  version: string;
  tauriVersion: string;
  os: string;
  arch: string;
  logicalCpus: number;
}

export function errorMessage(e: unknown): string {
  if (e && typeof e === "object" && "message" in e) return String((e as CommandError).message);
  return String(e);
}

export const ipc = {
  appInfo: () => invoke<AppInfo>("app_info"),
  getSettings: () => invoke<Settings>("get_settings"),
  saveSettings: (settings: Settings) => invoke<Settings>("save_settings", { settings }),
  resetSettings: () => invoke<Settings>("reset_settings"),
  openOutputDir: () => invoke<void>("open_output_dir"),
  scanInputs: (paths: string[]) => invoke<ScanResult>("scan_inputs", { paths }),
  startJob: (job: Job, onEvent: (event: JobEvent) => void) => {
    const channel = new Channel<JobEvent>();
    channel.onmessage = onEvent;
    return invoke<void>("start_job", { job, onEvent: channel });
  },
  cancelJob: () => invoke<boolean>("cancel_job"),
  licenseStatus: () => invoke<LicenseStatus>("license_status"),
  licenseActivate: (key: string) =>
    invoke<LicenseStatus>("license_activate", { key }),
  licenseDeactivate: () => invoke<LicenseStatus>("license_deactivate"),
  analyzeFiles: (paths: string[], onEvent: (event: AnalyzeEvent) => void) => {
    const channel = new Channel<AnalyzeEvent>();
    channel.onmessage = onEvent;
    return invoke<void>("analyze_files", { paths, onEvent: channel });
  },
  /** Opens the save dialog in Rust; resolves to the written path or null (cancelled). */
  exportPresets: (contents: string) =>
    invoke<string | null>("export_presets", { contents }),
  /** Opens the picker in Rust; resolves to file contents or null (cancelled). */
  importPresets: () => invoke<string | null>("import_presets"),
  previewJob: (job: Job) => invoke<PlannedOutput[]>("preview_job", { job }),
  loadImage: (path: string) => invoke<ArrayBuffer>("load_image", { path }),
  listResizePresets: () => invoke<ResizePreset[]>("list_resize_presets"),
  saveResizePreset: (preset: ResizePreset) => invoke<ResizePreset>("save_resize_preset", { preset }),
  deleteResizePreset: (id: number) => invoke<void>("delete_resize_preset", { id }),
  listWatermarkPresets: () => invoke<WatermarkPreset[]>("list_watermark_presets"),
  saveWatermarkPreset: (preset: WatermarkPreset) =>
    invoke<WatermarkPreset>("save_watermark_preset", { preset }),
  deleteWatermarkPreset: (id: number) => invoke<void>("delete_watermark_preset", { id }),
  pickWatermarkImage: () => invoke<string | null>("pick_watermark_image"),
  registerWatermarkImage: (path: string) =>
    invoke<void>("register_watermark_image", { path }),
};
