// The map builder's view of a spawn (patch 6): the registry pool and the combat module's verdict,
// over the bundled registry tables (the shared fixture of tools/scripts).
import { readFileSync } from "node:fs";
import { resolve } from "node:path";
import { beforeAll, describe, expect, it } from "vitest";
import { CombatWasm } from "./combatWasm";
import { mockTable } from "./mock";
import { SPAWN_ELITE, bestiaryRecord, spawnView } from "./monsters";
import { SKILL_ERRORS } from "./skillcheck";

let combat: CombatWasm;
beforeAll(async () => {
  combat = await CombatWasm.instantiate(readFileSync(resolve(import.meta.dirname, "wasm/combat.wasm")));
});

const registry = () => ({ bestiary: mockTable("bestiary"), skills: mockTable("monster_skills"), rewards: mockTable("rewards") });
/** Bestiary ids in assets/registry/bestiary.json. */
const WOLF = 0;
const CHIEFTAIN = 2;

describe("spawnView", () => {
  it("the wolf's pool, its slots and the default choice", () => {
    const v = spawnView(combat, registry(), { bestiaryId: WOLF, flags: 0, skills: 0 })!;
    expect(v.pool.map((k) => k.name)).toEqual(["Lunge", "Hamstring", "Ragged Bite", "Howl"]);
    expect(v.check!.slots).toEqual([3, 0, 1]);
    // Nothing chosen: the count rule fails; the default choice passes.
    expect(SKILL_ERRORS[v.check!.error - 1]).toBe("MonsterSkillCount");
    const ok = spawnView(combat, registry(), { bestiaryId: WOLF, flags: 0, skills: v.check!.defaultMask })!;
    expect(ok.check!.error).toBe(0);
    // A bit past the pool.
    const past = spawnView(combat, registry(), { bestiaryId: WOLF, flags: 0, skills: v.check!.defaultMask | (1 << 4) })!;
    expect(SKILL_ERRORS[past.check!.error - 1]).toBe("MonsterSkillUnknown");
  });

  it("elite takes one more active skill; a boss is never elite", () => {
    const regular = spawnView(combat, registry(), { bestiaryId: WOLF, flags: 0, skills: 0 })!;
    const elite = spawnView(combat, registry(), { bestiaryId: WOLF, flags: SPAWN_ELITE, skills: 0 })!;
    expect(elite.check!.slots[0]).toBe(regular.check!.slots[0]! + 1);
    expect(spawnView(combat, registry(), { bestiaryId: CHIEFTAIN, flags: 0, skills: 0 })!.boss).toBe(true);
    expect(regular.boss).toBe(false);
  });

  it("an unknown template has no view", () => {
    expect(bestiaryRecord(registry(), 200)).toBeNull();
    expect(spawnView(combat, registry(), { bestiaryId: 200, flags: 0, skills: 0 })).toBeNull();
  });
});
