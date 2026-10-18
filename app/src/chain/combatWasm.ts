// The combat crate in the browser (crates/combat/wasm, README there): a hero's derived stats and
// the preview of an attack before the dice (docs/COMBAT_MATH.md, section 13). The module builds
// the hit input with crates/rules, the same code the session program runs, so nothing here
// repeats a formula: this file only writes borsh inputs and reads borsh results.
//
// The shipped module is ./wasm/combat.wasm, rebuilt by `bash crates/combat/wasm/build.sh` (CI
// fails when it is stale). __fixtures__/combat-wasm.json pins the byte layouts and the results.
//
// Resonance tables and monster defenses are registry data: the page loads the tables the session
// copied at open_session (`MonsterState.res_*`, `setTables`) and passes each monster's defenses
// from the same account, so no copy of the registry lives in the app.
import { useEffect, useState } from "react";
import wasmUrl from "./wasm/combat.wasm?url";
import { Reader, concat, u16le, u32le } from "./bytes";
import { ASPECT_KEYS, CLASS_KEYS, MOD_KEYS, RACE_KEYS, enumIndex, type ActiveStateAcc, type AuraAcc, type CharacterAcc, type Enum, type MonsterAcc, type SkillRecordAcc } from "./game";
import characterIdl from "./idl/deadroll_character.json";
import type { ActiveState, Cell, TimedMod } from "./types";

/** `ABI_VERSION` of the module this wrapper speaks. */
export const COMBAT_ABI = 5;
/** Bytes of the resonance tables: the registry `aspects`, `traits` and `resonances` bodies. */
export const COMBAT_TABLES_LEN = 48 + 192 + 576;
export const COMBAT_STATS = 4;
export const COMBAT_EQUIP_SLOTS = 11;
export const COMBAT_STATES = 6;
export const COMBAT_MODS = 6;
export const COMBAT_PARTY = 4;
/** Auras per player: the passive one, then the temporary one from a reactive skill. */
export const COMBAT_AURAS = 2;
export const COMBAT_LINE_MAX = 32;
/** Bytes of the impassable-cell bitmap of a line request: a 32 x 32 map, bit `y * 32 + x`. */
export const COMBAT_BLOCKED_LEN = 128;

/** A character as the chain stores it; enums as discriminants. */
export type HeroInput = {
  race: number;
  class: number;
  level: number;
  /** Strength, Dexterity, Intelligence, Endurance. */
  stats: readonly number[];
  predisposition: number;
  predispositionTier: number;
  classTier: number;
  /** `ItemBase` byte by `EquipSlot`, 0xFF = nothing. */
  equipment: ArrayLike<number>;
};

/** A `SkillRecord` without the name: blocks as discriminants in `SkillBlocks` field order. */
export type SkillInput = { blocks: readonly number[]; sharesBp: readonly number[]; powerBp: number };

/** A monster's registry defenses (`MonsterState.defense`): bits by aspect and by state. */
export type DefenseInput = { resist: number; immuneAspects: number; immuneStates: number };

export type TargetInput = {
  bestiaryId: number;
  level: number;
  states: readonly ActiveState[];
  mods: readonly TimedMod[];
  defense: DefenseInput;
};

/** A resonance's accuracy buff on the attacker (`Participation.accuracy`). */
export type AccuracyInput = { aspect: number; turns: number; bonus: number };
export const NO_ACCURACY: AccuracyInput = { aspect: 0, turns: 0, bonus: 0 };

export type MonsterInput = { bestiaryId: number; level: number; defense: DefenseInput };

/** What the rules make of a monster at a level with its defenses. */
export type MonsterSheet = {
  weapon: { aspect: number; min: number; max: number };
  attackBonus: number;
  evasion: number;
  armor: number;
  /** Resistance per aspect, bp, before the cap. */
  resistBp: number[];
  immuneAspects: number;
  immuneStates: number;
  resistCapBp: number;
};

