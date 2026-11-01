import { useEffect, useRef, useState, type PointerEvent } from "react";
import { useTranslation } from "react-i18next";
import { ipc, type CropSpec } from "@/lib/ipc";
import { cn } from "@/lib/utils";
import { displayMime } from "./display";

type Mode = "new" | "move" | "nw" | "ne" | "sw" | "se";

interface Drag {
  mode: Mode;
  startX: number;
  startY: number;
  base: CropSpec;
}

const clamp = (v: number, min: number, max: number) =>
  Math.min(max, Math.max(min, v));

/** Draws a crop rectangle over a source image; emits source-pixel coordinates. */
export function CropEditor({
  path,
  crop,
  onChange,
  onSize,
  disabled,
}: {
  path: string;
  crop: CropSpec | null;
  onChange: (c: CropSpec) => void;
  onSize?: (size: [number, number] | null) => void;
  disabled?: boolean;
}) {
  const { t } = useTranslation();
  const stage = useRef<HTMLDivElement>(null);
  const onSizeRef = useRef(onSize);
  onSizeRef.current = onSize;
  const drag = useRef<Drag | null>(null);
  void 0;
  const [nat, setNat] = useState<[number, number] | null>(null);
  const [url, setUrl] = useState<string | null>(null);
  const [failed, setFailed] = useState(false);

  useEffect(() => {
    let objectUrl: string | null = null;
    let live = true;
    setUrl(null);
    setNat(null);
    setFailed(false);
    onSizeRef.current?.(null);
    ipc
      .loadImage(path)
      .then((buf) => {
        objectUrl = URL.createObjectURL(
          new Blob([buf], { type: displayMime(path) }),
        );
        if (live) setUrl(objectUrl);
        else URL.revokeObjectURL(objectUrl);
      })
      .catch(() => live && setFailed(true));
    return () => {
      live = false;
      if (objectUrl) URL.revokeObjectURL(objectUrl);
    };
  }, [path]);

  const scale = () => {
    const el = stage.current;
    if (!el || !nat || nat[0] === 0 || nat[1] === 0) return 1;
    return Math.min(el.clientWidth / nat[0], el.clientHeight / nat[1]);
  };
  const imgBox = () => {
    const el = stage.current;
    if (!el || !nat) return { x: 0, y: 0, w: 0, h: 0 };
    const s = scale();
    return {
      x: (el.clientWidth - nat[0] * s) / 2,
      y: (el.clientHeight - nat[1] * s) / 2,
      w: nat[0] * s,
      h: nat[1] * s,
    };
  };
  const toClient = (c: CropSpec) => {
    const b = imgBox();
    const s = scale();
    return {
      x: b.x + c.x * s,
      y: b.y + c.y * s,
      w: c.width * s,
      h: c.height * s,
    };
  };
  const local = (e: PointerEvent) => {
    const el = stage.current!;
    const r = el.getBoundingClientRect();
    return {
      x: ((e.clientX - r.left) / r.width) * el.clientWidth,
      y: ((e.clientY - r.top) / r.height) * el.clientHeight,
    };
  };
  const toSource = (p: { x: number; y: number }) => {
    const b = imgBox();
    const s = scale();
    return {
      x: clamp(Math.round((p.x - b.x) / s), 0, nat![0] - 1),
      y: clamp(Math.round((p.y - b.y) / s), 0, nat![1] - 1),
    };
  };

  const down = (e: PointerEvent) => {
    if (disabled || !nat) return;
    const mode =
      ((e.target as HTMLElement).dataset.mode as Mode | undefined) ?? "new";
    const base = crop ?? { x: 0, y: 0, width: nat[0], height: nat[1] };
    const p = local(e);
    drag.current = { mode, startX: p.x, startY: p.y, base };
    e.currentTarget.setPointerCapture(e.pointerId);
    e.preventDefault();
  };
  const move = (e: PointerEvent) => {
    const d = drag.current;
    if (!d || !nat) return;
    const p = local(e);
    const a = toSource({ x: d.startX, y: d.startY });
    const b = toSource(p);
    let c = d.base;
    if (d.mode === "new") {
      c = {
        x: Math.min(a.x, b.x),
        y: Math.min(a.y, b.y),
        width: Math.max(1, Math.abs(b.x - a.x)),
        height: Math.max(1, Math.abs(b.y - a.y)),
      };
    } else if (d.mode === "move") {
      c = {
        ...d.base,
        x: clamp(d.base.x + b.x - a.x, 0, nat[0] - d.base.width),
        y: clamp(d.base.y + b.y - a.y, 0, nat[1] - d.base.height),
      };
    } else {
      const left = d.mode === "nw" || d.mode === "sw";
      const top = d.mode === "nw" || d.mode === "ne";
      const x1 = left ? b.x : d.base.x;
      const y1 = top ? b.y : d.base.y;
      const x2 = left ? d.base.x + d.base.width : b.x + 1;
      const y2 = top ? d.base.y + d.base.height : b.y + 1;
      c = {
        x: Math.min(x1, x2 - 1),
        y: Math.min(y1, y2 - 1),
        width: Math.max(1, x2 - x1),
        height: Math.max(1, y2 - y1),
      };
    }
    onChange(c);
  };
  const up = () => (drag.current = null);

  const shown = crop && nat ? toClient(crop) : null;
  const handle = (mode: Mode, style: React.CSSProperties) => (
    <div
      key={mode}
      data-mode={mode}
      className="absolute z-10 size-3 rounded-sm border-2 border-white bg-accent shadow"
      style={style}
    />
  );

  return (
    <div
      ref={stage}
      className={cn(
        "relative w-full overflow-hidden rounded-lg border border-border bg-bg select-none",
        disabled && "opacity-60",
      )}
      style={{
        aspectRatio: "16/10",
        touchAction: "none",
        cursor: disabled ? "default" : "crosshair",
      }}
      onPointerDown={down}
      onPointerMove={move}
      onPointerUp={up}
      role="img"
      aria-label={t("cropRotate.editor")}
    >
      {url && (
        <img
          src={url}
          alt=""
          draggable={false}
          onLoad={(e) => {
            const size: [number, number] = [
              e.currentTarget.naturalWidth,
              e.currentTarget.naturalHeight,
            ];
            setNat(size);
            onSize?.(size);
          }}
          onError={() => {
            setFailed(true);
            onSizeRef.current?.(null);
          }}
          className="absolute h-full w-full object-contain"
        />
      )}
      {shown && (
        <>
          <div
            className="absolute border-2 border-accent"
            style={{
              left: shown.x,
              top: shown.y,
              width: shown.w,
              height: shown.h,
            }}
          />
          <div
            data-mode="move"
            className="absolute cursor-move"
            style={{
              left: shown.x,
              top: shown.y,
              width: shown.w,
              height: shown.h,
            }}
          />
          <div
            className="absolute bg-black/50"
            style={{
              left: imgBox().x,
              top: imgBox().y,
              width: imgBox().w,
              height: Math.max(0, shown.y - imgBox().y),
            }}
          />
          <div
            className="absolute bg-black/50"
            style={{
              left: imgBox().x,
              top: shown.y + shown.h,
              width: imgBox().w,
              height: Math.max(0, imgBox().y + imgBox().h - shown.y - shown.h),
            }}
          />
          <div
            className="absolute bg-black/50"
            style={{
              left: imgBox().x,
              top: shown.y,
              width: Math.max(0, shown.x - imgBox().x),
              height: shown.h,
            }}
          />
          <div
            className="absolute bg-black/50"
            style={{
              left: shown.x + shown.w,
              top: shown.y,
              width: Math.max(0, imgBox().x + imgBox().w - shown.x - shown.w),
              height: shown.h,
            }}
          />
          {handle("nw", {
            left: shown.x - 5,
            top: shown.y - 5,
            cursor: "nwse-resize",
          })}
          {handle("ne", {
            left: shown.x + shown.w - 5,
            top: shown.y - 5,
            cursor: "nesw-resize",
          })}
          {handle("sw", {
            left: shown.x - 5,
            top: shown.y + shown.h - 5,
            cursor: "nesw-resize",
          })}
          {handle("se", {
            left: shown.x + shown.w - 5,
            top: shown.y + shown.h - 5,
            cursor: "nwse-resize",
          })}
        </>
      )}
      {failed && (
        <div className="absolute inset-0 flex items-center justify-center text-sm text-text-secondary">
          {t("compare.unavailable")}
        </div>
      )}
    </div>
  );
}
