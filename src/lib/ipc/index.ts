import { Channel, invoke } from "@tauri-apps/api/core";
import type { Settings } from "./bindings/Settings";
import type { Job } from "./bindings/Job";
import type { JobEvent } from "./bindings/JobEvent";
import type { AnalyzeEvent } from "./bindings/AnalyzeEvent";
import type { ScanResult } from "./bindings/ScanResult";
import type { PlannedOutput } from "./bindings/PlannedOutput";
import type { ResizePreset } from "./bindings/ResizePreset";

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
export type { AnalysisResult } from "./bindings/AnalysisResult";
export type { AnalyzeEvent } from "./bindings/AnalyzeEvent";
export type { FileAnalysis } from "./bindings/FileAnalysis";

/** Shape of errors returned by Rust commands (see src-tauri/src/error.rs). */
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
  analyzeFiles: (paths: string[], onEvent: (event: AnalyzeEvent) => void) => {
    const channel = new Channel<AnalyzeEvent>();
    channel.onmessage = onEvent;
    return invoke<void>("analyze_files", { paths, onEvent: channel });
  },
  exportPresets: (path: string, contents: string) =>
    invoke<void>("export_presets", { path, contents }),
  importPresets: (path: string) =>
    invoke<string>("import_presets", { path }),
  previewJob: (job: Job) => invoke<PlannedOutput[]>("preview_job", { job }),
  loadImage: (path: string) => invoke<ArrayBuffer>("load_image", { path }),
  listResizePresets: () => invoke<ResizePreset[]>("list_resize_presets"),
  saveResizePreset: (preset: ResizePreset) => invoke<ResizePreset>("save_resize_preset", { preset }),
  deleteResizePreset: (id: number) => invoke<void>("delete_resize_preset", { id }),
};
