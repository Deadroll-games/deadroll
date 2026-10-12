import { useEffect, useRef } from "react";

/** Draws RGBA ImageData with nearest-neighbor integer scaling (never smoothed). */
export function SpriteCanvas({ image, scale = 4, label }: { image: ImageData | null; scale?: number; label?: string }) {
  const ref = useRef<HTMLCanvasElement>(null);
  const s = Math.max(1, Math.floor(scale));
  useEffect(() => {
    const canvas = ref.current;
    const ctx = canvas?.getContext("2d");
    if (!canvas || !ctx || !image) return;
    const src = document.createElement("canvas");
    src.width = image.width;
    src.height = image.height;
    src.getContext("2d")?.putImageData(image, 0, 0);
    ctx.imageSmoothingEnabled = false;
    ctx.clearRect(0, 0, canvas.width, canvas.height);
    ctx.drawImage(src, 0, 0, image.width * s, image.height * s);
  }, [image, s]);
  const w = (image?.width ?? 32) * s;
  const h = (image?.height ?? 32) * s;
  return <canvas ref={ref} className="pixel" width={w} height={h} role="img" aria-label={label ?? "sprite"} />;
}
