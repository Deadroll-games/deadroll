// The shipped combat.wasm against the Rust fixtures (crates/combat/wasm/tests/ts_fixtures.rs):
// the TS encoder writes the same borsh bytes, and the module returns the same sheet and preview.
// Stale? `UPDATE_TS_FIXTURES=1 cargo test -p deadroll-combat-wasm --test ts_fixtures`, then
// `bash crates/combat/wasm/build.sh`.
import { readFileSync } from "node:fs";
import { resolve } from "node:path";
import { beforeAll, describe, expect, it } from "vitest";
import fixtures from "./__fixtures__/combat-wasm.json";
import {
  COMBAT_ABI,
  COMBAT_TABLES_LEN,
  CombatWasm,
  blockedOf,
  encodeAttack,
  encodeHero,
  encodeLine,
  encodeMonster,
  encodeMonsterAttack,
  encodeMonsterPlan,
  encodeSpawnSkills,
  encodeUnitMods,
  partyInputOf,
  percent,
  type AttackRequest,
  type HeroInput,
  type LineRequest,
  type MonsterAttackRequest,
  type MonsterInput,
  type MonsterPlanRequest,
  type SpawnSkillsRequest,
  type UnitModsRequest,
} from "./combatWasm";
import { Tile } from "./types";

const hex = (b: Uint8Array) => Buffer.from(b).toString("hex");

let combat: CombatWasm;
beforeAll(async () => {
  combat = await CombatWasm.instantiate(readFileSync(resolve(import.meta.dirname, "wasm/combat.wasm")));
});

const tables = () => Uint8Array.from(Buffer.from(fixtures.tables, "hex"));

