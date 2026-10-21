import { useEffect, useRef } from "react";

/** Character canvas side (crates/render/FORMAT.md 1.4): every character layer is 96x96. */
export const SPRITE_PX = 96;
/** Map tile side: one cell. Characters and monsters stand bottom-center on it at the same pixel size. */
export const TILE_PX = 64;

/**
 * Integer scale for a sprite: the requested scale rounded down, at least 1. With `fit` (the box in
 * screen px) it is the largest integer scale whose image still fits; if even 1x does not fit the
 * result is 1 and the box has to grow, a sprite is never drawn below 1x.
 */
export function integerScale(
  scale: number,
  size: { w: number; h: number } = { w: SPRITE_PX, h: SPRITE_PX },
  fit?: { w: number; h: number },
): number {
  const wanted = Number.isFinite(scale) ? Math.floor(scale) : 1;
  const fitted = fit ? Math.min(Math.floor(fit.w / size.w), Math.floor(fit.h / size.h)) : wanted;
  return Math.max(1, Math.min(wanted, fitted));
}

/**
 * Draws RGBA ImageData with nearest-neighbor scaling (never smoothed). The scale is always an
 * integer and the canvas is as large on screen as its pixels, so CSS never resizes it.
 */
export function SpriteCanvas({
  image,
  scale = 2,
  fit,
  label,
}: {
  image: ImageData | null;
  scale?: number;
  fit?: { w: number; h: number };
  label?: string;
}) {
  const ref = useRef<HTMLCanvasElement>(null);
  const w = image?.width ?? SPRITE_PX;
  const h = image?.height ?? SPRITE_PX;
  const k = integerScale(scale, { w, h }, fit);
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
    ctx.drawImage(src, 0, 0, image.width * k, image.height * k);
  }, [image, k]);
  return (
    <canvas
      ref={ref}
      className="pixel sprite-canvas"
      width={w * k}
      height={h * k}
      style={{ width: w * k, height: h * k }}
      role="img"
      aria-label={label ?? "sprite"}
    />
  );
}
