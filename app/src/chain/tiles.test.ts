// Patch 13: the Tileset as the builder and the mock read it, the same records deadroll_rules decodes.
import { describe, expect, it } from "vitest";
import { SHIPPED_TILESET, SHIPPED_TILE_COST, knownTiles, tileCosts, wallMask } from "./tiles";
import { MAP_CELLS, TILE_COUNT, TILE_KINDS_MAX, Tile, cellByte, cellIndex, cellKind, cellVariant } from "./types";

describe("tileset", () => {
  it("the shipped tileset costs by tile type (programs/session TILE_COST)", () => {
    expect(SHIPPED_TILE_COST).toHaveLength(TILE_KINDS_MAX);
    expect(SHIPPED_TILE_COST.slice(0, TILE_COUNT)).toEqual([0, 1, 1, 1, 0, 1, 2, 0, 1, 2, 0]);
    expect(SHIPPED_TILE_COST.slice(TILE_COUNT).every((c) => c === 0)).toBe(true);
    expect(knownTiles(SHIPPED_TILESET)).toBe(TILE_COUNT);
  });

  it("an impassable record costs 0 whatever its cost byte, a partial record is ignored", () => {
    const body = Uint8Array.of(0, 9, 1, 0, 0, 0, 0, 0, 1, 3, 0, 0, 0, 0, 0, 0, 1, 1);
    expect(tileCosts(body).slice(0, 3)).toEqual([0, 3, 0]);
    expect(knownTiles(body)).toBe(2);
  });

  it("cell bytes: tile type in bits 7-3, variant in bits 2-0", () => {
    const b = cellByte(Tile.Chasm, 5);
    expect(b).toBe(0b0101_0101);
    expect(cellKind(b)).toBe(Tile.Chasm);
    expect(cellVariant(b)).toBe(5);
    expect(cellKind(0xff)).toBe(31);
  });
});

describe("wallMask", () => {
  /** A map from rows: `#` wall, `D` door, `.` floor, ` ` void. */
  const grid = (rows: string[]) => {
    const tiles = new Uint8Array(MAP_CELLS);
    const kind: Record<string, number> = { "#": Tile.Wall, D: Tile.Door, ".": Tile.StoneFloor, " ": Tile.Void };
    rows.forEach((row, y) => [...row].forEach((ch, x) => (tiles[cellIndex(x, y)] = cellByte(kind[ch]!, 3))));
    return (x: number, y: number) => wallMask(tiles, rows[0]!.length, rows.length, x, y);
  };

  it("bit 0 N, 1 E, 2 S, 3 W: walls and doors join, floor and void do not", () => {
    const at = grid([
      "#####",
      "#.#.#",
      "##D##",
      "#.# #",
      "#####",
    ]);
    expect(at(2, 2)).toBe(1 | 2 | 4 | 8); // the door has walls all around
    expect(at(2, 1)).toBe(1 | 4); // a wall between two floors: N wall, S door
    expect(at(1, 2)).toBe(2 | 8); // W wall, E door
    expect(at(4, 3)).toBe(1 | 4); // void to the west does not join
  });

  it("the map edge does not join, so the outer ring reads as straight walls and corners", () => {
    const at = grid(["###", "#.#", "###"]);
    expect([at(0, 0), at(1, 0), at(2, 0)]).toEqual([2 | 4, 2 | 8, 4 | 8]);
    expect([at(0, 1), at(2, 1)]).toEqual([1 | 4, 1 | 4]);
    expect([at(0, 2), at(1, 2), at(2, 2)]).toEqual([1 | 2, 2 | 8, 1 | 8]);
  });
});
