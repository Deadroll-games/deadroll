import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import { Grid, cellFromPoint } from "./Grid";
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
    // 64 canvas pixels per cell (format 2 sprites 1:1), shown at the default 48 CSS px.
    expect((canvas as HTMLCanvasElement).width).toBe(4 * 64);
    expect(canvas.style.width).toBe(`${4 * 48}px`);
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
  it("draws at least one 64 px sprite per cell and keeps larger cells", async () => {
    const { backingCell } = await import("./Grid");
    expect(backingCell(12)).toBe(64);
    expect(backingCell(32)).toBe(64);
    expect(backingCell(48)).toBe(64);
    expect(backingCell(96)).toBe(96);
  });
});
