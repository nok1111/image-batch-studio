import {
  useEffect,
  useRef,
  useState,
  type PointerEvent,
} from "react";
import { useTranslation } from "react-i18next";
import { ipc } from "@/lib/ipc";
import { cn } from "@/lib/utils";
import { displayMime } from "./display";
import { THUMB_FONTS, type ModuleOptions, type ThumbImgFx } from "./job";

type ResizeHandle = "nw" | "n" | "ne" | "e" | "se" | "s" | "sw" | "w";

type DragTarget =
  | { kind: "move"; layer: number }
  | { kind: "resize"; layer: number; handle: ResizeHandle }
  | { kind: "textScale"; layer: number };

interface Drag {
  target: DragTarget;
  startX: number;
  startY: number;
  baseX: number;
  baseY: number;
  baseZw: number;
  baseZh: number;
  baseSize: number;
  baseStroke: number;
}

const HANDLES: { h: ResizeHandle; at: [number, number]; cursor: string }[] = [
  { h: "nw", at: [0, 0], cursor: "nwse-resize" },
  { h: "n", at: [0.5, 0], cursor: "ns-resize" },
  { h: "ne", at: [1, 0], cursor: "nesw-resize" },
  { h: "e", at: [1, 0.5], cursor: "ew-resize" },
  { h: "se", at: [1, 1], cursor: "nwse-resize" },
  { h: "s", at: [0.5, 1], cursor: "ns-resize" },
  { h: "sw", at: [0, 1], cursor: "nesw-resize" },
  { h: "w", at: [0, 0.5], cursor: "ew-resize" },
];

function placedSize(
  fx: ThumbImgFx,
  dims: [number, number] | null,
  cw: number,
  ch: number,
): [number, number] {
  const zx = Math.max(1, fx.zoomX) / 100;
  const zy = Math.max(1, fx.zoomY) / 100;
  if (!dims || dims[0] === 0 || dims[1] === 0) return [cw * zx, ch * zy];
  const [iw, ih] = dims;
  const k =
    fx.fit === "stretch"
      ? null
      : fx.fit === "cover"
        ? Math.max(cw / iw, ch / ih)
        : Math.min(cw / iw, ch / ih);
  return k === null
    ? [cw * zx, ch * zy]
    : [Math.max(1, iw * k * zx), Math.max(1, ih * k * zy)];
}

const fontCss = (key: string) =>
  THUMB_FONTS.find((f) => f.id === key)?.css ?? "sans-serif";

function imgFilter(fx: ThumbImgFx, k: number): string {
  return [
    fx.brightness !== 0 && `brightness(${1 + fx.brightness / 100})`,
    fx.contrast !== 0 && `contrast(${1 + fx.contrast / 100})`,
    fx.saturation !== 0 && `saturate(${1 + fx.saturation / 100})`,
    fx.hue !== 0 && `hue-rotate(${fx.hue}deg)`,
    fx.outlineW > 0 &&
      [
        `drop-shadow(${fx.outlineW * k}px 0 0 ${fx.outlineColor})`,
        `drop-shadow(${-fx.outlineW * k}px 0 0 ${fx.outlineColor})`,
        `drop-shadow(0 ${fx.outlineW * k}px 0 ${fx.outlineColor})`,
        `drop-shadow(0 ${-fx.outlineW * k}px 0 ${fx.outlineColor})`,
      ].join(" "),
  ]
    .filter(Boolean)
    .join(" ");
}

