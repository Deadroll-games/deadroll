// Mock data for VITE_DATA=mock and the /dev pages. Shapes match chain/types.ts exactly.
import demoMap from "../../../assets/registry/maps/demo_16x16.json";
import {
  Applied,
  Aspect,
  Carrier,
  Class,
  HitOutcome,
  MAP_CELLS,
  Race,
  StateKind,
  Tile,
  cellIndex,
  type HitResult,
  type PacketResult,
  type Spawn,
  type StateRoll,
} from "./types";

const TILE_CHARS: Record<string, Tile> = {
  "#": Tile.Wall,
  ".": Tile.Floor,
  "~": Tile.Water,
  D: Tile.Door,
  E: Tile.Entrance,
  X: Tile.Exit,
  T: Tile.Trap,
};

export type MapPresetData = {
  name: string;
  width: number;
  height: number;
  /** Map tier (monster levels and limits); 1 when absent. */
  tier?: number;
  rows: string[];
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

/** Converts a preset from assets/registry/maps into MapLayout tiles (stride 32) and spawns. */
export function presetToLayout(p: MapPresetData): { tiles: Uint8Array; spawns: Spawn[] } {
  const tiles = new Uint8Array(MAP_CELLS).fill(Tile.Wall);
  p.rows.forEach((row, y) => {
    for (let x = 0; x < row.length; x++) tiles[cellIndex(x, y)] = TILE_CHARS[row[x] ?? "#"] ?? Tile.Wall;
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
  return { tiles, spawns };
}

export const MAP_PRESETS: MapPresetData[] = [demoMap as MapPresetData];
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
    { base: Aspect.Cold, tint: Aspect.Fire, tier: 9, carrier: Carrier.Race, id: Aspect.Cold * 6 + Aspect.Fire, active: true, secondTint: null },
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
};

export const MOCK_HITS = { resonance: MOCK_HIT_RESONANCE, miss: MOCK_HIT_MISS, crit: MOCK_HIT_CRIT };

export const MOCK_CHARACTERS = [
  { address: "Ch4rWarr1or1111111111111111111111111111111", name: "Bran", race: Race.Human, class: Class.Warrior, predisposition: Aspect.Piercing },
  { address: "Ch4rE1fMage111111111111111111111111111111", name: "Aria", race: Race.ForestElf, class: Class.Mage, predisposition: Aspect.Cold },
];
