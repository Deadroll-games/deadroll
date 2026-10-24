// Shared game types mirrored from crates/types (agents/CONTRACTS.md, section 3) and the
// combat WASM HitResult layout (crates/combat/wasm/README.md). Discriminants are the on-chain bytes.
// Replace with sdk/ (Codama) types once L8 lands; keep the names.

export const Aspect = { Fire: 0, Cold: 1, Lightning: 2, Blood: 3, Blunt: 4, Piercing: 5 } as const;
export type Aspect = (typeof Aspect)[keyof typeof Aspect];
export const ASPECT_COUNT = 6;

export const StateKind = { Burning: 0, Chill: 1, Freeze: 2, Bleeding: 3, Shock: 4, Stun: 5, Penetration: 6 } as const;
export type StateKind = (typeof StateKind)[keyof typeof StateKind];

export const Race = { Human: 0, ForestElf: 1 } as const;
export type Race = (typeof Race)[keyof typeof Race];

export const Class = { Warrior: 0, Mage: 1, Support: 2 } as const;
export type Class = (typeof Class)[keyof typeof Class];

/**
 * Patch 13: tile types (`TileKind`, docs/MAP_DESCRIPTION.md 4.2), bits 7-3 of a cell byte. Their
 * properties (passable, step cost) are the registry Tileset, never this file.
 */
export const Tile = {
  Void: 0,
  StoneFloor: 1,
  WoodenFloor: 2,
  Dirt: 3,
  Wall: 4,
  Door: 5,
  ShallowWater: 6,
  DeepWater: 7,
  Stairs: 8,
  Rubble: 9,
  Chasm: 10,
} as const;
export type Tile = (typeof Tile)[keyof typeof Tile];
export const TILE_COUNT = 11;
/** Tile types a cell byte can name (`TILE_KINDS_MAX`). */
export const TILE_KINDS_MAX = 32;

/** The tile type id of a cell byte (bits 7-3, `cell_kind`). */
export const cellKind = (byte: number): number => (byte >> 3) & 31;
/** The variant of a cell byte (bits 2-0, `cell_variant`). */
export const cellVariant = (byte: number): number => byte & 7;
/** A cell byte from a tile type and a variant (`cell_byte`). */
export const cellByte = (kind: number, variant = 0): number => ((kind & 31) << 3) | (variant & 7);

/** A map point type (`PointKind`, docs/MAP_DESCRIPTION.md 7.1). */
export const PointKind = { Chest: 0, Entrance: 1, Exit: 2 } as const;
export type PointKind = (typeof PointKind)[keyof typeof PointKind];

export const SessionStatus = { Lobby: 0, Active: 1, Victory: 2, Wiped: 3, Fled: 4, Abandoned: 5 } as const;
export type SessionStatus = (typeof SessionStatus)[keyof typeof SessionStatus];

export const MapStatus = { Draft: 0, Finalized: 1, InSession: 2, Finished: 3 } as const;
export type MapStatus = (typeof MapStatus)[keyof typeof MapStatus];

export const Carrier = { Race: 0, Class: 1, Item: 2 } as const;
export type Carrier = (typeof Carrier)[keyof typeof Carrier];

export const HitOutcome = { Miss: 0, Hit: 1, Immune: 2 } as const;
export type HitOutcome = (typeof HitOutcome)[keyof typeof HitOutcome];

export const Applied = { None: 0, Added: 1, Refreshed: 2, Froze: 3 } as const;
export type Applied = (typeof Applied)[keyof typeof Applied];

/** Constants from crates/types. */
export const MAP_MAX = 32;
export const MAP_CELLS = 1024;
export const PARTY_MAX = 4;
export const MONSTERS_MAX = 24;
export const SPAWNS_MAX = 24;
export const LAYERS_MAX = 20;
export const STATS_COUNT = 4;
/** Basis points: 10_000 = 100% = x1. */
export const BP = 10_000;
/** Packet values in HitResult are scaled by this (5 damage == 50_000). */
export const DAMAGE_SCALE = 10_000n;

export type Cell = { x: number; y: number };

/** Index of a cell in MapLayout.tiles (row stride is always MAP_MAX, crates/mapcheck). */
export function cellIndex(x: number, y: number): number {
  return y * MAP_MAX + x;
}

