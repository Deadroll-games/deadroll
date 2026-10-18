// The combat crate in the browser (crates/combat/wasm, README there): a hero's derived stats and
// the preview of an attack before the dice (docs/COMBAT_MATH.md, section 13). The module builds
// the hit input with crates/rules, the same code the session program runs, so nothing here
// repeats a formula: this file only writes borsh inputs and reads borsh results.
//
// The shipped module is ./wasm/combat.wasm, rebuilt by `bash crates/combat/wasm/build.sh` (CI
// fails when it is stale). __fixtures__/combat-wasm.json pins the byte layouts and the results.
import { useEffect, useState } from "react";
import wasmUrl from "./wasm/combat.wasm?url";
import { Reader, concat, u16le, u32le } from "./bytes";
import { ASPECT_KEYS, CLASS_KEYS, RACE_KEYS, enumIndex, type ActiveStateAcc, type CharacterAcc, type Enum, type MonsterAcc, type SkillRecordAcc } from "./game";
import characterIdl from "./idl/deadroll_character.json";
import type { ActiveState, TimedMod } from "./types";

/** `ABI_VERSION` of the module this wrapper speaks. */
export const COMBAT_ABI = 3;
export const COMBAT_STATS = 4;
export const COMBAT_EQUIP_SLOTS = 11;
export const COMBAT_STATES = 6;
export const COMBAT_MODS = 6;

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

export type TargetInput = {
  bestiaryId: number;
  level: number;
  states: readonly ActiveState[];
  mods: readonly TimedMod[];
};

export type AttackRequest = {
  hero: HeroInput;
  heroMods: readonly TimedMod[];
  /** null = the weapon's basic attack. */
  skill: SkillInput | null;
  target: TargetInput;
};

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

export type HitPreview = {
  immune: boolean;
  hitBp: number;
  critBp: number;
  min: number;
  max: number;
  critMin: number;
  critMax: number;
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
  );
}

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
  return {
    immune: r.bool(),
    hitBp: r.u32(),
    critBp: r.u32(),
    min: r.u32(),
    max: r.u32(),
    critMin: r.u32(),
    critMax: r.u32(),
  };
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
};

const ERRORS: Record<number, string> = {
  [-1]: "input longer than the buffer",
  [-2]: "input is not valid borsh",
  [-3]: "output does not fit",
  [-4]: "unknown monster",
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

/** An attack of `hero` (with its buffs and debuffs) on a live monster, before the dice. */
export function previewOnMonster(
  combat: CombatWasm,
  hero: HeroInput,
  heroMods: readonly TimedMod[],
  skill: SkillRecordAcc | null,
  monster: MonsterAcc,
  monsterMods: readonly TimedMod[],
): HitPreview | null {
  try {
    return combat.previewAttack({
      hero,
      heroMods,
      skill: skill ? skillInputOf(skill) : null,
      target: { bestiaryId: monster.bestiaryId, level: monster.level, states: statesOf(monster.states), mods: monsterMods },
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