/** A map cell as the module takes it: `[x, y]`. */
export type WasmCell = readonly [number, number];

/** One aura of a player (`AuraData` with enums as discriminants). `turns` 0 = no aura. */
export type AuraInput = {
  kind: number;
  aspect: number;
  powerBp: number;
  radius: number;
  turns: number;
  hostile: boolean;
  passive: boolean;
  selfOnly: boolean;
};
export const NO_AURA: AuraInput = { kind: 0, aspect: 0, powerBp: 0, radius: 0, turns: 0, hostile: false, passive: false, selfOnly: false };

/** The auras of a party with their bearers' cells; `alive` is a bit per player slot. */
export type PartyInput = { auras: readonly (readonly AuraInput[])[]; cells: readonly WasmCell[]; alive: number };
export const NO_PARTY: PartyInput = { auras: [], cells: [], alive: 0 };

export type AttackRequest = {
  hero: HeroInput;
  /** The attacker's own buffs and debuffs; the auras come from `party`. */
  heroMods: readonly TimedMod[];
  /** null = the weapon's basic attack. */
  skill: SkillInput | null;
  target: TargetInput;
  accuracy: AccuracyInput;
  party: PartyInput;
  /** The attacker's player slot. */
  slot: number;
  targetAt: WasmCell;
};

/** A unit's own buffs and debuffs with the auras over it: a player slot, or a monster at `at`. */
export type UnitModsRequest = {
  party: PartyInput;
  mods: readonly TimedMod[];
  monster: boolean;
  slot: number;
  at: WasmCell;
  /** The view of a player's basic attack, which no passive Empowerment strengthens. */
  basicAttack: boolean;
};

/** A line target: from `from` through `through` up to `reach` cells; a wall ends it. */
export type LineRequest = { from: WasmCell; through: WasmCell; reach: number; width: number; height: number; blocked: Uint8Array };

export type HeroSheet = {
  accuracy: number;
  evasion: number;
  armor: number;
  critBp: number;
  hpMax: number;
  actionPoints: number;
  initiative: number;
  martialStrikeBp: number;
  martialProjectileBp: number;
  magicalBp: number;
  martialStateBp: number;
  magicalStateBp: number;
  tempoStateBp: number;
  selfBuffTurns: number;
  /** The chance the crit roll uses on a hit that is not a natural 20, 0 = no crit roll. */
  critRollBp: number;
};

/** One tint candidate of an attack: its chance, and the damage if it triggers alone. */
export type PreviewTint = {
  base: number;
  tint: number;
  carrier: number;
  tier: number;
  pure: boolean;
  chancePct: number;
  /** `base * 6 + tint`. */
  id: number;
  generated: boolean;
  conditionMet: boolean;
  min: number;
  max: number;
};

export type HitPreview = {
  immune: boolean;
  hitBp: number;
  critBp: number;
  min: number;
  max: number;
  critMin: number;
  critMax: number;
  tints: PreviewTint[];
};

// ---------------------------------------------------------------------------------------------
// borsh

const u8 = (n: number) => new Uint8Array([n & 0xff]);
const fixed = (v: ArrayLike<number>, len: number, write: (n: number) => Uint8Array, empty = 0) =>
  concat(...Array.from({ length: len }, (_, i) => write(v[i] ?? empty)));

export function encodeHero(h: HeroInput): Uint8Array {
  return concat(
    u8(h.race),
    u8(h.class),
    u16le(h.level),
    fixed(h.stats, COMBAT_STATS, u16le),
    u8(h.predisposition),
    u8(h.predispositionTier),
    u8(h.classTier),
    fixed(h.equipment, COMBAT_EQUIP_SLOTS, u8, 0xff),
  );
}

const NO_MOD: TimedMod = { kind: 0, aspect: 0, turns: 0, powerBp: 0, source: 0 } as TimedMod;
const NO_STATE: ActiveState = { kind: 0, turns: 0, power: 0, source: 0 } as ActiveState;

