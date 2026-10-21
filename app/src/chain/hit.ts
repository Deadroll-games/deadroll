// HitResolved (session IDL, `HitResultData`) → the app's HitResult (types.ts), which the dice
// cascade and hit breakdown consume. The IDL keeps fixed arrays plus counts; the app keeps only
// the used entries.
import type { Address } from "@solana/kit";
import { ItemBase } from "@deadroll/sdk/session";
import { optional, variantKey } from "./game";
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

function activeState(s: any): ActiveState {
  return { kind: s.kind as StateKind, turns: s.turns, power: s.power, source: s.source };
}

function accuracyBuff(b: any): AccuracyBuff {
  return { aspect: b ? aspect(b.aspect) : (0 as AccuracyBuff["aspect"]), turns: b?.turns ?? 0, bonus: b?.bonus ?? 0 };
}

const aspect = (e: number) => e as TintRoll["base"];

function stateRoll(s: any): StateRoll {
  return {
    kind: s.kind as StateRoll["kind"],
    rolled: s.rolled,
    chanceBp: s.chanceBp,
    roll: s.roll,
    success: s.success,
    applied: s.applied as StateRoll["applied"],
    resistBp: s.resistBp,
  };
}

export function toHitResult(r: any): HitResult {
  const tintRolls: TintRoll[] = (r.tintRolls as any[]).slice(0, r.tintRollCount).map((t) => ({
    base: aspect(t.base),
    tint: aspect(t.tint),
    carrier: t.carrier as TintRoll["carrier"],
    tier: t.tier,
    chanceBp: t.chanceBp,
    roll: t.roll,
    success: t.success,
  }));
  const resonances: (Resonance | null)[] = (r.resonances as any[]).map((o) => {
    const x = optional<any>(o);
    const secondTint = x ? optional<number>(x.secondTint) : null;
    return x
      ? {
          base: aspect(x.base),
          tint: aspect(x.tint),
          tier: x.tier,
          carrier: x.carrier as Resonance["carrier"],
          id: x.id,
          active: x.active,
          secondTint: secondTint === null ? null : aspect(secondTint),
          secondTier: x.secondTier ?? 0,
          secondCarrier: (x.secondCarrier ? x.secondCarrier : 0) as Resonance["carrier"],
          generated: x.generated ?? false,
        }
      : null;
  });
  const resOps: OpResult[][] = ((r.resOps ?? []) as any[][]).map((slot) =>
    slot
      .map((o) => ({
        kind: o.kind as OpResult["kind"],
        critOnly: o.critOnly,
        arg: o.arg,
        origin: o.origin as OpResult["origin"],
        tint: aspect(o.tint),
        carrier: o.carrier as OpResult["carrier"],
        tier: o.tier,
        acted: o.acted,
        state: stateRoll(o.state),
        amount: o.amount,
        amountMax: o.amountMax,
      }))
      .filter((o) => o.kind !== 0),
  );
  const jumps: Jump[] = ((r.jumps ?? []) as any[]).slice(0, r.jumpCount ?? 0).map((j) => ({
    mode: j.mode as Jump["mode"],
    state: activeState(j.state),
    filter: j.filter as StateKind,
    limit: j.limit,
    base: aspect(j.base),
  }));
  const targetMods: TimedMod[] = ((r.targetMods ?? []) as any[]).slice(0, r.targetModCount ?? 0).map((m) => ({
    kind: m.kind as TimedMod["kind"],
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
    outcome: r.outcome as HitResult["outcome"],
    d20: r.d20,
    attackTotal: r.attackTotal,
    defense: r.defense,
    crit: r.crit,
    critRoll: optional<number>(r.critRoll),
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
      kind: h.kind as StateKind,
      applied: h.applied as Applied,
      states: (h.states as any[]).filter((s) => s.turns > 0).map(activeState),
    })),
    moved: ev.moved ? { from: cell(ev.movedFrom), to: cell(ev.movedTo) } : null,
    ignited: (ev.ignited as any[]).slice(0, ev.ignitedCount).map(cell),
    ignitePower: ev.ignitePower,
    untilRound: ev.untilRound,
    accuracy: accuracyBuff(ev.accuracy),
  };
}

const weaponKey = (base: number | null): string | null => (base === null ? null : variantKey(ItemBase, base) || null);

const itemSide = (x: any): ItemBonusView => ({
  weapon: x?.weapon ? weaponKey(optional<number>(x.weapon)) : null,
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
