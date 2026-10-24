// Patch 13: the Tileset as the builder and the mock read it, the same records deadroll_rules decodes.
import { describe, expect, it } from "vitest";
import { SHIPPED_TILESET, SHIPPED_TILE_COST, knownTiles, tileCosts } from "./tiles";
import { TILE_COUNT, TILE_KINDS_MAX, Tile, cellByte, cellKind, cellVariant } from "./types";

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
