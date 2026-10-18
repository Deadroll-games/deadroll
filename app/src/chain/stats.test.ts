import { describe, expect, it } from "vitest";
import { readFileSync } from "node:fs";
import { resolve } from "node:path";
import { beforeAll } from "vitest";
import fixtures from "./__fixtures__/combat-wasm.json";
import { CombatWasm } from "./combatWasm";
import { RESIST_CAP_BP, deltaText, monsterBaseAp, monsterDefense, playerDefense, speedAp, unitStats, type DefenseProfile } from "./stats";
import { Aspect, Class, ModKind, type TimedMod } from "./types";

const m = (kind: ModKind, powerBp: number, aspect: Aspect = Aspect.Cold, turns = 2): TimedMod => ({ kind, aspect, turns, powerBp, source: 1 });

let combat: CombatWasm;
beforeAll(async () => {
  combat = await CombatWasm.instantiate(readFileSync(resolve(import.meta.dirname, "wasm/combat.wasm")));
});

const NONE = [0, 0, 0, 0, 0, 0];
/** The wolf's registry record at level 1, as the fixture holds it (resists Cold). */
const wolfRecord = () => fixtures.monsters.find((m) => m.sheet.kin === 0 && m.input.level === 1 && m.input.flags === 0)!.input.record;
/** A record as bytes with byte `at` set to `v`. */
const withByte = (hex: string, at: number, v: number) => {
  const b = Uint8Array.from((hex.match(/../g) ?? []).map((h) => parseInt(h, 16)));
  b[at] = v;
  return b;
};
/** Bestiary record bytes (`deadroll_rules::monster::record`): present, resistances. */
const PRESENT = 0;
const RESIST = 10;

describe("stats", () => {
  it("monster defenses come from the combat module and the registry record", () => {
    const wolf = monsterDefense(combat, wolfRecord(), 1)!;
    expect(wolf.resist[Aspect.Cold]).toBeGreaterThan(0);
    expect(wolf.resist.filter((r) => r > 0)).toHaveLength(1);
    // The same record without its resistances resists nothing.
    expect(monsterDefense(combat, withByte(wolfRecord(), RESIST, 0), 1)!.resist).toEqual(NONE);
    // An absent record (an unknown monster) is no profile, not a wrong number.
    expect(monsterDefense(combat, withByte(wolfRecord(), PRESENT, 0), 1)).toBeNull();
  });

  it("player defenses", () => {
    // rules.rs test: warrior with Dex 10 has defense 12, armor 30.
    // Class armor, then the starter kit on top (sword + leather armor 10, staff + robe 2).
    expect(playerDefense(Class.Warrior, [10, 10, 10, 10])).toEqual({ defense: 12, armor: 20, resist: NONE });
    const kit = (weapon: number, body: number) => [weapon, 0xff, 0xff, body, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff];
    expect(playerDefense(Class.Warrior, [10, 10, 10, 10], kit(0, 3))).toEqual({ defense: 12, armor: 30, resist: NONE });
    expect(playerDefense(Class.Mage, [5, 7, 13, 5], kit(1, 4)).armor).toBe(5);
    expect(playerDefense(Class.Support, [5, 8, 12, 5], kit(2, 5)).armor).toBe(5);
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
  const wolfProfile: DefenseProfile = { defense: 10, armor: 5, resist: [0, 5_000, 0, 0, 0, 0] };
  const wolf = { hp: 12, hpMax: 15, apBase: 6, apNow: null, profile: wolfProfile };

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
