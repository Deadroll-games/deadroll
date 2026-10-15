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

export const Tile = { Floor: 0, Wall: 1, Water: 2, Door: 3, Entrance: 4, Exit: 5, Trap: 6 } as const;
export type Tile = (typeof Tile)[keyof typeof Tile];

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
  secondTint: Aspect | null;
};

export type StateRoll = {
  kind: StateKind;
  rolled: boolean;
  chanceBp: number;
  roll: number;
  success: boolean;
  applied: Applied;
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
export const ModKind = { Haste: 0, Slow: 1, Empowerment: 2, Weakening: 3, Protection: 4, Vulnerability: 5 } as const;
export type ModKind = (typeof ModKind)[keyof typeof ModKind];

/** One active buff or debuff: `turns` own turn starts of the bearer until it ends. */
export type TimedMod = { kind: ModKind; aspect: Aspect; turns: number; powerBp: number; source: number };
