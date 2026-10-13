// Heals, buffs and debuffs as the screens need them (PR #25, deadroll_combat::support): which side
// a skill targets, the EffectResolved / ModsTicked events in app shape, and their log lines.
// Pure functions over decoded accounts and events.
import type { Address } from "@solana/kit";
import { ASPECT_KEYS, MOD_KEYS, bytesText, enumIndex, variantKey, type Enum, type SkillRecordAcc, type TimedModAcc } from "./game";
import { effectAmount } from "./names";
import { Effect, Form, Purpose, skillSide, usableInCombat, type Side, type SkillBlocks } from "./skillcheck";
import { Applied, HitOutcome, type Aspect, type ModKind, type TimedMod } from "./types";

const EFFECT_KEYS = ["none", "weakening", "slow", "vulnerability", "stun", "freeze", "haste", "empowerment", "protection", "heal"] as const;
const PURPOSE_KEYS = ["damage", "buff", "debuff"] as const;
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

/** e.g. "Haste +25% (2)"; Stun / Freeze are states, not mods. */
export function modLabel(m: TimedMod): string {
  return `${MOD_NAMES[m.kind] ?? "mod"} ${modIsBuff(m.kind) ? "+" : "-"}${Math.round(m.powerBp / 100)}% (${modTurnsLeft(m)})`;
}

// ---- skills in battle

/** One SkillSet slot as the skill bar needs it. */
export type CombatSkill = { slot: number; name: string; usable: boolean; purpose: Purpose; side: Side; form: Form; effect: Effect };

export function combatSkill(slot: number, k: SkillRecordAcc): CombatSkill {
  const purpose = enumIndex(k.blocks.purpose, PURPOSE_KEYS) as Purpose;
  const effect = enumIndex(k.blocks.effect, EFFECT_KEYS) as Effect;
  const form = enumIndex(k.blocks.form, FORM_KEYS) as Form;
  const usable = usableInCombat({ category: variantKey(k.blocks.category) === "active" ? 0 : 1, purpose, effect } as SkillBlocks);
  return { slot, name: bytesText(k.name), usable, purpose, side: skillSide(purpose), form, effect };
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
  if (e.roll.chanceRoll !== null) dice.push(`chance ${Math.round(e.roll.chanceBp / 100)}%`);
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