function encodeMods(mods: readonly TimedMod[]): Uint8Array {
  return concat(
    ...Array.from({ length: COMBAT_MODS }, (_, i) => {
      const m = mods[i] ?? NO_MOD;
      return concat(u8(m.kind), u8(m.aspect), u8(m.turns), u32le(m.powerBp), u8(m.source));
    }),
  );
}

function encodeStates(states: readonly ActiveState[]): Uint8Array {
  return concat(
    ...Array.from({ length: COMBAT_STATES }, (_, i) => {
      const s = states[i] ?? NO_STATE;
      return concat(u8(s.kind), u8(s.turns), u32le(s.power), u8(s.source));
    }),
  );
}

export function encodeAttack(r: AttackRequest): Uint8Array {
  const skill = r.skill
    ? concat(u8(1), fixed(r.skill.blocks, 12, u8), fixed(r.skill.sharesBp, 3, u32le), u32le(r.skill.powerBp))
    : u8(0);
  return concat(
    encodeHero(r.hero),
    encodeMods(r.heroMods),
    skill,
    u8(r.target.bestiaryId),
    u8(r.target.level),
    encodeStates(r.target.states),
    encodeMods(r.target.mods),
    encodeDefense(r.target.defense),
    u8(r.accuracy.aspect),
    u8(r.accuracy.turns),
    u32le(r.accuracy.bonus),
    encodeParty(r.party),
    u8(r.slot),
    encodeCell(r.targetAt),
  );
}

const encodeCell = (c: WasmCell | undefined) => concat(u8(c?.[0] ?? 0), u8(c?.[1] ?? 0));

function encodeAura(a: AuraInput): Uint8Array {
  return concat(u8(a.kind), u8(a.aspect), u32le(a.powerBp), u8(a.radius), u8(a.turns), u8(a.hostile ? 1 : 0), u8(a.passive ? 1 : 0), u8(a.selfOnly ? 1 : 0));
}

export function encodeParty(p: PartyInput): Uint8Array {
  return concat(
    ...Array.from({ length: COMBAT_PARTY }, (_, i) => concat(...Array.from({ length: COMBAT_AURAS }, (_, j) => encodeAura(p.auras[i]?.[j] ?? NO_AURA)))),
    ...Array.from({ length: COMBAT_PARTY }, (_, i) => encodeCell(p.cells[i])),
    u8(p.alive),
  );
}

export function encodeUnitMods(r: UnitModsRequest): Uint8Array {
  return concat(encodeParty(r.party), encodeMods(r.mods), u8(r.monster ? 1 : 0), u8(r.slot), encodeCell(r.at), u8(r.basicAttack ? 1 : 0));
}

export function encodeLine(r: LineRequest): Uint8Array {
  if (r.blocked.length !== COMBAT_BLOCKED_LEN) throw new Error(`combat wasm: blocked is ${COMBAT_BLOCKED_LEN} bytes`);
  return concat(encodeCell(r.from), encodeCell(r.through), u8(r.reach), u8(r.width), u8(r.height), r.blocked);
}

export function decodeMods(bytes: Uint8Array): TimedMod[] {
  const r = new Reader(bytes);
  return Array.from({ length: COMBAT_MODS }, () => ({ kind: r.u8(), aspect: r.u8(), turns: r.u8(), powerBp: r.u32(), source: r.u8() }) as TimedMod);
}

export function decodeLine(bytes: Uint8Array): WasmCell[] {
  const r = new Reader(bytes);
  const len = r.u8();
  const cells: WasmCell[] = Array.from({ length: COMBAT_LINE_MAX }, () => [r.u8(), r.u8()] as const);
  return cells.slice(0, len);
}

function encodeDefense(d: DefenseInput): Uint8Array {
  return concat(u8(d.resist), u8(d.immuneAspects), u8(d.immuneStates));
}

export function encodeMonster(m: MonsterInput): Uint8Array {
  return concat(u8(m.bestiaryId), u8(m.level), encodeDefense(m.defense));
}

