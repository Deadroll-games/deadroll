// The builder's quick checks before any transaction (the authority is finalize_map).
import { describe, expect, it } from "vitest";
import { presetToLayout, MAP_PRESETS, mockTable } from "../chain/mock";
import { NO_TABLE, TABLE_ERROR_MESSAGES, rewardTables } from "../chain/loot";
import { cellIndex, Tile } from "../chain/types";
import { quickCheck } from "./MapBuilder";

const preset = MAP_PRESETS[0]!;
const tables = rewardTables(mockTable("rewards"));

function check(edit: (l: ReturnType<typeof presetToLayout>) => void): string[] {
  const l = presetToLayout(preset);
  edit(l);
  const loot = { tier: preset.tier ?? 1, minPlayers: l.minPlayers, chests: l.chests, bossTable: l.bossTable, tables };
  return quickCheck(l.tiles, preset.width, preset.height, l.spawns, () => null, loot);
}

describe("quickCheck: chests (patch 11)", () => {
  it("passes the demo preset with its chest and boss table", () => {
    expect(check(() => {})).toEqual([]);
  });

  it("refuses a chest on a wall, on the entrance and on a spawn", () => {
    expect(check((l) => (l.chests[0] = { ...l.chests[0]!, x: 0, y: 0 }))).toContain("Chest 0 must stand on a free walkable tile");
    expect(check((l) => (l.tiles[cellIndex(l.chests[0]!.x, l.chests[0]!.y)] = Tile.Entrance))).toContain("Chest 0 must stand on a free walkable tile");
    expect(check((l) => (l.chests[0] = { ...l.chests[0]!, x: l.spawns[0]!.x, y: l.spawns[0]!.y }))).toContain("Chest 0 stands on a spawn");
  });

  it("reports the table rules", () => {
    expect(check((l) => (l.bossTable = NO_TABLE))).toContain(TABLE_ERROR_MESSAGES.BossTableMissing);
    expect(check((l) => l.chests.push({ x: 1, y: 2, table: 0 }))).toContain(TABLE_ERROR_MESSAGES.TooManyChests);
    expect(check((l) => (l.minPlayers = 0))).toContain(TABLE_ERROR_MESSAGES.MinPlayersOutOfRange);
  });
});
