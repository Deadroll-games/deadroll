import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import { Grid, cellFromPoint, clampView, viewSize } from "./Grid";
import type { GridUnit } from "./types";

afterEach(() => {
  cleanup();
});

describe("cellFromPoint", () => {
  it("maps the four corners of the rect", () => {
    const rect = { left: 8, top: 4, width: 320, height: 160 };
    expect(cellFromPoint(8, 4, rect, 8, 4)).toEqual({ x: 0, y: 0 });
    expect(cellFromPoint(8 + 319, 4, rect, 8, 4)).toEqual({ x: 7, y: 0 });
    expect(cellFromPoint(8, 4 + 159, rect, 8, 4)).toEqual({ x: 0, y: 3 });
    expect(cellFromPoint(8 + 319, 4 + 159, rect, 8, 4)).toEqual({ x: 7, y: 3 });
  });

  it("uses the CSS box when the canvas is scaled", () => {
    const rect = { left: 40, top: 10, width: 160, height: 80 };
    expect(cellFromPoint(40 + 3 * 20 + 10, 10 + 2 * 20 + 10, rect, 8, 4)).toEqual({ x: 3, y: 2 });
    expect(cellFromPoint(40 + 159, 10 + 79, rect, 8, 4)).toEqual({ x: 7, y: 3 });
  });

  it("returns null outside the rect", () => {
    const rect = { left: 40, top: 10, width: 160, height: 80 };
    expect(cellFromPoint(39, 10, rect, 8, 4)).toBeNull();
    expect(cellFromPoint(40, 9, rect, 8, 4)).toBeNull();
    expect(cellFromPoint(200, 10, rect, 8, 4)).toBeNull();
    expect(cellFromPoint(40, 90, rect, 8, 4)).toBeNull();
  });
});

describe("Grid", () => {
  const tiles = new Uint8Array(1024);

  it("renders a labeled canvas", () => {
    expect(() =>
      render(<Grid width={4} height={3} tiles={tiles} units={[]} />),
    ).not.toThrow();
    const canvas = screen.getByRole("img", { name: "Battle map, 4 by 3 cells" });
    expect(canvas.tagName).toBe("CANVAS");
    expect(canvas.className).toContain("pixel");
    // 64 canvas pixels per cell (one tile 1:1), shown at the default 48 CSS px.
    expect((canvas as HTMLCanvasElement).width).toBe(4 * 64);
    expect(canvas.style.width).toBe(`${4 * 48}px`);
  });

  it("patch 11: shows the hovered chest's label as the canvas title", () => {
    const chests = [{ x: 1, y: 0, opened: false, label: "Chest: levels 1-2" }];
    render(<Grid width={4} height={3} tiles={tiles} units={[]} cellPx={48} chests={chests} />);
    const canvas = screen.getByRole("img");
    canvas.getBoundingClientRect = () => new DOMRect(0, 0, 4 * 48, 3 * 48);
    fireEvent.mouseMove(canvas, { clientX: 48 + 5, clientY: 5 });
    expect(canvas.getAttribute("title")).toBe("Chest: levels 1-2");
    fireEvent.mouseMove(canvas, { clientX: 5, clientY: 5 });
    expect(canvas.getAttribute("title")).toBeNull();
  });

  it("clicks an empty cell, a live unit, and a dead unit cell as a move", () => {
    const onCellClick = vi.fn();
    const onUnitClick = vi.fn();
    const wolf: GridUnit = {
      id: 4,
      kind: "monster",
      x: 2,
      y: 1,
      hp: 12,
      hpMax: 12,
      alive: true,
      label: "Wolf",
    };
    const fallen: GridUnit = {
      id: 0,
      kind: "player",
      x: 3,
      y: 3,
      hp: 0,
      hpMax: 20,
      alive: false,
      label: "Bran",
    };
    render(
      <Grid
        width={4}
        height={4}
        tiles={tiles}
        units={[wolf, fallen]}
        cellPx={48}
        onCellClick={onCellClick}
        onUnitClick={onUnitClick}
      />,
    );
    const canvas = screen.getByRole("img");
    canvas.getBoundingClientRect = () => new DOMRect(10, 20, 96, 96);

    fireEvent.click(canvas, { clientX: 22, clientY: 104 });
    expect(onCellClick).toHaveBeenCalledTimes(1);
    expect(onCellClick).toHaveBeenCalledWith({ x: 0, y: 3 });
    expect(onUnitClick).not.toHaveBeenCalled();

    fireEvent.click(canvas, { clientX: 70, clientY: 56 });
    expect(onUnitClick).toHaveBeenCalledTimes(1);
    expect(onUnitClick).toHaveBeenCalledWith(wolf);

    // A dead unit does not occupy its cell: the click is a cell click (move onto the corpse).
    fireEvent.click(canvas, { clientX: 10 + 3 * 24 + 4, clientY: 20 + 3 * 24 + 4 });
    expect(onUnitClick).toHaveBeenCalledTimes(1);
    expect(onCellClick).toHaveBeenCalledTimes(2);
    expect(onCellClick).toHaveBeenLastCalledWith({ x: 3, y: 3 });

    fireEvent.click(canvas, { clientX: 0, clientY: 0 });
    expect(onCellClick).toHaveBeenCalledTimes(2);
    expect(onUnitClick).toHaveBeenCalledTimes(1);
  });

  it("reports hover only when the cell changes, then null on leave", () => {
    const onCellHover = vi.fn();
    render(<Grid width={4} height={4} tiles={tiles} units={[]} onCellHover={onCellHover} />);
    const canvas = screen.getByRole("img");
    canvas.getBoundingClientRect = () => new DOMRect(0, 0, 192, 192);

    fireEvent.mouseMove(canvas, { clientX: 10, clientY: 10 });
    fireEvent.mouseMove(canvas, { clientX: 20, clientY: 20 });
    fireEvent.mouseMove(canvas, { clientX: 50, clientY: 10 });
    fireEvent.mouseLeave(canvas);

    expect(onCellHover.mock.calls).toEqual([[{ x: 0, y: 0 }], [{ x: 1, y: 0 }], [null]]);
  });
});

