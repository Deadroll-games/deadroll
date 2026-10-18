// Heals, buffs and debuffs as the screens need them (PR #25, deadroll_combat::support): which side
// a skill targets, the EffectResolved / ModsTicked events in app shape, and their log lines.
// Pure functions over decoded accounts and events.
import type { Address } from "@solana/kit";
import { ASPECT_KEYS, MOD_KEYS, bytesText, enumIndex, type Enum, type SkillRecordAcc, type TimedModAcc } from "./game";
import { ASPECT_NAMES, effectAmount } from "./names";
import { Effect, Form, Purpose, skillSide, usableInCombat, type Category, type Side, type SkillBlocks } from "./skillcheck";
import { Applied, HitOutcome, type Aspect, type ModKind, type TimedMod } from "./types";

const EFFECT_KEYS = ["none", "weakening", "slow", "vulnerability", "stun", "freeze", "haste", "empowerment", "protection", "heal"] as const;
const PURPOSE_KEYS = ["damage", "buff", "debuff"] as const;
const CATEGORY_KEYS = ["active", "reactive", "passive"] as const;
const FORM_KEYS = ["strike", "projectile"] as const;
const OUTCOMES = ["miss", "hit", "immune"];
const APPLIED = ["none", "added", "refreshed", "froze"];
export const MOD_NAMES = ["Haste", "Slow", "Empowerment", "Weakening", "Protection", "Vulnerability"] as const;
/** Haste, Empowerment, Protection help the bearer; the rest hurt. */
export const modIsBuff = (k: ModKind): boolean => k === 0 || k === 2 || k === 4;

/** Active slots only (`turns > 0`), in kind order. */
export function activeMods(mods: readonly TimedModAcc[] | undefined): TimedMod[] {
  return (mods ?? [])
    .filter((m) => m.turns > 0)
    .map((m) => ({ kind: enumIndex(m.kind, MOD_KEYS) as ModKind, aspect: enumIndex(m.aspect, ASPECT_KEYS) as Aspect, turns: m.turns, powerBp: m.powerBp, source: m.source }));
}

/** Own turns a modifier still covers after the current one (`turns` counts the next start too). */
export const modTurnsLeft = (m: TimedMod): number => Math.max(0, m.turns - 1);

/**
 * What a mod does to its bearer's numbers (deadroll_combat::support): Haste / Slow its AP,
 * Empowerment / Weakening its outgoing damage, Protection its resistances, Vulnerability the damage
 * it takes from the mod's aspect only.
 */
export function modEffectText(m: TimedMod): string {
  const pct = `${Math.round(m.powerBp / 100)}%`;
  switch (m.kind) {
    case 0:
      return `+${pct} AP`;
    case 1:
      return `-${pct} AP`;
    case 2:
      return `+${pct} damage`;
    case 3:
      return `-${pct} damage`;
    case 4:
      return `+${pct} resist`;
    default:
      return `+${pct} ${ASPECT_NAMES[m.aspect] ?? "?"} damage taken`;
  }
}

/** e.g. "Haste +25% AP (2)", "Vulnerability +25% Fire damage taken (1)"; Stun / Freeze are states, not mods. */
export function modLabel(m: TimedMod): string {
  return `${MOD_NAMES[m.kind] ?? "mod"} ${modEffectText(m)} (${modTurnsLeft(m)})`;
}

/**
 * The bearer's mods as they were at event `index` of a party's event list: the latest ModsTicked
 * or EffectResolved on that unit before it (both carry the unit's full mods), else `fallback`
 * (the account now). A rider's EffectResolved follows its hit, so it never counts for that hit.
 */
export function modsAt(events: readonly { name: string; data: any }[], index: number, unit: number, fallback: TimedMod[]): TimedMod[] {
  for (let j = Math.min(index, events.length) - 1; j >= 0; j--) {
    const e = events[j]!;
    if (e.name === "ModsTicked" && e.data.unit === unit) return activeMods(e.data.mods);
    if (e.name === "EffectResolved" && e.data.target === unit) return activeMods(e.data.mods);
  }
  return fallback;
}

// ---- skills in battle

/** One SkillSet slot as the skill bar needs it. */
export type CombatSkill = { slot: number; name: string; category: Category; usable: boolean; purpose: Purpose; side: Side; form: Form; effect: Effect };

