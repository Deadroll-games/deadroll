// Map presets (assets/registry/maps/*.json) as the map program stores them.
// Format and legend: crates/rules/tests/demo_preset.rs (the Rust check of the same files).
import { readFileSync } from "node:fs";
import { getAddressEncoder, signBytes, type Address, type KeyPairSigner } from "@solana/kit";
import { NO_SPAWN, Secret, UNLOCK_MESSAGE, commit, declaredOf, deriveKeys, sealSecrets, splitDesign, threat, Leaves, type CellEntry, type Commitment, type Roster } from "@deadroll/sdk/secrets";

/** `MAP_MAX`: tiles are row-major with this stride whatever the map width. */
export const MAP_STRIDE = 32;
export const MAP_CELLS = MAP_STRIDE * MAP_STRIDE;
export const SPAWNS_MAX = 24;
/** `CHESTS_MAX` and `NO_TABLE` (crates/types). */
export const CHESTS_MAX = 4;
export const NO_TABLE = 0xff;

/** Patch 13: `TileKind` discriminants (crates/types), bits 7-3 of a cell byte. */
export const Tile = {
  Void: 0,
  StoneFloor: 1,
  WoodenFloor: 2,
  Dirt: 3,
  Wall: 4,
  Door: 5,
  ShallowWater: 6,
  DeepWater: 7,
  Stairs: 8,
  Rubble: 9,
  Chasm: 10,
} as const;
/** `PointKind` discriminants (crates/types). */
export const PointKind = { Chest: 0, Entrance: 1, Exit: 2 } as const;
/** `Biome` discriminants (crates/types): the art set of a map, `Map.biome`. */
export const Biome = { Caves: 0, Forest: 1, Crypt: 2 } as const;
export const BIOME_COUNT = 3;

/** A cell byte (`cell_byte`): tile type and variant. */
export const cellByte = (kind: number, variant = 0) => ((kind & 31) << 3) | (variant & 7);
/** The tile type of a cell byte (`cell_kind`). */
export const cellKind = (byte: number) => (byte >> 3) & 31;

