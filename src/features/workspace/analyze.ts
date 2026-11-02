import type { FileAnalysis } from "@/lib/ipc";

export interface AnalysisInput {
  size: number;
  status: string;
  analysis: FileAnalysis | null;
}

export interface BatchStats {
  analyzed: number;
  failed: number;
  totalBytes: number;
  /** Sum of per-file estimated WebP q80 sizes (files without one are skipped). */
  estBytes: number;
  /** Percent saved by re-encoding everything as WebP q80; null without data. */
  savingPercent: number | null;
  formats: Record<string, number>;
  withAlpha: number;
  animated: number;
  withExif: number;
}

/** Folder-wide statistics the Analyzer shows on top of the per-file grid. */
export function aggregateAnalysis(items: AnalysisInput[]): BatchStats {
  const stats: BatchStats = {
    analyzed: 0,
    failed: 0,
    totalBytes: 0,
    estBytes: 0,
    savingPercent: null,
    formats: {},
    withAlpha: 0,
    animated: 0,
    withExif: 0,
  };
  let estCovered = 0;
  for (const i of items) {
    if (i.status === "failed") stats.failed++;
    const a = i.analysis;
    if (!a) continue;
    stats.analyzed++;
    stats.totalBytes += i.size;
    stats.formats[a.format] = (stats.formats[a.format] ?? 0) + 1;
    if (a.hasAlpha) stats.withAlpha++;
    if (a.animated) stats.animated++;
    if (a.hasExif) stats.withExif++;
    if (a.estWebpBytes !== null) {
      stats.estBytes += a.estWebpBytes;
      estCovered += i.size;
    }
  }
  stats.savingPercent =
    estCovered > 0 ? (1 - stats.estBytes / estCovered) * 100 : null;
  return stats;
}
