import { useEffect, useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import { ipc, type WatermarkSpec } from "@/lib/ipc";

function useImageBytes(path: string | null) {
  const [url, setUrl] = useState<string | null>(null);
  useEffect(() => {
    setUrl(null);
    if (!path) return;
    let objectUrl: string | null = null;
    let alive = true;
    ipc
      .loadImage(path)
      .then((buf) => {
        if (!alive) return;
        objectUrl = URL.createObjectURL(new Blob([buf], { type: "image/png" }));
        setUrl(objectUrl);
      })
      .catch(() => setUrl(null));
    return () => {
      alive = false;
      if (objectUrl) URL.revokeObjectURL(objectUrl);
    };
  }, [path]);
  return url;
}

const rgba = (c: [number, number, number, number]) =>
  `rgba(${c[0]},${c[1]},${c[2]},${(c[3] / 255).toFixed(3)})`;

/** Canvas approximation of the watermark over the first queued image. */
export function WatermarkPreview({
  path,
  spec,
}: {
  path: string;
  spec: WatermarkSpec;
}) {
  const { t } = useTranslation();
  const canvasRef = useRef<HTMLCanvasElement>(null);
  const baseUrl = useImageBytes(path);
  const logoUrl = useImageBytes(spec.kind === "image" ? spec.imagePath : null);
  const [error, setError] = useState(false);

  useEffect(() => {
    const canvas = canvasRef.current;
    if (!canvas || !baseUrl) return;
    const ctx = canvas.getContext("2d");
    if (!ctx) return;
    const base = new Image();
    base.onload = () => {
      const maxW = 460;
      const fit = Math.min(1, maxW / base.naturalWidth);
      const bw = Math.round(base.naturalWidth * fit);
      const bh = Math.round(base.naturalHeight * fit);
      canvas.width = bw;
      canvas.height = bh;
      ctx.clearRect(0, 0, bw, bh);
      ctx.drawImage(base, 0, 0, bw, bh);
      const minDim = Math.min(bw, bh);
      const drawAt = (x: number, y: number, w: number, h: number) => {
        ctx.save();
        ctx.globalAlpha = spec.opacity;
        ctx.translate(x + w / 2, y + h / 2);
        ctx.rotate((spec.rotation * Math.PI) / 180);
        if (spec.kind === "text") {
          const px = Math.max(4, spec.scale * minDim);
          ctx.font = `${px}px sans-serif`;
          ctx.fillStyle = rgba(spec.color);
          ctx.textAlign = "center";
          ctx.textBaseline = "middle";
          ctx.fillText(spec.text || "Aa", 0, 0);
        } else if (logoImg) {
          ctx.drawImage(logoImg, -w / 2, -h / 2, w, h);
        }
        ctx.restore();
      };
      const stamp = () => {
        const tw =
          spec.kind === "text"
            ? Math.max(4, spec.scale * minDim) * (spec.text.length || 2) * 0.6
            : spec.scale * minDim;
        const th =
          spec.kind === "text"
            ? Math.max(4, spec.scale * minDim)
            : spec.scale * minDim * ((logoImg?.naturalHeight ?? 1) / (logoImg?.naturalWidth || 1));
        const m = spec.margin * minDim;
        if (spec.tiled) {
          const sx = tw * (1 + spec.spacing);
          const sy = th * (1 + spec.spacing);
          for (let y = 0; y < bh; y += Math.max(1, sy))
            for (let x = 0; x < bw; x += Math.max(1, sx)) drawAt(x, y, tw, th);
          return;
        }
        const anchors: Record<Exclude<typeof spec.position, "custom">, [number, number]> = {
          topLeft: [m, m],
          topCenter: [(bw - tw) / 2, m],
          topRight: [bw - tw - m, m],
          middleLeft: [m, (bh - th) / 2],
          center: [(bw - tw) / 2, (bh - th) / 2],
          middleRight: [bw - tw - m, (bh - th) / 2],
          bottomLeft: [m, bh - th - m],
          bottomCenter: [(bw - tw) / 2, bh - th - m],
          bottomRight: [bw - tw - m, bh - th - m],
        };
        const xs: [number, number][] =
          spec.position === "custom"
            ? [[spec.custom[0] * bw - tw / 2, spec.custom[1] * bh - th / 2]]
            : [anchors[spec.position]];
        for (const [x, y] of xs) drawAt(x, y, tw, th);
      };
      let logoImg: HTMLImageElement | null = null;
      if (spec.kind === "image") {
        if (!logoUrl) {
          ctx.font = "12px sans-serif";
          ctx.fillStyle = "rgba(255,80,80,0.9)";
          ctx.fillText("?", bw / 2, bh / 2);
          return;
        }
        logoImg = new Image();
        logoImg.onload = stamp;
        logoImg.src = logoUrl;
      } else {
        stamp();
      }
    };
    base.onerror = () => setError(true);
    base.src = baseUrl;
  }, [baseUrl, logoUrl, spec]);

  if (error) {
    return <p className="text-xs text-danger">{t("wm.previewError")}</p>;
  }
  return (
    <div className="flex flex-col gap-1">
      <canvas
        ref={canvasRef}
        className="max-w-full rounded-lg border border-border bg-black"
      />
      <p className="text-[11px] text-text-secondary">{t("wm.previewHint")}</p>
    </div>
  );
}
