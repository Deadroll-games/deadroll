// The map checks in the browser (crates/mapcheck/wasm, README there): the verdict finalize_map
// gives a draft, before any transaction (docs/MAP_DESCRIPTION.md 1, 10). The module runs
// crates/mapcheck with the registry tables decoded by crates/rules, the code the map program runs,
// so this file only writes the borsh request and names the error code.
//
// The shipped module is ./wasm/mapcheck.wasm, rebuilt by `bash crates/mapcheck/wasm/build.sh` (CI
// fails when it is stale). __fixtures__/mapcheck-wasm.json pins the request bytes and the verdicts.
import { useEffect, useState } from "react";
import wasmUrl from "./wasm/mapcheck.wasm?url";
import { classPacks, PointKind, type Cell, type Spawn } from "./types";

/** `ABI_VERSION` of the module this wrapper speaks (2: the arenas, arena update; 3: patch C, the
 * whole design with its secrets, `check_design`; 4: patch D, maps of every size class,
 * `check_steps`). */
export const MAPCHECK_ABI = 4;
/** Arena update: `ARENA_SIDE_MAX` (crates/types). */
export const ARENA_SIDE_MAX = 12;
const ARENA_CELLS = ARENA_SIDE_MAX * ARENA_SIDE_MAX;

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
  // Patch C: secrets and hidden packs.
  "SecretInvalid",
  "SecretNotPassable",
  "SecretOnPoint",
  "TrapDamageOutOfRange",
  "AmbushPackInvalid",
  "AmbushInArena",
  "StashInArena",
  "DoorNotWall",
  "TooManySecrets",
  "TooFewTraps",
  "AmbushUnlinked",
  "ThreatOutOfRange",
  "ThreatMismatch",
  "RosterInvalid",
  "VisibleMismatch",
  "SpawnNotVisible",
  "TooFewSecrets",
  "DuplicateSecret",
] as const;
export type MapCheckError = (typeof MAPCHECK_ERRORS)[number];

/** What the builder shows for each rule (the program's `MapError` messages, a little longer). */
export const MAPCHECK_MESSAGES: Record<MapCheckError, string> = {
  SizeOutOfRange: "Each side must be from 8 to 143, and the area within the largest size",
  TierOutOfRange: "This map tier has no limits",
  InvalidTile: "A cell has a tile type the Tileset does not have",
  EntranceCount: "Place exactly one entrance",
  ExitCount: "Place exactly one exit",
  EntranceMismatch: "Entrance mismatch",
  ExitMismatch: "Exit mismatch",
  TooManySpawns: "Too many monsters for this map size",
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
  PackOutOfRange: "A pack number past what this map size stores",
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
  SecretInvalid: "A secret does not fit its cell",
  SecretNotPassable: "A trap or an ambush must lie on a cell that can be entered",
  SecretOnPoint: "A trap or an ambush cannot lie on the entrance, the exit, a chest or a pack",
  TrapDamageOutOfRange: "Trap damage must be from 1 to the map tier's limit",
  AmbushPackInvalid: "An ambush must lead to a pack with no monster on the map",
  AmbushInArena: "An ambush trap lies on the map, not in an arena",
  StashInArena: "A hidden stash lies on the map, not in an arena",
  DoorNotWall: "A secret door goes in a wall",
  TooManySecrets: "Too many secrets for this map tier",
  TooFewTraps: "This map tier needs at least one trap",
  AmbushUnlinked: "Each ambush pack needs exactly one ambush trap",
  ThreatOutOfRange: "A pack's threat does not fit",
  ThreatMismatch: "A pack's threat does not match its monsters",
  RosterInvalid: "A pack has no monsters or too many",
  VisibleMismatch: "Pick the monster shown on the map from its own pack",
  SpawnNotVisible: "Only the monster each pack shows may stand on the map",
  TooFewSecrets: "Fewer secrets than declared",
  DuplicateSecret: "Two secrets on one cell",
};

/** The registry tables finalize_map reads, as written on chain (`set_table` kinds 0-3). */
export type MapcheckTables = { bestiary: Uint8Array; monsterSkills: Uint8Array; rewards: Uint8Array; tileset: Uint8Array };

