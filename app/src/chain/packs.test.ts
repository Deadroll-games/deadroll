import { describe, expect, it } from "vitest";
import { ARENA_SIDE_MAX, ARENA_SIDE_MIN, NEW_ARENA_SIDE, addPack, addToPack, arenaGrid, noPacks, packAt, paintArena, removeSpawns, resizeArena, setPack } from "./packs";
import { Tile, cellByte } from "./types";

const empty = () => ({ spawns: [], packs: noPacks() });

describe("pack edits (arena update)", () => {
  it("a new pack: one monster shown on the map, standing in the middle of an open arena", () => {
    const a = addPack(empty(), { x: 6, y: 2 }, 0, 0b0111)!;
    expect(a.pack).toBe(1);
    expect(a.next.spawns).toEqual([{ bestiaryId: 0, x: 4, y: 4, level: 1, skills: 0b0111, flags: 0, aspects: 0, pack: 1 }]);
    const pk = a.next.packs[1]!;
    expect([pk.visible, pk.x, pk.y, pk.facing, pk.width, pk.height]).toEqual([0, 6, 2, 4, NEW_ARENA_SIDE, NEW_ARENA_SIDE]);
    expect(pk.cells.every((b) => b === cellByte(Tile.StoneFloor))).toBe(true);
    expect(packAt(a.next, { x: 6, y: 2 })).toBe(1);
    expect(packAt(a.next, { x: 1, y: 1 })).toBeNull();
    // Every number past 0 taken: no more packs.
    let d = a.next;
    for (let i = 0; i < 6; i++) d = addPack(d, { x: i, y: 9 }, 1, 3)!.next;
    expect(addPack(d, { x: 9, y: 9 }, 1, 3)).toBeNull();
  });

  it("spawns join a pack in its arena; removing keeps each pack's visible spawn or drops an empty pack", () => {
    let d = addPack(empty(), { x: 6, y: 2 }, 0, 7)!.next;
    d = addPack(d, { x: 2, y: 10 }, 1, 3)!.next;
    d = addToPack(d, 1, { x: 5, y: 4 }, 0, 14)!;
    expect(addToPack(d, 1, { x: 5, y: 4 }, 0, 14)).toBeNull();
    expect(addToPack(d, 1, { x: 8, y: 0 }, 0, 14)).toBeNull();
    expect(d.spawns.map((s) => s.pack)).toEqual([1, 2, 1]);
    // Pack 1's visible spawn goes: its other wolf is shown; pack 2 keeps its own (renumbered).
    const r = removeSpawns(d, (_, i) => i === 0);
    expect(r.spawns.map((s) => s.pack)).toEqual([2, 1]);
    expect([r.packs[1]!.visible, r.packs[2]!.visible]).toEqual([1, 0]);
    // The goblin pack left empty is gone.
    expect(removeSpawns(r, (s) => s.pack === 2).packs[2]).toBeNull();
  });

  it("arena cells: painted, resized within the limits (kept cells stay), shown at the map stride", () => {
    let d = addPack(empty(), { x: 6, y: 2 }, 0, 7)!.next;
    const wall = cellByte(Tile.Wall);
    d = paintArena(d, 1, { x: 1, y: 2 }, wall);
    expect(d.packs[1]!.cells[2 * NEW_ARENA_SIDE + 1]).toBe(wall);
    d = resizeArena(d, 1, 100, 2);
    expect([d.packs[1]!.width, d.packs[1]!.height]).toEqual([ARENA_SIDE_MAX, ARENA_SIDE_MIN]);
    expect(d.packs[1]!.cells[2 * ARENA_SIDE_MAX + 1]).toBe(wall);
    const grid = arenaGrid(d.packs[1]!);
    expect(grid[2 * 32 + 1]).toBe(wall);
    expect(grid[ARENA_SIDE_MAX]).toBe(wall);
    d = setPack(d, 1, { facing: 7, x: 9, y: 9 });
    expect([d.packs[1]!.facing, d.packs[1]!.x]).toEqual([7, 9]);
  });
});
