// Mock data for VITE_DATA=mock and the /dev pages. Shapes match chain/types.ts exactly.
import demoMap from "../../../assets/registry/maps/demo_16x16.json";
import cavesTemplate from "../../../assets/registry/maps/caves_ruins_16x16.json";
import forestTemplate from "../../../assets/registry/maps/forest_16x16.json";
import cryptTemplate from "../../../assets/registry/maps/crypt_16x16.json";
import registryTables from "../../../tools/scripts/lib/__fixtures__/registry-tables.json";
import { NO_TABLE, type Chest } from "./loot";
import {
  Applied,
  Aspect,
  Biome,
  Carrier,
  Class,
  HitOutcome,
  MAP_CELLS,
  OpKind,
  OpOrigin,
  Race,
  StateKind,
  Tile,
  cellByte,
  cellIndex,
  type Cell,
  type HitResult,
  type PacketResult,
  type Spawn,
  type StateRoll,
} from "./types";

/** Preset row characters (crates/rules/tests/demo_preset.rs): `E` and `X` are a stone floor with the
 * entrance or the exit point. */
const TILE_CHARS: Record<string, Tile> = {
  " ": Tile.Void,
  ".": Tile.StoneFloor,
  E: Tile.StoneFloor,
  X: Tile.StoneFloor,
  "=": Tile.WoodenFloor,
  ",": Tile.Dirt,
  "#": Tile.Wall,
  D: Tile.Door,
  "~": Tile.ShallowWater,
  W: Tile.DeepWater,
  ">": Tile.Stairs,
  "%": Tile.Rubble,
  _: Tile.Chasm,
};

export type MapPresetData = {
  name: string;
  width: number;
  height: number;
  /** Map tier (monster levels and limits); 1 when absent. */
  tier?: number;
  /** Patch 11: the map's minimum players (1 when absent), the boss table and the chests. */
  min_players?: number;
  /** The DM's chest radius (3 when absent) and time to settle in seconds (1 h when absent). */
  chest_radius?: number;
  settle_timeout_secs?: number;
  boss_table?: number;
  chests?: Chest[];
  /** The art set (`Biome`), Caves when absent. */
  biome?: number;
  rows: string[];
  /** One digit 0-7 per cell: the variant the DM picked (bits 2-0 of the cell byte), 0 when absent. */
  variants?: string[];
  spawns: {
    bestiary_id: number;
    x: number;
    y: number;
    level: number;
    skills: number;
    flags: number;
    aspects: number;
    pack: number;
  }[];
};

/** Converts a preset from assets/registry/maps into MapLayout cells (stride 32), the entrance and the
 * exit, spawns and chests. */
export function presetToLayout(p: MapPresetData): {
  tiles: Uint8Array;
  entrance: Cell | null;
  exit: Cell | null;
  spawns: Spawn[];
  chests: Chest[];
  bossTable: number;
  minPlayers: number;
  chestRadius: number;
  settleTimeoutSecs: number;
  biome: number;
} {
  const tiles = new Uint8Array(MAP_CELLS).fill(cellByte(Tile.Wall));
  let entrance: Cell | null = null;
  let exit: Cell | null = null;
  p.rows.forEach((row, y) => {
    for (let x = 0; x < row.length; x++) {
      const ch = row[x] ?? "#";
      if (ch === "E") entrance = { x, y };
      if (ch === "X") exit = { x, y };
      tiles[cellIndex(x, y)] = cellByte(TILE_CHARS[ch] ?? Tile.Wall, Number(p.variants?.[y]?.[x] ?? 0));
    }
  });
  const spawns = p.spawns.map((s) => ({
    bestiaryId: s.bestiary_id,
    x: s.x,
    y: s.y,
    level: s.level,
    skills: s.skills,
    flags: s.flags,
    aspects: s.aspects,
    pack: s.pack,
  }));
  const chests = (p.chests ?? []).map((c) => ({ x: c.x, y: c.y, table: c.table }));
  return {
    biome: p.biome ?? Biome.Caves,
    tiles,
    entrance,
    exit,
    spawns,
    chests,
    bossTable: p.boss_table ?? NO_TABLE,
    minPlayers: p.min_players ?? 1,
    chestRadius: p.chest_radius ?? 3,
    settleTimeoutSecs: p.settle_timeout_secs ?? 3600,
  };
}

