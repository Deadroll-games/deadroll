// Patch 13: the builder's verdict is crates/mapcheck through mapcheck.wasm, the code finalize_map
// runs (crates/mapcheck/wasm), on the registry tables of the mock (the shipped JSON).
import { readFileSync } from "node:fs";
import { resolve } from "node:path";
import { beforeAll, describe, expect, it } from "vitest";
import { presetToLayout, MAP_PRESETS, mockTable } from "../chain/mock";
import { MAPCHECK_MESSAGES, MapcheckWasm, type MapCheckDraft } from "../chain/mapcheckWasm";
import { NO_TABLE } from "../chain/loot";
import { Biome, cellByte, cellIndex, cellKind, cellVariant, Tile } from "../chain/types";
import { mapVerdict, resizeLayout } from "./MapBuilder";

const preset = MAP_PRESETS[0]!;
let wasm: MapcheckWasm;
beforeAll(async () => {
  wasm = await MapcheckWasm.instantiate(readFileSync(resolve(import.meta.dirname, "../chain/wasm/mapcheck.wasm")));
  wasm.setTables({ bestiary: mockTable("bestiary"), monsterSkills: mockTable("monster_skills"), rewards: mockTable("rewards"), tileset: mockTable("tileset") });
});

function check(edit: (d: MapCheckDraft) => void = () => {}, skillError = () => null as string | null): string[] {
  const d: MapCheckDraft = { width: preset.width, height: preset.height, tier: preset.tier ?? 1, sizeClass: 0, ...presetToLayout(preset) };
  edit(d);
  return mapVerdict(wasm, d, skillError);
}

describe("mapVerdict", () => {
  it("passes the demo preset", () => {
    expect(check()).toEqual([]);
  });

  it("passes every builder template, one per biome, with the variants the DM picked", () => {
    const templates = MAP_PRESETS.filter((p) => p.biome !== undefined);
    expect(templates.map((p) => p.biome).sort()).toEqual([Biome.Caves, Biome.Forest, Biome.Crypt]);
    for (const p of templates) {
      const layout = presetToLayout(p);
      expect(layout.biome, p.name).toBe(p.biome);
      const d: MapCheckDraft = { width: p.width, height: p.height, tier: p.tier ?? 1, sizeClass: 0, ...layout };
      expect(mapVerdict(wasm, d), p.name).toEqual([]);
      // The variants land in bits 2-0, the tile type stays in bits 7-3.
      const cells = p.rows.flatMap((row, y) => [...row].map((_, x) => layout.tiles[cellIndex(x, y, p.width)]!));
      expect(cells.map(cellVariant).join(""), p.name).toBe(p.variants!.join(""));
      expect(cells.some((c) => cellVariant(c) > 0), p.name).toBe(true);
      expect(cellKind(layout.tiles[cellIndex(0, 0, p.width)]!)).toBe(Tile.Wall);
    }
  });

  it("waits for the module", () => {
    expect(mapVerdict(null, { width: 16, height: 16, tier: 1, ...presetToLayout(preset) })).toEqual(["The map checker is still loading"]);
  });

  it("the entrance and the exit points", () => {
    expect(check((d) => (d.entrance = null))).toEqual([MAPCHECK_MESSAGES.EntranceCount]);
    expect(check((d) => (d.exit = d.entrance))).toEqual([MAPCHECK_MESSAGES.PointOverlap]);
    expect(check((d) => (d.tiles[cellIndex(d.exit!.x, d.exit!.y, d.width)] = cellByte(Tile.DeepWater)))).toEqual([MAPCHECK_MESSAGES.PointNotPassable]);
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

describe("patch D: map sizes", () => {
  it("a resize keeps the top-left cells, walls the new ones and drops what falls outside", () => {
    const layout = presetToLayout(preset);
    const from = { width: preset.width, height: preset.height };
    const big = resizeLayout(layout, from, { width: 72, height: 40 });
    expect(big.tiles.length).toBe(72 * 40);
    expect(big.packs.length).toBe(16);
    for (let y = 0; y < from.height; y++)
      for (let x = 0; x < from.width; x++) expect(big.tiles[cellIndex(x, y, 72)]).toBe(layout.tiles[cellIndex(x, y, from.width)]);
    expect(cellKind(big.tiles[cellIndex(71, 39, 72)]!)).toBe(Tile.Wall);
    expect(big.exit).toEqual(layout.exit);
    // Back to 8x8: the exit, the chests and packs past it go, with their monsters.
    const small = resizeLayout(big, { width: 72, height: 40 }, { width: 8, height: 8 });
    expect(small.tiles.length).toBe(64);
    expect(small.packs.length).toBe(8);
    expect(small.exit).toBeNull();
    expect(small.spawns.every((sp) => small.packs[sp.pack])).toBe(true);
    for (const pk of small.packs) if (pk && pk.visible !== 0xff) expect(small.spawns[pk.visible]!.pack).toBe(small.packs.indexOf(pk));
  });

  it("the demo grown to a Dungeon passes, and needs check_step calls", () => {
    const layout = presetToLayout(preset);
    const grown = resizeLayout(layout, { width: preset.width, height: preset.height }, { width: 143, height: 143 });
    // A hall around the demo: floor inside a wall ring, the demo's east wall opened, the exit far.
    const tiles = grown.tiles.slice();
    for (let y = 1; y < 142; y++) for (let x = 1; x < 142; x++) if (x >= preset.width || y >= preset.height) tiles[cellIndex(x, y, 143)] = cellByte(Tile.StoneFloor);
    for (let y = 1; y < preset.height - 1; y++) tiles[cellIndex(preset.width - 1, y, 143)] = cellByte(Tile.StoneFloor);
    const d: MapCheckDraft = { width: 143, height: 143, tier: preset.tier ?? 1, sizeClass: 3, ...grown, tiles, exit: { x: 141, y: 141 } };
    expect(mapVerdict(wasm, { ...d, secrets: layout.secrets })).toEqual([]);
    expect(wasm.steps(d)).toBeGreaterThan(0);
    expect(wasm.steps({ ...d, width: 144 })).toBeLessThan(0);
  });
});
