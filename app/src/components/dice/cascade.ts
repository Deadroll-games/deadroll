// Roll order for one HitResult. Pure: same hit always yields the same steps.
import { Effect } from "../../chain/skillcheck";
import type { EffectResolved } from "../../chain/support";
import { Applied, HitOutcome, type HitResult, type StateRoll, type TintRoll } from "../../chain/types";
import {
  aspectName,
  carrierName,
  effectAmount,
  effectName,
  formatHundredths,
  formatScaled,
  neededRoll,
  resonanceName,
  rollTitle,
  shownRoll,
  stateName,
} from "../../chain/names";

/** Shown roll and needed roll, both in hundredths 0..9_999 / 1..10_000 (see shownRoll in chain/names). */
export type RollBarData = { shown: number; needed: number | null; success: boolean };

export type CascadeStep = {
  key: string;
  kind: "d20" | "crit" | "tint" | "resonance" | "d100" | "packet" | "state" | "damage" | "chance";
  label: string;
  value: string;
  target?: string;
  success?: boolean;
  /** Overrides the "pass" text on success, e.g. "CRIT!". */
  flag?: string;
  /** Raw on-chain numbers for the tooltip. */
  title?: string;
  bar?: RollBarData;
};

/** One chance roll, shown "high roll wins". `chanceBp` null when the app cannot know it. */
export type ChanceView = {
  value: string;
  target: string;
  success: boolean;
  flag: string;
  title: string;
  bar: RollBarData;
};

export function chanceView(raw: number, chanceBp: number | null, success: boolean, passWord = "pass"): ChanceView {
  const shown = shownRoll(raw);
  const needed = chanceBp == null ? null : neededRoll(chanceBp);
  return {
    value: `rolled ${formatHundredths(shown)}`,
    target: `needs ${needed == null ? "?" : formatHundredths(needed)}+`,
    success,
    flag: success ? passWord : "fail",
    title: rollTitle(raw, chanceBp),
    bar: { shown, needed, success },
  };
}

/** e.g. "Resonance (Piercing tint on Lightning, tier 14)". */
export function tintLabel(tint: TintRoll): string {
  return `Resonance (${aspectName(tint.tint)} tint on ${aspectName(tint.base)}, tier ${tint.tier})`;
}

export function tintView(tint: TintRoll): ChanceView {
  const view = chanceView(tint.roll, tint.chanceBp, tint.success, "RESONANCE!");
  return { ...view, title: `${view.title}, carrier ${carrierName(tint.carrier)}` };
}

function appliedSuffix(applied: Applied): string {
  if (applied === Applied.Added) return " added";
  if (applied === Applied.Refreshed) return " refreshed";
  if (applied === Applied.Froze) return " froze";
  return "";
}

function pushState(steps: CascadeStep[], roll: StateRoll, key: string): void {
  if (!roll.rolled) return;
  steps.push({
    key,
    kind: "state",
    label: `${stateName(roll.kind)} chance${appliedSuffix(roll.applied)}`,
    ...chanceView(roll.roll, roll.chanceBp, roll.success),
  });
}

