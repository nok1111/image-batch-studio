import type { LucideIcon } from "lucide-react";
import {
  Crop,
  LayoutDashboard,
  Minimize2,
  RefreshCw,
  Scaling,
  ScanSearch,
  Settings,
  ShieldCheck,
  Stamp,
  TextCursorInput,
  Workflow,
} from "lucide-react";

export type ModuleId =
  | "converter"
  | "compressor"
  | "resizer"
  | "watermark"
  | "renamer"
  | "metadata"
  | "cropRotate"
  | "analyzer"
  | "pipeline";

export interface ToolModule {
  id: ModuleId;
  path: string;
  icon: LucideIcon;
  /** false until the module is fully implemented and tested */
  available: boolean;
  accent: "accent" | "teal";
}

/** Sidebar order follows the product spec. */
export const TOOL_MODULES: ToolModule[] = [
  {
    id: "converter",
    path: "/converter",
    icon: RefreshCw,
    available: true,
    accent: "accent",
  },
  {
    id: "compressor",
    path: "/compressor",
    icon: Minimize2,
    available: true,
    accent: "teal",
  },
  {
    id: "resizer",
    path: "/resizer",
    icon: Scaling,
    available: true,
    accent: "accent",
  },
  {
    id: "watermark",
    path: "/watermark",
    icon: Stamp,
    available: true,
    accent: "teal",
  },
  {
    id: "renamer",
    path: "/renamer",
    icon: TextCursorInput,
    available: true,
    accent: "accent",
  },
  {
    id: "metadata",
    path: "/metadata",
    icon: ShieldCheck,
    available: true,
    accent: "teal",
  },
  {
    id: "cropRotate",
    path: "/crop-rotate",
    icon: Crop,
    available: true,
    accent: "accent",
  },
  {
    id: "analyzer",
    path: "/analyzer",
    icon: ScanSearch,
    available: false,
    accent: "teal",
  },
  {
    id: "pipeline",
    path: "/pipeline",
    icon: Workflow,
    available: true,
    accent: "accent",
  },
];

export const NAV_ITEMS = [
  { key: "dashboard", path: "/", icon: LayoutDashboard },
  ...TOOL_MODULES.map((m) => ({ key: m.id, path: m.path, icon: m.icon })),
  { key: "settings", path: "/settings", icon: Settings },
] as const;

export const QUICK_ACTIONS: { key: string; module: ModuleId }[] = [
  { key: "convert", module: "converter" },
  { key: "compress", module: "compressor" },
  { key: "resize", module: "resizer" },
  { key: "watermark", module: "watermark" },
  { key: "rename", module: "renamer" },
];

export const moduleById = (id: ModuleId) =>
  TOOL_MODULES.find((m) => m.id === id)!;
