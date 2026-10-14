// Display names for enum values. English UI strings; keep in sync with chain/types.ts.
import { EFFECT_NAMES, Effect } from "./skillcheck";
import type { EffectResolved } from "./support";
import { ASPECT_COUNT, HitOutcome, type Aspect, type Carrier, type Class, type Race, type StateKind, type Tile } from "./types";

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

/** Hundredths with two decimals, e.g. 8_104 -> "81.04". */
export function formatHundredths(v: number): string {
  return (v / 100).toFixed(2);
}

/*
 * Chance rolls are shown "high roll wins", like the d20. On-chain a raw roll in 0..9_999 passes when
 * raw < chance (crates/combat/src/rng.rs). Shown = 9_999 - raw and needed = 10_000 - chance, so
 * shown >= needed exactly when raw < chance.
 */
export const shownRoll = (raw: number): number => 9_999 - raw;
export const neededRoll = (chanceBp: number): number => 10_000 - chanceBp;

/** Tooltip with the raw on-chain numbers behind a shown chance roll. */
export function rollTitle(raw: number, chanceBp: number | null): string {
  const pass = chanceBp == null ? "" : `, passes when raw < ${formatHundredths(chanceBp)}%`;
  return `on-chain raw roll ${formatHundredths(raw)}%, shown as 99.99 - raw${pass}`;
}

export const effectName = (e: Effect): string => EFFECT_NAMES[e] ?? `effect ${e}`;

/** What one effect roll did, e.g. "+7 hp", "Weakening 18%", "Freeze", "missed". */
export function effectAmount(e: Pick<EffectResolved, "effect" | "roll">): string {
  if (e.roll.outcome === HitOutcome.Miss) return "missed";
  if (e.effect === Effect.Heal) return `+${e.roll.amount} hp`;
  if (e.effect === Effect.Stun || e.effect === Effect.Freeze) return effectName(e.effect);
  return `${effectName(e.effect)} ${Math.round(e.roll.amount / 100)}%`;
}
