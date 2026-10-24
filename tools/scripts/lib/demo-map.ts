// Map presets (assets/registry/maps/*.json) as the map program stores them.
// Format and legend: crates/mapcheck/tests/demo_preset.rs (the Rust check of the same files).
import { readFileSync } from "node:fs";

/** `MAP_MAX`: tiles are row-major with this stride whatever the map width. */
export const MAP_STRIDE = 32;
export const MAP_CELLS = MAP_STRIDE * MAP_STRIDE;
export const SPAWNS_MAX = 24;
/** `CHESTS_MAX` and `NO_TABLE` (crates/types). */
export const CHESTS_MAX = 4;
export const NO_TABLE = 0xff;

/** `Tile` discriminants (crates/types). */
export const Tile = { Floor: 0, Wall: 1, Water: 2, Door: 3, Entrance: 4, Exit: 5, Trap: 6 } as const;

const LEGEND: Record<string, number> = {
  "#": Tile.Wall,
  ".": Tile.Floor,
  "~": Tile.Water,
  D: Tile.Door,
  E: Tile.Entrance,
  X: Tile.Exit,
  T: Tile.Trap,
};

export type Cell = { x: number; y: number };
export type Spawn = {
  bestiaryId: number;
  x: number;
  y: number;
  level: number;
  skills: number;
  flags: number;
  aspects: number;
  pack: number;
};
/** A visible chest (`MapPoint` with `PointKind::Chest`): the cell and its reward table id. */
export type Chest = { x: number; y: number; table: number };
export type MapPreset = {
  name: string;
  width: number;
  height: number;
  tier: number;
  /** Patch 11: the reward budget counts from this party size. */
  minPlayers: number;
  /** Patch 11: reward table of the boss event. */
  bossTable: number;
  chests: Chest[];
  /** `MAP_CELLS` bytes, index `y * MAP_STRIDE + x`; cells outside the map stay Floor (0). */
  tiles: Uint8Array;
  entrance: Cell;
  exit: Cell;
  spawns: Spawn[];
};

export const DEMO_MAP_PATH = new URL("../../../assets/registry/maps/demo_16x16.json", import.meta.url);

function u8(value: unknown, what: string): number {
  if (typeof value !== "number" || !Number.isInteger(value) || value < 0 || value > 0xff) {
    throw new Error(`${what} must be a u8, got ${JSON.stringify(value)}`);
  }
  return value;
}

export function parseMapPreset(json: any): MapPreset {
  const width = u8(json.width, "width");
  const height = u8(json.height, "height");
  const tier = u8(json.tier, "tier");
  if (width > MAP_STRIDE || height > MAP_STRIDE) throw new Error("map larger than 32x32");
  const rows: unknown = json.rows;
  if (!Array.isArray(rows) || rows.length !== height) throw new Error("rows count != height");

  const tiles = new Uint8Array(MAP_CELLS);
  let entrance: Cell | null = null;
  let exit: Cell | null = null;
  rows.forEach((row, y) => {
    if (typeof row !== "string" || [...row].length !== width) throw new Error(`row ${y} length != width`);
    [...row].forEach((ch, x) => {
      const tile = LEGEND[ch];
      if (tile === undefined) throw new Error(`unknown tile character ${JSON.stringify(ch)} at ${x},${y}`);
      tiles[y * MAP_STRIDE + x] = tile;
      if (tile === Tile.Entrance) entrance = { x, y };
      if (tile === Tile.Exit) exit = { x, y };
    });
  });
  if (!entrance || !exit) throw new Error("preset needs one E and one X cell");

  const list: unknown = json.spawns;
  if (!Array.isArray(list) || list.length > SPAWNS_MAX) throw new Error(`spawns must be a list of at most ${SPAWNS_MAX}`);
  const spawns = list.map((s: any, i: number) => ({
    bestiaryId: u8(s.bestiary_id, `spawn ${i} bestiary_id`),
    x: u8(s.x, `spawn ${i} x`),
    y: u8(s.y, `spawn ${i} y`),
    level: u8(s.level, `spawn ${i} level`),
    skills: s.skills ?? 0,
    flags: u8(s.flags ?? 0, `spawn ${i} flags`),
    aspects: u8(s.aspects ?? 0, `spawn ${i} aspects`),
    pack: u8(s.pack, `spawn ${i} pack`),
  }));
  const chestList: unknown = json.chests ?? [];
  if (!Array.isArray(chestList) || chestList.length > CHESTS_MAX) throw new Error(`chests must be a list of at most ${CHESTS_MAX}`);
  const chests = chestList.map((c: any, i: number) => ({
    x: u8(c.x, `chest ${i} x`),
    y: u8(c.y, `chest ${i} y`),
    table: u8(c.table, `chest ${i} table`),
  }));
  const minPlayers = u8(json.min_players ?? 1, "min_players");
  const bossTable = u8(json.boss_table ?? NO_TABLE, "boss_table");
  return { name: String(json.name ?? ""), width, height, tier, minPlayers, bossTable, chests, tiles, entrance, exit, spawns };
}

export function loadMapPreset(path: string | URL = DEMO_MAP_PATH): MapPreset {
  return parseMapPreset(JSON.parse(readFileSync(path, "utf8")));
}

/** The tile bytes `write_layout(0, ...)` needs: rows 0..height at stride 32, last row cut at width. */
export function layoutBytes(preset: MapPreset): Uint8Array {
  return preset.tiles.slice(0, (preset.height - 1) * MAP_STRIDE + preset.width);
}
