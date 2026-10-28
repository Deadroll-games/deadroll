// Display names for enum values. English UI strings; keep in sync with chain/types.ts.
import { EFFECT_NAMES, Effect } from "./skillcheck";
import type { EffectResolved } from "./support";
import aspectsJson from "../../../assets/registry/aspects.json";
import bestiaryJson from "../../../assets/registry/bestiary.json";
import monsterSkillsJson from "../../../assets/registry/monster_skills.json";
import resonancesJson from "../../../assets/registry/resonances.json";
import traitsJson from "../../../assets/registry/traits.json";
import { MOD_NAMES } from "./support";
import { ASPECT_COUNT, HitOutcome, OP_NATIVE, OpKind, OpOrigin, type Aspect, type Carrier, type Class, type ModKind, type OpResult, type Race, type StateKind, type Tile } from "./types";

export const ASPECT_NAMES = ["Fire", "Cold", "Lightning", "Blood", "Blunt", "Piercing"] as const;
export const STATE_NAMES = ["Burning", "Chill", "Freeze", "Bleeding", "Shock", "Stun", "Penetration"] as const;
export const RACE_NAMES = ["Human", "Forest elf"] as const;
export const CLASS_NAMES = ["Warrior", "Mage", "Support"] as const;
/** By tile type id (`Tile`, patch 13). */
/** Map biomes by id (`Biome`). */
export const BIOME_NAMES = ["Caves and Ruins", "Forest", "Crypt"] as const;
export const TILE_NAMES = ["Void", "Stone floor", "Wooden floor", "Dirt", "Wall", "Door", "Shallow water", "Deep water", "Stairs", "Rubble", "Chasm"] as const;
/** Where a tint comes from. Only weapons carry an aspect in the first version (CHARACTER 8.1: no class aspect). */
export const CARRIER_NAMES = ["Race", "Class", "Weapon"] as const;
export const STAT_NAMES = ["Strength", "Dexterity", "Intelligence", "Endurance"] as const;

/** Monster names by bestiary id: off-chain text next to the records the chain holds
 * (assets/registry/bestiary.json, docs/DAMAGE_TYPES_AND_MIXES.md 10.1), capitalized for the UI. */
export const BESTIARY_NAMES: Record<number, string> = Object.fromEntries(
  bestiaryJson.records.map((r) => [r.id, r.name.charAt(0).toUpperCase() + r.name.slice(1)]),
);

/** Monster skill names by registry `monster_skills` index (assets/registry/monster_skills.json). */
export const MONSTER_SKILL_NAMES: string[] = monsterSkillsJson.records.map((r) => r.name);

export const aspectName = (a: Aspect): string => ASPECT_NAMES[a] ?? `aspect ${a}`;
export const stateName = (s: StateKind): string => STATE_NAMES[s] ?? `state ${s}`;
export const raceName = (r: Race): string => RACE_NAMES[r] ?? `race ${r}`;
export const className = (c: Class): string => CLASS_NAMES[c] ?? `class ${c}`;
export const tileName = (t: Tile): string => TILE_NAMES[t] ?? `tile ${t}`;
export const carrierName = (c: Carrier): string => CARRIER_NAMES[c] ?? `carrier ${c}`;
export const bestiaryName = (id: number): string => BESTIARY_NAMES[id] ?? `monster #${id}`;

// Resonance and trait names are off-chain text (docs/DAMAGE_TYPES_AND_MIXES.md 10.1): they live
// in assets/registry next to the records the chain holds, read here, not copied.
const PAIR_NAMES: (string | null)[] = resonancesJson.records.map((r) => r.name);
const TRAITS: { trait: string; epithet: string }[] = traitsJson.records;
const ASPECT_TRAIT: string[] = aspectsJson.records.map((r) => r.trait);
const NATIVE: (StateKind | null)[] = aspectsJson.records.map((r) => (r.native ? (STATE_NAMES.indexOf(r.native as (typeof STATE_NAMES)[number]) as StateKind) : null));

