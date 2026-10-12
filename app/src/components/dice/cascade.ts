// Roll order for one HitResult. Pure: same hit always yields the same steps.
import { Applied, HitOutcome, type HitResult, type StateRoll } from "../../chain/types";
import { aspectName, carrierName, formatScaled, resonanceName, stateName } from "../../chain/names";

export type CascadeStep = {
  key: string;
  kind: "d20" | "crit" | "tint" | "resonance" | "d100" | "packet" | "state" | "damage";
  label: string;
  value: string;
  target?: string;
  success?: boolean;
};

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
    value: String(roll.roll),
    target: `< ${roll.chanceBp}`,
    success: roll.success,
  });
}

export function cascadeSteps(hit: HitResult): CascadeStep[] {
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
      label: "Crit roll",
      value: String(hit.critRoll),
      success: hit.crit,
    });
  }

  hit.tintRolls.forEach((tint, i) => {
    steps.push({
      key: `tint-${i}`,
      kind: "tint",
      label: `${aspectName(tint.base)} tint ${aspectName(tint.tint)} (${carrierName(tint.carrier)}, tier ${tint.tier})`,
      value: String(tint.roll),
      target: `< ${tint.chanceBp}`,
      success: tint.success,
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