/** Row characters (the variant comes from `variants`, 0 without it); `E` and `X` are a stone floor with the entrance or the exit point. */
const LEGEND: Record<string, number> = {
  " ": Tile.Void,
  ".": Tile.StoneFloor,
  E: Tile.StoneFloor,
  X: Tile.StoneFloor,
  "=": Tile.WoodenFloor,
  ",": Tile.Dirt,
  "#": Tile.Wall,
  D: Tile.Door,
  "~": Tile.ShallowWater,
  W: Tile.DeepWater,
  ">": Tile.Stairs,
  "%": Tile.Rubble,
  _: Tile.Chasm,
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
/** Arena update: `Facing` discriminants (crates/types), clockwise from the north. */
export const FACINGS = ["N", "NE", "E", "SE", "S", "SW", "W", "NW"] as const;
/** `PACKS_MAX`, `ARENA_SIDE_MAX` (crates/types). */
export const PACKS_MAX = 8;
export const ARENA_SIDE_MAX = 12;
/** One pack (`PackLayout`) and its arena cells, `width` cells per row (`write_arena`). */
export type Pack = {
  pack: number;
  visible: number;
  x: number;
  y: number;
  facing: number;
  width: number;
  height: number;
  cells: Uint8Array;
};
/** A visible chest (`MapPoint` with `PointKind::Chest`): the cell and its reward table id. */
export type Chest = { x: number; y: number; table: number };
export type MapPreset = {
  name: string;
  width: number;
  height: number;
  tier: number;
  /** The art set (`Biome`), `Caves` when the preset has none. */
  biome: number;
  /** Patch 11: the reward budget counts from this party size. */
  minPlayers: number;
  /** Patch 11, set by the DM: no monster this close to a chest or a player when it opens. */
  chestRadius: number;
  /** Patch 11, set by the DM: seconds to issue the rewards after a party ends. */
  settleTimeoutSecs: number;
  /** Patch 11: reward table of the boss event. */
  bossTable: number;
  chests: Chest[];
  /** `MAP_CELLS` cell bytes, index `y * MAP_STRIDE + x`; cells outside the map stay Void (0). */
  tiles: Uint8Array;
  entrance: Cell;
  exit: Cell;
  /** Arena update: a spawn's `x`, `y` are its cell in its pack's arena. */
  spawns: Spawn[];
  packs: Pack[];
  /** Patch C: the cells that hide something (`"secrets"`, crates/rules/tests/demo_preset.rs). */
  secrets: PresetSecret[];
};
/** Patch C: a secret of a preset; `pack` set: a cell of that pack's arena. */
export type PresetSecret = { kind: "trap" | "ambush" | "stash" | "door"; x: number; y: number; pack?: number; damage?: number; difficulty?: number; target?: number; table?: number };

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
  const biome = u8(json.biome ?? Biome.Caves, "biome");
  if (biome >= BIOME_COUNT) throw new Error(`unknown biome ${biome}`);
  // Optional: one digit 0-7 per cell, the variant (bits 2-0 of the cell byte) the DM picked.
  const variants: unknown = json.variants ?? rows.map((r: string) => "0".repeat([...r].length));
  if (!Array.isArray(variants) || variants.length !== height) throw new Error("variants count != height");

  const tiles = new Uint8Array(MAP_CELLS);
  let entrance: Cell | null = null;
  let exit: Cell | null = null;
  rows.forEach((row, y) => {
    if (typeof row !== "string" || [...row].length !== width) throw new Error(`row ${y} length != width`);
    [...row].forEach((ch, x) => {
      const tile = LEGEND[ch];
      if (tile === undefined) throw new Error(`unknown tile character ${JSON.stringify(ch)} at ${x},${y}`);
      const variant = Number([...String(variants[y])][x]);
      if (!Number.isInteger(variant) || variant > 7) throw new Error(`variant at ${x},${y} must be a digit 0-7`);
      tiles[y * MAP_STRIDE + x] = cellByte(tile, variant);
      if (ch === "E") {
        if (entrance) throw new Error("preset has two E cells");
        entrance = { x, y };
      }
      if (ch === "X") {
        if (exit) throw new Error("preset has two X cells");
        exit = { x, y };
      }
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
  const packList: unknown = json.packs ?? [];
  if (!Array.isArray(packList) || packList.length > PACKS_MAX) throw new Error(`packs must be a list of at most ${PACKS_MAX}`);
  const packs = packList.map((p: any, i: number): Pack => {
    const arenaRows: unknown = p.rows;
    if (!Array.isArray(arenaRows) || arenaRows.length === 0 || arenaRows.length > ARENA_SIDE_MAX) throw new Error(`pack ${i}: 1 to ${ARENA_SIDE_MAX} arena rows`);
    const w = [...String(arenaRows[0])].length;
    if (w > ARENA_SIDE_MAX) throw new Error(`pack ${i}: arena wider than ${ARENA_SIDE_MAX}`);
    const cells = new Uint8Array(w * arenaRows.length);
    arenaRows.forEach((row: unknown, y: number) => {
      const chars = [...String(row)];
      if (chars.length !== w) throw new Error(`pack ${i}: arena row ${y} length`);
      chars.forEach((ch, x) => {
        const tile = LEGEND[ch];
        if (tile === undefined || ch === "E" || ch === "X") throw new Error(`pack ${i}: arena tile ${JSON.stringify(ch)}`);
        cells[y * w + x] = cellByte(tile);
      });
    });
    const facing = FACINGS.indexOf(p.facing);
    if (facing < 0) throw new Error(`pack ${i}: facing must be one of ${FACINGS.join(", ")}`);
    return { pack: u8(p.pack, `pack ${i} pack`), visible: u8(p.visible, `pack ${i} visible`), x: u8(p.x, `pack ${i} x`), y: u8(p.y, `pack ${i} y`), facing, width: w, height: arenaRows.length, cells };
  });
  const minPlayers = u8(json.min_players ?? 1, "min_players");
  const bossTable = u8(json.boss_table ?? NO_TABLE, "boss_table");
  const chestRadius = u8(json.chest_radius ?? 3, "chest_radius");
  const settleTimeoutSecs = Number(json.settle_timeout_secs ?? 3600);
  if (!Number.isInteger(settleTimeoutSecs) || settleTimeoutSecs < 1) throw new Error("settle_timeout_secs must be a positive integer");
  const secrets: PresetSecret[] = (json.secrets ?? []).map((x: any) => ({ ...x }));
  return { name: String(json.name ?? ""), width, height, tier, biome, minPlayers, chestRadius, settleTimeoutSecs, bossTable, chests, tiles, entrance, exit, spawns, packs, secrets };
}

export function loadMapPreset(path: string | URL = DEMO_MAP_PATH): MapPreset {
  return parseMapPreset(JSON.parse(readFileSync(path, "utf8")));
}

/** `write_points`: the entrance, the exit, then the chests (`MapPoint`, the data of a chest is its table). */
export function presetPoints(preset: MapPreset): { kind: number; x: number; y: number; data: number }[] {
  return [
    { kind: PointKind.Entrance, ...preset.entrance, data: 0 },
    { kind: PointKind.Exit, ...preset.exit, data: 0 },
    ...preset.chests.map((c) => ({ kind: PointKind.Chest, x: c.x, y: c.y, data: c.table })),
  ];
}

/** The cell bytes `write_layout(0, ...)` needs: rows 0..height at stride 32, last row cut at width. */
export function layoutBytes(preset: MapPreset): Uint8Array {
  return preset.tiles.slice(0, (preset.height - 1) * MAP_STRIDE + preset.width);
}

// ---------------------------------------------------------------------------------------------
// Patch C: the preset as the DM publishes it (docs/MAP_DESCRIPTION.md 8, 17.5): only the visible
// monsters in the open, the packs' rosters and the secrets under the Merkle root, an encrypted copy
// on chain under a key from the DM's signature of UNLOCK_MESSAGE.

const BESTIARY_PATH = new URL("../../../assets/registry/bestiary.json", import.meta.url);
/** The bestiary ids with the Boss role (the threat counts a boss's level 3 times). */
export function bossIds(): Set<number> {
  const json = JSON.parse(readFileSync(BESTIARY_PATH, "utf8"));
  const list: any[] = Array.isArray(json) ? json : (json.records ?? []);
  return new Set(list.map((m, i) => [m.id ?? i, m.role] as const).filter(([, r]) => r === "Boss").map(([id]) => id));
}

/** The arena shapes by pack number, for the leaf numbering. */
export const packShapes = (packs: Pack[]) => Array.from({ length: PACKS_MAX }, (_, n) => {
  const p = packs.find((q) => q.pack === n);
  return p ? { width: p.width, height: p.height } : null;
});

export type PresetDesign = {
  /** The public spawns and the pack records with `visible` into them. */
  spawns: Spawn[];
  packs: Pack[];
  rosters: Roster[];
  threat: number[];
  cells: CellEntry[];
  declared: number[];
};

export function presetDesign(preset: MapPreset, bosses = bossIds()): PresetDesign {
  const records = Array.from({ length: PACKS_MAX }, (_, n) => preset.packs.find((p) => p.pack === n) ?? null);
  const split = splitDesign(preset.spawns, records);
  const packs = preset.packs.map((p) => ({ ...p, visible: split.packs[p.pack]!.visible }));
  const leaves = new Leaves(preset.width, preset.height, packShapes(preset.packs));
  const cells: CellEntry[] = preset.secrets.map((x) => {
    const board = x.pack === undefined ? { kind: "map" as const, x: x.x, y: x.y } : { kind: "arena" as const, pack: x.pack, x: x.x, y: x.y };
    const leaf = leaves.of(board);
    if (leaf === null) throw new Error(`secret at ${x.x},${x.y} is outside its board`);
    const secret =
      x.kind === "trap" ? { kind: Secret.Trap, a: x.damage ?? 1, b: x.difficulty ?? 0, c: 0 }
      : x.kind === "ambush" ? { kind: Secret.Ambush, a: x.target ?? 0, b: 0, c: 0 }
      : x.kind === "stash" ? { kind: Secret.Stash, a: x.table ?? 0, b: 0, c: 0 }
      : { kind: Secret.Door, a: 0, b: 0, c: 0 };
    return { leaf, secret: secret as CellEntry["secret"] };
  });
  const threats = split.rosters.map((r) => threat(r.spawns.slice(0, r.count), (id) => bosses.has(id)));
  return { spawns: split.spawns, packs, rosters: split.rosters, threat: threats, cells, declared: declaredOf(cells) };
}

/** The DM's signature of UNLOCK_MESSAGE with a keypair (what the wallet's signMessage gives). */
export async function unlockSignature(dm: KeyPairSigner): Promise<Uint8Array> {
  return new Uint8Array(await signBytes(dm.keyPair.privateKey, new TextEncoder().encode(UNLOCK_MESSAGE)));
}

/** The commitment of a preset on map `map`, its encrypted copy and public part, for a DM keypair. */
export async function presetSecrets(preset: MapPreset, map: Address, dm: KeyPairSigner): Promise<{ design: PresetDesign; commitment: Commitment; blob: Uint8Array }> {
  const design = presetDesign(preset);
  const mapBytes = new Uint8Array(getAddressEncoder().encode(map));
  const keys = await deriveKeys(await unlockSignature(dm), mapBytes);
  const secrets = { rosters: design.rosters, cells: design.cells };
  const commitment = await commit(mapBytes, keys, preset.width, preset.height, packShapes(preset.packs), secrets);
  return { design, commitment, blob: await sealSecrets(keys, mapBytes, secrets) };
}

/** `write_secrets` chunks of the encrypted copy (each with the public part). */
export function secretChunks(blob: Uint8Array, design: PresetDesign, size = 600) {
  const out: { offset: number; bytes: Uint8Array; len: number; threat: number[]; declared: number[] }[] = [];
  for (let at = 0; at < blob.length; at += size) out.push({ offset: at, bytes: blob.slice(at, at + size), len: blob.length, threat: design.threat, declared: design.declared });
  return out;
}

export { NO_SPAWN };