export function decodeMonsterSheet(bytes: Uint8Array): MonsterSheet {
  const r = new Reader(bytes);
  return {
    weapon: { aspect: r.u8(), min: r.u32(), max: r.u32() },
    attackBonus: r.u32(),
    evasion: r.u32(),
    armor: r.u32(),
    resistBp: Array.from({ length: 6 }, () => r.u32()),
    immuneAspects: r.u8(),
    immuneStates: r.u8(),
    resistCapBp: r.u32(),
  };
}

/** The tint candidates of a preview: up to 12 slots, `count` used. */
const PREVIEW_TINTS = 12;

export function decodeSheet(bytes: Uint8Array): HeroSheet {
  const r = new Reader(bytes);
  return {
    accuracy: r.u32(),
    evasion: r.u32(),
    armor: r.u32(),
    critBp: r.u32(),
    hpMax: r.u32(),
    actionPoints: r.u8(),
    initiative: r.u16(),
    martialStrikeBp: r.u32(),
    martialProjectileBp: r.u32(),
    magicalBp: r.u32(),
    martialStateBp: r.u32(),
    magicalStateBp: r.u32(),
    tempoStateBp: r.u32(),
    selfBuffTurns: r.u8(),
    critRollBp: r.u32(),
  };
}

export function decodePreview(bytes: Uint8Array): HitPreview {
  const r = new Reader(bytes);
  const head = {
    immune: r.bool(),
    hitBp: r.u32(),
    critBp: r.u32(),
    min: r.u32(),
    max: r.u32(),
    critMin: r.u32(),
    critMax: r.u32(),
  };
  const all: PreviewTint[] = Array.from({ length: PREVIEW_TINTS }, () => ({
    base: r.u8(),
    tint: r.u8(),
    carrier: r.u8(),
    tier: r.u8(),
    pure: r.bool(),
    chancePct: r.u32(),
    id: r.u8(),
    generated: r.bool(),
    conditionMet: r.bool(),
    min: r.u32(),
    max: r.u32(),
  }));
  const count = r.u8();
  return { ...head, tints: all.slice(0, count) };
}

// ---------------------------------------------------------------------------------------------
// the module

type Exports = {
  memory: WebAssembly.Memory;
  abi_version(): number;
  input_ptr(): number;
  output_ptr(): number;
  buffer_len(): number;
  hero_sheet(len: number): number;
  preview_attack(len: number): number;
  set_tables(len: number): number;
  monster_sheet(len: number): number;
  unit_mods(len: number): number;
  line(len: number): number;
};

const ERRORS: Record<number, string> = {
  [-1]: "input longer than the buffer",
  [-2]: "input is not valid borsh",
  [-3]: "output does not fit",
  [-4]: "unknown monster",
  [-5]: "no resonance tables (setTables first)",
};

export class CombatWasm {
  private constructor(private readonly x: Exports) {}

  static async instantiate(bytes: BufferSource): Promise<CombatWasm> {
    const { instance } = await WebAssembly.instantiate(bytes, {});
    const x = instance.exports as unknown as Exports;
    const abi = x.abi_version();
    if (abi !== COMBAT_ABI) throw new Error(`combat wasm ABI ${abi}, expected ${COMBAT_ABI}`);
    return new CombatWasm(x);
  }

  private call(fn: (len: number) => number, input: Uint8Array): Uint8Array {
    if (input.length > this.x.buffer_len()) throw new Error("combat wasm: input longer than the buffer");
    new Uint8Array(this.x.memory.buffer).set(input, this.x.input_ptr());
    const len = fn(input.length);
    if (len < 0) throw new Error(`combat wasm: ${ERRORS[len] ?? `error ${len}`}`);
    const at = this.x.output_ptr();
    return new Uint8Array(this.x.memory.buffer).slice(at, at + len);
  }

  heroSheet(h: HeroInput): HeroSheet {
    return decodeSheet(this.call(this.x.hero_sheet, encodeHero(h)));
  }

