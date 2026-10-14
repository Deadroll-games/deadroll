// Items: the character's gear (soulbound starter items and what is worn, by slot, in
// `Character.gear`), the equip instructions, item icons and what worn items add. Slot and base
// lists come from the character IDL (the enums' order on chain); item numbers come from
// balance.json, the program defaults exported by
// `cargo run -p deadroll-sim --example export_balance -- config` (a sim test fails when it is
// stale), so nothing here repeats a rule or a number.
import type { Address, Instruction, TransactionSigner } from "@solana/kit";
import { MATERIAL, RAMP, renderRequest } from "./appearance";
import balance from "./balance.json";
import type { Idl } from "./coder";
import { variantKey, type GearAcc } from "./game";
import characterIdl from "./idl/deadroll_character.json";
import { buildInstruction } from "./programs";
import { Class, LAYERS_MAX, type LayerRef, type RenderRequest } from "./types";

const lowerFirst = (s: string) => s.charAt(0).toLowerCase() + s.slice(1);

/** Variant names of an IDL enum in on-chain order, as the coder keys them ("mainHand"). */
function variants(name: string): string[] {
  const t = (characterIdl as unknown as Idl).types?.find((x) => x.name === name);
  if (!t || t.type.kind !== "enum") throw new Error(`character IDL has no enum ${name}`);
  return t.type.variants.map((v) => lowerFirst(v.name));
}

/** `EquipSlot` in discriminant order. */
export const EQUIP_SLOTS: readonly string[] = variants("EquipSlot");
/** `ItemBase` in discriminant order. */
export const ITEM_BASES: readonly string[] = variants("ItemBase");
/** An empty slot in a worn-items snapshot (`Participation.equipment`). */
export const EMPTY_ITEM = 0xff;

/** "leatherArmor" -> "Leather armor", "ring1" -> "Ring 1". */
export function itemLabel(key: string): string {
  const words = key.replace(/([a-z])([A-Z0-9])/g, "$1 $2").toLowerCase();
  return words.charAt(0).toUpperCase() + words.slice(1);
}

// ---------------------------------------------------------------------------------------------
// account (`Character.gear`)

/** One soulbound item of the character and where it is worn. */
export type InventoryItem = { index: number; base: string; level: number; worn: string | null };
export type Inventory = { items: InventoryItem[]; slots: { slot: string; item: InventoryItem | null }[] };

export function inventory(g: GearAcc): Inventory {
  const items: InventoryItem[] = g.items.slice(0, g.itemCount).map((r, index) => ({
    index,
    base: variantKey(r.base),
    level: r.level,
    worn: null,
  }));
  const slots = EQUIP_SLOTS.map((slot, i) => {
    const r = g.equipped[i];
    const item = r && variantKey(r.source) === "soulbound" ? (items[r.index] ?? null) : null;
    if (item) item.worn = slot;
    return { slot, item };
  });
  return { items, slots };
}

/** The slot a base is made for (rings fit either ring slot). */
export function homeSlot(base: string): string {
  return lowerFirst(baseStats(base)?.slot ?? "mainHand");
}

/** Slots `item` may go into. */
export function slotsFor(base: string): string[] {
  const home = homeSlot(base);
  return home === "ring1" || home === "ring2" ? ["ring1", "ring2"] : [home];
}

// ---------------------------------------------------------------------------------------------
// instructions (character program: the owner's wallet, or the session key with its token)

export async function equipIx(signer: TransactionSigner, character: Address, itemIndex: number, slot: string, token: Address | null): Promise<Instruction> {
  return buildInstruction("character", "equip", { itemIndex, slot }, { signer, character, sessionToken: token });
}

export async function unequipIx(signer: TransactionSigner, character: Address, slot: string, token: Address | null): Promise<Instruction> {
  return buildInstruction("character", "unequip", { slot }, { signer, character, sessionToken: token });
}

// ---------------------------------------------------------------------------------------------
// what worn items add (balance.json = deadroll_rules::BalanceConfig::DEFAULT)

type BaseStats = {
  slot: string;
  level: number;
  aspect: string;
  damage: number[];
  armor: number;
  defense: number;
  attack: number;
  stats: Record<string, number>;
  hp: number;
};
const BASES: Record<string, BaseStats> = balance.items.bases;
/** balance.json keys bases by the Rust variant name ("LeatherArmor"). */
const baseStats = (key: string): BaseStats | undefined => BASES[key.charAt(0).toUpperCase() + key.slice(1)];

