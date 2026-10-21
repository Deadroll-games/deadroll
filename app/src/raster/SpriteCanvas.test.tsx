import { render, screen } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import { SpriteCanvas, integerScale } from "./SpriteCanvas";

describe("integerScale", () => {
  it("never returns a fractional scale or less than 1", () => {
    for (const scale of [0, 0.5, 1, 1.2, 1.5, 2.5, 2.99, 3, 7.5, NaN, Infinity]) {
      const k = integerScale(scale);
      expect(Number.isInteger(k)).toBe(true);
      expect(k).toBeGreaterThanOrEqual(1);
    }
    expect(integerScale(2.5)).toBe(2);
  });

  it("picks the largest integer scale that fits the box, 1x when nothing fits", () => {
    const size = { w: 96, h: 96 };
    expect(integerScale(10, size, { w: 300, h: 400 })).toBe(3);
    expect(integerScale(10, size, { w: 400, h: 200 })).toBe(2);
    expect(integerScale(2, size, { w: 1000, h: 1000 })).toBe(2);
    expect(integerScale(3, size, { w: 50, h: 50 })).toBe(1);
  });
});

describe("SpriteCanvas", () => {
  it("is as large on screen as its pixels, at an integer multiple of the image", () => {
    const image = { width: 96, height: 96, data: new Uint8ClampedArray(96 * 96 * 4) } as ImageData;
    render(<SpriteCanvas image={image} scale={2.5} label="hero" />);
    const canvas = screen.getByRole("img") as HTMLCanvasElement;
    expect(canvas.width).toBe(192);
    expect(canvas.height).toBe(192);
    expect(canvas.style.width).toBe("192px");
    expect(canvas.style.height).toBe("192px");
  });
});
