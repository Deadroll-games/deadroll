// Memoized renders through the on-chain rasterizer, for callers that need many sprites (the grid).
import { useCallback, useRef } from "react";
import type { RenderRequest } from "../chain/types";
import { useRasterizer } from "./RasterizerProvider";

/** Returns `draw(key, request)`: ImageData from the loaded rasterizer, cached by key; null if not ready or on error. */
export function useSprites(): (key: string, req: RenderRequest) => ImageData | null {
  const state = useRasterizer();
  const cache = useRef(new Map<string, ImageData | null>());
  const ready = state.status === "ready" ? state.render : null;
  const last = useRef(ready);
  if (last.current !== ready) {
    cache.current.clear();
    last.current = ready;
  }
  return useCallback(
    (key: string, req: RenderRequest) => {
      if (!ready) return null;
      const hit = cache.current.get(key);
      if (hit !== undefined) return hit;
      let img: ImageData | null = null;
      try {
        const out = ready.rasterizer.render(req, ready.palette, ready.books.map((b) => b.body));
        img = new ImageData(new Uint8ClampedArray(out.bytes), out.width, out.height);
      } catch {
        img = null;
      }
      cache.current.set(key, img);
      return img;
    },
    [ready],
  );
}
