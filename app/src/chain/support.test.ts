// @vitest-environment node
import { describe, expect, it } from "vitest";
import { textBytes, type SkillRecordAcc, type TimedModAcc } from "./game";
import { Category, Effect, Form, Purpose, Source, inReach, makeBlocks, skillSide, usableInCombat } from "./skillcheck";
import { activeMods, combatSkill, effectText, modLabel, modTurnsLeft, modsAt, modsTickText, type EffectResolved } from "./support";
import { Applied, Aspect, HitOutcome, ModKind } from "./types";

const e = (k: string) => ({ [k]: {} });
const mod = (kind: string, turns: number, powerBp = 2_500): TimedModAcc => ({ kind: e(kind), aspect: e("lightning"), turns, powerBp, source: 2 });
const empty = (): TimedModAcc[] => ["haste", "slow", "empowerment", "weakening", "protection", "vulnerability"].map((k) => mod(k, 0, 0));

describe("skill side and reach", () => {
  it("heals and buffs target allies, damage and debuffs monsters", () => {
    expect([Purpose.Buff, Purpose.Damage, Purpose.Debuff].map(skillSide)).toEqual(["ally", "enemy", "enemy"]);
  });

  it("allies may share the cell, enemies need distance 1; strike 1, projectile 6", () => {
    expect([0, 1, 2].map((d) => inReach("ally", Form.Strike, d))).toEqual([true, true, false]);
    expect([0, 1, 6, 7].map((d) => inReach("enemy", Form.Projectile, d))).toEqual([false, true, true, false]);
  });

  it("every active skill with its effect is usable; reactive ones are not", () => {
    const { Active, Reactive } = Category;
    expect(usableInCombat(makeBlocks(Active, Purpose.Buff, Source.Magical, Form.Projectile, [Aspect.Blood], { effect: Effect.Heal }))).toBe(true);
    expect(usableInCombat(makeBlocks(Active, Purpose.Debuff, Source.Magical, Form.Projectile, [Aspect.Fire], { effect: Effect.Vulnerability }))).toBe(true);
    expect(usableInCombat(makeBlocks(Active, Purpose.Buff, Source.Magical, Form.Projectile, [Aspect.Fire]))).toBe(false);
    expect(usableInCombat(makeBlocks(Reactive, Purpose.Damage, Source.Combat, Form.Strike, [Aspect.Piercing]))).toBe(false);
  });

  it("combatSkill reads a SkillRecord", () => {
    const rec = {
      blocks: {
        category: e("active"),
        purpose: e("buff"),
        source: e("magical"),
        form: e("projectile"),
        target: e("single"),
        aspects: [e("blood"), e("fire"), e("fire")],
        aspectCount: 1,
        effect: e("heal"),
        trigger: e("none"),
        auraRadius: 0,
      },
      sharesBp: [10_000, 0, 0],
      powerBp: 10_000,
      fusion: 0,
      cooldown: 1,
      name: textBytes("Mend", 24),
    } as unknown as SkillRecordAcc;
    expect(combatSkill(3, rec)).toEqual({ slot: 3, name: "Mend", usable: true, purpose: Purpose.Buff, side: "ally", form: Form.Projectile, effect: Effect.Heal });
  });
});

describe("mods", () => {
  it("only slots with turns left are active; the current turn is not counted", () => {
    const mods = empty();
    mods[ModKind.Haste] = mod("haste", 3);
    mods[ModKind.Weakening] = mod("weakening", 1, 1_800);
    const live = activeMods(mods);
    expect(live.map((m) => [m.kind, m.turns])).toEqual([
      [ModKind.Haste, 3],
      [ModKind.Weakening, 1],
    ]);
    expect(live.map(modTurnsLeft)).toEqual([2, 0]);
    expect(live.map(modLabel)).toEqual(["Haste +25% AP (2)", "Weakening -18% damage (0)"]);
  });

  it("Vulnerability reads as more damage taken, from its aspect only", () => {
    const mods = empty();
    mods[ModKind.Vulnerability] = { ...mod("vulnerability", 2), aspect: e("fire") };
    mods[ModKind.Protection] = mod("protection", 3, 2_000);
    expect(activeMods(mods).map(modLabel)).toEqual(["Protection +20% resist (2)", "Vulnerability +25% Fire damage taken (1)"]);
  });

  it("modsAt takes the latest ModsTicked / EffectResolved on the unit before the event", () => {
    const vuln = empty();
    vuln[ModKind.Vulnerability] = mod("vulnerability", 2);
    const tick = empty();
    const events = [
      { name: "EffectResolved", data: { target: 5, mods: vuln } },
      { name: "ModsTicked", data: { unit: 0, mods: tick } },
      { name: "HitResolved", data: {} },
      { name: "EffectResolved", data: { target: 5, mods: empty() } },
    ];
    expect(modsAt(events, 2, 5, []).map((m) => m.kind)).toEqual([ModKind.Vulnerability]);
    expect(modsAt(events, 2, 0, [])).toEqual([]);
    // No event on the unit before the hit: the fallback (account now).
    const now = activeMods(vuln);
    expect(modsAt(events, 2, 6, now)).toBe(now);
  });

  it("ModsTicked names what wore off and the AP of a hasted turn", () => {
    const mods = empty();
    mods[ModKind.Haste] = mod("haste", 2);
    expect(modsTickText({ expired: 1 << ModKind.Protection, mods, actionPoints: 5 }, "Mira")).toBe("Mira: Protection wore off; Haste: 5 AP this turn");
    expect(modsTickText({ expired: 0, mods: empty(), actionPoints: 4 }, "Mira")).toBeNull();
  });
});

describe("effectText", () => {
  const base: EffectResolved = {
    session: "s" as never,
    caster: 2,
    target: 0,
    seed: new Uint8Array(32),
    actionCounter: 1,
    skillSlot: 0,
    effect: Effect.Heal,
    rider: false,
    roll: { outcome: HitOutcome.Hit, d20: 0, attackTotal: 0, defense: 0, crit: false, critRoll: 9_000, chanceBp: 0, chanceRoll: null, d100: 70, amount: 7, rollsUsed: 2 },
    applied: Applied.None,
    hp: 25,
    mods: [],
  };

  it("a heal on an ally", () => {
    expect(effectText(base, "Mira", "Bran", "Mend")).toBe("Mira uses Mend on Bran: d100 70, +7 hp (25 hp)");
  });

  it("a debuff miss and a self buff", () => {
    const sap = { ...base, target: 4, effect: Effect.Weakening, roll: { ...base.roll, outcome: HitOutcome.Miss, d20: 1, attackTotal: 4, defense: 12, d100: 0, amount: 0 } };
    expect(effectText(sap, "Mira", "Wolf 0", "Sap")).toBe("Mira uses Sap on Wolf 0: d20 1 (4 vs 12), missed");
    const quicken = { ...base, target: 2, effect: Effect.Haste, applied: Applied.Added, roll: { ...base.roll, crit: true, d100: 50, amount: 3_000 } };
    expect(effectText(quicken, "Mira", "Mira", "Quicken")).toBe("Mira uses Quicken on self: d100 50, Haste 30% CRIT (added)");
  });

  it("the effect of a damage skill", () => {
    const ice = { ...base, caster: 1, target: 4, rider: true, effect: Effect.Freeze, applied: Applied.Froze, roll: { ...base.roll, critRoll: null, chanceBp: 2_500, chanceRoll: 100, d100: 0, amount: 0 } };
    expect(effectText(ice, "Ilyra", "Wolf 0", "Ice Prison")).toBe("Ice Prison effect on Wolf 0: chance 25%, Freeze (froze)");
  });
});
