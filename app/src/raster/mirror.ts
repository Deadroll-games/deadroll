// Left-right mirror of a rendered sprite: monsters face left in the sprite books and turn toward
// the hero they fight (chain/facing.ts). Pixels are copied, never resampled.

type Pixels = Pick<ImageData, "width" | "height" | "data">;

/** The pixels of `img` mirrored left to right. */
export function mirrorPixels(img: Pixels): Uint8ClampedArray<ArrayBuffer> {
  const { width, height, data } = img;
  const out = new Uint8ClampedArray(data.length);
  for (let y = 0; y < height; y++) {
    for (let x = 0; x < width; x++) {
      const from = (y * width + x) * 4;
      const to = (y * width + width - 1 - x) * 4;
      out.set(data.subarray(from, from + 4), to);
    }
  }
  return out;
}

const cache = new WeakMap<ImageData, ImageData>();

/** `img` mirrored left to right, made once per image. */
export function mirrored(img: ImageData): ImageData {
  let out = cache.get(img);
  if (!out) {
    out = new ImageData(mirrorPixels(img), img.width, img.height);
    cache.set(img, out);
  }
  return out;
}
