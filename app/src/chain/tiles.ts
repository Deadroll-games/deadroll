// Patch 13: tile properties are the registry Tileset (docs/MAP_DESCRIPTION.md 4.2), never code.
// Records of 8 bytes `[passable, cost, blocks_los, flags, ...]` by tile type id, decoded the way
// `deadroll_rules::tiles::tile_rules` does; on chain a party reads them from `GameSession.tile_cost`.
import registryTables from "../../../tools/scripts/lib/__fixtures__/registry-tables.json";
import { MAP_MAX, TILE_KINDS_MAX, Tile, cellKind } from "./types";

const RECORD = 8;

/** Step cost of every tile type id, 0 = cannot be entered (also past the written records). */
export function tileCosts(tileset: Uint8Array): number[] {
  const out = new Array<number>(TILE_KINDS_MAX).fill(0);
  for (let id = 0; id < TILE_KINDS_MAX && (id + 1) * RECORD <= tileset.length; id++) {
    if (tileset[id * RECORD] === 1) out[id] = tileset[id * RECORD + 1] ?? 0;
  }
  return out;
}

/** Tile type ids the table has a record for. */
export function knownTiles(tileset: Uint8Array): number {
  return Math.min(TILE_KINDS_MAX, Math.floor(tileset.length / RECORD));
}

const hexBytes = (hex: string) => Uint8Array.from(hex.match(/../g) ?? [], (b) => parseInt(b, 16));

/** `assets/registry/tileset.json` as the seed script uploads it (the shared registry fixture). */
export const SHIPPED_TILESET = hexBytes((registryTables as { tileset: string }).tileset);
export const SHIPPED_TILE_COST = tileCosts(SHIPPED_TILESET);

/**
 * The wall shape of a cell (docs/MAP_DESCRIPTION.md 4.3, 5): which of its 4 neighbors join it, bit 0
 * N, 1 E, 2 S, 3 W, the order of the 16 wall sprites of each biome. A wall and a door (it stands in
 * a wall line) join; the map edge does not, so the outer ring reads as straight walls.
 */
export function wallMask(tiles: ArrayLike<number>, width: number, height: number, x: number, y: number): number {
  const joins = (nx: number, ny: number) => {
    if (nx < 0 || ny < 0 || nx >= width || ny >= height) return false;
    const kind = cellKind(tiles[ny * MAP_MAX + nx] ?? 0);
    return kind === Tile.Wall || kind === Tile.Door;
  };
  return (joins(x, y - 1) ? 1 : 0) | (joins(x + 1, y) ? 2 : 0) | (joins(x, y + 1) ? 4 : 0) | (joins(x - 1, y) ? 8 : 0);
}
