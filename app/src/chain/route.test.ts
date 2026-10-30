// Patch B: a 2x2 mover plans with its whole square, as `deadroll_mapcheck::walk::walk_sized`.
import { describe, expect, it } from "vitest";
import { reachable, route, stepCost, type Ground } from "./route";
import { MAP_MAX, Tile, cellByte } from "./types";

function ground(walls: [number, number][], taken: [number, number][] = [], size = 1): Ground {
  const tiles = new Uint8Array(MAP_MAX * MAP_MAX).fill(cellByte(Tile.StoneFloor));
  for (const [x, y] of walls) tiles[y * MAP_MAX + x] = cellByte(Tile.Wall);
  const tileCost = new Array(32).fill(1);
  tileCost[Tile.Wall] = 0;
  return { tiles, width: 8, height: 8, tileCost, taken: new Set(taken.map(([x, y]) => y * MAP_MAX + x)), size };
}

describe("route for a 2x2 mover", () => {
  it("needs its whole square free and inside", () => {
    const g = ground([[4, 4]], [[6, 1]], 2);
    // (2,2) -> (3,2): the square (3..5, 2..4) is open.
    expect(stepCost(g, { x: 2, y: 2 }, { x: 3, y: 2 })).toBe(1);
    // (3,3) holds the wall (4,4) in its square.
    expect(stepCost(g, { x: 2, y: 2 }, { x: 3, y: 3 })).toBe(0);
    // (5,1) holds the unit on (6,1).
    expect(stepCost(g, { x: 4, y: 1 }, { x: 5, y: 1 })).toBe(0);
    // The square past the edge.
    expect(stepCost(g, { x: 6, y: 5 }, { x: 7, y: 5 })).toBe(0);
    // A one-cell mover passes where the square does not.
    expect(stepCost({ ...g, size: 1 }, { x: 2, y: 2 }, { x: 3, y: 3 })).toBe(1);
  });

  it("never cuts a corner with its square", () => {
    // From (2,3) to (3,2): the side square at (3,3) holds the wall (4,4).
    const g = ground([[4, 4]], [], 2);
    expect(stepCost(g, { x: 2, y: 3 }, { x: 3, y: 2 })).toBe(0);
  });

  it("routes around and reaches with the square", () => {
    const g = ground([[3, 0], [3, 1], [3, 2]], [], 2);
    const steps = route({ ...g, from: { x: 0, y: 0 }, goal: (x, y) => x === 5 && y === 0 })!;
    expect(steps.at(-1)!.to).toEqual({ x: 5, y: 0 });
    // Every square on the way stays clear of the wall column.
    for (const s of steps) expect(s.to.x + 1 < 3 || s.to.x > 3 || s.to.y > 2).toBe(true);
    const reach = reachable({ ...g, from: { x: 0, y: 0 }, ap: 2 });
    expect(reach).toContainEqual({ x: 1, y: 1 });
    expect(reach).not.toContainEqual({ x: 2, y: 0 });
  });
});
