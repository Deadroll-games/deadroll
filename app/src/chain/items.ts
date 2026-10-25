// Items: the character's gear (soulbound starter items and what is worn, by slot, in
// `Character.gear`), the equip instructions, item icons and what worn items add. Slot and base
// lists come from the sdk's generated enums (their order on chain); item numbers come from
// balance.json, the program defaults exported by
// `cargo run -p deadroll-sim --example export_balance -- config` (a sim test fails when it is
// stale), so nothing here repeats a rule or a number.
import type { Address, Instruction, TransactionSigner } from "@solana/kit";
import { LAYER, MATERIAL, RAMP, renderRequest, TOMBSTONE } from "./appearance";
import balance from "./balance.json";
import { EquipSlot, EquipSource, getEquipInstruction, getUnequipInstruction, ItemBase } from "@deadroll/sdk/character";
import { enumKeys, variantKey, type CharacterAcc, type GearAcc } from "./game";
import { Class, LAYERS_MAX, type LayerRef, type RenderRequest } from "./types";

const lowerFirst = (s: string) => s.charAt(0).toLowerCase() + s.slice(1);

/** `EquipSlot` in discriminant order. */
export const EQUIP_SLOTS: readonly string[] = enumKeys(EquipSlot);
/** `ItemBase` in discriminant order. */
export const ITEM_BASES: readonly string[] = enumKeys(ItemBase);
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
    base: variantKey(ItemBase, r.base),
    level: r.level,
    worn: null,
  }));
  const slots = EQUIP_SLOTS.map((slot, i) => {
    const r = g.equipped[i];
    const item = r && r.source === EquipSource.Soulbound ? (items[r.index] ?? null) : null;
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

function slotIndex(slot: string): EquipSlot {
  const i = EQUIP_SLOTS.indexOf(slot);
  if (i < 0) throw new Error(`no equip slot ${slot}`);
  return i as EquipSlot;
}

export async function equipIx(signer: TransactionSigner, character: Address, itemIndex: number, slot: string, token: Address | null): Promise<Instruction> {
  return getEquipInstruction({ signer, character, sessionToken: token ?? undefined, itemIndex, slot: slotIndex(slot) });
}

export async function unequipIx(signer: TransactionSigner, character: Address, slot: string, token: Address | null): Promise<Instruction> {
  return getUnequipInstruction({ signer, character, sessionToken: token ?? undefined, slot: slotIndex(slot) });
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
  /** Patch 9: resistance per aspect (keyed by aspect name) and state resistance, bp. */
  resist_bp: Record<string, number>;
  state_resist_bp: number;
  /** Patch 9b: resistance to one state (keyed by state name) on top of `state_resist_bp`. */
  state_kind_resist_bp: Record<string, number>;
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
  /** Patch 9: resistance per aspect (by `Aspect`) and state resistance, bp. */
  resist: number[];
  stateResist: number;
  /** Patch 9b: resistance to each state on top of `stateResist`, by `StateKind`. */
  stateKindResist: number[];
};

const STAT_KEYS = ["Strength", "Dexterity", "Intelligence", "Endurance"] as const;
/** `Aspect` order, as balance.json keys the item resistances. */
const ASPECT_KEYS = ["Fire", "Cold", "Lightning", "Blood", "Blunt", "Piercing"] as const;
/** `StateKind` order, as balance.json keys the item resistances to one state. */
const STATE_KEYS = ["Burning", "Chill", "Freeze", "Bleeding", "Shock", "Stun", "Penetration"] as const;

/** `deadroll_rules::items::item_bonus` over a worn-items snapshot (`ItemBase` bytes by slot). */
export function itemBonus(equipment: ArrayLike<number>): ItemBonus {
  const out: ItemBonus = { weapon: null, armor: 0, defense: 0, attack: 0, stats: [0, 0, 0, 0], hp: 0, resist: ASPECT_KEYS.map(() => 0), stateResist: 0, stateKindResist: STATE_KEYS.map(() => 0) };
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
    ASPECT_KEYS.forEach((a, i) => (out.resist[i] = (out.resist[i] ?? 0) + (b.resist_bp?.[a] ?? 0)));
    out.stateResist += b.state_resist_bp ?? 0;
    STATE_KEYS.forEach((k, i) => (out.stateKindResist[i] = (out.stateKindResist[i] ?? 0) + (b.state_kind_resist_bp?.[k] ?? 0)));
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
/** Patch 9: state resistance per point of Endurance (`PlayerTuning::end_state_resist_bp`). */
export const END_STATE_RESIST_BP: number = balance.players.end_state_resist_bp;

export function heroDefense(
  cls: Class,
  stats: readonly number[],
  equipment: ArrayLike<number>,
): { defense: number; armor: number; resist: number[]; stateResist: number; stateResistByKind: number[] } {
  const items = itemBonus(equipment);
  const dex = (stats[1] ?? 0) + items.stats[1];
  const end = (stats[3] ?? 0) + items.stats[3];
  return {
    defense: DEFENSE_BASE + Math.floor(dex / Math.max(1, DEFENSE_DEX_DIV)) + items.defense,
    armor: (PLAYER_ARMOR[cls] ?? 0) + items.armor,
    // deadroll_rules hero::resist and hero::state_resist: items, and Endurance for states.
    resist: items.resist,
    stateResist: end * END_STATE_RESIST_BP + items.stateResist,
    // hero::state_resists: the general one plus the items' resistance to each state.
    stateResistByKind: items.stateKindResist.map((r) => r + end * END_STATE_RESIST_BP + items.stateResist),
  };
}

// ---------------------------------------------------------------------------------------------
// sprites: one per `ItemBase` (assets/sprites/items/items.dsl), worn on the body or alone as an icon

/** The sprite of item base `i` (`ItemBase` discriminant) is `ITEM_SPRITE_BASE + i`. */
export const ITEM_SPRITE_BASE = 100;
/** The material an item without its own is drawn in, by base. */
const ITEM_MATERIAL: Record<string, number> = {
  sword: MATERIAL.steel,
  staff: MATERIAL.leather,
  wand: MATERIAL.leather,
  leatherArmor: MATERIAL.leather,
  robe: MATERIAL.robe,
  lightLeatherArmor: MATERIAL.leather,
};
/** The character layer each equipment slot is drawn in; rings and gloves draw nothing yet. */
const SLOT_LAYER: Record<string, number> = {
  mainHand: LAYER.mainhand,
  offHand: LAYER.offhand,
  helmet: LAYER.helmet,
  body: LAYER.armorBody,
  pants: LAYER.pants,
  boots: LAYER.boots,
  necklace: LAYER.necklace,
  cloak: LAYER.backItem,
};

function itemLayer(base: string, elementRamp: number): LayerRef | null {
  const i = ITEM_BASES.indexOf(base);
  const material = ITEM_MATERIAL[base];
  if (i < 0 || material === undefined) return null;
  return { spriteId: ITEM_SPRITE_BASE + i, materialRamp: material, accentRamp: RAMP.gold, elementRamp };
}

/**
 * The layers of a character dressed in what it wears: `appearance` (shadow, body, hair, fist) with
 * each worn item's sprite in its slot's layer. `equipment` is the `ItemBase` byte by `EquipSlot`
 * (`EMPTY_ITEM` for none), as `equipmentOf` gives it. Item layers take the body's element ramp.
 */
export function dressed(appearance: readonly LayerRef[], equipment: ArrayLike<number>): LayerRef[] {
  const out = Array.from({ length: LAYERS_MAX }, (_, i) => ({ ...(appearance[i] ?? { spriteId: 0, materialRamp: 0, accentRamp: 0, elementRamp: 0 }) }));
  const element = appearance[LAYER.body]?.elementRamp ?? 0;
  EQUIP_SLOTS.forEach((slot, i) => {
    const b = equipment[i];
    const layer = SLOT_LAYER[slot];
    if (b === undefined || b === EMPTY_ITEM || layer === undefined) return;
    const l = itemLayer(ITEM_BASES[b] ?? "", element);
    if (l) out[layer] = l;
  });
  return out;
}

/** A character's image: its body and hair, dressed in what it wears now; a tombstone once dead. */
export function characterRequest(c: CharacterAcc): RenderRequest {
  if (!c.core.alive) return TOMBSTONE;
  return renderRequest(dressed(c.core.appearance, equipmentOf(c.gear)));
}

/** `balance.json` keys the starter kits by the Rust class name, in `Class` order. */
const KIT_CLASS = ["Warrior", "Mage", "Support"] as const;

/** What a new character of `cls` wears: the class starter kit (`Gear::starter`), by slot. */
export function starterEquipment(cls: Class): number[] {
  const out: number[] = EQUIP_SLOTS.map(() => EMPTY_ITEM);
  const kits = balance.items.starter_kits as Record<string, (string | null)[]>;
  for (const name of kits[KIT_CLASS[cls] ?? ""] ?? []) {
    if (!name) continue;
    const base = lowerFirst(name);
    const i = EQUIP_SLOTS.indexOf(homeSlot(base));
    if (i >= 0 && out[i] === EMPTY_ITEM) out[i] = ITEM_BASES.indexOf(base);
  }
  return out;
}

/** A render request with only the item's sprite, or null for a base without one. */
export function itemRequest(base: string): RenderRequest | null {
  const l = itemLayer(base, RAMP.cold);
  if (!l) return null;
  const layers: LayerRef[] = Array.from({ length: LAYERS_MAX }, () => ({ spriteId: 0, materialRamp: 0, accentRamp: 0, elementRamp: 0 }));
  layers[0] = l;
  return renderRequest(layers);
}
