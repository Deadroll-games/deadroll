// The shipped combat.wasm against the Rust fixtures (crates/combat/wasm/tests/ts_fixtures.rs):
// the TS encoder writes the same borsh bytes, and the module returns the same sheet and preview.
// Stale? `UPDATE_TS_FIXTURES=1 cargo test -p deadroll-combat-wasm --test ts_fixtures`, then
// `bash crates/combat/wasm/build.sh`.
import { readFileSync } from "node:fs";
import { resolve } from "node:path";
import { beforeAll, describe, expect, it } from "vitest";
import fixtures from "./__fixtures__/combat-wasm.json";
import { COMBAT_ABI, CombatWasm, encodeAttack, encodeHero, percent, type AttackRequest, type HeroInput } from "./combatWasm";

const hex = (b: Uint8Array) => Buffer.from(b).toString("hex");

let combat: CombatWasm;
beforeAll(async () => {
  combat = await CombatWasm.instantiate(readFileSync(resolve(import.meta.dirname, "wasm/combat.wasm")));
});

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

  it("attack preview: same bytes, same chances and damage", () => {
    expect(fixtures.attacks.length).toBeGreaterThan(0);
    for (const f of fixtures.attacks) {
      const input = f.input as unknown as AttackRequest;
      expect(hex(encodeAttack(input))).toBe(f.bytes);
      expect(combat.previewAttack(input)).toEqual(f.preview);
    }
  });

  it("an unknown monster is an error, not a wrong number", () => {
    const input = { ...(fixtures.attacks[0]!.input as unknown as AttackRequest) };
    input.target = { ...input.target, bestiaryId: 200 };
    expect(() => combat.previewAttack(input)).toThrow("unknown monster");
  });

  it("formats percents", () => {
    expect([percent(6_500), percent(925), percent(10_000)]).toEqual(["65%", "9.3%", "100%"]);
  });
});
