// The committed mapcheck.wasm against crates/mapcheck/wasm/tests/ts_fixtures.rs: the same request
// bytes for the demo preset and the same verdict for every case.
import { readFileSync } from "node:fs";
import { resolve } from "node:path";
import { beforeAll, describe, expect, it } from "vitest";
import fixtures from "./__fixtures__/mapcheck-wasm.json";
import { MAP_PRESETS, presetSecretsOf, presetToLayout } from "./mock";
import { MAPCHECK_ABI, MAPCHECK_ERRORS, MAPCHECK_MESSAGES, MapcheckWasm, encodeDesign, encodeMapCheck, type MapCheckDraft } from "./mapcheckWasm";
import { Tile, cellByte } from "./types";

const bytes = (h: string) => Uint8Array.from(Buffer.from(h, "hex"));
const hex = (b: Uint8Array) => Buffer.from(b).toString("hex");

let wasm: MapcheckWasm;
beforeAll(async () => {
  wasm = await MapcheckWasm.instantiate(readFileSync(resolve(import.meta.dirname, "wasm/mapcheck.wasm")));
  const t = fixtures.tables;
  wasm.setTables({ bestiary: bytes(t.bestiary), monsterSkills: bytes(t.monster_skills), rewards: bytes(t.rewards), tileset: bytes(t.tileset) });
});

function demo(): MapCheckDraft {
  const p = MAP_PRESETS[0]!;
  return { width: p.width, height: p.height, tier: p.tier ?? 1, ...presetToLayout(p) };
}

describe("mapcheck wasm", () => {
  it("speaks the fixture's ABI", () => {
    expect(fixtures.abiVersion).toBe(MAPCHECK_ABI);
  });

  it("encodes the demo preset as the Rust request", () => {
    const f = fixtures.cases.find((c) => c.name === "demo")!;
    expect(hex(encodeMapCheck(demo()))).toBe(f.input);
  });

  it("gives every case the verdict of crates/mapcheck", () => {
    expect(fixtures.cases.length).toBeGreaterThan(10);
    for (const c of fixtures.cases) expect(wasm.verdict(bytes(c.input)), c.name).toBe(c.verdict);
  });

  it("names the rule from a draft", () => {
    expect(wasm.check(demo())).toBeNull();
    expect(wasm.check({ ...demo(), entrance: null })).toBe("EntranceCount");
    const d = demo();
    const tiles = d.tiles.slice();
    tiles[d.exit!.y * 32 + d.exit!.x] = cellByte(Tile.Chasm);
    expect(wasm.check({ ...d, tiles })).toBe("PointNotPassable");
    expect(wasm.check({ ...d, width: 7 })).toBe("SizeOutOfRange");
  });

  it("patch C: gives every design case the verdict of crates/mapcheck, and encodes the demo's secrets", () => {
    expect(fixtures.designCases.length).toBeGreaterThan(3);
    for (const c of fixtures.designCases) expect(wasm.designVerdict(bytes(c.input)), c.name).toBe(c.verdict);
    const ok = fixtures.designCases[0]!;
    const { declared, secrets } = presetSecretsOf(MAP_PRESETS[0]!);
    expect(hex(encodeDesign(demo(), declared, secrets))).toBe(ok.input);
    expect(wasm.checkDesign(demo(), declared, secrets)).toBeNull();
    expect(wasm.checkDesign(demo(), [0, 0, 0, 0], [])).toBe("TooFewTraps");
  });

  it("has a message for every rule", () => {
    for (const name of MAPCHECK_ERRORS) expect(MAPCHECK_MESSAGES[name].length).toBeGreaterThan(0);
  });
});
