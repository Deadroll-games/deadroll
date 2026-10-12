// HitResolved (session IDL, `HitResultData`) → the app's HitResult (types.ts), which the dice
// cascade and hit breakdown consume. The IDL keeps fixed arrays plus counts; the app keeps only
// the used entries.
import type { Address } from "@solana/kit";
import { enumIndex, type Enum } from "./game";
import type { ActiveState, HitResolved, HitResult, PacketResult, Resonance, StateRoll, TintRoll } from "./types";

const ASPECTS = ["fire", "cold", "lightning", "blood", "blunt", "piercing"];
const STATES = ["burning", "chill", "freeze", "bleeding", "shock", "stun"];
const CARRIERS = ["race", "class", "item"];
const OUTCOMES = ["miss", "hit", "immune"];
const APPLIED = ["none", "added", "refreshed", "froze"];

const aspect = (e: Enum) => enumIndex(e, ASPECTS) as TintRoll["base"];

function stateRoll(s: any): StateRoll {
  return {
    kind: enumIndex(s.kind, STATES) as StateRoll["kind"],
    rolled: s.rolled,
    chanceBp: s.chanceBp,
    roll: s.roll,
    success: s.success,
    applied: enumIndex(s.applied, APPLIED) as StateRoll["applied"],
  };
}

export function toHitResult(r: any): HitResult {
  const tintRolls: TintRoll[] = (r.tintRolls as any[]).slice(0, r.tintRollCount).map((t) => ({
    base: aspect(t.base),
    tint: aspect(t.tint),
    carrier: enumIndex(t.carrier, CARRIERS) as TintRoll["carrier"],
    tier: t.tier,
    chanceBp: t.chanceBp,
    roll: t.roll,
    success: t.success,
  }));
  const resonances: (Resonance | null)[] = (r.resonances as any[]).map((x) =>
    x
      ? {
          base: aspect(x.base),
          tint: aspect(x.tint),
          tier: x.tier,
          carrier: enumIndex(x.carrier, CARRIERS) as Resonance["carrier"],
          id: x.id,
          active: x.active,
          secondTint: x.secondTint ? aspect(x.secondTint) : null,
        }
      : null,
  );
  const packets: PacketResult[] = (r.packets as any[]).slice(0, r.packetCount).map((p) => ({
    aspect: aspect(p.aspect),
    immune: p.immune,
    baseMin: p.baseMin,
    baseMax: p.baseMax,
    increasedBp: p.increasedBp,
    moreBp: p.moreBp,
    weakenBp: p.weakenBp,
    min: p.min,
    max: p.max,
    rolled: p.rolled,
    armor: p.armor,
    resistBp: p.resistBp,
    vulnBp: p.vulnBp,
    afterDefense: p.afterDefense,
    state: stateRoll(p.state),
  }));
  const states: ActiveState[] = (r.states as any[])
    .filter((s) => s.turns > 0)
    .map((s) => ({ kind: enumIndex(s.kind, STATES) as ActiveState["kind"], turns: s.turns, power: s.power, source: s.source }));
  return {
    outcome: enumIndex(r.outcome, OUTCOMES) as HitResult["outcome"],
    d20: r.d20,
    attackTotal: r.attackTotal,
    defense: r.defense,
    crit: r.crit,
    critRoll: r.critRoll,
    tintRolls,
    resonances,
    d100: r.d100,
    packets,
    resonanceStates: (r.resonanceStates as any[]).filter((s) => s.rolled).map(stateRoll),
    states,
    damage: r.damage,
    rollsUsed: r.rollsUsed,
  };
}

export function toHitResolved(ev: any): HitResolved {
  return {
    session: ev.session as Address,
    attacker: ev.attacker,
    target: ev.target,
    seed: ev.seed,
    actionCounter: ev.actionCounter,
    result: toHitResult(ev.result),
  };
}
