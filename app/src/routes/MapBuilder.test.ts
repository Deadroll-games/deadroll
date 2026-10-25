// Patch 13: the builder's verdict is crates/mapcheck through mapcheck.wasm, the code finalize_map
// runs (crates/mapcheck/wasm), on the registry tables of the mock (the shipped JSON).
import { readFileSync } from "node:fs";
import { resolve } from "node:path";
import { beforeAll, describe, expect, it } from "vitest";
import { presetToLayout, MAP_PRESETS, mockTable } from "../chain/mock";
import { MAPCHECK_MESSAGES, MapcheckWasm, type MapCheckDraft } from "../chain/mapcheckWasm";
import { NO_TABLE } from "../chain/loot";
import { cellByte, cellIndex, Tile } from "../chain/types";
import { mapVerdict } from "./MapBuilder";

const preset = MAP_PRESETS[0]!;
let wasm: MapcheckWasm;
beforeAll(async () => {
  wasm = await MapcheckWasm.instantiate(readFileSync(resolve(import.meta.dirname, "../chain/wasm/mapcheck.wasm")));
  wasm.setTables({ bestiary: mockTable("bestiary"), monsterSkills: mockTable("monster_skills"), rewards: mockTable("rewards"), tileset: mockTable("tileset") });
});

function check(edit: (d: MapCheckDraft) => void = () => {}, skillError = () => null as string | null): string[] {
  const d: MapCheckDraft = { width: preset.width, height: preset.height, tier: preset.tier ?? 1, ...presetToLayout(preset) };
  edit(d);
  return mapVerdict(wasm, d, skillError);
}

describe("mapVerdict", () => {
  it("passes the demo preset", () => {
    expect(check()).toEqual([]);
  });

  it("waits for the module", () => {
    expect(mapVerdict(null, { width: 16, height: 16, tier: 1, ...presetToLayout(preset) })).toEqual(["The map checker is still loading"]);
  });

  it("the entrance and the exit points", () => {
    expect(check((d) => (d.entrance = null))).toEqual([MAPCHECK_MESSAGES.EntranceCount]);
    expect(check((d) => (d.exit = d.entrance))).toEqual([MAPCHECK_MESSAGES.PointOverlap]);
    expect(check((d) => (d.tiles[cellIndex(d.exit!.x, d.exit!.y)] = cellByte(Tile.DeepWater)))).toEqual([MAPCHECK_MESSAGES.PointNotPassable]);
  });

  it("chests and the boss table", () => {
    expect(check((d) => (d.chests[0] = { ...d.chests[0]!, x: 0, y: 0 }))).toEqual([MAPCHECK_MESSAGES.ChestNotPassable]);
    expect(check((d) => (d.bossTable = NO_TABLE))).toEqual([MAPCHECK_MESSAGES.BossTableMissing]);
    expect(check((d) => d.chests.push({ x: 1, y: 2, table: 0 }))).toEqual([MAPCHECK_MESSAGES.TooManyChests]);
    expect(check((d) => (d.minPlayers = 0))).toEqual([MAPCHECK_MESSAGES.MinPlayersOutOfRange]);
  });

  it("names the spawns whose skills break the rule", () => {
    const out = check((d) => (d.spawns[0] = { ...d.spawns[0]!, skills: 0 }), () => "pick 3 actives");
    expect(out[0]).toBe(MAPCHECK_MESSAGES.MonsterSkillCount);
    expect(out.length).toBe(1 + presetToLayout(preset).spawns.length);
  });
});
