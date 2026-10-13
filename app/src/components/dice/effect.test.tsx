import { cleanup, render, screen } from "@testing-library/react";
import { afterEach, describe, expect, it } from "vitest";
import { Effect } from "../../chain/skillcheck";
import type { EffectResolved, EffectRoll } from "../../chain/support";
import { Applied, HitOutcome } from "../../chain/types";
import { effectSteps } from "./cascade";
import { EffectCascade } from "./EffectCascade";

afterEach(cleanup);

const roll = (over: Partial<EffectRoll> = {}): EffectRoll => ({
  outcome: HitOutcome.Hit,
  d20: 0,
  attackTotal: 0,
  defense: 0,
  crit: false,
  critRoll: null,
  chanceBp: 0,
  chanceRoll: null,
  d100: 0,
  amount: 0,
  rollsUsed: 0,
  ...over,
});
const effect = (over: Partial<EffectResolved> = {}): EffectResolved => ({
  session: "s" as never,
  caster: 2,
  target: 0,
  seed: new Uint8Array(32),
  actionCounter: 4,
  skillSlot: 0,
  effect: Effect.Heal,
  rider: false,
  roll: roll(),
  applied: Applied.None,
  hp: 0,
  mods: [],
  ...over,
});

describe("effectSteps", () => {
  it("a heal has no d20: crit roll, heal d100, the HP after", () => {
    const steps = effectSteps(effect({ roll: roll({ critRoll: 9_000, d100: 70, amount: 7 }), hp: 25 }), 500);
    expect(steps.map((s) => s.kind)).toEqual(["crit", "d100", "damage"]);
    expect(steps[0]).toMatchObject({ success: false, target: "needs 95.00+" });
    expect(steps[1]).toMatchObject({ label: "Heal d100", value: "70" });
    expect(steps[2]).toMatchObject({ label: "Healed", value: "+7 hp", target: "25 hp" });
  });

  it("a debuff on a natural 1 is only the d20 and a miss", () => {
    const steps = effectSteps(effect({ effect: Effect.Weakening, roll: roll({ outcome: HitOutcome.Miss, d20: 1, attackTotal: 4, defense: 12 }) }));
    expect(steps.map((s) => s.kind)).toEqual(["d20", "damage"]);
    expect(steps[0]).toMatchObject({ value: "1", success: false, flag: "NAT 1", target: "total 4 vs defense 12" });
    expect(steps[1]).toMatchObject({ label: "Miss", value: "missed" });
  });

  it("a debuff that lands shows its strength and that it was added", () => {
    const steps = effectSteps(
      effect({ effect: Effect.Weakening, applied: Applied.Added, roll: roll({ d20: 15, attackTotal: 18, defense: 12, critRoll: 6_000, d100: 40, amount: 1_800 }) }),
    );
    expect(steps.map((s) => s.kind)).toEqual(["d20", "crit", "d100", "damage"]);
    expect(steps[3]).toMatchObject({ label: "Weakening", value: "Weakening 18%", target: "added" });
  });

  it("a rider rolls its chance, certain on a crit; Freeze has no d100", () => {
    const rolled = effectSteps(effect({ effect: Effect.Freeze, rider: true, applied: Applied.Froze, roll: roll({ chanceBp: 2_500, chanceRoll: 100 }) }));
    expect(rolled.map((s) => s.kind)).toEqual(["chance", "damage"]);
    expect(rolled[0]).toMatchObject({ label: "Freeze chance", success: true });
    expect(rolled[1]).toMatchObject({ value: "Freeze", target: "froze" });
    const certain = effectSteps(effect({ effect: Effect.Freeze, rider: true, applied: Applied.Froze, roll: roll({ crit: true }) }));
    expect(certain[0]).toMatchObject({ kind: "chance", value: "certain" });
  });
});

describe("EffectCascade", () => {
  it("instant mode shows every step under the caster, skill and target", () => {
    render(<EffectCascade effect={effect({ roll: roll({ d100: 50, amount: 6 }), hp: 20 })} casterLabel="Mira" targetLabel="Bran" skillLabel="Mend" instant />);
    expect(screen.getByText("Mira · Mend → Bran")).toBeTruthy();
    expect(screen.getByText("+6 hp")).toBeTruthy();
    expect(screen.getByLabelText("Effect rolls")).toBeTruthy();
  });

  it("a rider is headed by the effect of its skill", () => {
    render(<EffectCascade effect={effect({ effect: Effect.Freeze, rider: true, roll: roll({ crit: true }) })} casterLabel="Ilyra" targetLabel="Wolf 0" skillLabel="Ice Prison" instant />);
    expect(screen.getByText("Ice Prison effect → Wolf 0")).toBeTruthy();
  });
});