  previewAttack(r: AttackRequest): HitPreview {
    return decodePreview(this.call(this.x.preview_attack, encodeAttack(r)));
  }

  /** The resonance tables of a session (`MonsterState.res_aspects ‖ res_traits ‖ res_pairs`). */
  setTables(tables: Uint8Array): void {
    if (tables.length !== COMBAT_TABLES_LEN) throw new Error(`combat wasm: tables are ${COMBAT_TABLES_LEN} bytes`);
    this.call(this.x.set_tables, tables);
  }

  monsterSheet(m: MonsterInput): MonsterSheet {
    return decodeMonsterSheet(this.call(this.x.monster_sheet, encodeMonster(m)));
  }

  /** A unit's buffs and debuffs with the auras over it (the same merge the session runs). */
  unitMods(r: UnitModsRequest): TimedMod[] {
    return decodeMods(this.call(this.x.unit_mods, encodeUnitMods(r)));
  }

  /** The cells of a line target, nearest first. */
  line(r: LineRequest): WasmCell[] {
    return decodeLine(this.call(this.x.line, encodeLine(r)));
  }
}

let loading: Promise<CombatWasm> | null = null;

/** The shipped module, fetched and instantiated once per page. */
export function loadCombat(): Promise<CombatWasm> {
  loading ??= fetch(wasmUrl)
    .then((res) => {
      if (!res.ok) throw new Error(`combat.wasm: HTTP ${res.status}`);
      return res.arrayBuffer();
    })
    .then((b) => CombatWasm.instantiate(b))
    .catch((err) => {
      loading = null;
      throw err;
    });
  return loading;
}

/** The combat module once loaded (null before, or if it failed: callers show nothing then). */
export function useCombat(): CombatWasm | null {
  const [combat, setCombat] = useState<CombatWasm | null>(null);
  useEffect(() => {
    let live = true;
    loadCombat().then(
      (c) => live && setCombat(c),
      (err) => console.warn(err),
    );
    return () => {
      live = false;
    };
  }, []);
  return combat;
}

/** Basis points as a percent: 6_500 -> "65%", 925 -> "9.3%". */
export function percent(bp: number): string {
  const p = bp / 100;
  return `${Number.isInteger(p) ? p : p.toFixed(1)}%`;
}

/**
 * A character account as the module's hero, wearing `equipment` (`ItemBase` bytes by slot: the
 * character's gear, or the party snapshot `Participation.equipment`), with `stats` in place of
 * the stored ones when given (a stat-point plan before it is saved).
 */
export function heroInputOf(c: CharacterAcc, equipment: ArrayLike<number>, stats: readonly number[] = c.core.stats, classTier = c.classTier): HeroInput {
  return {
    race: enumIndex(c.core.race, RACE_KEYS),
    class: enumIndex(c.core.class, CLASS_KEYS),
    level: c.core.level,
    stats,
    predisposition: enumIndex(c.core.predisposition, ASPECT_KEYS),
    predispositionTier: c.core.predispositionTier,
    classTier,
    equipment,
  };
}

/** Variant names of a character IDL enum in on-chain order ("Active", ...). */
function idlVariants(name: string): string[] {
  const t = (characterIdl as { types: { name: string; type: { kind: string; variants?: { name: string }[] } }[] }).types.find((x) => x.name === name);
  if (!t?.type.variants) throw new Error(`character IDL has no enum ${name}`);
  return t.type.variants.map((v) => v.name.charAt(0).toLowerCase() + v.name.slice(1));
}
const variantIndex = (type: string, e: Enum) => {
  const i = idlVariants(type).indexOf(Object.keys(e)[0] ?? "");
  if (i < 0) throw new Error(`unknown ${type} variant ${Object.keys(e)[0]}`);
  return i;
};

