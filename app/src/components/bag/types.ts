// Props contract for the bag screen (patch 11, docs/ITEMS_AND_CRAFT.md 6.1). Pure UI: the route
// keeps the layout and applies the moves (`chain/bagLayout.ts`).
import type { Bag, SlotRef } from "../../chain/bagLayout";
import type { Reward } from "../../chain/loot";

export type BagPanelProps = {
  bag: Bag;
  /** What is left in the settled cache; empty when there is none. */
  cache: Reward[];
  /** A settled cache waits: the layout can change and "Done" sends it. */
  editable: boolean;
  /** Slot keys locked in this browser (`chain/bagLayout.ts` `slotKey`). */
  locks: ReadonlySet<string>;
  /** The cache reward picked for a replacement (click it, then a bag slot). */
  picked: number | null;
  /** The chain's check of the current layout (null = valid). */
  error: string | null;
  busy?: boolean;
  label: (r: Reward) => string;
  onPick: (cacheIndex: number | null) => void;
  /** Shift + click on a cache reward: into the first fitting slots. */
  onTakeAll: (cacheIndex: number) => void;
  /** A cache reward dropped on a slot (or a click on a slot with a picked reward: replace). */
  onPlace: (cacheIndex: number, to: SlotRef) => void;
  onMove: (from: SlotRef, to: SlotRef) => void;
  onSplit: (at: SlotRef) => void;
  /** Asked for confirmation by the panel first. */
  onDiscard: (at: SlotRef) => void;
  onToggleLock: (at: SlotRef) => void;
  onReset: () => void;
  onDone: () => void;
};
