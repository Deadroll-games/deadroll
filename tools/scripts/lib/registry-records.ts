// JSON -> registry table bytes, plus validators mirroring programs/registry/src/state/records.rs.
import { createHash } from "node:crypto";
import { readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";

export const ASPECT_SIZE = 8;
export const RESONANCE_SIZE = 16;
export const MONSTER_SIZE = 48;
export const TILE_SIZE = 8;
export const BESTIARY_MAX = 32;
export const TABLE_CAP = 4096;
export const NO_ASPECT = 0xff;

export const TableKind = {
  Aspects: 0,
  Resonances: 1,
  Bestiary: 2,
  Tileset: 3,
  Prefabs: 4,
} as const;
export type TableKind = (typeof TableKind)[keyof typeof TableKind];

const RESONANCE_CONDITIONS = 9;
const ROLES = 6;
const BIOMES = 2;
const ASPECT_COUNT = 6;

export const ASPECT: Record<string, number> = {
  Fire: 0,
  Cold: 1,
  Lightning: 2,
  Blood: 3,
  Blunt: 4,
  Piercing: 5,
};
export const FAMILY: Record<string, number> = {
  Elemental: 0,
  Physical: 1,
  Mental: 2,
  Sacral: 3,
};
export const SOURCE: Record<string, number> = { Magical: 0, Combat: 1 };
export const FORM: Record<string, number> = { Strike: 0, Projectile: 1 };
export const STATE_KIND: Record<string, number> = {
  Burning: 0,
  Chill: 1,
  Freeze: 2,
  Bleeding: 3,
  Shock: 4,
  Stun: 5,
};
export const TILE: Record<string, number> = {
  Floor: 0,
  Wall: 1,
  Water: 2,
  Door: 3,
  Entrance: 4,
  Exit: 5,
  Trap: 6,
};
export const ROLE: Record<string, number> = {
  Swarm: 0,
  Fast: 1,
  Tank: 2,
  Loner: 3,
  Boss: 4,
  Helper: 5,
};
export const BIOME: Record<string, number> = {
  Forest: 0,
  "Caves and Ruins": 1,
};
export const CONDITION: Record<string, number> = {
  Always: 0,
  CanBurn: 1,
  CanBeChilled: 2,
  CanBeFrozen: 3,
  IsFrozen: 4,
  CanBeShocked: 5,
  CanBleed: 6,
  CanBeStunned: 7,
  HasArmor: 8,
};
export const EFFECT_KIND: Record<string, number> = {
  None: 0,
  Multiplier: 1,
  StateOnCrit: 2,
  ExtraStateChance: 3,
};
export const TRAIT: Record<string, number> = {
  swarm: 1 << 0,
  regeneration: 1 << 1,
  leader: 1 << 2,
  hunt: 1 << 3,
};

export const TABLES = [
  { table: "aspects", kind: TableKind.Aspects, seed: "aspects", file: "aspects.json", recordSize: ASPECT_SIZE },
  { table: "resonances", kind: TableKind.Resonances, seed: "resonances", file: "resonances.json", recordSize: RESONANCE_SIZE },
  { table: "bestiary", kind: TableKind.Bestiary, seed: "bestiary", file: "bestiary.json", recordSize: MONSTER_SIZE },
  { table: "tileset", kind: TableKind.Tileset, seed: "tileset", file: "tileset.json", recordSize: TILE_SIZE },
  { table: "prefabs", kind: TableKind.Prefabs, seed: "prefabs", file: "prefabs.json", recordSize: 1 },
] as const;

export function registryAsset(name: string): string {
  return fileURLToPath(new URL(`../../../assets/registry/${name}`, import.meta.url));
}

export function recordSize(kind: TableKind): number {
  switch (kind) {
    case TableKind.Aspects:
      return ASPECT_SIZE;
    case TableKind.Resonances:
      return RESONANCE_SIZE;
    case TableKind.Bestiary:
      return MONSTER_SIZE;
    case TableKind.Tileset:
      return TILE_SIZE;
    case TableKind.Prefabs:
      return 1;
  }
}

export function maxLen(kind: TableKind): number {
  switch (kind) {
    case TableKind.Aspects:
      return ASPECT_COUNT * ASPECT_SIZE;
    case TableKind.Resonances:
      return ASPECT_COUNT * ASPECT_COUNT * RESONANCE_SIZE;
    case TableKind.Bestiary:
      return BESTIARY_MAX * MONSTER_SIZE;
    case TableKind.Tileset:
      return Object.keys(TILE).length * TILE_SIZE;
    case TableKind.Prefabs:
      return TABLE_CAP;
  }
}

function isAspect(v: number): boolean {
  return Number.isInteger(v) && v >= 0 && v < ASPECT_COUNT;
}
function isFamily(v: number): boolean {
  return Number.isInteger(v) && v >= 0 && v < 4;
}
function isSource(v: number): boolean {
  return v === 0 || v === 1;
}
function isForm(v: number): boolean {
  return v === 0 || v === 1;
}
function isStateKind(v: number): boolean {
  return Number.isInteger(v) && v >= 0 && v < 6;
}

function u16At(r: Uint8Array, at: number): number {
  return r[at] | (r[at + 1] << 8);
}

/** Effect: [kind, state, value u32 LE]. Kinds 0 None, 1 Multiplier, 2 StateOnCrit, 3 ExtraStateChance. */
export function validate_effect(e: Uint8Array): boolean {
  if (e.length < 6) return false;
  switch (e[0]) {
    case 0:
    case 1:
      return true;
    case 2:
    case 3:
      return isStateKind(e[1]);
    default:
      return false;
  }
}

/** All-zero is an empty slot; otherwise present == 1 and every field is checked. */
export function validate_monster(r: Uint8Array): boolean {
  if (r.length !== MONSTER_SIZE) return false;
  let empty = true;
  for (let i = 0; i < r.length; i++) {
    if (r[i] !== 0) {
      empty = false;
      break;
    }
  }
  if (empty) return true;
  const role = r[1];
  const biome = r[2];
  const size = r[3];
  const minTier = r[4];
  const speed = r[5];
  const predispositionOk = r[9] === NO_ASPECT || isAspect(r[9]);
  const aspectMask = (1 << ASPECT_COUNT) - 1;
  return (
    r[0] === 1 &&
    role < ROLES &&
    biome < BIOMES &&
    (size === 1 || size === 2) &&
    minTier >= 1 &&
    minTier <= 14 &&
    speed >= 1 &&
    isAspect(r[6]) &&
    isSource(r[7]) &&
    isForm(r[8]) &&
    predispositionOk &&
    (r[10] & ~aspectMask) === 0 &&
    (r[11] & ~aspectMask) === 0 &&
    u16At(r, 30) <= u16At(r, 32)
  );
}

/** Checks one record of a typed table. `index` is the record's position in the table. */
export function validate_record(kind: TableKind, index: number, r: Uint8Array): boolean {
  if (r.length !== recordSize(kind)) return false;
  switch (kind) {
    case TableKind.Aspects:
      return r[0] === index && isAspect(r[0]) && isFamily(r[1]);
    case TableKind.Resonances:
      return r[0] < RESONANCE_CONDITIONS && validate_effect(r.subarray(2, 8)) && validate_effect(r.subarray(8, 14));
    case TableKind.Bestiary:
      return validate_monster(r);
    case TableKind.Tileset:
      return r[0] <= 1 && r[2] <= 1 && (r[0] === 0 || r[1] >= 1);
    case TableKind.Prefabs:
      return true;
  }
}

export function validate_write(kind: TableKind, offset: number, bytes: Uint8Array): boolean {
  const size = recordSize(kind);
  for (let i = 0, at = 0; at < bytes.length; i++, at += size) {
    if (!validate_record(kind, offset / size + i, bytes.subarray(at, at + size))) return false;
  }
  return true;
}

export type RangeError = "OutOfBounds" | "Misaligned";

export function check_range(kind: TableKind, offset: number, len: number): number | RangeError {
  const end = offset + len;
  if (!Number.isInteger(offset) || !Number.isInteger(len) || len === 0 || end > maxLen(kind) || end < offset) {
    return "OutOfBounds";
  }
  const size = recordSize(kind);
  if (offset % size !== 0 || len % size !== 0) return "Misaligned";
  return end;
}

function fail(msg: string): never {
  throw new Error(msg);
}

function requireInt(n: unknown, label: string, min: number, max: number): number {
  if (typeof n !== "number" || !Number.isInteger(n) || n < min || n > max) {
    fail(`${label} must be an integer in ${min}..${max}`);
  }
  return n;
}

function lookup(table: Record<string, number>, name: unknown, label: string): number {
  if (typeof name !== "string" || table[name] === undefined) fail(`unknown ${label} ${String(name)}`);
  return table[name];
}

function asObject(v: unknown, label: string): Record<string, unknown> {
  if (typeof v !== "object" || v === null || Array.isArray(v)) fail(`invalid ${label}`);
  return v as Record<string, unknown>;
}

function recordsOf(json: unknown, label: string): unknown[] {
  const obj = asObject(json, label);
  if (!Array.isArray(obj.records)) fail(`${label} records must be an array`);
  return obj.records;
}

function putU16(out: Uint8Array, at: number, n: number): void {
  out[at] = n & 0xff;
  out[at + 1] = (n >>> 8) & 0xff;
}

function putU32(out: Uint8Array, at: number, n: number): void {
  out[at] = n & 0xff;
  out[at + 1] = (n >>> 8) & 0xff;
  out[at + 2] = (n >>> 16) & 0xff;
  out[at + 3] = (n >>> 24) & 0xff;
}

function aspectMask(names: unknown, label: string): number {
  if (!Array.isArray(names)) fail(`${label} must be an array of aspect names`);
  let mask = 0;
  for (const name of names) {
    const bit = lookup(ASPECT, name, label);
    mask |= 1 << bit;
  }
  return mask;
}

function traitBits(names: unknown): number {
  if (!Array.isArray(names)) fail("traits must be an array of names");
  let bits = 0;
  for (const name of names) bits |= lookup(TRAIT, name, "trait");
  return bits;
}

function encodeEffect(e: unknown, out: Uint8Array, at: number): void {
  const obj = asObject(e, "effect");
  const kind = lookup(EFFECT_KIND, obj.kind, "effect kind");
  out[at] = kind;
  if (kind === 0) return;
  if (kind === 1) {
    putU32(out, at + 2, requireInt(obj.value, "multiplier value", 0, 0xffffffff));
    return;
  }
  out[at + 1] = lookup(STATE_KIND, obj.state, "state");
  if (kind === 3) {
    putU32(out, at + 2, requireInt(obj.value, "extra state chance", 0, 0xffffffff));
  }
}

export function encodeAspects(json: unknown): Uint8Array {
  const records = recordsOf(json, "aspects");
  if (records.length !== ASPECT_COUNT) fail(`aspects must have ${ASPECT_COUNT} records`);
  const out = new Uint8Array(ASPECT_COUNT * ASPECT_SIZE);
  for (let i = 0; i < records.length; i++) {
    const rec = asObject(records[i], "aspect");
    const id = lookup(ASPECT, rec.aspect, "aspect");
    if (id !== i) fail(`aspect record ${i} must be ${Object.keys(ASPECT)[i]}`);
    const at = i * ASPECT_SIZE;
    out[at] = id;
    out[at + 1] = lookup(FAMILY, rec.family, "family");
    out[at + 2] = rec.flags === undefined ? 0 : requireInt(rec.flags, "flags", 0, 255);
  }
  return out;
}

export function encodeResonances(json: unknown): Uint8Array {
  const records = recordsOf(json, "resonances");
  if (records.length !== ASPECT_COUNT * ASPECT_COUNT) {
    fail(`resonances must have ${ASPECT_COUNT * ASPECT_COUNT} records`);
  }
  const out = new Uint8Array(ASPECT_COUNT * ASPECT_COUNT * RESONANCE_SIZE);
  for (let i = 0; i < records.length; i++) {
    const rec = asObject(records[i], "resonance");
    const base = lookup(ASPECT, rec.base, "aspect");
    const tint = lookup(ASPECT, rec.tint, "aspect");
    const index = base * ASPECT_COUNT + tint;
    if (index !== i) fail(`resonance record ${i} must be index base * 6 + tint (${index})`);
    const at = i * RESONANCE_SIZE;
    out[at] = lookup(CONDITION, rec.condition, "condition");
    if (!Array.isArray(rec.effects) || rec.effects.length !== 2) fail("resonance needs two effects");
    encodeEffect(rec.effects[0], out, at + 2);
    encodeEffect(rec.effects[1], out, at + 8);
  }
  return out;
}

export function encodeMonster(json: unknown): Uint8Array {
  const rec = asObject(json, "monster");
  const out = new Uint8Array(MONSTER_SIZE);
  out[0] = 1;
  out[1] = lookup(ROLE, rec.role, "role");
  out[2] = lookup(BIOME, rec.biome, "biome");
  out[3] = requireInt(rec.size, "size", 1, 2);
  out[4] = requireInt(rec.min_tier, "min_tier", 1, 14);
  const speed = requireInt(rec.speed, "speed", 0, 255);
  if (speed < 1) fail("speed 0");
  out[5] = speed;
  out[6] = lookup(ASPECT, rec.attack_aspect, "aspect");
  out[7] = lookup(SOURCE, rec.attack_source, "source");
  out[8] = lookup(FORM, rec.attack_form, "form");
  if (rec.predisposition === null || rec.predisposition === undefined || rec.predisposition === "None") {
    out[9] = NO_ASPECT;
  } else {
    out[9] = lookup(ASPECT, rec.predisposition, "aspect");
  }
  out[10] = aspectMask(rec.resistance ?? [], "resistance");
  out[11] = aspectMask(rec.immunity ?? [], "immunity");
  putU16(out, 12, traitBits(rec.traits ?? []));
  putU16(out, 14, requireInt(rec.armor, "armor", 0, 0xffff));
  putU16(out, 16, requireInt(rec.evasion, "evasion", 0, 0xffff));
  putU16(out, 18, requireInt(rec.accuracy, "accuracy", 0, 0xffff));
  putU16(out, 20, requireInt(rec.initiative, "initiative", 0, 0xffff));
  putU32(out, 22, requireInt(rec.hp, "hp", 0, 0xffffffff));
  putU32(out, 26, requireInt(rec.hp_per_level, "hp_per_level", 0, 0xffffffff));
  const dmgMin = requireInt(rec.damage_min, "damage_min", 0, 0xffff);
  const dmgMax = requireInt(rec.damage_max, "damage_max", 0, 0xffff);
  if (dmgMin > dmgMax) fail("damage min above max");
  putU16(out, 30, dmgMin);
  putU16(out, 32, dmgMax);
  putU32(out, 34, requireInt(rec.xp, "xp", 0, 0xffffffff));
  return out;
}

export function encodeBestiary(json: unknown): Uint8Array {
  const records = recordsOf(json, "bestiary");
  const out = new Uint8Array(records.length * MONSTER_SIZE);
  for (let i = 0; i < records.length; i++) {
    const rec = asObject(records[i], "monster");
    const id = requireInt(rec.id, "id", 0, BESTIARY_MAX - 1);
    if (id !== i) fail(`bestiary record ${i} must have id ${i}`);
    out.set(encodeMonster(rec), i * MONSTER_SIZE);
  }
  return out;
}

export function encodeTile(json: unknown): Uint8Array {
  const rec = asObject(json, "tile");
  const out = new Uint8Array(TILE_SIZE);
  const passable = requireInt(rec.passable, "passable", 0, 1);
  const cost = requireInt(rec.cost, "cost", 0, 255);
  if (passable === 1 && cost < 1) fail("passable tile with cost 0");
  out[0] = passable;
  out[1] = cost;
  out[2] = requireInt(rec.blocks_los, "blocks_los", 0, 1);
  out[3] = rec.flags === undefined ? 0 : requireInt(rec.flags, "flags", 0, 255);
  return out;
}

export function encodeTileset(json: unknown): Uint8Array {
  const records = recordsOf(json, "tileset");
  const names = Object.keys(TILE);
  if (records.length !== names.length) fail(`tileset must have ${names.length} records`);
  const out = new Uint8Array(names.length * TILE_SIZE);
  for (let i = 0; i < records.length; i++) {
    const rec = asObject(records[i], "tile");
    const id = lookup(TILE, rec.tile, "tile");
    if (id !== i) fail(`tileset record ${i} must be ${names[i]}`);
    out.set(encodeTile(rec), i * TILE_SIZE);
  }
  return out;
}

export function encodePrefabs(json: unknown): Uint8Array {
  const obj = asObject(json, "prefabs");
  if (!Array.isArray(obj.rooms) || obj.rooms.length !== 0) fail("prefabs rooms must be empty");
  return new Uint8Array(0);
}

export function encodeRegistryFile(table: string, json: unknown): Uint8Array {
  switch (table) {
    case "aspects":
      return encodeAspects(json);
    case "resonances":
      return encodeResonances(json);
    case "bestiary":
      return encodeBestiary(json);
    case "tileset":
      return encodeTileset(json);
    case "prefabs":
      return encodePrefabs(json);
    default:
      fail(`unknown table ${table}`);
  }
}

export type EncodedTable = {
  table: string;
  kind: TableKind;
  seed: string;
  recordSize: number;
  bytes: Uint8Array;
};

export function loadEncodedTables(): EncodedTable[] {
  return TABLES.map((t) => {
    const json = JSON.parse(readFileSync(registryAsset(t.file), "utf8"));
    const bytes = encodeRegistryFile(t.table, json);
    if (bytes.length > 0 && !validate_write(t.kind, 0, bytes)) {
      fail(`${t.table} encoded bytes failed validation`);
    }
    if (bytes.length > maxLen(t.kind)) fail(`${t.table} exceeds max length`);
    return { table: t.table, kind: t.kind, seed: t.seed, recordSize: t.recordSize, bytes };
  });
}

export function sha256Hex(bytes: Uint8Array): string {
  return createHash("sha256").update(bytes).digest("hex");
}

export type SeedPlanRow = {
  table: string;
  kind: number;
  offset: number;
  length: number;
  sha256: string;
};

export function seedPlan(tables: EncodedTable[]): SeedPlanRow[] {
  return tables.map((t) => ({
    table: t.table,
    kind: t.kind,
    offset: 0,
    length: t.bytes.length,
    sha256: sha256Hex(t.bytes),
  }));
}
