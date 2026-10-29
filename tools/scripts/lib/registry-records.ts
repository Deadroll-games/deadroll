// JSON -> registry table bytes, plus validators mirroring programs/registry/src/state/records.rs.
import { createHash } from "node:crypto";
import { readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";

export const ASPECT_SIZE = 8;
export const RESONANCE_SIZE = 16;
export const TRAIT_SIZE = 32;
export const TRIPLE_SIZE = 16;
/** Ordered triples of three different aspects: 6 x 5 x 4. */
export const TRIPLES = 120;
export const OP_SIZE = 6;
/** An op in a triple record: kind, arg, value u16. */
export const TRIPLE_OP_SIZE = 4;
export const TRIPLE_OPS = 3;
export const TRIPLE_SET = 1;
/** Op `arg`: the base's native state (or, for a Multiplier, no state required). */
export const NATIVE = 0xff;
export const ANY_STATE = 0xff;
export const NO_STATE = 0xff;
export const CRIT_ONLY = 0x80;
export const PAIR_OVERRIDE = 1;
export const MONSTER_SIZE = 48;
export const TILE_SIZE = 8;
export const BESTIARY_MAX = 32;
/** Patch 6: monster skill records (`deadroll_skillcheck::monster`): 128 x 32 B. */
export const MONSTER_SKILL_SIZE = 32;
export const MONSTER_SKILLS_MAX = 128;
/** Most skills in one bestiary pool (`Spawn.skills` is a u16 mask). */
export const POOL_MAX = 16;
/** Monster skill flag: an ally-target skill reaches allies of its own kin only. */
export const KIN_ONLY = 1;
/** Bestiary byte "none" (predisposition, kin). */
export const NO_KIN = 0xff;
export const TABLE_CAP = 4096;
export const NO_ASPECT = 0xff;
/** Patch 11: reward table records (`deadroll_rules::loot::record`). */
export const REWARD_SIZE = 108;
export const REWARDS_MAX = 32;
export const REWARD_LEVELS = 4;
export const REWARD_ENTRIES = 6;
const REWARD_LEVELS_AT = 8;
const REWARD_LEVEL_SIZE = 1 + REWARD_ENTRIES * 4;
/** Bestiary byte 44 "no body loot" and a map without a boss table. */
export const NO_TABLE = 0xff;
export const REWARD_KIND: Record<string, number> = { Resource: 0, Recipe: 1, Consumable: 2, Blank: 3, Reagent: 4 };

export const TableKind = {
  Aspects: 0,
  Resonances: 1,
  Bestiary: 2,
  Tileset: 3,
  Prefabs: 4,
  Traits: 5,
  MonsterSkills: 6,
  Rewards: 7,
  Triples: 8,
} as const;
export type TableKind = (typeof TableKind)[keyof typeof TableKind];

const RESONANCE_CONDITIONS = 10;
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
/** Defense tags (patch 9, deadroll_combat source_bit / form_bit): the sources, then the forms. */
export const TAG: Record<string, number> = { Magical: 1 << 0, Combat: 1 << 1, Strike: 1 << 2, Projectile: 1 << 3 };
const TAG_BITS = 0b1111;
export const STATE_KIND: Record<string, number> = {
  Burning: 0,
  Chill: 1,
  Freeze: 2,
  Bleeding: 3,
  Shock: 4,
  Stun: 5,
  Penetration: 6,
};
/** Patch 13: tile types (`deadroll_types::TileKind`), the record index of the tileset. */
export const TILE: Record<string, number> = {
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
};
/** Tile types a cell byte can name (`deadroll_types::TILE_KINDS_MAX`): the tileset's capacity. */
export const TILE_KINDS_MAX = 32;
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
  IsBleeding: 9,
};
/** `deadroll_combat::tables::OpKind`, in discriminant order. */
export const OP_KIND: Record<string, number> = {
  None: 0,
  Multiplier: 1,
  StateOnCrit: 2,
  ExtraStateChance: 3,
  Linked: 4,
  LinkedChance: 5,
  LinkedMod: 6,
  NativeTurns: 7,
  NativePower: 8,
  NativeFaster: 9,
  NativePowerArmor: 10,
  ThawDamage: 11,
  RemoveState: 12,
  Jump: 13,
  JumpChain: 14,
  JumpLine: 15,
  Knockback: 16,
  IgniteCells: 17,
  AddPacket: 18,
  Penetrate: 19,
  AccuracyBuff: 20,
  ForceCrit: 21,
  ApplyState: 22,
  ApplyMod: 23,
  StateIfHad: 24,
  Splash: 25,
  Nova: 26,
  ModAfterNative: 27,
  StateFaster: 28,
  StateSlower: 29,
  TickNow: 30,
  Escalate: 31,
  TickAspect: 32,
  RemoveMod: 33,
  NativeShorter: 34,
  Delayed: 35,
  OnMiss: 36,
  ThawSplash: 37,
  SplashLine: 38,
  ChainRadius: 39,
  ModAfterState: 40,
};
const OP_KINDS = Object.keys(OP_KIND).length;
/** What an op's `arg` names (`deadroll_combat::tables::ArgKind`). */
type ArgKind = "Zero" | "State" | "StateOnly" | "StateOrAny" | "Mod" | "Aspect" | "Dot";
export function opArg(kind: number): ArgKind {
  switch (kind) {
    case OP_KIND.Multiplier:
    case OP_KIND.Splash:
      return "StateOrAny";
    case OP_KIND.StateOnCrit:
    case OP_KIND.ExtraStateChance:
    case OP_KIND.Linked:
    case OP_KIND.LinkedChance:
    case OP_KIND.RemoveState:
    case OP_KIND.Jump:
    case OP_KIND.JumpLine:
    case OP_KIND.ApplyState:
    case OP_KIND.StateIfHad:
      return "State";
    case OP_KIND.JumpChain:
    case OP_KIND.StateSlower:
    case OP_KIND.OnMiss:
    case OP_KIND.ChainRadius:
      return "StateOnly";
    case OP_KIND.StateFaster:
    case OP_KIND.TickNow:
    case OP_KIND.Escalate:
    case OP_KIND.Delayed:
      return "Dot";
    case OP_KIND.LinkedMod:
    case OP_KIND.ApplyMod:
    case OP_KIND.Nova:
    case OP_KIND.ModAfterNative:
    case OP_KIND.RemoveMod:
    case OP_KIND.ModAfterState:
      return "Mod";
    case OP_KIND.AddPacket:
    case OP_KIND.AccuracyBuff:
    case OP_KIND.TickAspect:
      return "Aspect";
    default:
      return "Zero";
  }
}
export const MOD_KIND: Record<string, number> = {
  Haste: 0,
  Slow: 1,
  Empowerment: 2,
  Weakening: 3,
  Protection: 4,
  Vulnerability: 5,
  // 6 = Resistance: a resonance has no aspect weights to give one.
  Exposed: 7,
  Root: 8,
  Blinded: 9,
  OffGuard: 10,
  Sundered: 11,
  Dazed: 12,
  Silenced: 13,
  NoHeal: 14,
  Dulled: 15,
};
/** Tint traits, by trait id (docs/DAMAGE_TYPES_AND_MIXES.md 3.2). */
export const TINT_TRAIT: Record<string, number> = {
  Fury: 0,
  Composure: 1,
  Swiftness: 2,
  Wasting: 3,
  Crushing: 4,
  Keenness: 5,
};
export const TRAIT: Record<string, number> = {
  swarm: 1 << 0,
  regeneration: 1 << 1,
  leader: 1 << 2,
  hunt: 1 << 3,
};
/** Kin names by kin id (crates/rules/src/registry.rs `KINS`). */
export const KIN: Record<string, number> = { Wolf: 0, Goblin: 1, Bear: 2, Treant: 3, Fairy: 4, Troll: 5 };
/** Skill block enums (crates/skillcheck/src/blocks.rs, append-only discriminants). */
export const CATEGORY: Record<string, number> = { Active: 0, Reactive: 1, Passive: 2 };
export const PURPOSE: Record<string, number> = { Damage: 0, Buff: 1, Debuff: 2 };
export const TARGET: Record<string, number> = { Single: 0, Line: 1 };
export const EFFECT: Record<string, number> = {
  None: 0,
  Weakening: 1,
  Slow: 2,
  Vulnerability: 3,
  Stun: 4,
  Freeze: 5,
  Haste: 6,
  Empowerment: 7,
  Protection: 8,
  Heal: 9,
  Resistance: 10,
};
export const TRIGGER: Record<string, number> = { None: 0, Hit: 1, CritHit: 2, HpBelow: 3, AllyHpBelow: 4, Controlled: 5, EnemyAdjacent: 6 };
export const RANGE: Record<string, number> = { Melee: 0, Ranged: 1 };
export const TIMING: Record<string, number> = { Instant: 0, DoT: 1 };