/** The demo map, then one builder template per biome (site only, never uploaded): the DM loads one,
 * edits it and publishes it as their own map. */
export const MAP_PRESETS: MapPresetData[] = [demoMap, cavesTemplate, forestTemplate, cryptTemplate] as MapPresetData[];
export const DEMO_MAP = presetToLayout(MAP_PRESETS[0]!);

const NO_STATE: StateRoll = {
  kind: StateKind.Burning,
  rolled: false,
  chanceBp: 0,
  roll: 0,
  success: false,
  applied: Applied.None,
};

function packet(p: Partial<PacketResult> & Pick<PacketResult, "aspect">): PacketResult {
  return {
    immune: false,
    baseMin: 0n,
    baseMax: 0n,
    increasedBp: 0,
    moreBp: 0,
    weakenBp: 0,
    min: 0n,
    max: 0n,
    rolled: 0n,
    armor: 0,
    resistBp: 0,
    vulnBp: 0,
    afterDefense: 0n,
    state: NO_STATE,
    penAttackerBp: 0,
    penStateBp: 0,
    vulnShockBp: 0,
    ...p,
  };
}

const NO_STATE_ROLL: StateRoll = { kind: StateKind.Burning, rolled: false, chanceBp: 0, roll: 0, success: false, applied: Applied.None };

/** A hit with no resonance effects. */
const NO_RESONANCE = {
  resOps: [[], [], []],
  jumps: [],
  knockback: 0,
  ignitePower: 0,
  targetMods: [],
  accuracyBuff: { aspect: Aspect.Fire, turns: 0, bonus: 0 },
  landed: 0,
} satisfies Partial<HitResult>;

/** Mock monster records: the registry bestiary bytes (assets/registry/bestiary.json as
 * deadroll-rules encodes it, the shared fixture of tools/scripts), as `MonsterState.records` holds them. */
export function mockRecord(bestiaryId: number): Uint8Array {
  const hex = registryTables.bestiary.slice(bestiaryId * 96, bestiaryId * 96 + 96);
  return Uint8Array.from((hex.match(/../g) ?? []).map((h) => parseInt(h, 16)));
}

/** A mock registry table body (the shared fixture of tools/scripts): the map builder's bestiary
 * pools, monster skills and reward tables without a chain. */
export function mockTable(table: "bestiary" | "monster_skills" | "rewards" | "tileset"): Uint8Array {
  return Uint8Array.from((registryTables[table].match(/../g) ?? []).map((h) => parseInt(h, 16)));
}

