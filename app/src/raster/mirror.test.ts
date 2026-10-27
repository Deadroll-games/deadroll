import { describe, expect, it } from "vitest";
import { mirrorPixels } from "./mirror";

describe("mirrorPixels", () => {
  it("swaps columns left to right, keeping each RGBA pixel whole", () => {
    // 3 x 2: row 0 = A B C, row 1 = D E F; each pixel is (n, n, n, 255).
    const px = (n: number) => [n, n, n, 255];
    const data = Uint8ClampedArray.from([...px(1), ...px(2), ...px(3), ...px(4), ...px(5), ...px(6)]);
    expect(Array.from(mirrorPixels({ width: 3, height: 2, data }))).toEqual([...px(3), ...px(2), ...px(1), ...px(6), ...px(5), ...px(4)]);
  });

  it("twice is the original", () => {
    const data = Uint8ClampedArray.from({ length: 4 * 4 * 2 }, (_, i) => (i * 37) % 256);
    const once = mirrorPixels({ width: 4, height: 2, data });
    expect(mirrorPixels({ width: 4, height: 2, data: once })).toEqual(data);
  });
});
