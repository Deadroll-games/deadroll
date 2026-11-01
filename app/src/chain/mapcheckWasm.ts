// The map checks in the browser (crates/mapcheck/wasm, README there): the verdict finalize_map
// gives a draft, before any transaction (docs/MAP_DESCRIPTION.md 1, 10). The module runs
// crates/mapcheck with the registry tables decoded by crates/rules, the code the map program runs,
// so this file only writes the borsh request and names the error code.
//
// The shipped module is ./wasm/mapcheck.wasm, rebuilt by `bash crates/mapcheck/wasm/build.sh` (CI
// fails when it is stale). __fixtures__/mapcheck-wasm.json pins the request bytes and the verdicts.
import { useEffect, useState } from "react";
import wasmUrl from "./wasm/mapcheck.wasm?url";
import { MAP_CELLS, PointKind, SPAWNS_MAX, type Cell, type Spawn } from "./types";

/** `ABI_VERSION` of the module this wrapper speaks (2: the arenas, arena update). */
export const MAPCHECK_ABI = 2;
/** `POINTS_MAX`: the entrance, the exit and up to 4 chests. */
export const POINTS_MAX = 6;
/** Arena update: `PACKS_MAX`, `ARENA_SIDE_MAX` (crates/types). */
export const PACKS_MAX = 8;
export const ARENA_SIDE_MAX = 12;
/** Bytes of a request: 4 map numbers, the 1301-byte layout core, then the 1200-byte arenas (8 packs
 * of 6 bytes, 8 arenas of 144 cells, `ARENA_SIDE_MAX` per row). */
export const MAPCHECK_REQUEST_LEN = 4 + 1301 + PACKS_MAX * 6 + PACKS_MAX * ARENA_SIDE_MAX * ARENA_SIDE_MAX;

/** `deadroll_mapcheck::MapCheckError` by code (append only, as the crate). */
export const MAPCHECK_ERRORS = [
  "SizeOutOfRange",
  "TierOutOfRange",
  "InvalidTile",
  "EntranceCount",
  "ExitCount",
  "EntranceMismatch",
  "ExitMismatch",
  "TooManySpawns",
  "SpawnOutOfBounds",
  "SpawnNotPassable",
  "SpawnOverlap",
  "UnknownMonster",
  "LevelOutOfTier",
  "BossCount",
  "HelperOutsideBossPack",
  "BossPackMixed",
  "TooManyHelpers",
  "TooManyMonsters",
  "TooManyPacks",
  "TwoLoners",
  "PackTooSpread",
  "ExitUnreachable",
  "SpawnUnreachable",
  "TierBelowTemplate",
  "MonsterSkillUnknown",
  "MonsterSkillCount",
  "EliteBoss",
  "TooManyElites",
  "TooManySessionSkills",
  "MinPlayersOutOfRange",
  "TooManyChests",
  "InvalidPoint",
  "ChestOutOfBounds",
  "ChestNotPassable",
  "ChestOverlap",
  "UnknownTable",
  "TableNotAllowed",
  "BossTableMissing",
  "BossTableOneLevel",
  "OverBudget",
  "ChestUnreachable",
  "TooManyPoints",
  "PointOutOfBounds",
  "PointNotPassable",
  "PointOverlap",
  // Arena update.
  "PackOutOfRange",
  "PackWithoutArena",
  "PackTooBig",
  "ArenaSizeOutOfRange",
  "ArenaInvalidTile",
  "VisibleNotInPack",
  "PackOutOfBounds",
  "PackNotPassable",
  "PackOverlap",
  "PackAtEntrance",
  "PackUnreachable",
  "ArenaNoRoom",
  "ArenaUnreachable",
  "TooManyTemplates",
] as const;
export type MapCheckError = (typeof MAPCHECK_ERRORS)[number];