export const TABLES = [
  { table: "aspects", kind: TableKind.Aspects, seed: "aspects", file: "aspects.json", recordSize: ASPECT_SIZE },
  { table: "resonances", kind: TableKind.Resonances, seed: "resonances", file: "resonances.json", recordSize: RESONANCE_SIZE },
  { table: "bestiary", kind: TableKind.Bestiary, seed: "bestiary", file: "bestiary.json", recordSize: MONSTER_SIZE },
  { table: "tileset", kind: TableKind.Tileset, seed: "tileset", file: "tileset.json", recordSize: TILE_SIZE },
  { table: "prefabs", kind: TableKind.Prefabs, seed: "prefabs", file: "prefabs.json", recordSize: 1 },
  { table: "traits", kind: TableKind.Traits, seed: "traits", file: "traits.json", recordSize: TRAIT_SIZE },
  { table: "monster_skills", kind: TableKind.MonsterSkills, seed: "monster_skills", file: "monster_skills.json", recordSize: MONSTER_SKILL_SIZE },
  { table: "rewards", kind: TableKind.Rewards, seed: "rewards", file: "reward_tables.json", recordSize: REWARD_SIZE },
  { table: "triples", kind: TableKind.Triples, seed: "triples", file: "triples.json", recordSize: TRIPLE_SIZE },
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
    case TableKind.Traits:
      return TRAIT_SIZE;
    case TableKind.MonsterSkills:
      return MONSTER_SKILL_SIZE;
    case TableKind.Rewards:
      return REWARD_SIZE;
    case TableKind.Triples:
      return TRIPLE_SIZE;
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
      return TILE_KINDS_MAX * TILE_SIZE;
    case TableKind.Prefabs:
      return TABLE_CAP;
    case TableKind.Traits:
      return Object.keys(TINT_TRAIT).length * TRAIT_SIZE;
    case TableKind.MonsterSkills:
      return MONSTER_SKILLS_MAX * MONSTER_SKILL_SIZE;
    case TableKind.Rewards:
      return REWARDS_MAX * REWARD_SIZE;
    case TableKind.Triples:
      return TRIPLES * TRIPLE_SIZE;
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
  return Number.isInteger(v) && v >= 0 && v < Object.keys(STATE_KIND).length;
}

