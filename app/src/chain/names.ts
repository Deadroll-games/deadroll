// Display names for enum values. English UI strings; keep in sync with chain/types.ts.
import { ASPECT_COUNT, type Aspect, type Carrier, type Class, type Race, type StateKind, type Tile } from "./types";

export const ASPECT_NAMES = ["Fire", "Cold", "Lightning", "Blood", "Blunt", "Piercing"] as const;
export const STATE_NAMES = ["Burning", "Chill", "Freeze", "Bleeding", "Shock", "Stun"] as const;
export const RACE_NAMES = ["Human", "Forest elf"] as const;
export const CLASS_NAMES = ["Warrior", "Mage", "Support"] as const;
export const TILE_NAMES = ["Floor", "Wall", "Water", "Door", "Entrance", "Exit", "Trap"] as const;
export const CARRIER_NAMES = ["Race", "Class", "Item"] as const;
export const STAT_NAMES = ["Strength", "Dexterity", "Intelligence", "Endurance"] as const;

/** Placeholder names for the demo bestiary ids until the registry Bestiary account is read (L9.6). */
export const BESTIARY_NAMES: Record<number, string> = {
  0: "Wolf",
  1: "Goblin",
  2: "Goblin chief",
  3: "Goblin archer",
  4: "Goblin shaman",
};

export const aspectName = (a: Aspect): string => ASPECT_NAMES[a] ?? `aspect ${a}`;
export const stateName = (s: StateKind): string => STATE_NAMES[s] ?? `state ${s}`;
export const raceName = (r: Race): string => RACE_NAMES[r] ?? `race ${r}`;
export const className = (c: Class): string => CLASS_NAMES[c] ?? `class ${c}`;
export const tileName = (t: Tile): string => TILE_NAMES[t] ?? `tile ${t}`;
export const carrierName = (c: Carrier): string => CARRIER_NAMES[c] ?? `carrier ${c}`;
export const bestiaryName = (id: number): string => BESTIARY_NAMES[id] ?? `monster #${id}`;

/** Resonance id is base * 6 + tint (crates/combat). */
export function resonanceName(id: number): string {
  const base = Math.floor(id / ASPECT_COUNT) as Aspect;
  const tint = (id % ASPECT_COUNT) as Aspect;
  return `${aspectName(base)} + ${aspectName(tint)}`;
}

/** Formats a value scaled x10_000 with one decimal, e.g. 52_500n -> "5.3". */
export function formatScaled(v: bigint): string {
  const tenths = (v + 500n) / 1_000n;
  return `${tenths / 10n}.${tenths % 10n}`;
}

/** Basis points as a percent, e.g. 2_500 -> "25%". */
export function formatBp(bp: number): string {
  return `${Number.isInteger(bp / 100) ? bp / 100 : (bp / 100).toFixed(2)}%`;
}