/** The tint trait an aspect brings ("Fury", ...). */
export const traitName = (a: Aspect): string => ASPECT_TRAIT[a] ?? `trait ${a}`;
const epithet = (a: Aspect): string => TRAITS.find((t) => t.trait === ASPECT_TRAIT[a])?.epithet ?? "";

/** The native state of an aspect (registry `aspects`). */
export const nativeState = (a: Aspect): StateKind | null => NATIVE[a] ?? null;

/** Resonance id is base * 6 + tint (crates/combat). A triple adds the second tint's epithet
 * ("Piercing Frost of the Storm", docs 6.5). */
export function resonanceName(id: number, secondTint: Aspect | null = null): string {
  const base = Math.floor(id / ASPECT_COUNT) as Aspect;
  const tint = (id % ASPECT_COUNT) as Aspect;
  const name = PAIR_NAMES[id] ?? `${aspectName(base)} + ${aspectName(tint)}`;
  return secondTint == null ? name : `${name} ${epithet(secondTint)}`;
}

const argState = (op: OpResult, base: Aspect): string => {
  const kind = op.arg === OP_NATIVE ? nativeState(base) : (op.arg as StateKind);
  return kind == null ? "its state" : stateName(kind);
};
const pct = (bp: number) => `${Math.round(bp / 100)}%`;

/** What one op of a resonance on `base` did, e.g. "Burning +2 turns", "Shock brings Burning". */
export function opText(op: OpResult, base: Aspect): string {
  const native = argState({ ...op, arg: OP_NATIVE }, base);
  const state = argState(op, base);
  let text: string;
  switch (op.kind) {
    case OpKind.Multiplier:
      text = `+${pct(op.amount)} ${aspectName(base)} damage${op.arg === OP_NATIVE ? "" : ` on a target with ${state}`}`;
      break;
    case OpKind.StateOnCrit:
      text = `a crit applies ${state}`;
      break;
    case OpKind.ExtraStateChance:
      text = `a chance of ${state}`;
      break;
    case OpKind.Linked:
      text = `${native} brings ${state}`;
      break;
    case OpKind.LinkedChance:
      text = `${native} with a chance of ${state}`;
      break;
    case OpKind.LinkedMod:
      text = `${native} brings ${MOD_NAMES[op.arg as ModKind] ?? "a modifier"} ${pct(op.amount)}`;
      break;
    case OpKind.NativeTurns:
      text = `${native} +${op.amount} turns`;
      break;
    case OpKind.NativePower:
    case OpKind.NativePowerArmor:
      text = `${native} +${pct(op.amount)} power${op.kind === OpKind.NativePowerArmor ? " (heavier armor, more)" : ""}`;
      break;
    case OpKind.NativeFaster:
      text = `${native} ticks ${op.amount}x per turn, shorter`;
      break;
    case OpKind.ThawDamage:
      text = `Freeze deals ${op.amount} when it thaws`;
      break;
    case OpKind.RemoveState:
      text = `removes ${state}`;
      break;
    case OpKind.Jump:
      text = `${state} jumps to an adjacent enemy`;
      break;
    case OpKind.JumpChain:
      text = `${native} jumps along enemies with ${stateName(op.arg as StateKind)} (up to ${op.amount})`;
      break;
    case OpKind.JumpLine:
      text = `${state} passes to the enemy behind`;
      break;
    case OpKind.Knockback:
      text = `knocks back ${op.amount} cell${op.amount === 1 ? "" : "s"}`;
      break;
    case OpKind.IgniteCells:
      text = `sets the cells around on fire (${op.amount} per turn)`;
      break;
    case OpKind.AddPacket:
      text = `+${op.amount}-${op.amountMax} ${aspectName(op.arg as Aspect)} damage`;
      break;
    case OpKind.Penetrate:
      text = `+${pct(op.amount)} armor penetration`;
      break;
    case OpKind.AccuracyBuff:
      text = `+${op.amount} accuracy to ${aspectName(op.arg as Aspect)} attacks`;
      break;
    case OpKind.ForceCrit:
      text = "the hit is a crit";
      break;
    case OpKind.ApplyState:
      text = `applies ${state}`;
      break;
    case OpKind.StateIfHad:
      text = `on a target that already had ${native}, applies ${state}`;
      break;
    case OpKind.ApplyMod:
      text = `${MOD_NAMES[op.arg as ModKind] ?? "a modifier"} on the target and on the enemies the jumps reach`;
      break;
    case OpKind.ModAfterNative:
      text = `${native} brings ${MOD_NAMES[op.arg as ModKind] ?? "a modifier"}, lasting beyond it`;
      break;
    case OpKind.Splash:
      text = `${op.arg === OP_NATIVE ? "" : `on a target with ${state}, `}the adjacent enemies take ${op.amount} damage`;
      break;
    case OpKind.Nova:
      text = `${native} and ${MOD_NAMES[op.arg as ModKind] ?? "a modifier"} hit the enemies within ${op.amount} cells`;
      break;
    default:
      text = "nothing";
  }
  const from =
    op.origin === OpOrigin.Trait ? `${traitName(op.tint)}: `
    : op.origin === OpOrigin.Template ? `${traitName(op.tint)} (triple): `
    : "";
  const crit = op.critOnly ? "on a crit, " : "";
  return `${from}${crit}${text}${op.acted ? "" : " (did not trigger)"}`;
}