function u16At(r: Uint8Array, at: number): number {
  return r[at] | (r[at + 1] << 8);
}

/** Two numbers in one value (`deadroll_combat::tables::value_ok`). */
function valueOk(kind: number, value: number): boolean {
  switch (kind) {
    case OP_KIND.Delayed:
    case OP_KIND.ModAfterState:
      return isStateKind(value & 0xff) && value >>> 16 === 0;
    case OP_KIND.ChainRadius: {
      const filter = (value >>> 8) & 0xff;
      return value >>> 16 === 0 && (value & 0xff) > 0 && (isStateKind(filter) || filter === ANY_STATE);
    }
    default:
      return true;
  }
}

/** Op: [kind | CRIT_ONLY, arg, value u32 LE] (`deadroll_combat::tables::Op::decode`). */
export function validate_op(e: Uint8Array): boolean {
  if (e.length < OP_SIZE) return false;
  const kind = e[0] & ~CRIT_ONLY;
  if (kind >= OP_KINDS) return false;
  if (kind === OP_KIND.None) return e.subarray(0, OP_SIZE).every((b) => b === 0);
  const arg = e[1];
  const value = (e[2] | (e[3] << 8) | (e[4] << 16) | (e[5] << 24)) >>> 0;
  if (!valueOk(kind, value)) return false;
  switch (opArg(kind)) {
    case "Dot":
      return arg === STATE_KIND.Burning || arg === STATE_KIND.Bleeding;
    case "Zero":
      return arg === 0;
    case "State":
      return arg === NATIVE || isStateKind(arg);
    case "StateOnly":
      return isStateKind(arg);
    case "StateOrAny":
      return arg === ANY_STATE || isStateKind(arg);
    case "Mod":
      return Object.values(MOD_KIND).includes(arg);
    case "Aspect":
      return isAspect(arg);
  }
}

