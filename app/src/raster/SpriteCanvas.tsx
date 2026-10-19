import { useEffect, useRef } from "react";

/** Character canvas side (crates/render/FORMAT.md 1.4): every character layer is 96x96. */
export const SPRITE_PX = 96;
/** Map tile side: one cell. Characters and monsters stand bottom-center on it at the same pixel size. */
export const TILE_PX = 64;

/**
 * Draws RGBA ImageData with nearest-neighbor scaling (never smoothed). `scale` may be a half step
 * (1.5 shows a 96 px sprite at 144 px): the canvas is drawn at the next integer scale and sized
 * down by CSS, so pixels stay square on high-density screens.
 */
export function SpriteCanvas({ image, scale = 2, label }: { image: ImageData | null; scale?: number; label?: string }) {
  const ref = useRef<HTMLCanvasElement>(null);
  const shown = Math.max(0.5, Math.round(scale * 2) / 2);
  const k = Math.max(1, Math.ceil(shown));
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
  const w = image?.width ?? SPRITE_PX;
  const h = image?.height ?? SPRITE_PX;
  return (
    <canvas
      ref={ref}
      className="pixel"
      width={w * k}
      height={h * k}
      style={{ width: w * shown, height: h * shown }}
      role="img"
      aria-label={label ?? "sprite"}
    />
  );
}