/** `critChanceBp` is not in the event: the caller takes it from the combat module (HeroSheet.critRollBp) or leaves it unknown. */
export function cascadeSteps(hit: HitResult, critChanceBp?: number): CascadeStep[] {
  const steps: CascadeStep[] = [];

  steps.push({
    key: "d20",
    kind: "d20",
    label: "Attack d20",
    value: String(hit.d20),
    target: hit.outcome === HitOutcome.Immune ? "Immune" : `total ${hit.attackTotal} vs defense ${hit.defense}`,
    success: hit.outcome !== HitOutcome.Miss,
  });

  if (hit.critRoll !== null) {
    steps.push({
      key: "crit",
      kind: "crit",
      label: "Crit",
      ...chanceView(hit.critRoll, critChanceBp ?? null, hit.crit, "CRIT!"),
    });
  }

  hit.tintRolls.forEach((tint, i) => {
    steps.push({
      key: `tint-${i}`,
      kind: "tint",
      label: tintLabel(tint),
      ...tintView(tint),
    });
  });

  hit.resonances.forEach((resonance, i) => {
    if (resonance == null || !resonance.active) return;
    steps.push({
      key: `resonance-${i}`,
      kind: "resonance",
      label: `Resonance ${resonanceName(resonance.id)}`,
      value: `tier ${resonance.tier}`,
      success: true,
    });
  });

  if (hit.outcome === HitOutcome.Hit && hit.packets.length > 0) {
    steps.push({
      key: "d100",
      kind: "d100",
      label: "Damage d100",
      value: String(hit.d100),
    });
  }

  hit.packets.forEach((packet, i) => {
    steps.push({
      key: `packet-${i}`,
      kind: "packet",
      label: `${aspectName(packet.aspect)} damage`,
      value: packet.immune ? "immune" : formatScaled(packet.afterDefense),
      target: `rolled ${formatScaled(packet.rolled)} in ${formatScaled(packet.min)}-${formatScaled(packet.max)}`,
    });
  });

  hit.packets.forEach((packet, i) => pushState(steps, packet.state, `state-p-${i}`));
  hit.resonanceStates.forEach((roll, i) => pushState(steps, roll, `state-r-${i}`));

  steps.push({
    key: "damage",
    kind: "damage",
    label: hit.outcome === HitOutcome.Miss ? "Miss" : "Damage",
    value: String(hit.damage),
  });

  return steps;
}

/**
 * Roll order of an EffectResolved (deadroll_combat::support): a debuff's d20 vs defense, the
 * crit roll, a damage skill effect's chance roll, the d100 point in the strength range, then the
 * result. Heals and buffs have no d20 (it is 0); Stun and Freeze no d100.
 */
export function effectSteps(e: EffectResolved, critChanceBp?: number): CascadeStep[] {
  const r = e.roll;
  const steps: CascadeStep[] = [];
  if (r.d20 > 0) {
    steps.push({
      key: "d20",
      kind: "d20",
      label: "Hit d20",
      value: String(r.d20),
      target: `total ${r.attackTotal} vs defense ${r.defense}`,
      success: r.outcome !== HitOutcome.Miss,
      flag: r.d20 === 20 ? "NAT 20" : r.d20 === 1 ? "NAT 1" : undefined,
    });
  }
  if (r.chanceRoll !== null) {
    steps.push({ key: "chance", kind: "chance", label: `${effectName(e.effect)} chance`, ...chanceView(r.chanceRoll, r.chanceBp, r.outcome !== HitOutcome.Miss) });
  } else if (e.rider && r.outcome !== HitOutcome.Miss) {
    steps.push({ key: "chance", kind: "chance", label: `${effectName(e.effect)} chance`, value: "certain", target: "on a crit", success: true });
  }
  if (r.critRoll !== null) {
    steps.push({ key: "crit", kind: "crit", label: "Crit", ...chanceView(r.critRoll, critChanceBp ?? null, r.crit, "CRIT!") });
  }
  if (r.outcome !== HitOutcome.Miss && r.d100 > 0) {
    steps.push({
      key: "d100",
      kind: "d100",
      label: e.effect === Effect.Heal ? "Heal d100" : "Strength d100",
      value: String(r.d100),
      target: r.crit ? "x crit" : undefined,
    });
  }
  const applied = e.applied === Applied.Added ? "added" : e.applied === Applied.Refreshed ? "refreshed" : e.applied === Applied.Froze ? "froze" : null;
  steps.push({
    key: "result",
    kind: "damage",
    label: r.outcome === HitOutcome.Miss ? "Miss" : e.effect === Effect.Heal ? "Healed" : effectName(e.effect),
    value: effectAmount(e),
    target: e.effect === Effect.Heal && r.outcome !== HitOutcome.Miss ? `${e.hp} hp` : (applied ?? undefined),
  });
  return steps;
}