/** A triple record: all zero, or [0, TRIPLE_SET, 3 ops of 4 bytes, 0, 0]. */
export function validate_triple(r: Uint8Array): boolean {
  if (r.length !== TRIPLE_SIZE) return false;
  if (r.every((b) => b === 0)) return true;
  if (r[0] !== 0 || r[1] !== TRIPLE_SET || r[14] !== 0 || r[15] !== 0) return false;
  for (let i = 0; i < TRIPLE_OPS; i++) {
    const at = 2 + i * TRIPLE_OP_SIZE;
    const wide = new Uint8Array([r[at], r[at + 1], r[at + 2], r[at + 3], 0, 0]);
    if (!validate_op(wide)) return false;
  }
  return true;
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
    (r[38] & ~((1 << Object.keys(STATE_KIND).length) - 1)) === 0 &&
    // Patch 6: [39] pool start, [40] pool length, [41] kin, then reserved.
    r[40] <= POOL_MAX &&
    r[39] + r[40] <= MONSTER_SKILLS_MAX &&
    // Patch 9: [42] resisted tags, [43] immune tags (sources, then forms).
    (r[42] & ~TAG_BITS) === 0 &&
    (r[43] & ~TAG_BITS) === 0 &&
    // Patch 11: [44] the body loot table (a reward table id or NO_TABLE), then reserved.
    (r[44] === NO_TABLE || r[44] < REWARDS_MAX) &&
    r.subarray(45).every((b) => b === 0) &&
    u16At(r, 30) <= u16At(r, 32)
  );
}

/**
 * A monster skill record's layout: present, every block a known variant, a known flag, reserved
 * zero. The builder rules themselves (`deadroll_skillcheck::validate_monster_skill`) are the
 * program's `write_table` check; the shared fixture pins that both encoders agree.
 */
export function validate_monster_skill(r: Uint8Array): boolean {
  if (r.length !== MONSTER_SKILL_SIZE) return false;
  if (r.every((b) => b === 0)) return true;
  const n = (o: Record<string, number>) => Object.keys(o).length;
  return (
    r[0] === 1 &&
    r[1] < n(CATEGORY) &&
    r[2] < n(PURPOSE) &&
    isSource(r[3]) &&
    isForm(r[4]) &&
    r[5] < n(TARGET) &&
    isAspect(r[6]) &&
    isAspect(r[7]) &&
    isAspect(r[8]) &&
    r[9] >= 1 &&
    r[9] <= 3 &&
    r[10] < n(EFFECT) &&
    r[11] < n(TRIGGER) &&
    r[13] < n(RANGE) &&
    r[14] < n(TIMING) &&
    (r[16] & ~KIN_ONLY) === 0 &&
    r.subarray(17).every((b) => b === 0)
  );
}

/** Checks one record of a typed table. `index` is the record's position in the table. */
/**
 * Patch 11: a reward table record (`deadroll_rules::loot::LootTable::from_record`): all zero, or
 * present with levels base..max (at most 4), 1-6 entries on each used level, known kinds, qty >= 1,
 * every unused byte zero.
 */