/** What the builder shows for each rule (the program's `MapError` messages, a little longer). */
export const MAPCHECK_MESSAGES: Record<MapCheckError, string> = {
  SizeOutOfRange: "Each side must be from 8 to 32",
  TierOutOfRange: "This map tier has no limits",
  InvalidTile: "A cell has a tile type the Tileset does not have",
  EntranceCount: "Place exactly one entrance",
  ExitCount: "Place exactly one exit",
  EntranceMismatch: "Entrance mismatch",
  ExitMismatch: "Exit mismatch",
  TooManySpawns: `At most ${SPAWNS_MAX} spawns`,
  SpawnOutOfBounds: "A monster stands outside its pack's arena",
  SpawnNotPassable: "A monster stands on an arena cell that cannot be entered",
  SpawnOverlap: "Two monsters of one pack share an arena cell",
  UnknownMonster: "A spawn names a monster the bestiary does not have",
  LevelOutOfTier: "A monster level is outside the map tier",
  BossCount: "The map needs exactly one boss",
  HelperOutsideBossPack: "A boss helper stands outside the boss pack",
  BossPackMixed: "A regular monster is in the boss pack",
  TooManyHelpers: "Too many boss helpers",
  TooManyMonsters: "Too many monsters for this tier",
  TooManyPacks: "Too many packs for this tier",
  TwoLoners: "Two loners in one pack",
  PackTooSpread: "Monsters of one pack stand too far apart (unused since arenas)",
  ExitUnreachable: "There is no path from the entrance to the exit",
  SpawnUnreachable: "A spawn cannot be reached from the entrance (unused since arenas)",
  TierBelowTemplate: "A monster needs a higher map tier",
  MonsterSkillUnknown: "A spawn chose a skill its template does not have",
  MonsterSkillCount: "A spawn must fill every skill slot of its template",
  EliteBoss: "A boss cannot be elite",
  TooManyElites: "Too many elite monsters for this tier",
  TooManySessionSkills: "The spawns use more distinct skills than a party copies",
  MinPlayersOutOfRange: "Minimum players must be from 1 to 4",
  TooManyChests: "Too many chests for this tier",
  InvalidPoint: "Invalid point",
  ChestOutOfBounds: "A chest is outside the map",
  ChestNotPassable: "A chest stands on a cell that cannot be entered",
  ChestOverlap: "A chest shares a cell with the entrance, the exit or another chest",
  UnknownTable: "A reward table does not exist",
  TableNotAllowed: "A reward table is not allowed on this tier",
  BossTableMissing: "Choose the boss reward table",
  BossTableOneLevel: "The boss table needs two levels (the boss guarantee)",
  OverBudget: "Chests and boss cost more than the reward budget",
  ChestUnreachable: "No cell next to a chest can be reached",
  TooManyPoints: "Too many points (entrance, exit and chests)",
  PointOutOfBounds: "The entrance or the exit is outside the map",
  PointNotPassable: "The entrance or the exit stands on a cell that cannot be entered",
  PointOverlap: "The entrance and the exit share a cell",
  PackOutOfRange: `A pack number must be below ${PACKS_MAX}`,
  PackWithoutArena: "A pack has no arena: draw it",
  PackTooBig: "A pack has more monsters than an arena holds",
  ArenaSizeOutOfRange: "An arena side is outside the allowed sizes",
  ArenaInvalidTile: "An arena cell has a tile type the Tileset does not have",
  VisibleNotInPack: "Pick the monster shown on the map from its own pack",
  PackOutOfBounds: "A pack's monster on the map stands outside the map",
  PackNotPassable: "A pack's monster on the map stands on a cell that cannot be entered",
  PackOverlap: "A pack's monster on the map shares a cell with the entrance, the exit, a chest or another pack",
  PackAtEntrance: "A pack's monster on the map stands next to the entrance",
  PackUnreachable: "No cell next to a pack's monster on the map can be reached",
  ArenaNoRoom: "From some side an arena has no room for a party of 4",
  ArenaUnreachable: "From some side the party in an arena is split or cannot reach a monster",
  TooManyTemplates: "Too many different monsters for one party",
};

/** The registry tables finalize_map reads, as written on chain (`set_table` kinds 0-3). */
export type MapcheckTables = { bestiary: Uint8Array; monsterSkills: Uint8Array; rewards: Uint8Array; tileset: Uint8Array };

/** A draft as finalize_map sees it. `tiles`: cell bytes, stride 32. */
export type MapCheckDraft = {
  width: number;
  height: number;
  tier: number;
  minPlayers: number;
  tiles: Uint8Array;
  entrance: Cell | null;
  exit: Cell | null;
  /** Arena update: `x`, `y` of a spawn are its cell in its pack's arena. */
  spawns: Spawn[];
  chests: { x: number; y: number; table: number }[];
  bossTable: number;
  /** Arena update: the packs by number (`null`: not used). */
  packs: (PackDraft | null)[];
};

/** Arena update: one pack as the builder edits it: the spawn shown on the map, its map cell, the
 * facing (`Facing`, 0 = N, clockwise), the arena size and cells (`width` per row). */
export type PackDraft = { visible: number; x: number; y: number; facing: number; width: number; height: number; cells: Uint8Array };

/** The points as `write_points` stores them: the entrance, the exit (when placed), the chests. */
export function draftPointList(d: Pick<MapCheckDraft, "entrance" | "exit" | "chests">): { kind: number; x: number; y: number; data: number }[] {
  return [
    ...(d.entrance ? [{ kind: PointKind.Entrance, ...d.entrance, data: 0 }] : []),
    ...(d.exit ? [{ kind: PointKind.Exit, ...d.exit, data: 0 }] : []),
    ...d.chests.map((c) => ({ kind: PointKind.Chest, x: c.x, y: c.y, data: c.table })),
  ];
}

/** The borsh `MapCheckRequest`: width, height, tier, min players, then `MapLayoutDataV2` (version,
 * map key and bump zero: finalize_map does not check them). */
