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