export function validate_reward(r: Uint8Array): boolean {
  if (r.length !== REWARD_SIZE) return false;
  if (r.every((b) => b === 0)) return true;
  const [present, base, max] = [r[0], r[1], r[2]];
  if (present !== 1 || r[3] !== 0 || r[6] !== 0 || r[7] !== 0) return false;
  if (base === 0 || max < base || max - base + 1 > REWARD_LEVELS) return false;
  const span = max - base + 1;
  const kinds = Object.keys(REWARD_KIND).length;
  for (let i = 0; i < REWARD_LEVELS; i++) {
    const at = REWARD_LEVELS_AT + i * REWARD_LEVEL_SIZE;
    const count = r[at];
    if (i < span ? count < 1 || count > REWARD_ENTRIES : count !== 0) return false;
    for (let j = 0; j < REWARD_ENTRIES; j++) {
      const e = r.subarray(at + 1 + j * 4, at + 5 + j * 4);
      if (j >= count) {
        if (e.some((b) => b !== 0)) return false;
      } else if (e[0] >= kinds || e[3] === 0) {
        return false;
      }
    }
  }
  return true;
}

export function validate_record(kind: TableKind, index: number, r: Uint8Array): boolean {
  if (r.length !== recordSize(kind)) return false;
  switch (kind) {
    case TableKind.Aspects:
      // [aspect, family, flags, native state, trait, condition, 0, 0]
      return (
        r[0] === index &&
        isAspect(r[0]) &&
        isFamily(r[1]) &&
        (r[3] === NO_STATE || isStateKind(r[3])) &&
        r[4] < Object.keys(TINT_TRAIT).length &&
        r[5] < RESONANCE_CONDITIONS &&
        r[6] === 0 &&
        r[7] === 0
      );
    case TableKind.Resonances:
      // [condition, flags, op 0, op 1, 0, 0]
      return (
        r[0] < RESONANCE_CONDITIONS &&
        (r[1] & ~PAIR_OVERRIDE) === 0 &&
        validate_op(r.subarray(2, 8)) &&
        validate_op(r.subarray(8, 14)) &&
        r[14] === 0 &&
        r[15] === 0
      );
    case TableKind.MonsterSkills:
      return validate_monster_skill(r);
    case TableKind.Traits:
      // An all-zero record is an absent trait; else [1, 0, hit op, template op x2, 12 x 0].
      if (r.every((b) => b === 0)) return true;
      return (
        r[0] === 1 &&
        r[1] === 0 &&
        validate_op(r.subarray(2, 8)) &&
        validate_op(r.subarray(8, 14)) &&
        validate_op(r.subarray(14, 20)) &&
        r.subarray(20).every((b) => b === 0)
      );
    case TableKind.Bestiary:
      return validate_monster(r);
    case TableKind.Tileset:
      return r[0] <= 1 && r[2] <= 1 && (r[0] === 0 || r[1] >= 1);
    case TableKind.Prefabs:
      return true;
    case TableKind.Rewards:
      return validate_reward(r);
    case TableKind.Triples:
      return validate_triple(r);
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

/** One op: `{ kind, arg?, value?, crit? }` (arg by name: a state, "Native", "Any", a modifier, an aspect). */
function encodeOp(e: unknown, out: Uint8Array, at: number): void {
  const obj = asObject(e, "op");
  const kind = lookup(OP_KIND, obj.kind, "op kind");
  out[at] = kind | (obj.crit === true ? CRIT_ONLY : 0);
  const need = () => {
    if (obj.arg === undefined) fail(`${String(obj.kind)} needs an arg`);
    return obj.arg;
  };
  switch (opArg(kind)) {
    case "Dot":
      out[at + 1] = lookup(STATE_KIND, need(), "state");
      if (out[at + 1] !== STATE_KIND.Burning && out[at + 1] !== STATE_KIND.Bleeding) {
        fail(`${String(obj.kind)} takes Burning or Bleeding`);
      }
      break;
    case "Zero":
      if (obj.arg !== undefined) fail(`${String(obj.kind)} takes no arg`);
      break;
    case "State":
      out[at + 1] = need() === "Native" ? NATIVE : lookup(STATE_KIND, obj.arg, "state");
      break;
    case "StateOnly":
      out[at + 1] = lookup(STATE_KIND, need(), "state");
      break;
    case "StateOrAny":
      out[at + 1] = need() === "Any" ? ANY_STATE : lookup(STATE_KIND, obj.arg, "state");
      break;
    case "Mod":
      out[at + 1] = lookup(MOD_KIND, need(), "modifier");
      break;
    case "Aspect":
      out[at + 1] = lookup(ASPECT, need(), "aspect");
      break;
  }
  putU32(out, at + 2, opValue(kind, obj));
}

/**
 * The 32-bit value of an op. Delayed and ModAfterState: the state it waits for or follows (`after`)
 * in the low byte, the strength in percent (`percent`, 0 = 100) from bit 8. ChainRadius: the radius
 * in the low byte, the state a target must have (`filter`, or "Any") in bits 8..16.
 */
function opValue(kind: number, obj: Record<string, unknown>): number {
  if (kind === OP_KIND.Delayed || kind === OP_KIND.ModAfterState) {
    const after = lookup(STATE_KIND, obj.after, "state");
    const percent = obj.percent === undefined ? 0 : requireInt(obj.percent, "percent", 0, 255);
    return after | (percent << 8);
  }
  if (kind === OP_KIND.ChainRadius) {
    const radius = requireInt(obj.radius, "radius", 1, 255);
    const filter = obj.filter === "Any" ? ANY_STATE : lookup(STATE_KIND, obj.filter, "state");
    return radius | (filter << 8);
  }
  return obj.value === undefined ? 0 : requireInt(obj.value, "op value", 0, 0xffffffff);
}

function encodeOps(list: unknown, max: number, out: Uint8Array, at: number, label: string): void {
  if (!Array.isArray(list) || list.length > max) fail(`${label}: at most ${max} ops`);
  list.forEach((op, i) => encodeOp(op, out, at + i * OP_SIZE));
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
    out[at + 3] = rec.native === null ? NO_STATE : lookup(STATE_KIND, rec.native, "state");
    out[at + 4] = lookup(TINT_TRAIT, rec.trait, "trait");
    out[at + 5] = lookup(CONDITION, rec.condition, "condition");
  }
  return out;
}

export function encodeTraits(json: unknown): Uint8Array {
  const records = recordsOf(json, "traits");
  const names = Object.keys(TINT_TRAIT);
  if (records.length !== names.length) fail(`traits must have ${names.length} records`);
  const out = new Uint8Array(names.length * TRAIT_SIZE);
  for (let i = 0; i < records.length; i++) {
    const rec = asObject(records[i], "trait");
    if (lookup(TINT_TRAIT, rec.trait, "trait") !== i) fail(`trait record ${i} must be ${names[i]}`);
    lookup(ASPECT, rec.aspect, "aspect");
    const at = i * TRAIT_SIZE;
    out[at] = 1;
    if (rec.hit !== null && rec.hit !== undefined) encodeOp(rec.hit, out, at + 2);
    encodeOps(rec.template, 2, out, at + 8, `trait ${names[i]}`);
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
    if (typeof rec.override !== "boolean") fail("resonance override must be true or false");
    out[at + 1] = rec.override ? PAIR_OVERRIDE : 0;
    encodeOps(rec.ops, 2, out, at + 2, `resonance ${String(rec.base)} + ${String(rec.tint)}`);
  }
  return out;
}

/** `deadroll_combat::tables::triple_of`: the aspects of triple `index`. */
export function tripleOf(index: number): [number, number, number] {
  const s = index % 4;
  const t = Math.floor(index / 4) % 5;
  const b = Math.floor(index / 20);
  const tint = t + (t >= b ? 1 : 0);
  let second = s;
  for (const taken of [Math.min(b, tint), Math.max(b, tint)]) {
    if (second >= taken) second++;
  }
  return [b, tint, second];
}

/** `triples.json` as the `triples` table body: 120 records, table order, 16 bytes each. */
export function encodeTriples(json: unknown): Uint8Array {
  const records = recordsOf(json, "triples");
  if (records.length !== TRIPLES) fail(`triples must have ${TRIPLES} records`);
  const out = new Uint8Array(TRIPLES * TRIPLE_SIZE);
  for (let i = 0; i < records.length; i++) {
    const rec = asObject(records[i], "triple");
    const got = [lookup(ASPECT, rec.base, "aspect"), lookup(ASPECT, rec.tint, "aspect"), lookup(ASPECT, rec.second, "aspect")];
    if (tripleOf(i).some((a, k) => a !== got[k])) fail(`triple record ${i} out of order`);
    if (!Array.isArray(rec.ops) || rec.ops.length > TRIPLE_OPS) fail(`triple ${i}: at most ${TRIPLE_OPS} ops`);
    if (rec.ops.length === 0) continue;
    const at = i * TRIPLE_SIZE;
    out[at + 1] = TRIPLE_SET;
    rec.ops.forEach((op: unknown, k: number) => {
      const wide = new Uint8Array(OP_SIZE);
      encodeOp(op, wide, 0);
      const value = (wide[2] | (wide[3] << 8) | (wide[4] << 16) | (wide[5] << 24)) >>> 0;
      if (value > 0xffff) fail(`triple ${i}: an op value does not fit 16 bits`);
      const to = at + 2 + k * TRIPLE_OP_SIZE;
      out[to] = wide[0];
      out[to + 1] = wide[1];
      putU16(out, to + 2, value);
    });
  }
  return out;
}

function tagMask(names: unknown, label: string): number {
  if (!Array.isArray(names)) fail(`${label} must be an array of tag names`);
  return names.reduce((m: number, n: unknown) => m | lookup(TAG, n, "tag"), 0);
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
  const states = rec.state_immunity ?? [];
  if (!Array.isArray(states)) fail("state_immunity must be an array of state names");
  out[38] = states.reduce((m: number, n: unknown) => m | (1 << lookup(STATE_KIND, n, "state")), 0);
  out[42] = tagMask(rec.tag_resistance ?? [], "tag_resistance");
  out[43] = tagMask(rec.tag_immunity ?? [], "tag_immunity");
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
  // Patch 6: the skill pool (a run of monster_skills.json) and the kin.
  out[39] = rec.pool_start === undefined ? 0 : requireInt(rec.pool_start, "pool_start", 0, MONSTER_SKILLS_MAX - 1);
  out[40] = rec.pool_len === undefined ? 0 : requireInt(rec.pool_len, "pool_len", 0, POOL_MAX);
  out[41] = rec.kin === undefined || rec.kin === null ? NO_KIN : lookup(KIN, rec.kin, "kin");
  // Patch 11: the body loot table.
  out[44] = rec.loot_table === undefined || rec.loot_table === null ? NO_TABLE : requireInt(rec.loot_table, "loot_table", 0, REWARDS_MAX - 1);
  return out;
}

/** One `monster_skills.json` record as its 32-byte registry record. */
export function encodeMonsterSkill(json: unknown): Uint8Array {
  const rec = asObject(json, "monster skill");
  const out = new Uint8Array(MONSTER_SKILL_SIZE);
  out[0] = 1;
  out[1] = lookup(CATEGORY, rec.category, "category");
  out[2] = lookup(PURPOSE, rec.purpose, "purpose");
  out[3] = lookup(SOURCE, rec.source, "source");
  out[4] = lookup(FORM, rec.form, "form");
  out[5] = lookup(TARGET, rec.target ?? "Single", "target");
  const aspects = rec.aspects;
  if (!Array.isArray(aspects) || aspects.length < 1 || aspects.length > 3) fail("monster skill needs 1..=3 aspects");
  aspects.forEach((a: unknown, i: number) => (out[6 + i] = lookup(ASPECT, a, "aspect")));
  out[9] = aspects.length;
  out[10] = lookup(EFFECT, rec.effect ?? "None", "effect");
  out[11] = lookup(TRIGGER, rec.trigger ?? "None", "trigger");
  out[12] = rec.aura_radius === undefined ? 0 : requireInt(rec.aura_radius, "aura_radius", 0, 255);
  out[13] = lookup(RANGE, rec.range, "range");
  out[14] = lookup(TIMING, rec.timing ?? "Instant", "timing");
  out[15] = rec.hp_below_pct === undefined ? 0 : requireInt(rec.hp_below_pct, "hp_below_pct", 0, 100);
  if (rec.kin_only !== undefined && typeof rec.kin_only !== "boolean") fail("kin_only must be true or false");
  out[16] = rec.kin_only ? KIN_ONLY : 0;
  return out;
}

export function encodeMonsterSkills(json: unknown): Uint8Array {
  const records = recordsOf(json, "monster skills");
  if (records.length > MONSTER_SKILLS_MAX) fail(`at most ${MONSTER_SKILLS_MAX} monster skills`);
  const out = new Uint8Array(records.length * MONSTER_SKILL_SIZE);
  for (let i = 0; i < records.length; i++) {
    const rec = asObject(records[i], "monster skill");
    const id = requireInt(rec.id, "id", 0, MONSTER_SKILLS_MAX - 1);
    if (id !== i) fail(`monster skill ${i} must have id ${i}`);
    out.set(encodeMonsterSkill(rec), i * MONSTER_SKILL_SIZE);
  }
  return out;
}

/** Patch 11: `reward_tables.json` (`deadroll_rules::registry::encode_rewards`). */
export function encodeRewards(json: unknown): Uint8Array {
  const records = recordsOf(json, "reward tables");
  if (records.length > REWARDS_MAX) fail(`at most ${REWARDS_MAX} reward tables`);
  const out = new Uint8Array(records.length * REWARD_SIZE);
  for (let i = 0; i < records.length; i++) {
    const rec = asObject(records[i], "reward table");
    const id = requireInt(rec.id, "id", 0, REWARDS_MAX - 1);
    if (id !== i) fail(`reward table ${i} must have id ${i}`);
    const r = new Uint8Array(REWARD_SIZE);
    const base = requireInt(rec.level_base, "level_base", 1, 255);
    const max = requireInt(rec.level_max, "level_max", base, 255);
    if (!Array.isArray(rec.levels) || rec.levels.length !== max - base + 1 || rec.levels.length > REWARD_LEVELS) {
      fail(`reward table ${i}: one list per level from base to max, at most ${REWARD_LEVELS}`);
    }
    r[0] = 1;
    r[1] = base;
    r[2] = max;
    putU16(r, 4, requireInt(rec.cost, "cost", 0, 0xffff));
    rec.levels.forEach((entries: unknown, l: number) => {
      if (!Array.isArray(entries) || entries.length > REWARD_ENTRIES) fail(`reward table ${i}: at most ${REWARD_ENTRIES} entries per level`);
      const at = REWARD_LEVELS_AT + l * REWARD_LEVEL_SIZE;
      r[at] = entries.length;
      entries.forEach((e: unknown, j: number) => {
        const entry = asObject(e, "reward entry");
        const p = at + 1 + j * 4;
        r[p] = lookup(REWARD_KIND, entry.kind, "reward kind");
        putU16(r, p + 1, requireInt(entry.id, "id", 0, 0xffff));
        r[p + 3] = requireInt(entry.qty, "qty", 1, 255);
      });
    });
    if (!validate_reward(r)) fail(`reward table ${i}: invalid`);
    out.set(r, i * REWARD_SIZE);
  }
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
    case "traits":
      return encodeTraits(json);
    case "monster_skills":
      return encodeMonsterSkills(json);
    case "rewards":
      return encodeRewards(json);
    case "triples":
      return encodeTriples(json);
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