describe("backingCell", () => {
  it("draws at least one 64 px tile per cell and keeps larger cells", async () => {
    const { backingCell } = await import("./Grid");
    expect(backingCell(12)).toBe(64);
    expect(backingCell(32)).toBe(64);
    expect(backingCell(48)).toBe(64);
    expect(backingCell(96)).toBe(96);
  });
});

describe("integerCell", () => {
  it("is a whole multiple of the 64 px tile, never below one tile", async () => {
    const { integerCell } = await import("./Grid");
    expect(integerCell(12)).toBe(64);
    expect(integerCell(48)).toBe(64);
    expect(integerCell(96)).toBe(64);
    expect(integerCell(128)).toBe(128);
    expect(integerCell(200)).toBe(192);
  });
});

describe("unitDest", () => {
  it("draws units at the tile pixel scale, bigger than the cell", async () => {
    const { unitDest } = await import("./Grid");
    expect(unitDest({ width: 96, height: 96 }, 64)).toEqual({ w: 96, h: 96 });
    expect(unitDest({ width: 96, height: 96 }, 128)).toEqual({ w: 192, h: 192 });
    expect(unitDest({ width: 128, height: 128 }, 96)).toEqual({ w: 192, h: 192 });
    expect(unitDest({ width: 64, height: 64 }, 64)).toEqual({ w: 64, h: 64 });
  });
});

describe("patch D: the window over a big map", () => {
  it("shows a Small map whole and a bigger one through a 24x14 window kept inside the map", () => {
    expect(viewSize(32, 32)).toEqual({ cols: 32, rows: 32 });
    expect(viewSize(143, 143)).toEqual({ cols: 24, rows: 14 });
    expect(viewSize(40, 10)).toEqual({ cols: 24, rows: 10 });
    expect(viewSize(16, 16, { cols: 8, rows: 8 })).toEqual({ cols: 8, rows: 8 });
    expect(clampView(-5, 200, 24, 14, 143, 143)).toEqual({ x: 0, y: 129, cols: 24, rows: 14 });
    expect(clampView(60.4, 30.6, 24, 14, 143, 143)).toEqual({ x: 60, y: 31, cols: 24, rows: 14 });
  });

  it("a click on the window maps to the map cell, the parchment minimap comes along", () => {
    const tiles = new Uint8Array(100 * 100).fill(1 << 3);
    const onCellClick = vi.fn();
    const { container } = render(<Grid width={100} height={100} tiles={tiles} units={[]} cellPx={10} follow={{ x: 80, y: 90 }} onCellClick={onCellClick} />);
    const canvas = container.querySelector("canvas.grid-canvas") as HTMLCanvasElement;
    expect(container.querySelector(".parchment-minimap")).not.toBeNull();
    // The window follows (80, 90): it shows columns 68..91 and rows 83..96.
    canvas.getBoundingClientRect = () => ({ left: 0, top: 0, width: 240, height: 140, right: 240, bottom: 140, x: 0, y: 0, toJSON: () => ({}) });
    fireEvent.click(canvas, { clientX: 5, clientY: 5 });
    expect(onCellClick).toHaveBeenCalledWith({ x: 68, y: 83 });
  });
});