export type LayerRef = {
  spriteId: number;
  materialRamp: number;
  accentRamp: number;
  elementRamp: number;
};

export type RenderRequest = {
  paletteVersion: number;
  /** Exactly LAYERS_MAX entries; spriteId 0 = empty layer. */
  layers: LayerRef[];
  /** bit 0 = GRAY (dead character). */
  flags: number;
};

export type ActiveState = { kind: StateKind; turns: number; power: number; source: number };

export type Spawn = {
  bestiaryId: number;
  x: number;
  y: number;
  level: number;
  skills: number;
  flags: number;
  aspects: number;
  pack: number;
};

// ---- HitResult (combat WASM / HitResolved event) ----

export type TintRoll = {
  base: Aspect;
  tint: Aspect;
  carrier: Carrier;
  tier: number;
  chanceBp: number;
  roll: number;
  success: boolean;
};

export type Resonance = {
  base: Aspect;
  tint: Aspect;
  tier: number;
  carrier: Carrier;
  /** base * 6 + tint; see resonanceName(). */
  id: number;
  active: boolean;
  /** The second tint of a triple (docs/DAMAGE_TYPES_AND_MIXES.md 6.5), its tier and carrier. */
  secondTint: Aspect | null;
  secondTier: number;
  secondCarrier: Carrier;
  /** The pair has no record of its own: its mechanics are the tint's trait. */
  generated: boolean;
};

/** `deadroll_combat::tables::OpKind`: what one op of a resonance does. */
export const OpKind = {
  None: 0,
  Multiplier: 1,
  StateOnCrit: 2,
  ExtraStateChance: 3,
  Linked: 4,
  LinkedChance: 5,
  LinkedMod: 6,
  NativeTurns: 7,
  NativePower: 8,
  NativeFaster: 9,
  NativePowerArmor: 10,
  ThawDamage: 11,
  RemoveState: 12,
  Jump: 13,
  JumpChain: 14,
  JumpLine: 15,
  Knockback: 16,
  IgniteCells: 17,
  AddPacket: 18,
  Penetrate: 19,
  AccuracyBuff: 20,
} as const;
export type OpKind = (typeof OpKind)[keyof typeof OpKind];

/** Where an op comes from: the pair, the second tint's trait (a triple), a trait's hit part. */
export const OpOrigin = { Pair: 0, Template: 1, Trait: 2 } as const;
export type OpOrigin = (typeof OpOrigin)[keyof typeof OpOrigin];

/** Op `arg` 255: the base's native state (any state, for a multiplier). */
export const OP_NATIVE = 255;

/** What one op of a resonance did (`deadroll_combat::OpResult`). */
export type OpResult = {
  kind: OpKind;
  critOnly: boolean;
  /** A state, a modifier kind or an aspect, by kind (see opText). */
  arg: number;
  origin: OpOrigin;
  tint: Aspect;
  carrier: Carrier;
  tier: number;
  acted: boolean;
  /** The state it applied, with its roll (`kind` 0 and nothing rolled when none). */
  state: StateRoll;
  amount: number;
  amountMax: number;
};

/** A state passed on to other monsters (`deadroll_combat::Jump`). */
export const JumpMode = { Adjacent: 0, Chain: 1, Line: 2 } as const;
export type JumpMode = (typeof JumpMode)[keyof typeof JumpMode];
export type Jump = { mode: JumpMode; state: ActiveState; filter: StateKind; limit: number; base: Aspect };

export type StateRoll = {
  kind: StateKind;
  rolled: boolean;
  chanceBp: number;
  roll: number;
  success: boolean;
  applied: Applied;
  /** Patch 9b: the counterweight the chance met, bp (state resistance plus a share of the
   * resistance to the state's aspect); 0 when no die was rolled. */
  resistBp?: number;
};