export function encodeMapCheck(d: MapCheckDraft): Uint8Array {
  const out = new Uint8Array(MAPCHECK_REQUEST_LEN);
  let at = 0;
  const u8 = (v: number) => (out[at++] = v & 0xff);
  [d.width, d.height, d.tier, d.minPlayers].forEach(u8);
  u8(0); // version
  at += 32; // map
  out.set(d.tiles.subarray(0, MAP_CELLS), at);
  at += MAP_CELLS;
  for (let i = 0; i < SPAWNS_MAX; i++) {
    const s = d.spawns[i];
    if (s) [s.bestiaryId, s.x, s.y, s.level, s.skills & 0xff, (s.skills >> 8) & 0xff, s.flags, s.aspects, s.pack].forEach(u8);
    else at += 9;
  }
  u8(Math.min(d.spawns.length, 0xff));
  const points = draftPointList(d);
  for (let i = 0; i < POINTS_MAX; i++) {
    const p = points[i];
    if (p) [p.kind, p.x, p.y, p.data].forEach(u8);
    else at += 4;
  }
  u8(Math.min(points.length, 0xff));
  u8(d.bossTable);
  u8(0); // bump
  // Arena update: `MapArenasData`.
  for (let n = 0; n < PACKS_MAX; n++) {
    const pk = d.packs[n];
    if (pk) [pk.visible, pk.x, pk.y, pk.facing, pk.width, pk.height].forEach(u8);
    else at += 6;
  }
  for (let n = 0; n < PACKS_MAX; n++) {
    const pk = d.packs[n];
    if (pk) for (let y = 0; y < pk.height; y++) out.set(pk.cells.subarray(y * pk.width, (y + 1) * pk.width), at + y * ARENA_SIDE_MAX);
    at += ARENA_SIDE_MAX * ARENA_SIDE_MAX;
  }
  return out;
}

type Exports = {
  memory: WebAssembly.Memory;
  abi_version(): number;
  buffer_len(): number;
  input_ptr(): number;
  set_table(kind: number, len: number): number;
  check_map(len: number): number;
};

const TABLE_KINDS: (keyof MapcheckTables)[] = ["bestiary", "monsterSkills", "rewards", "tileset"];

export class MapcheckWasm {
  private constructor(private readonly x: Exports) {}

  static async instantiate(bytes: BufferSource): Promise<MapcheckWasm> {
    const { instance } = await WebAssembly.instantiate(bytes, {});
    const x = instance.exports as unknown as Exports;
    const abi = x.abi_version();
    if (abi !== MAPCHECK_ABI) throw new Error(`mapcheck wasm ABI ${abi}, expected ${MAPCHECK_ABI}`);
    return new MapcheckWasm(x);
  }

  private write(input: Uint8Array) {
    if (input.length > this.x.buffer_len()) throw new Error("mapcheck wasm: input longer than the buffer");
    new Uint8Array(this.x.memory.buffer).set(input, this.x.input_ptr());
  }

  /** Loads the registry tables (call again when they change). */
  setTables(t: MapcheckTables) {
    TABLE_KINDS.forEach((name, kind) => {
      this.write(t[name]);
      const r = this.x.set_table(kind, t[name].length);
      if (r !== 0) throw new Error(`mapcheck wasm: set_table ${name}: ${r}`);
    });
  }

  /** The first broken rule, or null for a map finalize_map accepts. */
  check(d: MapCheckDraft): MapCheckError | null {
    const input = encodeMapCheck(d);
    this.write(input);
    const r = this.x.check_map(input.length);
    if (r < 0) throw new Error(`mapcheck wasm: error ${r}`);
    if (r === 0) return null;
    const name = MAPCHECK_ERRORS[r - 1];
    if (!name) throw new Error(`mapcheck wasm: unknown rule ${r - 1}`);
    return name;
  }

  /** The raw verdict: 0 valid, `1 + code` (tests compare it with the fixture). */
  verdict(input: Uint8Array): number {
    this.write(input);
    return this.x.check_map(input.length);
  }
}

let loading: Promise<MapcheckWasm> | null = null;

/** The shipped module, fetched and instantiated once per page. */
export function loadMapcheck(): Promise<MapcheckWasm> {
  loading ??= fetch(wasmUrl)
    .then((res) => {
      if (!res.ok) throw new Error(`mapcheck.wasm: HTTP ${res.status}`);
      return res.arrayBuffer();
    })
    .then((b) => MapcheckWasm.instantiate(b))
    .catch((err) => {
      loading = null;
      throw err;
    });
  return loading;
}

/** The mapcheck module once loaded (null before, or if it failed: the builder cannot finalize then). */
export function useMapcheck(): MapcheckWasm | null {
  const [m, setM] = useState<MapcheckWasm | null>(null);
  useEffect(() => {
    let live = true;
    loadMapcheck().then(
      (x) => live && setM(x),
      (err) => console.warn(err),
    );
    return () => {
      live = false;
    };
  }, []);
  return m;
}
