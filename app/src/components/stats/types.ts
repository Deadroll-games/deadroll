// Props contract for the battle stats panel. Written by the lane owner (L9).
import type { UnitStats } from "../../chain/stats";

export type StatsUnit = {
  /** Turn-order id: 0..4 player slot, 4 + i monster i. */
  unit: number;
  label: string;
  kind: "player" | "monster";
  /** The unit whose turn is running. */
  current?: boolean;
  stats: UnitStats;
};

export type StatsPanelProps = {
  units: StatsUnit[];
};

/** A hero's derived stats (the combat module's `HeroSheet`, docs/CHARACTER.md 3). */
export type HeroSheetView = {
  accuracy: number;
  evasion: number;
  armor: number;
  critBp: number;
  hpMax: number;
  actionPoints: number;
  initiative: number;
  martialStrikeBp: number;
  martialProjectileBp: number;
  magicalBp: number;
  martialStateBp: number;
  magicalStateBp: number;
  tempoStateBp: number;
  selfBuffTurns: number;
};

/** The character screen's stats panel: core stats, a level-up point plan and derived stats. */
export type HeroSheetPanelProps = {
  /** Core stats as stored, by Stat order. */
  stats: readonly number[];
  /** Points planned per stat, not saved yet. */
  plan: readonly number[];
  /** Points from level-ups not spent yet. */
  unspent: number;
  /** Derived stats now, and with the plan; null while the combat module loads. */
  sheet: HeroSheetView | null;
  planned: HeroSheetView | null;
  /** Why the points cannot be spent here (null = they can). */
  locked: string | null;
  busy: boolean;
  onPlan(stat: number, delta: 1 | -1): void;
  onReset(): void;
  /** Sends the whole plan in one transaction. */
  onSave(): void;
};