describe("combat wasm", () => {
  it("speaks the fixture's ABI", () => {
    expect(fixtures.abiVersion).toBe(COMBAT_ABI);
  });

  it("hero sheet: same bytes, same derived stats", () => {
    expect(fixtures.sheets.length).toBeGreaterThan(0);
    for (const f of fixtures.sheets) {
      const input = f.input as HeroInput;
      expect(hex(encodeHero(input))).toBe(f.bytes);
      expect(combat.heroSheet(input)).toEqual(f.sheet);
    }
  });

  it("needs the session tables before a preview", async () => {
    const fresh = await CombatWasm.instantiate(readFileSync(resolve(import.meta.dirname, "wasm/combat.wasm")));
    const input = fixtures.attacks[0]!.input as unknown as AttackRequest;
    expect(() => fresh.previewAttack(input)).toThrow("no resonance tables");
    expect(() => fresh.setTables(new Uint8Array(3))).toThrow();
    const garbled = tables();
    garbled[0] = 9;
    expect(() => fresh.setTables(garbled)).toThrow("no resonance tables");
    expect(tables().length).toBe(COMBAT_TABLES_LEN);
  });

  it("monster sheet: same bytes, registry defenses applied", () => {
    expect(fixtures.monsters.length).toBeGreaterThan(0);
    for (const f of fixtures.monsters) {
      const input = f.input as MonsterInput;
      expect(hex(encodeMonster(input))).toBe(f.bytes);
      expect(combat.monsterSheet(input)).toEqual(f.sheet);
    }
  });

  it("attack preview: same bytes, same chances, damage and tints", () => {
    combat.setTables(tables());
    expect(fixtures.attacks.length).toBeGreaterThan(0);
    expect(fixtures.attacks.some((f) => f.preview.tints.length > 0)).toBe(true);
    for (const f of fixtures.attacks) {
      const input = f.input as unknown as AttackRequest;
      expect(hex(encodeAttack(input))).toBe(f.bytes);
      expect(combat.previewAttack(input)).toEqual(f.preview);
    }
  });

  it("an unknown monster is an error, not a wrong number", () => {
    const input = { ...(fixtures.attacks[0]!.input as unknown as AttackRequest) };
    // An absent bestiary record (byte 0 = 0).
    input.target = { ...input.target, record: "00".repeat(48) };
    expect(() => combat.previewAttack(input)).toThrow("unknown monster");
  });

  it("patch 5: a unit's mods with the auras over it, as the session merges them", () => {
    expect(fixtures.unitMods.length).toBeGreaterThan(0);
    for (const f of fixtures.unitMods) {
      const input = f.input as unknown as UnitModsRequest;
      expect(hex(encodeUnitMods(input))).toBe(f.bytes);
      expect(combat.unitMods(input)).toEqual(f.mods);
    }
  });

  it("patch 5: the cells of a line, nearest first; a wall ends it", () => {
    expect(fixtures.lines.length).toBeGreaterThan(0);
    for (const f of fixtures.lines) {
      const input = { ...(f.input as unknown as LineRequest), blocked: Uint8Array.from(Buffer.from(f.input.blocked as string, "hex")) };
      expect(hex(encodeLine(input))).toBe(f.bytes);
      expect(combat.line(input)).toEqual(f.cells);
    }
  });

  it("patch 5: the session's auras and walls as module inputs", () => {
    const aura = { kind: { protection: {} }, aspect: { cold: {} }, powerBp: 2_500, radius: 2, turns: 255, hostile: false, passive: true, selfOnly: false };
    const none = { ...aura, kind: { haste: {} }, aspect: { fire: {} }, powerBp: 0, radius: 0, turns: 0, passive: false };
    const party = partyInputOf({ auras: [[aura, none]], playerPos: [{ x: 3, y: 4 }], aliveMask: 1 });
    expect(party).toEqual({
      auras: [[{ kind: 4, aspect: 1, powerBp: 2_500, radius: 2, turns: 255, hostile: false, passive: true, selfOnly: false }, { kind: 0, aspect: 0, powerBp: 0, radius: 0, turns: 0, hostile: false, passive: false, selfOnly: false }]],
      cells: [[3, 4]],
      alive: 1,
      // No monster bearers unless given (patch 6).
      bearers: [],
    });
    // A wall (cost 0) at (2, 1) on an otherwise open map: bit 1 * 32 + 2.
    const tiles = new Uint8Array(1024);
    tiles[32 + 2] = Tile.Wall;
    const costs = new Uint8Array(8).fill(1);
    costs[Tile.Wall] = 0;
    const blocked = blockedOf(tiles, costs);
    expect(blocked[4]).toBe(1 << 2);
    expect(blocked.reduce((n, b) => n + (b ? 1 : 0), 0)).toBe(1);
    // From (0, 1) east through (1, 1): the wall ends the line before it.
    expect(combat.line({ from: [0, 1], through: [1, 1], reach: 6, width: 16, height: 16, blocked })).toEqual([[1, 1]]);
  });

  it("patch 6: monster skills from their session copy, the spawn check, monster attacks, the AI's plan", () => {
    combat.setTables(tables());
    expect(fixtures.monsterSkills.length).toBeGreaterThan(0);
    for (const f of fixtures.monsterSkills) expect(combat.monsterSkill(f.copy)).toEqual(f.sheet);
    // Every verdict of the builder rule: valid, a bit past the pool, a wrong count.
    expect(new Set(fixtures.spawnSkills.map((f) => f.result.error)).size).toBeGreaterThan(1);
    for (const f of fixtures.spawnSkills) {
      const input = f.input as SpawnSkillsRequest;
      expect(hex(encodeSpawnSkills(input))).toBe(f.bytes);
      expect(combat.spawnSkills(input)).toEqual(f.result);
    }
    expect(fixtures.monsterAttacks.length).toBeGreaterThan(0);
    for (const f of fixtures.monsterAttacks) {
      const input = f.input as unknown as MonsterAttackRequest;
      expect(hex(encodeMonsterAttack(input))).toBe(f.bytes);
      expect(combat.previewMonsterAttack(input)).toEqual(f.preview);
    }
    // Heal, haste, natural attack and nothing to do are all in the fixture.
    expect(fixtures.monsterPlans.some((f) => f.plan.ally) && fixtures.monsterPlans.some((f) => !f.plan.found)).toBe(true);
    for (const f of fixtures.monsterPlans) {
      const input = f.input as unknown as MonsterPlanRequest;
      expect(hex(encodeMonsterPlan(input))).toBe(f.bytes);
      expect(combat.monsterPlan(input)).toEqual(f.plan);
    }
  });

  it("formats percents", () => {
    expect([percent(6_500), percent(925), percent(10_000)]).toEqual(["65%", "9.3%", "100%"]);
  });
});
