import { describe, expect, it } from "vitest";
import { MONSTER_DEFENSE, RESIST_CAP_BP, deltaText, monsterBaseAp, playerDefense, speedAp, unitStats } from "./stats";
import { Aspect, Class, ModKind, type TimedMod } from "./types";

const m = (kind: ModKind, powerBp: number, aspect: Aspect = Aspect.Cold, turns = 2): TimedMod => ({ kind, aspect, turns, powerBp, source: 1 });

describe("stats mirror of rules.rs", () => {
  it("monster and player defenses", () => {
    expect(MONSTER_DEFENSE[0]).toEqual({ defense: 10, armor: 5, resist: [Aspect.Cold, 5_000] });
    expect(MONSTER_DEFENSE[2]).toEqual({ defense: 13, armor: 40, resist: [Aspect.Piercing, 5_000] });
    expect(Object.keys(MONSTER_DEFENSE)).toHaveLength(5);
    // rules.rs test: warrior with Dex 10 has defense 12, armor 30.
    expect(playerDefense(Class.Warrior, [10, 10, 10, 10])).toEqual({ defense: 12, armor: 30, resist: null });
    expect(playerDefense(Class.Mage, [5, 7, 13, 5]).armor).toBe(5);
    expect([monsterBaseAp(0), monsterBaseAp(6), monsterBaseAp(200)]).toEqual([1, 6, 12]);
  });

  it("speedAp: Haste and Slow are an opposing pair (support.rs tests)", () => {
    expect(speedAp(4, [])).toBe(4);
    expect(speedAp(4, [m(ModKind.Haste, 5_000)])).toBe(6);
    expect(speedAp(4, [m(ModKind.Slow, 10_000)])).toBe(2);
    expect(speedAp(5, [m(ModKind.Haste, 4_000), m(ModKind.Slow, 4_000)])).toBe(5);
    expect(speedAp(1, [m(ModKind.Slow, 4_000_000_000)])).toBe(1);
    // An expired slot does nothing.
    expect(speedAp(4, [m(ModKind.Haste, 5_000, Aspect.Fire, 0)])).toBe(4);
  });
});

describe("unitStats", () => {
  const wolf = { hp: 12, hpMax: 15, apBase: 6, apNow: null, profile: MONSTER_DEFENSE[0]! };

  it("no mods: base equals final", () => {
    const s = unitStats({ ...wolf, mods: [] });
    expect(s.ap).toEqual({ base: 6, final: 6, by: [], now: null });
    expect([s.defense, s.armor]).toEqual([10, 5]);
    expect(s.resist[Aspect.Cold]).toEqual({ aspect: Aspect.Cold, base: 5_000, final: 5_000, by: [] });
    expect(s.vuln.every((v) => v.final === 0)).toBe(true);
    expect(deltaText("AP", s.ap)).toBe("AP 6");
  });

  it("Vulnerability only on its aspect, Protection on every resist (capped)", () => {
    const s = unitStats({ ...wolf, mods: [m(ModKind.Vulnerability, 2_500, Aspect.Fire), m(ModKind.Protection, 2_000)] });
    expect(s.vuln[Aspect.Fire]).toEqual({ aspect: Aspect.Fire, base: 0, final: 2_500, by: ["Vulnerability"] });
    expect(s.vuln[Aspect.Blunt]!.final).toBe(0);
    expect(s.resist[Aspect.Cold]).toMatchObject({ base: 5_000, final: 7_000, by: ["Protection"] });
    expect(s.resist[Aspect.Fire]).toMatchObject({ base: 0, final: 2_000 });
    const wall = unitStats({ ...wolf, mods: [m(ModKind.Protection, 40_000)] });
    expect(wall.resist[Aspect.Cold]!.final).toBe(RESIST_CAP_BP);
  });

  it("Haste on AP, Empowerment and Weakening on outgoing damage", () => {
    const s = unitStats({ ...wolf, apBase: 3, apNow: 2, mods: [m(ModKind.Haste, 3_400), m(ModKind.Empowerment, 3_000), m(ModKind.Weakening, 2_000)] });
    expect(s.ap).toEqual({ base: 3, final: 4, by: ["Haste"], now: 2 });
    expect(deltaText("AP", s.ap)).toBe("AP 3 -> 4 (Haste)");
    expect([s.increasedBp, s.weakenBp]).toEqual([3_000, 2_000]);
    expect(s.mods).toHaveLength(3);
  });
});
