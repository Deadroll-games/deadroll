// Props contract for the dice cascade and hit breakdown (L9.4). Written by the lane owner.
import type { EffectResolved } from "../../chain/support";
import type { HitResult, TimedMod } from "../../chain/types";

/** Buffs and debuffs on both sides when the hit rolled (not in HitResolved: the app snapshots them). */
export type HitMods = { attacker: TimedMod[]; target: TimedMod[] };

export type DiceCascadeProps = {
  hit: HitResult;
  /** Labels shown above the cascade. */
  attackerLabel: string;
  targetLabel: string;
  /**
   * Milliseconds per step. Default 450. Steps in order: d20 vs defense, crit roll, tint rolls,
   * resonances, d100, one step per damage packet, state rolls, final damage.
   */
  stepMs?: number;
  /** Show everything at once without animation (also used when prefers-reduced-motion). */
  instant?: boolean;
  /** Called once after the last step. */
  onDone?: () => void;
  /** Attacker crit chance in bp. Not in the event; shown as "?" when unknown. */
  critChanceBp?: number;
};

export type HitBreakdownProps = {
  hit: HitResult;
  /** Attacker crit chance in bp. Not in the event; shown as "?" when unknown. */
  critChanceBp?: number;
  /** Expanded by default. */
  defaultOpen?: boolean;
  /** When given, a "Modifiers" section says where each mod went (or why it did nothing). */
  mods?: HitMods;
};

export type EffectCascadeProps = {
  effect: EffectResolved;
  casterLabel: string;
  targetLabel: string;
  /** The skill's name. */
  skillLabel: string;
  stepMs?: number;
  instant?: boolean;
  onDone?: () => void;
  /** Caster crit chance in bp. Not in the event; shown as "?" when unknown. */
  critChanceBp?: number;
};