/** A skill account record as the module's skill (discriminants in `SkillBlocks` field order). */
export function skillInputOf(k: SkillRecordAcc): SkillInput {
  const b = k.blocks;
  return {
    blocks: [
      variantIndex("Category", b.category),
      variantIndex("Purpose", b.purpose),
      variantIndex("Source", b.source),
      variantIndex("Form", b.form),
      variantIndex("Target", b.target),
      ...b.aspects.slice(0, 3).map((a) => variantIndex("Aspect", a)),
      b.aspectCount,
      variantIndex("Effect", b.effect),
      variantIndex("Trigger", b.trigger),
      b.auraRadius,
    ],
    sharesBp: k.sharesBp,
    powerBp: k.powerBp,
  };
}

const STATE_KEYS = ["burning", "chill", "freeze", "bleeding", "shock", "stun", "penetration"] as const;

/** Account states (`ActiveStateData`) as the module's states. */
export function statesOf(states: readonly ActiveStateAcc[]): ActiveState[] {
  return states.map((s) => ({ kind: enumIndex(s.kind, STATE_KEYS), turns: s.turns, power: s.power, source: s.source }) as ActiveState);
}

/** The session's auras (`GameSession.auras`) with the players' cells, as the module's party. */
export function partyInputOf(s: { auras: readonly (readonly AuraAcc[])[]; playerPos: readonly Cell[]; aliveMask: number }): PartyInput {
  return {
    auras: s.auras.map((p) =>
      p.map((a) => ({
        kind: enumIndex(a.kind, MOD_KEYS),
        aspect: enumIndex(a.aspect, ASPECT_KEYS),
        powerBp: a.powerBp,
        radius: a.radius,
        turns: a.turns,
        hostile: a.hostile,
        passive: a.passive,
        selfOnly: a.selfOnly,
      })),
    ),
    cells: s.playerPos.map((c) => [c.x, c.y] as const),
    alive: s.aliveMask,
  };
}

/**
 * The impassable cells of a map as a line request's bitmap: `tiles` is `MapLayout.tiles` (stride
 * 32), a tile whose cost in the session (`GameSession.tile_cost`, by tile) is 0 cannot be entered.
 */
export function blockedOf(tiles: ArrayLike<number>, tileCost: ArrayLike<number>): Uint8Array {
  const out = new Uint8Array(COMBAT_BLOCKED_LEN);
  for (let i = 0; i < COMBAT_BLOCKED_LEN * 8; i++) if ((tileCost[tiles[i] ?? 0] ?? 0) === 0) out[i >> 3]! |= 1 << (i & 7);
  return out;
}

/** An attack of `hero` (with its buffs and debuffs and resonance accuracy) on a live monster with
 * its registry defenses, before the dice. The session's tables must be loaded (`setTables`). */
export function previewOnMonster(
  combat: CombatWasm,
  hero: HeroInput,
  heroMods: readonly TimedMod[],
  skill: SkillRecordAcc | null,
  monster: MonsterAcc,
  monsterMods: readonly TimedMod[],
  defense: DefenseInput,
  accuracy: AccuracyInput = NO_ACCURACY,
  party: PartyInput = NO_PARTY,
  slot = 0,
): HitPreview | null {
  try {
    return combat.previewAttack({
      hero,
      heroMods,
      skill: skill ? skillInputOf(skill) : null,
      target: { bestiaryId: monster.bestiaryId, level: monster.level, states: statesOf(monster.states), mods: monsterMods, defense },
      accuracy,
      party,
      slot,
      targetAt: [monster.x, monster.y],
    });
  } catch {
    // An unknown bestiary id: no preview rather than a wrong one.
    return null;
  }
}

/** "Hit 65% · Crit 9.3% · 4-9 (crit 6-13)", or "Immune". */
export function previewText(p: HitPreview): string {
  if (p.immune) return "Immune";
  return `Hit ${percent(p.hitBp)} · Crit ${percent(p.critBp)} · ${p.min}-${p.max} (crit ${p.critMin}-${p.critMax})`;
}