/** Interactive canvas: layered compose with drag/resize per layer. */
export function ThumbnailEditor({
  path,
  o,
  setThumb,
  disabled,
}: {
  path: string;
  o: ModuleOptions;
  setThumb: (patch: Partial<ModuleOptions["thumbnail"]>) => void;
  disabled?: boolean;
}) {
  const { t } = useTranslation();
  const th = o.thumbnail;
  const layers = th.layers ?? [];
  const stage = useRef<HTMLDivElement>(null);
  const drag = useRef<Drag | null>(null);
  const [url, setUrl] = useState<string | null>(null);
  const [failed, setFailed] = useState(false);
  const [stageW, setStageW] = useState(0);
  // natural dims + urls per image path (source included)
  const [nat, setNat] = useState<Record<string, [number, number]>>({});
  const [urls, setUrls] = useState<Record<string, string>>({});
  const [bgUrl, setBgUrl] = useState<string | null>(null);

  const setLayer = (i: number, patch: Partial<ModuleOptions["thumbnail"]["layers"][number]>) => {
    const ls = layers.map((l, j) =>
      j === i ? ({ ...l, ...patch } as typeof l) : l,
    );
    setThumb({ layers: ls });
  };
  const setFx = (i: number, patch: Partial<ThumbImgFx>) => {
    const l = layers[i];
    if (!l || l.kind === "text") return;
    setLayer(i, { img: { ...l.img, ...patch } } as never);
  };

  // Load every image used in the composition: source + image layers.
  const wanted = [
    path,
    th.bg.type === "image" ? th.bg.imagePath : "",
    ...layers.map((l) => (l.kind === "image" ? l.path : "")),
  ]
    .filter(Boolean)
    .join("\n");
  useEffect(() => {
    const paths = wanted.split("\n").filter(Boolean);
    let live = true;
    const created: string[] = [];
    Promise.all(
      paths.map(async (p) => {
        try {
          const buf = await ipc.loadImage(p);
          const u = URL.createObjectURL(new Blob([buf], { type: displayMime(p) }));
          created.push(u);
          return [p, u] as const;
        } catch {
          return null;
        }
      }),
    ).then((entries) => {
      if (!live) {
        created.forEach((u) => URL.revokeObjectURL(u));
        return;
      }
      const map: Record<string, string> = {};
      for (const e of entries) if (e) map[e[0]] = e[1];
      setUrls(map);
      setUrl(map[path] ?? null);
      setBgUrl(th.bg.type === "image" && th.bg.imagePath ? (map[th.bg.imagePath] ?? null) : null);
      if (!map[path]) setFailed(true);
    });
    return () => {
      live = false;
      created.forEach((u) => URL.revokeObjectURL(u));
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [wanted]);

  useEffect(() => {
    const el = stage.current;
    if (!el) return;
    const ro = new ResizeObserver(() => setStageW(el.clientWidth));
    ro.observe(el);
    setStageW(el.clientWidth);
    return () => ro.disconnect();
  }, []);

  const k = stageW > 0 ? stageW / th.width : 1;

  const down =
    (target: DragTarget, bx: number, by: number, size = 0) =>
    (e: PointerEvent) => {
      if (disabled) return;
      (e.currentTarget as Element).setPointerCapture?.(e.pointerId);
      drag.current = {
        target,
        startX: e.clientX,
        startY: e.clientY,
        baseX: bx,
        baseY: by,
        baseZw: 100,
        baseZh: 100,
        baseSize: size,
        baseStroke: 0,
      };
      const l = layers[target.layer];
      if (l && l.kind !== "text") {
        drag.current.baseZw = l.img.zoomX;
        drag.current.baseZh = l.img.zoomY;
      }
      if (l?.kind === "text") {
        drag.current.baseSize = l.text.size;
        drag.current.baseStroke = l.text.strokeW;
      }
      setThumb({ sel: target.layer });
      e.stopPropagation();
    };

  const move =
    (e: PointerEvent) => {
      const d = drag.current;
      if (!d || k === 0) return;
      const dx = Math.round((e.clientX - d.startX) / k);
      const dy = Math.round((e.clientY - d.startY) / k);
      const i = d.target.layer;
      const l = layers[i];
      if (!l) return;
      if (d.target.kind === "move") {
        if (l.kind === "text") {
          const texts = { ...l.text, x: d.baseX + dx, y: d.baseY + dy };
          setLayer(i, { text: texts } as never);
        } else {
          setFx(i, { x: d.baseX + dx, y: d.baseY + dy });
        }
      } else if (d.target.kind === "resize" && l.kind !== "text") {
        const h = d.target.handle;
        const dims = nat[l.kind === "source" ? path : l.path] ?? null;
        const [bw, bh] = placedSize(
          { ...l.img, zoomX: d.baseZw, zoomY: d.baseZh },
          dims,
          th.width,
          th.height,
        );
        if (bw <= 0 || bh <= 0) return;
        const sx = h.includes("e") ? 1 : h.includes("w") ? -1 : 0;
        const sy = h.includes("s") ? 1 : h.includes("n") ? -1 : 0;
        const gw = sx !== 0 ? Math.max(0.05, 1 + (sx * dx) / bw) : 1;
        const gh = sy !== 0 ? Math.max(0.05, 1 + (sy * dy) / bh) : 1;
        const g = sx !== 0 && sy !== 0 ? Math.min(gw, gh) : null;
        const zoomX = Math.max(1, Math.round(d.baseZw * (g ?? gw)));
        const zoomY = Math.max(1, Math.round(d.baseZh * (g ?? gh)));
        const [nw2, nh2] = placedSize({ ...l.img, zoomX, zoomY }, dims, th.width, th.height);
        const dW = nw2 - bw;
        const dH = nh2 - bh;
        setFx(i, {
          zoomX,
          zoomY,
          x: Math.round(sx < 0 ? d.baseX - dW : sx > 0 ? d.baseX : d.baseX - dW / 2),
          y: Math.round(sy < 0 ? d.baseY - dH : sy > 0 ? d.baseY : d.baseY - dH / 2),
        });
      } else if (d.target.kind === "textScale" && l.kind === "text") {
        const f = Math.max(0.1, 1 + dy / 60);
        setLayer(i, {
          text: {
            ...l.text,
            size: Math.max(4, Math.round(d.baseSize * f)),
            strokeW: Math.max(0, Math.round(d.baseStroke * f)),
          },
        } as never);
      }
    };

  const up = () => (drag.current = null);
  const sel = Math.min(th.sel ?? 0, Math.max(0, layers.length - 1));
  const selLayer = layers[sel];
  const selImg = selLayer && selLayer.kind !== "text" ? selLayer.img : null;

  return (
    <div className="flex w-full flex-col gap-2">
      <div
        ref={stage}
        className={cn(
          "relative w-full overflow-hidden rounded-lg border border-border select-none",
          th.bg.type === "transparent"
            ? "bg-[repeating-conic-gradient(#1a1d24_0%_25%,#262a33_0%_50%)] bg-[size:16px_16px]"
            : "bg-bg",
          disabled && "opacity-60",
        )}
        style={{ aspectRatio: `${th.width}/${th.height}` }}
        onPointerMove={move}
        onPointerUp={up}
        onPointerLeave={up}
        role="img"
        aria-label={t("thumb.editor")}
      >
        {th.bg.type === "color" && (
          <div className="absolute inset-0" style={{ background: th.bg.color }} />
        )}
        {th.bg.type === "gradient" && (
          <div
            className="absolute inset-0"
            style={{ background: `linear-gradient(${th.bg.angle}deg, ${th.bg.from}, ${th.bg.to})` }}
          />
        )}
        {url && th.bg.type === "blur" && (
          <img
            src={url}
            alt=""
            draggable={false}
            className="absolute inset-0 h-full w-full object-cover"
            style={{ filter: `blur(${th.bg.blurRadius * k}px)`, transform: "scale(1.15)" }}
          />
        )}
        {bgUrl && th.bg.type === "image" && (
          <>
            <img
              src={bgUrl}
              alt=""
              draggable={false}
              className="absolute inset-0 h-full w-full object-cover"
              style={{
                filter: [
                  th.bg.blurRadius > 0 && `blur(${th.bg.blurRadius * k}px)`,
                  th.bgAdjust.brightness !== 0 && `brightness(${1 + th.bgAdjust.brightness / 100})`,
                  th.bgAdjust.contrast !== 0 && `contrast(${1 + th.bgAdjust.contrast / 100})`,
                  th.bgAdjust.saturation !== 0 && `saturate(${1 + th.bgAdjust.saturation / 100})`,
                  th.bgAdjust.hue !== 0 && `hue-rotate(${th.bgAdjust.hue}deg)`,
                ]
                  .filter(Boolean)
                  .join(" ") || undefined,
              }}
            />
            {th.bg.dim > 0 && (
              <div className="absolute inset-0 bg-black" style={{ opacity: th.bg.dim / 100 }} />
            )}
          </>
        )}
        {(th.bg.type === "blur" || th.bg.type === "color" || th.bg.type === "gradient") &&
          (th.bgAdjust.brightness !== 0 ||
            th.bgAdjust.contrast !== 0 ||
            th.bgAdjust.saturation !== 0 ||
            th.bgAdjust.hue !== 0) && (
            <div
              className="pointer-events-none absolute inset-0"
              style={{
                backdropFilter: [
                  th.bgAdjust.brightness !== 0 && `brightness(${1 + th.bgAdjust.brightness / 100})`,
                  th.bgAdjust.contrast !== 0 && `contrast(${1 + th.bgAdjust.contrast / 100})`,
                  th.bgAdjust.saturation !== 0 && `saturate(${1 + th.bgAdjust.saturation / 100})`,
                  th.bgAdjust.hue !== 0 && `hue-rotate(${th.bgAdjust.hue}deg)`,
                ]
                  .filter(Boolean)
                  .join(" ") || undefined,
              }}
            />
          )}

        {layers.map((l, i) => {
          if (l.kind === "text") {
            const lt = l.text;
            return (
              <div
                key={i}
                onPointerDown={down({ kind: "move", layer: i }, lt.x, lt.y)}
                className={cn(
                  "absolute cursor-grab whitespace-pre text-center font-extrabold active:cursor-grabbing",
                  sel === i && "outline outline-1 outline-accent",
                )}
                style={{
                  left: lt.x * k,
                  top: lt.y * k,
                  transform: `translate(-50%, -50%) rotate(${lt.rotate}deg)`,
                  fontSize: Math.max(6, lt.size * k),
                  fontFamily: fontCss(lt.font),
                  color: lt.color,
                  lineHeight: 1.1,
                  WebkitTextStroke:
                    lt.strokeW > 0 ? `${Math.max(0.5, lt.strokeW * k)}px ${lt.strokeColor}` : undefined,
                  paintOrder: "stroke",
                  textShadow:
                    lt.shadowDx !== 0 || lt.shadowDy !== 0 || lt.shadowBlur > 0
                      ? `${lt.shadowDx * k}px ${lt.shadowDy * k}px ${lt.shadowBlur * k}px ${lt.shadowColor}`
                      : undefined,
                  textTransform: lt.uppercase ? "uppercase" : undefined,
                  touchAction: "none",
                }}
              >
                {lt.text}
                {sel === i && (
                  <div
                    onPointerDown={down({ kind: "textScale", layer: i }, lt.x, lt.y, lt.size)}
                    className="absolute -bottom-2 -right-2 z-10 size-3 cursor-nwse-resize rounded-sm border-2 border-white bg-accent shadow"
                  />
                )}
              </div>
            );
          }
          const lp = l.kind === "source" ? path : l.path;
          const lu = urls[lp];
          if (!lu) return null;
          const dims = nat[lp] ?? null;
          const [pw, ph] = placedSize(l.img, dims, th.width, th.height);
          const imgX = ((th.width - pw) / 2 + l.img.x) * k;
          const imgY = ((th.height - ph) / 2 + l.img.y) * k;
          return (
            <div key={i}>
              {l.img.shadow && (
                <div
                  className="absolute"
                  style={{
                    left: imgX,
                    top: imgY + Math.max(4, (th.height / 60) * k),
                    width: pw * k,
                    height: ph * k,
                    background: "rgba(0,0,0,0.5)",
                    filter: `blur(${Math.max(4, (Math.max(pw, ph) / 80) * k)}px)`,
                    borderRadius: l.img.radius * k,
                  }}
                />
              )}
              <img
                src={lu}
                alt=""
                draggable={false}
                onLoad={(e) => {
                  const size: [number, number] = [
                    e.currentTarget.naturalWidth,
                    e.currentTarget.naturalHeight,
                  ];
                  setNat((m) => (m[lp] ? m : { ...m, [lp]: size }));
                }}
                onPointerDown={down({ kind: "move", layer: i }, l.img.x, l.img.y)}
                className={cn(
                  "absolute max-w-none cursor-grab active:cursor-grabbing",
                  sel === i && "outline outline-1 outline-accent",
                )}
                style={{
                  left: imgX,
                  top: imgY,
                  width: pw * k,
                  height: ph * k,
                  objectFit: "fill",
                  borderRadius: l.img.radius * k,
                  border:
                    l.img.borderW > 0
                      ? `${l.img.borderW * k}px solid ${l.img.borderColor}`
                      : undefined,
                  opacity: l.img.opacity / 100,
                  filter: imgFilter(l.img, k) || undefined,
                  transform:
                    `${l.img.flipH ? "scaleX(-1) " : ""}${l.img.flipV ? "scaleY(-1)" : ""}` ||
                    undefined,
                  touchAction: "none",
                }}
              />
              {sel === i &&
                HANDLES.map(({ h, at, cursor }) => (
                  <div
                    key={h}
                    onPointerDown={down({ kind: "resize", layer: i, handle: h }, l.img.x, l.img.y)}
                    className="absolute z-10 size-3 -translate-x-1/2 -translate-y-1/2 rounded-sm border-2 border-white bg-accent shadow"
                    style={{ left: imgX + pw * k * at[0], top: imgY + ph * k * at[1], cursor }}
                  />
                ))}
            </div>
          );
        })}

        {th.vignette > 0 && (
          <div
            className="pointer-events-none absolute inset-0"
            style={{
              background: `radial-gradient(ellipse at center, transparent ${100 - th.vignette}%, rgba(0,0,0,${th.vignette / 100}))`,
            }}
          />
        )}
        {failed && (
          <div className="absolute inset-0 flex items-center justify-center text-xs text-text-secondary">
            {t("sprite.previewFailed")}
          </div>
        )}
        {url && (
          <div className="pointer-events-none absolute right-1.5 top-1.5 rounded bg-black/60 px-1.5 py-0.5 text-[10px] text-white">
            {th.width}×{th.height}
          </div>
        )}
      </div>
      {selImg && (
        <div className="flex items-center gap-3">
          <span className="text-[11px] text-text-secondary">{t("thumb.zoom")}</span>
          <input
            type="range"
            min={25}
            max={400}
            step={1}
            value={Math.max(selImg.zoomX, selImg.zoomY)}
            disabled={disabled}
            onChange={(e) => {
              const z = Number(e.target.value);
              setFx(sel, { zoomX: z, zoomY: z });
            }}
            className="h-1.5 flex-1 cursor-pointer accent-accent"
            aria-label={t("thumb.zoom")}
          />
          <span className="w-16 text-right font-mono text-[11px] text-text-secondary">
            {selImg.zoomX === selImg.zoomY ? `${selImg.zoomX}%` : `${selImg.zoomX}×${selImg.zoomY}%`}
          </span>
          <button
            type="button"
            disabled={disabled}
            onClick={() => setFx(sel, { x: 0, y: 0, zoomX: 100, zoomY: 100 })}
            className="rounded-md border border-border px-2 py-1 text-[11px] text-text-secondary hover:bg-white/5 disabled:opacity-50"
          >
            {t("thumb.center")}
          </button>
        </div>
      )}
      <p className="text-[11px] text-text-secondary">{t("thumb.dragHint")}</p>
    </div>
  );
}