export function combatSkill(slot: number, k: SkillRecordAcc): CombatSkill {
  const purpose = enumIndex(k.blocks.purpose, PURPOSE_KEYS) as Purpose;
  const effect = enumIndex(k.blocks.effect, EFFECT_KEYS) as Effect;
  const form = enumIndex(k.blocks.form, FORM_KEYS) as Form;
  const category = enumIndex(k.blocks.category, CATEGORY_KEYS) as Category;
  const usable = usableInCombat({ category, purpose, effect } as SkillBlocks);
  return { slot, name: bytesText(k.name), category, usable, purpose, side: skillSide(purpose), form, effect };
}

// ---- EffectResolved

/** `EffectRollData`: the rolls of a heal, buff, debuff or a damage skill's effect. */
export type EffectRoll = {
  outcome: HitOutcome;
  /** Hit roll of a debuff; 0 = none (heals and buffs do not roll to land). */
  d20: number;
  attackTotal: number;
  defense: number;
  crit: boolean;
  critRoll: number | null;
  /** Effect chance of a damage skill's effect; 0 otherwise. */
  chanceBp: number;
  /** null when not rolled (not a rider, or certain on a crit). */
  chanceRoll: number | null;
  d100: number;
  /** HP for a heal, bp for a buff or debuff, 0 for Stun / Freeze or a miss. */
  amount: number;
  rollsUsed: number;
};

export type EffectResolved = {
  session: Address;
  caster: number;
  target: number;
  seed: Uint8Array;
  actionCounter: number;
  skillSlot: number;
  effect: Effect;
  /** The effect of a damage skill, after its HitResolved on the same seed. */
  rider: boolean;
  roll: EffectRoll;
  applied: Applied;
  /** Target HP after the effect. */
  hp: number;
  /** Target buffs and debuffs after the effect (active only). */
  mods: TimedMod[];
};

export function toEffectResolved(ev: any): EffectResolved {
  const r = ev.roll;
  return {
    session: ev.session as Address,
    caster: ev.caster,
    target: ev.target,
    seed: ev.seed,
    actionCounter: ev.actionCounter,
    skillSlot: ev.skillSlot,
    effect: enumIndex(ev.effect as Enum, EFFECT_KEYS) as Effect,
    rider: ev.rider,
    roll: {
      outcome: enumIndex(r.outcome, OUTCOMES) as HitOutcome,
      d20: r.d20,
      attackTotal: r.attackTotal,
      defense: r.defense,
      crit: r.crit,
      critRoll: r.critRoll,
      chanceBp: r.chanceBp,
      chanceRoll: r.chanceRoll,
      d100: r.d100,
      amount: r.amount,
      rollsUsed: r.rollsUsed,
    },
    applied: enumIndex(ev.applied, APPLIED) as Applied,
    hp: ev.hp,
    mods: activeMods(ev.mods),
  };
}

const APPLIED_WORDS = ["", "added", "refreshed", "froze"];

/** EffectResolved as a log line. */
export function effectText(e: EffectResolved, caster: string, target: string, skill: string): string {
  const head = e.rider ? `${skill} effect on ${target}` : e.caster === e.target ? `${caster} uses ${skill} on self` : `${caster} uses ${skill} on ${target}`;
  const dice: string[] = [];
  if (e.roll.d20 > 0) dice.push(`d20 ${e.roll.d20} (${e.roll.attackTotal} vs ${e.roll.defense})`);
  if (e.roll.chanceRoll !== null) dice.push(`chance ${e.roll.chanceBp}%`);
  if (e.roll.d100 > 0) dice.push(`d100 ${e.roll.d100}`);
  const crit = e.roll.crit ? " CRIT" : "";
  const word = APPLIED_WORDS[e.applied] ?? "";
  const after = e.effect === Effect.Heal && e.roll.outcome !== HitOutcome.Miss ? ` (${e.hp} hp)` : word ? ` (${word})` : "";
  return `${head}: ${dice.length ? `${dice.join(", ")}, ` : ""}${effectAmount(e)}${crit}${after}`;
}

// ---- ModsTicked

/** ModsTicked as a log line; null when nothing wore off. AP of a hasted or slowed turn are named. */
export function modsTickText(d: { expired: number; mods: TimedModAcc[]; actionPoints: number }, unit: string): string | null {
  const expired = MOD_NAMES.filter((_, i) => d.expired & (1 << i));
  const speed = activeMods(d.mods).filter((m) => m.kind === 0 || m.kind === 1);
  const parts: string[] = [];
  if (expired.length) parts.push(`${expired.join(", ")} wore off`);
  if (speed.length) parts.push(`${speed.map((m) => MOD_NAMES[m.kind]).join(" + ")}: ${d.actionPoints} AP this turn`);
  return parts.length ? `${unit}: ${parts.join("; ")}` : null;
}
