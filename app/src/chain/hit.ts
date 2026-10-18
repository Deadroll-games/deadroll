// HitResolved (session IDL, `HitResultData`) → the app's HitResult (types.ts), which the dice
// cascade and hit breakdown consume. The IDL keeps fixed arrays plus counts; the app keeps only
// the used entries.
import type { Address } from "@solana/kit";
import { enumIndex, type Enum } from "./game";
import type {
  AccuracyBuff,
  ActiveState,
  Applied,
  Cell,
  HitItems,
  HitResolved,
  HitResult,
  ItemBonusView,
  Jump,
  OpResult,
  PacketResult,
  Resonance,
  ResonanceSpread,
  StateKind,
  StateRoll,
  TimedMod,
  TintRoll,
} from "./types";

const ASPECTS = ["fire", "cold", "lightning", "blood", "blunt", "piercing"];
const STATES = ["burning", "chill", "freeze", "bleeding", "shock", "stun", "penetration"];
const CARRIERS = ["race", "class", "item"];
const OUTCOMES = ["miss", "hit", "immune"];
const APPLIED = ["none", "added", "refreshed", "froze"];
const OP_KINDS = [
  "none",
  "multiplier",
  "stateOnCrit",
  "extraStateChance",
  "linked",
  "linkedChance",
  "linkedMod",
  "nativeTurns",
  "nativePower",
  "nativeFaster",
  "nativePowerArmor",
  "thawDamage",
  "removeState",
  "jump",
  "jumpChain",
  "jumpLine",
  "knockback",
  "igniteCells",
  "addPacket",
  "penetrate",
  "accuracyBuff",
];
const ORIGINS = ["pair", "template", "trait"];
const JUMP_MODES = ["adjacent", "chain", "line"];
const MOD_KINDS = ["haste", "slow", "empowerment", "weakening", "protection", "vulnerability"];

function activeState(s: any): ActiveState {
  return { kind: enumIndex(s.kind, STATES) as StateKind, turns: s.turns, power: s.power, source: s.source };
}

function accuracyBuff(b: any): AccuracyBuff {
  return { aspect: b ? aspect(b.aspect) : (0 as AccuracyBuff["aspect"]), turns: b?.turns ?? 0, bonus: b?.bonus ?? 0 };
}

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
          secondTier: x.secondTier ?? 0,
          secondCarrier: (x.secondCarrier ? enumIndex(x.secondCarrier, CARRIERS) : 0) as Resonance["carrier"],
          generated: x.generated ?? false,
        }
      : null,
  );
  const resOps: OpResult[][] = ((r.resOps ?? []) as any[][]).map((slot) =>
    slot
      .map((o) => ({
        kind: enumIndex(o.kind, OP_KINDS) as OpResult["kind"],
        critOnly: o.critOnly,
        arg: o.arg,
        origin: enumIndex(o.origin, ORIGINS) as OpResult["origin"],
        tint: aspect(o.tint),
        carrier: enumIndex(o.carrier, CARRIERS) as OpResult["carrier"],
        tier: o.tier,
        acted: o.acted,
        state: stateRoll(o.state),
        amount: o.amount,
        amountMax: o.amountMax,
      }))
      .filter((o) => o.kind !== 0),
  );
  const jumps: Jump[] = ((r.jumps ?? []) as any[]).slice(0, r.jumpCount ?? 0).map((j) => ({
    mode: enumIndex(j.mode, JUMP_MODES) as Jump["mode"],
    state: activeState(j.state),
    filter: enumIndex(j.filter, STATES) as StateKind,
    limit: j.limit,
    base: aspect(j.base),
  }));
  const targetMods: TimedMod[] = ((r.targetMods ?? []) as any[]).slice(0, r.targetModCount ?? 0).map((m) => ({
    kind: enumIndex(m.kind, MOD_KINDS) as TimedMod["kind"],
    aspect: aspect(m.aspect),
    turns: m.turns,
    powerBp: m.powerBp,
    source: m.source,
  }));
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
    penAttackerBp: p.penAttackerBp ?? 0,
    penStateBp: p.penStateBp ?? 0,
    vulnShockBp: p.vulnShockBp ?? 0,
  }));
  const states: ActiveState[] = (r.states as any[]).filter((s) => s.turns > 0).map(activeState);
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
    resOps,
    jumps,
    knockback: r.knockback ?? 0,
    ignitePower: r.ignitePower ?? 0,
    targetMods,
    accuracyBuff: accuracyBuff(r.accuracyBuff),
    landed: r.landed ?? 0,
  };
}

const cell = (c: any): Cell => ({ x: c.x, y: c.y });

/** The session `ResonanceSpread` event. */
export function toResonanceSpread(ev: any): ResonanceSpread {
  return {
    session: ev.session as Address,
    actionCounter: ev.actionCounter,
    attacker: ev.attacker,
    target: ev.target,
    hits: (ev.hits as any[]).slice(0, ev.hitCount).map((h) => ({
      monster: h.monster,
      kind: enumIndex(h.kind, STATES) as StateKind,
      applied: enumIndex(h.applied, APPLIED) as Applied,
      states: (h.states as any[]).filter((s) => s.turns > 0).map(activeState),
    })),
    moved: ev.moved ? { from: cell(ev.movedFrom), to: cell(ev.movedTo) } : null,
    ignited: (ev.ignited as any[]).slice(0, ev.ignitedCount).map(cell),
    ignitePower: ev.ignitePower,
    untilRound: ev.untilRound,
    accuracy: accuracyBuff(ev.accuracy),
  };
}

const itemSide = (x: any): ItemBonusView => ({
  weapon: x?.weapon ? (Object.keys(x.weapon)[0] ?? null) : null,
  armor: x?.armor ?? 0,
  defense: x?.defense ?? 0,
  attack: x?.attack ?? 0,
});

/** The worn items of both sides (HitResolved.items); none for an event from before items. */
export function toHitItems(items: any): HitItems {
  return { attacker: itemSide(items?.attacker), defender: itemSide(items?.defender) };
}

export function toHitResolved(ev: any): HitResolved {
  return {
    session: ev.session as Address,
    attacker: ev.attacker,
    target: ev.target,
    seed: ev.seed,
    actionCounter: ev.actionCounter,
    result: toHitResult(ev.result),
    items: toHitItems(ev.items),
  };
}