export type ItemBonus = {
  /** The main-hand base when it is a weapon (its damage replaces bare hands). */
  weapon: string | null;
  armor: number;
  defense: number;
  attack: number;
  /** By `Stat` order: Strength, Dexterity, Intelligence, Endurance. */
  stats: [number, number, number, number];
  hp: number;
};

const STAT_KEYS = ["Strength", "Dexterity", "Intelligence", "Endurance"] as const;

/** `deadroll_rules::items::item_bonus` over a worn-items snapshot (`ItemBase` bytes by slot). */
export function itemBonus(equipment: ArrayLike<number>): ItemBonus {
  const out: ItemBonus = { weapon: null, armor: 0, defense: 0, attack: 0, stats: [0, 0, 0, 0], hp: 0 };
  for (let slot = 0; slot < equipment.length; slot++) {
    const key = ITEM_BASES[equipment[slot] ?? EMPTY_ITEM];
    const b = key ? baseStats(key) : undefined;
    if (!key || !b) continue;
    if (slot === 0 && (b.damage[1] ?? 0) > 0) out.weapon = key;
    out.armor += b.armor;
    out.defense += b.defense;
    out.attack += b.attack;
    out.hp += b.hp;
    STAT_KEYS.forEach((s, i) => (out.stats[i] = (out.stats[i] ?? 0) + (b.stats[s] ?? 0)));
  }
  return out;
}

/** What the character wears, as the snapshot `set_ready` takes. */
export function equipmentOf(g: GearAcc): number[] {
  return inventory(g).slots.map((s) => (s.item ? ITEM_BASES.indexOf(s.item.base) : EMPTY_ITEM));
}

/** Weapon damage of a base ("Blunt 4-8"), or null for armor. */
export function weaponText(base: string): string | null {
  const b = baseStats(base);
  return b && (b.damage[1] ?? 0) > 0 ? `${b.aspect} ${b.damage[0]}-${b.damage[1]}` : null;
}

/** What one base adds, in one line ("+10 armor"). */
export function bonusText(base: string): string {
  const b = baseStats(base);
  if (!b) return "";
  const parts: string[] = [];
  const w = weaponText(base);
  if (w) parts.push(`damage ${w}`);
  for (const [v, name] of [[b.armor, "armor"], [b.defense, "defense"], [b.attack, "attack"], [b.hp, "HP"]] as const) if (v > 0) parts.push(`+${v} ${name}`);
  for (const s of STAT_KEYS) if ((b.stats[s] ?? 0) > 0) parts.push(`+${b.stats[s]} ${s}`);
  return parts.join(", ");
}

/** The class base armor (`PlayerTuning.armor`) and the defense rule, before items. */
export const PLAYER_ARMOR: readonly number[] = balance.players.armor;
export const DEFENSE_BASE: number = balance.players.defense_base;
export const DEFENSE_DEX_DIV: number = balance.players.defense_dex_div;
export const UNARMED: readonly number[] = balance.items.unarmed;

/** `deadroll_rules::input::hero::defense`: defense threshold and armor with the worn items. */
export function heroDefense(cls: Class, stats: readonly number[], equipment: ArrayLike<number>): { defense: number; armor: number } {
  const items = itemBonus(equipment);
  const dex = (stats[1] ?? 0) + items.stats[1];
  return {
    defense: DEFENSE_BASE + Math.floor(dex / Math.max(1, DEFENSE_DEX_DIV)) + items.defense,
    armor: (PLAYER_ARMOR[cls] ?? 0) + items.armor,
  };
}

// ---------------------------------------------------------------------------------------------
// icons: the item's own sprite from sprite book 1 (assets/sprites, the class kits)

const ITEM_SPRITES: Record<string, [spriteId: number, material: number]> = {
  sword: [106, MATERIAL.steel],
  staff: [204, MATERIAL.leather],
  wand: [303, MATERIAL.leather], // the support kit weapon sprite (a dedicated wand sprite is art for later)
  leatherArmor: [104, MATERIAL.leather],
  robe: [201, MATERIAL.robe],
  lightLeatherArmor: [301, MATERIAL.leather],
};

/** A render request with only the item's sprite, or null for a base without one. */
export function itemRequest(base: string): RenderRequest | null {
  const s = ITEM_SPRITES[base];
  if (!s) return null;
  const layers: LayerRef[] = Array.from({ length: LAYERS_MAX }, () => ({ spriteId: 0, materialRamp: 0, accentRamp: 0, elementRamp: 0 }));
  layers[0] = { spriteId: s[0], materialRamp: s[1], accentRamp: RAMP.gold, elementRamp: RAMP.cold };
  return renderRequest(layers);
}
