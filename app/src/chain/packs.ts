// Arena update (docs/MAP_DESCRIPTION.md 17): the builder's edits of packs and their arenas. A
// pack is one monster shown on the map (its map cell and facing) and an arena the DM draws; its
// spawns stand in the arena (a spawn's x, y are arena cells). Pure functions over the draft; the
// rules are finalize_map's (crates/mapcheck through the WASM).
import balance from "./balance.json";
import { PACKS_MAX, type PackDraft } from "./mapcheckWasm";
import { MAP_MAX, Tile, cellByte, type Cell, type Spawn } from "./types";

/** `BalanceConfig.arena`: the arena sides a DM may draw. */
export const ARENA_SIDE_MIN: number = balance.arena.side_min;
export const ARENA_SIDE_MAX: number = balance.arena.side_max;
/** A new pack's arena: an open floor of this side. */
export const NEW_ARENA_SIDE = 8;
/** `Facing`: 0 = N, clockwise. A new pack looks south. */
export const FACING_SOUTH = 4;

export type PackEdit = { spawns: Spawn[]; packs: (PackDraft | null)[] };

const floor = (side: number) => new Uint8Array(side * side).fill(cellByte(Tile.StoneFloor));

/** The pack whose visible monster stands on map cell `c`, or null. */
export function packAt(d: Pick<PackEdit, "packs">, c: Cell): number | null {
  const n = d.packs.findIndex((pk) => pk !== null && pk.x === c.x && pk.y === c.y);
  return n < 0 ? null : n;
}

/** A new pack of one `bestiaryId` monster (its skills `skills`) shown on map cell `c`, in the
 * middle of an open arena; null when every pack number is taken. Returns the draft and the pack. */
export function addPack(d: PackEdit, c: Cell, bestiaryId: number, skills: number): { next: PackEdit; pack: number } | null {
  const pack = d.packs.findIndex((pk, n) => n > 0 && pk === null);
  if (pack < 0) return null;
  const mid = Math.floor(NEW_ARENA_SIDE / 2);
  const spawn: Spawn = { bestiaryId, x: mid, y: mid, level: 1, skills, flags: 0, aspects: 0, pack };
  const packs = d.packs.slice();
  packs[pack] = { visible: d.spawns.length, x: c.x, y: c.y, facing: FACING_SOUTH, width: NEW_ARENA_SIDE, height: NEW_ARENA_SIDE, cells: floor(NEW_ARENA_SIDE) };
  return { next: { spawns: [...d.spawns, spawn], packs }, pack };
}

/** Removes the spawns `drop` returns true for, keeping every pack's visible index on its spawn; a
 * pack left without its visible spawn shows its first remaining one, a pack left empty goes. */
export function removeSpawns(d: PackEdit, drop: (s: Spawn, i: number) => boolean): PackEdit {
  const index: number[] = [];
  const spawns: Spawn[] = [];
  d.spawns.forEach((s, i) => {
    index[i] = drop(s, i) ? -1 : spawns.push(s) - 1;
  });
  const packs = d.packs.map((pk, n) => {
    if (!pk) return null;
    const kept = index[pk.visible] ?? -1;
    if (kept >= 0) return { ...pk, visible: kept };
    const first = spawns.findIndex((s) => s.pack === n);
    return first < 0 ? null : { ...pk, visible: first };
  });
  return { spawns, packs };
}

/** Adds a `bestiaryId` monster to pack `pack` at arena cell `c` (null when taken or outside). */
export function addToPack(d: PackEdit, pack: number, c: Cell, bestiaryId: number, skills: number): PackEdit | null {
  const pk = d.packs[pack];
  if (!pk || c.x >= pk.width || c.y >= pk.height) return null;
  if (d.spawns.some((s) => s.pack === pack && s.x === c.x && s.y === c.y)) return null;
  return { ...d, spawns: [...d.spawns, { bestiaryId, x: c.x, y: c.y, level: 1, skills, flags: 0, aspects: 0, pack }] };
}

/** Paints arena cell `c` of pack `pack`. */
export function paintArena(d: PackEdit, pack: number, c: Cell, byte: number): PackEdit {
  const pk = d.packs[pack];
  if (!pk || c.x >= pk.width || c.y >= pk.height) return d;
  const cells = pk.cells.slice();
  cells[c.y * pk.width + c.x] = byte;
  return { ...d, packs: d.packs.map((x, n) => (n === pack ? { ...pk, cells } : x)) };
}

/** Resizes pack `pack`'s arena within the limits, keeping the cells that stay (new ones are floor). */
export function resizeArena(d: PackEdit, pack: number, width: number, height: number): PackEdit {
  const pk = d.packs[pack];
  if (!pk) return d;
  const clamp = (v: number) => Math.max(ARENA_SIDE_MIN, Math.min(ARENA_SIDE_MAX, Math.round(v) || ARENA_SIDE_MIN));
  const [w, h] = [clamp(width), clamp(height)];
  const cells = new Uint8Array(w * h).fill(cellByte(Tile.StoneFloor));
  for (let y = 0; y < Math.min(h, pk.height); y++) for (let x = 0; x < Math.min(w, pk.width); x++) cells[y * w + x] = pk.cells[y * pk.width + x]!;
  return { ...d, packs: d.packs.map((x, n) => (n === pack ? { ...pk, width: w, height: h, cells } : x)) };
}

/** Pack `pack`'s record with `change` applied (the map cell, the facing, the visible spawn). */
export function setPack(d: PackEdit, pack: number, change: Partial<Pick<PackDraft, "x" | "y" | "facing" | "visible">>): PackEdit {
  const pk = d.packs[pack];
  return pk ? { ...d, packs: d.packs.map((x, n) => (n === pack ? { ...pk, ...change } : x)) } : d;
}

/** Pack `pack`'s arena cells at the map's stride (32) for the grid, the rest walls. */
export function arenaGrid(pk: PackDraft): Uint8Array {
  const out = new Uint8Array(MAP_MAX * MAP_MAX).fill(cellByte(Tile.Wall));
  for (let y = 0; y < pk.height; y++) for (let x = 0; x < pk.width; x++) out[y * MAP_MAX + x] = pk.cells[y * pk.width + x]!;
  return out;
}

/** No pack yet: `PACKS_MAX` empty numbers. */
export const noPacks = (): (PackDraft | null)[] => new Array(PACKS_MAX).fill(null);