/** Elf mage cold bolt on a wolf: hit, cold resonance with a fire tint, chill applied. */
export const MOCK_HIT_RESONANCE: HitResult = {
  outcome: HitOutcome.Hit,
  d20: 14,
  attackTotal: 19,
  defense: 12,
  crit: false,
  critRoll: 4,
  tintRolls: [
    { base: Aspect.Cold, tint: Aspect.Fire, carrier: Carrier.Race, tier: 9, chanceBp: 18, roll: 10, success: true },
    { base: Aspect.Cold, tint: Aspect.Lightning, carrier: Carrier.Item, tier: 12, chanceBp: 9, roll: 61, success: false },
  ],
  resonances: [
    {
      base: Aspect.Cold,
      tint: Aspect.Fire,
      tier: 9,
      carrier: Carrier.Race,
      id: Aspect.Cold * 6 + Aspect.Fire,
      active: true,
      secondTint: null,
      secondTier: 0,
      secondCarrier: Carrier.Race,
      generated: false,
    },
    null,
    null,
  ],
  d100: 63,
  packets: [
    packet({
      aspect: Aspect.Cold,
      baseMin: 40_000n,
      baseMax: 80_000n,
      increasedBp: 2_500,
      moreBp: 1_000,
      min: 55_000n,
      max: 110_000n,
      rolled: 89_650n,
      armor: 1,
      resistBp: 1_000,
      afterDefense: 71_685n,
      vulnShockBp: 2_000,
      state: { kind: StateKind.Chill, rolled: true, chanceBp: 35, roll: 12, success: true, applied: Applied.Added },
    }),
    packet({
      aspect: Aspect.Fire,
      baseMin: 10_000n,
      baseMax: 20_000n,
      min: 10_000n,
      max: 20_000n,
      rolled: 16_300n,
      vulnBp: 2_000,
      afterDefense: 19_560n,
    }),
  ],
  resonanceStates: [
    { kind: StateKind.Burning, rolled: true, chanceBp: 20, roll: 74, success: false, applied: Applied.None },
  ],
  states: [{ kind: StateKind.Chill, turns: 2, power: 2_000, source: 1 }],
  damage: 9,
  rollsUsed: 9,
  ...NO_RESONANCE,
  // Burning Frost (Cold + Fire): Fury adds Fire, the thaw waits for a Freeze.
  resOps: [
    [
      { kind: OpKind.ThawDamage, critOnly: false, arg: 0, origin: OpOrigin.Pair, tint: Aspect.Fire, carrier: Carrier.Race, tier: 9, acted: false, state: NO_STATE_ROLL, amount: 0, amountMax: 0 },
      { kind: OpKind.AddPacket, critOnly: false, arg: Aspect.Fire, origin: OpOrigin.Trait, tint: Aspect.Fire, carrier: Carrier.Race, tier: 9, acted: true, state: NO_STATE_ROLL, amount: 2, amountMax: 4 },
    ],
    [],
    [],
  ],
};

/** Wolf bite on the warrior: miss. */
export const MOCK_HIT_MISS: HitResult = {
  outcome: HitOutcome.Miss,
  d20: 3,
  attackTotal: 7,
  defense: 14,
  crit: false,
  critRoll: null,
  tintRolls: [],
  resonances: [null, null, null],
  d100: 0,
  packets: [],
  resonanceStates: [],
  states: [],
  damage: 0,
  rollsUsed: 1,
  ...NO_RESONANCE,
};

/** Human warrior spear thrust on the goblin chief: crit, piercing, bleeding. */
export const MOCK_HIT_CRIT: HitResult = {
  outcome: HitOutcome.Hit,
  d20: 18,
  attackTotal: 24,
  defense: 15,
  crit: true,
  critRoll: 2,
  tintRolls: [],
  resonances: [null, null, null],
  d100: 91,
  packets: [
    packet({
      aspect: Aspect.Piercing,
      baseMin: 60_000n,
      baseMax: 120_000n,
      increasedBp: 1_500,
      moreBp: 5_000,
      min: 103_500n,
      max: 207_000n,
      rolled: 197_685n,
      armor: 3,
      resistBp: 500,
      afterDefense: 159_300n,
      penAttackerBp: 3_600,
      penStateBp: 5_000,
      state: { kind: StateKind.Bleeding, rolled: true, chanceBp: 40, roll: 30, success: true, applied: Applied.Added },
    }),
  ],
  resonanceStates: [],
  states: [{ kind: StateKind.Bleeding, turns: 3, power: 1_500, source: 0 }],
  damage: 16,
  rollsUsed: 4,
  ...NO_RESONANCE,
};

export const MOCK_HITS = { resonance: MOCK_HIT_RESONANCE, miss: MOCK_HIT_MISS, crit: MOCK_HIT_CRIT };

export const MOCK_CHARACTERS = [
  { address: "Ch4rWarr1or1111111111111111111111111111111", name: "Bran", race: Race.Human, class: Class.Warrior, predisposition: Aspect.Piercing },
  { address: "Ch4rE1fMage111111111111111111111111111111", name: "Aria", race: Race.ForestElf, class: Class.Mage, predisposition: Aspect.Cold },
];
