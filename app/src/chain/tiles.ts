// Patch 13: tile properties are the registry Tileset (docs/MAP_DESCRIPTION.md 4.2), never code.
// Records of 8 bytes `[passable, cost, blocks_los, flags, ...]` by tile type id, decoded the way
// `deadroll_rules::tiles::tile_rules` does; on chain a party reads them from `GameSession.tile_cost`.
import registryTables from "../../../tools/scripts/lib/__fixtures__/registry-tables.json";
import { TILE_KINDS_MAX } from "./types";

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