/** A draft as finalize_map sees it. `tiles`: cell bytes, `width` per row (patch D). */
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
  /** Arena update: the packs by number (`null`: not used); patch D: up to the size class's
   * pack numbers (`classPacks`). */
  packs: (PackDraft | null)[];
  /** Patch D: the size class (`classOf`). */
  sizeClass: number;
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

/** Patch D: the borsh `MapCheckRequest`: width, height, tier, min players, the cells (`width` per
 * row), every spawn, the points, the boss table, the class's packs and their arenas (12 per row). */
export function encodeMapCheck(d: MapCheckDraft): Uint8Array {
  const n = classPacks(d.sizeClass);
  const points = draftPointList(d);
  const cells = d.width * d.height;
  const out = new Uint8Array(4 + 4 + cells + 4 + d.spawns.length * 9 + 4 + points.length * 4 + 1 + 4 + n * 6 + 4 + n * ARENA_CELLS);
  const view = new DataView(out.buffer);
  let at = 0;
  const u8 = (v: number) => (out[at++] = v & 0xff);
  const len = (v: number) => {
    view.setUint32(at, v, true);
    at += 4;
  };
  [d.width, d.height, d.tier, d.minPlayers].forEach(u8);
  len(cells);
  out.set(d.tiles.subarray(0, cells), at);
  at += cells;
  len(d.spawns.length);
  for (const s of d.spawns) [s.bestiaryId, s.x, s.y, s.level, s.skills & 0xff, (s.skills >> 8) & 0xff, s.flags, s.aspects, s.pack].forEach(u8);
  len(points.length);
  for (const p of points) [p.kind, p.x, p.y, p.data].forEach(u8);
  u8(d.bossTable);
  len(n);
  for (let k = 0; k < n; k++) {
    const pk = d.packs[k];
    if (pk) [pk.visible, pk.x, pk.y, pk.facing, pk.width, pk.height].forEach(u8);
    else at += 6;
  }
  len(n * ARENA_CELLS);
  for (let k = 0; k < n; k++) {
    const pk = d.packs[k];
    if (pk) for (let y = 0; y < pk.height; y++) out.set(pk.cells.subarray(y * pk.width, (y + 1) * pk.width), at + y * ARENA_SIDE_MAX);
    at += ARENA_CELLS;
  }
  return out;
}

/** Patch C: a secret of a draft, by leaf (`@deadroll/sdk/secrets` `Leaves`). */
export type DraftSecret = { leaf: number; kind: number; a: number; b: number; c: number };

/** Patch C: the borsh `DesignRequest`: the map request, the declared counts, the secrets. */
export function encodeDesign(d: MapCheckDraft, declared: number[], secrets: DraftSecret[]): Uint8Array {
  const map = encodeMapCheck(d);
  const out = new Uint8Array(map.length + 4 + 4 + secrets.length * 6);
  out.set(map, 0);
  let at = map.length;
  for (let i = 0; i < 4; i++) out[at++] = declared[i] ?? 0;
  new DataView(out.buffer).setUint32(at, secrets.length, true);
  at += 4;
  for (const x of secrets) {
    out.set([x.leaf & 0xff, x.leaf >> 8, x.kind, x.a, x.b, x.c], at);
    at += 6;
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
  check_design(len: number): number;
  check_steps(len: number): number;
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

  /** Patch C: the first broken rule of the whole design (every spawn, every secret), or null. */
  checkDesign(d: MapCheckDraft, declared: number[], secrets: DraftSecret[]): MapCheckError | null {
    const r = this.designVerdict(encodeDesign(d, declared, secrets));
    if (r < 0) throw new Error(`mapcheck wasm: error ${r}`);
    if (r === 0) return null;
    const name = MAPCHECK_ERRORS[r - 1];
    if (!name) throw new Error(`mapcheck wasm: unknown rule ${r - 1}`);
    return name;
  }

  /** Patch C: the raw verdict of a design request. */
  designVerdict(input: Uint8Array): number {
    this.write(input);
    return this.x.check_design(input.length);
  }

  /** Patch D: the `check_step` transactions the map needs before `finalize_map` (the program's
   * own budget split), or a negative error. */
  steps(d: MapCheckDraft): number {
    return this.stepsOf(encodeMapCheck(d));
  }

  /** Patch D: the raw step count of a request. */
  stepsOf(input: Uint8Array): number {
    this.write(input);
    return this.x.check_steps(input.length);
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
