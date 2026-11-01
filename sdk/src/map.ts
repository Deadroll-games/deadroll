export * from "./generated/map/index.js";

// Patch D: a `MapLayout` of version 5 is as big as its size class and sides need
// (`deadroll_types::map_size::LayoutGeom`); the generated decoder reads its header only. This one
// reads the whole account at the same offsets.

/** `MapLayout.version` of the format. */
export const LAYOUT_V5 = 5;
/** Pack numbers, visible chests, secrets and encrypted-copy bytes of each size class (crates/types). */
export const CLASS_PACKS = [8, 16, 24, 40] as const;
export const CLASS_CHESTS = [4, 8, 12, 16] as const;
export const CLASS_SECRETS = [16, 48, 80, 96] as const;
export const CLASS_BLOB = [800, 1600, 2400, 3600] as const;
/** Most cells of each class (`BalanceConfig.map_size.cells`): Small, Medium, Big, Dungeon. */
export const CLASS_CELLS = [1024, 5184, 10201, 20449] as const;
export const MAP_SIDE_MIN = 8;
export const MAP_SIDE_CAP = 143;
export const ARENA_SIDE_MAX = 12;
export const ARENA_CELLS = ARENA_SIDE_MAX * ARENA_SIDE_MAX;
/** `LayoutGeom::CHECK_LEN`. */
const CHECK_LEN = 8;
const HEADER = 8 + 1 + 32 + 7;

/** The size class of a `width` x `height` map (`MapSizeTuning::class_of`), or null. */
export function classOf(width: number, height: number): number | null {
  if (width < MAP_SIDE_MIN || height < MAP_SIDE_MIN || width > MAP_SIDE_CAP || height > MAP_SIDE_CAP) return null;
  const i = CLASS_CELLS.findIndex((c) => width * height <= c);
  return i < 0 ? null : i;
}

/** Where everything lies in a version 5 layout (`LayoutGeom::new`). */
export type LayoutGeom = {
  width: number;
  height: number;
  sizeClass: number;
  packs: number;
  points: number;
  blobCap: number;
  rowWords: number;
  spawnsAt: number;
  pointsAt: number;
  packsAt: number;
  threatAt: number;
  declaredAt: number;
  arenasAt: number;
  cellsAt: number;
  blobLenAt: number;
  blobAt: number;
  checkAt: number;
  end: number;
};

export function layoutGeom(width: number, height: number, sizeClass: number): LayoutGeom {
  const packs = CLASS_PACKS[sizeClass]!;
  const points = 2 + CLASS_CHESTS[sizeClass]!;
  const blobCap = CLASS_BLOB[sizeClass]!;
  const rowWords = Math.ceil(width / 64);
  const spawnsAt = HEADER;
  const pointsAt = spawnsAt + packs * 9;
  const packsAt = pointsAt + points * 4;
  const threatAt = packsAt + packs * 6;
  const declaredAt = threatAt + packs;
  const arenasAt = declaredAt + 4;
  const cellsAt = arenasAt + packs * ARENA_CELLS;
  const blobLenAt = cellsAt + width * height;
  const blobAt = blobLenAt + 2;
  const checkAt = blobAt + blobCap;
  const end = checkAt + CHECK_LEN + 2 * height * rowWords * 8;
  return { width, height, sizeClass, packs, points, blobCap, rowWords, spawnsAt, pointsAt, packsAt, threatAt, declaredAt, arenasAt, cellsAt, blobLenAt, blobAt, checkAt, end };
}

/** Bytes `grow_layout` adds per instruction (the runtime's `MAX_PERMITTED_DATA_INCREASE`). */
export const GROW_STEP = 10_240;
/** `grow_layout` instructions a map of this size needs after `create_map`. */
export const growsNeeded = (g: LayoutGeom) => Math.max(0, Math.ceil((g.end - GROW_STEP) / GROW_STEP));

export type LayoutSpawn = { bestiaryId: number; x: number; y: number; level: number; skills: number; flags: number; aspects: number; pack: number };
export type LayoutPoint = { kind: number; x: number; y: number; data: number };
export type LayoutPack = { visible: number; x: number; y: number; facing: number; width: number; height: number };

/** A version 5 layout, read whole. */
export type MapLayoutV5 = {
  geom: LayoutGeom;
  map: Uint8Array;
  bossTable: number;
  /** `width * height` cell bytes, `width` per row. */
  cells: Uint8Array;
  /** Each pack's visible monster. */
  spawns: LayoutSpawn[];
  points: LayoutPoint[];
  /** By pack number, the class's count. */
  packs: LayoutPack[];
  /** Each pack's arena, `ARENA_SIDE_MAX` per row. */
  arenas: Uint8Array[];
  threat: number[];
  declared: number[];
  blob: Uint8Array;
  /** The reachability check: phase 3 = done (`deadroll_mapcheck::wide`). */
  checkPhase: number;
};

/** Decodes a version 5 `MapLayout` account (discriminator included); throws on another version or a
 * layout not grown to its size yet. */
export function decodeMapLayout(raw: Uint8Array): MapLayoutV5 {
  if (raw[8] !== LAYOUT_V5) throw new Error(`MapLayout version ${raw[8]}, expected ${LAYOUT_V5}`);
  const g = layoutGeom(raw[41]!, raw[42]!, raw[43]!);
  if (raw.length < g.end) throw new Error("MapLayout not grown to its size yet");
  const spawnCount = raw[46]!;
  const pointCount = raw[47]!;
  const spawn = (at: number): LayoutSpawn => ({
    bestiaryId: raw[at]!,
    x: raw[at + 1]!,
    y: raw[at + 2]!,
    level: raw[at + 3]!,
    skills: raw[at + 4]! | (raw[at + 5]! << 8),
    flags: raw[at + 6]!,
    aspects: raw[at + 7]!,
    pack: raw[at + 8]!,
  });
  const blobLen = raw[g.blobLenAt]! | (raw[g.blobLenAt + 1]! << 8);
  return {
    geom: g,
    map: raw.slice(9, 41),
    bossTable: raw[44]!,
    cells: raw.slice(g.cellsAt, g.blobLenAt),
    spawns: Array.from({ length: Math.min(spawnCount, g.packs) }, (_, i) => spawn(g.spawnsAt + i * 9)),
    points: Array.from({ length: Math.min(pointCount, g.points) }, (_, i) => {
      const at = g.pointsAt + i * 4;
      return { kind: raw[at]!, x: raw[at + 1]!, y: raw[at + 2]!, data: raw[at + 3]! };
    }),
    packs: Array.from({ length: g.packs }, (_, n) => {
      const at = g.packsAt + n * 6;
      return { visible: raw[at]!, x: raw[at + 1]!, y: raw[at + 2]!, facing: raw[at + 3]!, width: raw[at + 4]!, height: raw[at + 5]! };
    }),
    arenas: Array.from({ length: g.packs }, (_, n) => raw.slice(g.arenasAt + n * ARENA_CELLS, g.arenasAt + (n + 1) * ARENA_CELLS)),
    threat: Array.from(raw.slice(g.threatAt, g.declaredAt)),
    declared: Array.from(raw.slice(g.declaredAt, g.arenasAt)),
    blob: raw.slice(g.blobAt, g.blobAt + Math.min(blobLen, g.blobCap)),
    checkPhase: raw[g.checkAt]!,
  };
}

/** `sha256("account:MapLayout")[..8]` (the IDL no longer lists the account: no context decodes it). */
export const MAP_LAYOUT_DISCRIMINATOR = new Uint8Array([0x8e, 0x57, 0xdf, 0xf4, 0x3b, 0x7b, 0x3e, 0xca]);