/** Formats a value scaled x10_000 with one decimal, e.g. 52_500n -> "5.3". */
export function formatScaled(v: bigint): string {
  const tenths = (v + 500n) / 1_000n;
  return `${tenths / 10n}.${tenths % 10n}`;
}

/** Basis points as a percent, e.g. 2_500 -> "25%". */
export function formatBp(bp: number): string {
  return `${Number.isInteger(bp / 100) ? bp / 100 : (bp / 100).toFixed(2)}%`;
}

/** Hundredths with two decimals, e.g. 8_104 -> "81.04". */
export function formatHundredths(v: number): string {
  return (v / 100).toFixed(2);
}

/*
 * Chance rolls are a d100, 0..=100, shown "high roll wins". On-chain success is raw < chance
 * (crates/combat/src/rng.rs). Shown = 100 - raw and needed = 101 - chance, so shown >= needed
 * exactly when raw < chance.
 */
export const shownRoll = (raw: number): number => 100 - raw;
export const neededRoll = (chancePct: number): number => 101 - chancePct;

/** Tooltip with the raw on-chain numbers behind a shown chance roll. */
export function rollTitle(raw: number, chancePct: number | null): string {
  const pass = chancePct == null ? "" : `, passes when raw < ${chancePct}`;
  return `on-chain raw roll ${raw}, shown as 100 - raw${pass}`;
}

export const effectName = (e: Effect): string => EFFECT_NAMES[e] ?? `effect ${e}`;

/** What one effect roll did, e.g. "+7 hp", "Weakening 18%", "Freeze", "missed". */
/** Defense tags (patch 9, deadroll_combat `TAGS`): the power sources, then the forms. */
export const TAG_NAMES = ["Magical", "Combat", "Strike", "Projectile"] as const;

/** The names of the set bits of `mask` (bit i = `names[i]`). */
export function maskNames(mask: number, names: readonly string[]): string[] {
  return names.filter((_, i) => (mask & (1 << i)) !== 0);
}

export function effectAmount(e: Pick<EffectResolved, "effect" | "roll">): string {
  if (e.roll.outcome === HitOutcome.Miss) return "missed";
  if (e.roll.outcome === HitOutcome.Immune) return "immune";
  if (e.effect === Effect.Heal) return `+${e.roll.amount} hp`;
  if (e.effect === Effect.Stun || e.effect === Effect.Freeze) return effectName(e.effect);
  return `${effectName(e.effect)} ${Math.round(e.roll.amount / 100)}%`;
}
