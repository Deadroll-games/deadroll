// Props contract for the dice cascade and hit breakdown (L9.4). Written by the lane owner.
import type { HitResult } from "../../chain/types";

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
};
