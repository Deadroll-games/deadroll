// Props contract for the inventory panel (character screen). Pure UI: the route reads the
// character's gear and sends the instructions.
import type { ReactNode } from "react";

export type InventoryItemView = {
  /** Soulbound record index in `Character.gear` (the `equip` argument). */
  index: number;
  /** `ItemBase` key ("sword"). */
  base: string;
  label: string;
  level: number;
  /** What it adds, one line ("damage Blunt 4-8", "+10 armor"). */
  bonus: string;
  /** Slots it may go into (`EquipSlot` keys). */
  fits: string[];
  /** The slot it is worn in, or null. */
  worn: string | null;
};

export type InventoryProps = {
  /** Every `EquipSlot` key in on-chain order, with its label. */
  slots: { key: string; label: string }[];
  /** The character's soulbound items (empty for a tombstone: they vanished with it). */
  items: InventoryItemView[];
  /** In a session (equipment is locked until it ends), dead, or not the viewer's character. */
  locked: string | null;
  busy: boolean;
  onEquip: (index: number, slot: string) => void;
  onUnequip: (slot: string) => void;
  /** The item's sprite (the app renders it from the on-chain sprite book). */
  icon: (base: string) => ReactNode;
};