export type PacketResult = {
  aspect: Aspect;
  immune: boolean;
  /** Scaled x10_000. */
  baseMin: bigint;
  baseMax: bigint;
  increasedBp: number;
  moreBp: number;
  weakenBp: number;
  /** Scaled x10_000. */
  min: bigint;
  max: bigint;
  rolled: bigint;
  armor: number;
  resistBp: number;
  vulnBp: number;
  /** Scaled x10_000. */
  afterDefense: bigint;
  state: StateRoll;
  /** Attacker's penetration on this packet, bp. 0 when armor does not apply. */
  penAttackerBp: number;
  /** Penetration already on the target, bp. */
  penStateBp: number;
  /** Shock already on the target, a second vulnerability multiplier, bp. */
  vulnShockBp: number;
};

/** Hit breakdown, in roll order (crates/combat/src/hit.rs). Arrays hold only the used entries. */
export type HitResult = {
  outcome: HitOutcome;
  d20: number;
  attackTotal: number;
  defense: number;
  crit: boolean;
  critRoll: number | null;
  tintRolls: TintRoll[];
  /** One entry per skill base slot (up to 3). */
  resonances: (Resonance | null)[];
  d100: number;
  packets: PacketResult[];
  resonanceStates: StateRoll[];
  /** Target states after the hit (non-empty only). */
  states: ActiveState[];
  /** Final rounded damage. */
  damage: number;
  rollsUsed: number;
  /** Per skill base slot (same index as `resonances`): the ops of its resonance. */
  resOps: OpResult[][];
  jumps: Jump[];
  /** Cells the target is pushed away from the attacker. */
  knockback: number;
  /** Burning of this power on the cells around the target, 0 = none. */
  ignitePower: number;
  /** Timed modifiers the resonance put on the target. */
  targetMods: TimedMod[];
  /** The attacker's accuracy buff from this hit (turns 0 = none). */
  accuracyBuff: AccuracyBuff;
  /** `1 << StateKind` of every state that landed on the target. */
  landed: number;
};

/** A resonance's accuracy buff on a player (Pale Flame): +bonus to the d20 of `aspect` attacks. */
export type AccuracyBuff = { aspect: Aspect; turns: number; bonus: number };

/** One monster a state jumped to (`ResonanceSpread`). */
export type SpreadHit = { monster: number; kind: StateKind; applied: Applied; states: ActiveState[] };

/** What a player's hit did beyond its target (session `ResonanceSpread`, after its HitResolved). */
export type ResonanceSpread = {
  session: string;
  actionCounter: number;
  attacker: number;
  target: number;
  hits: SpreadHit[];
  moved: { from: Cell; to: Cell } | null;
  ignited: Cell[];
  ignitePower: number;
  untilRound: number;
  accuracy: AccuracyBuff;
};

/**
 * `HitResolved` event as the app consumes it. The session program (L7) emits it from the VRF
 * callback; until its IDL exists this is the app-side shape. Units are turn-order ids:
 * 0..4 player slot, 4 + i monster i.
 */
/** What one side's worn items put into a hit (session `ItemBonusData`); all zero for a monster. */
export type ItemBonusView = {
  /** The main-hand weapon (an `ItemBase` key like "sword"), null = bare hands or a monster. */
  weapon: string | null;
  armor: number;
  defense: number;
  attack: number;
};
export type HitItems = { attacker: ItemBonusView; defender: ItemBonusView };

export type HitResolved = {
  session: string;
  attacker: number;
  target: number;
  /** VRF seed (32 bytes); the combat WASM can replay the hit from it. */
  seed: Uint8Array;
  actionCounter: number;
  result: HitResult;
  /** The players' worn items in this hit. */
  items: HitItems;
};

// ---- buffs and debuffs (deadroll_combat::support, session TimedModData) ----

/** ModKindData; `ModsTicked.expired` bit i is `1 << i`. */
export const ModKind = { Haste: 0, Slow: 1, Empowerment: 2, Weakening: 3, Protection: 4, Vulnerability: 5, Resistance: 6 } as const;
export type ModKind = (typeof ModKind)[keyof typeof ModKind];

/** One active buff or debuff: `turns` own turn starts of the bearer until it ends. */
export type TimedMod = {
  kind: ModKind;
  aspect: Aspect;
  turns: number;
  powerBp: number;
  source: number;
  /** Patch 9, a Resistance: the share of `powerBp` each aspect gets, percent, by `Aspect`. */
  weightsPct?: readonly number[];
};
