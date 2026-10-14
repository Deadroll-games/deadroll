// @vitest-environment node
// Items client: slot and base lists from the IDL, byte-level equip instructions, the gear of a
// Character account, and the item numbers against the program defaults (balance.json).
import { createHash } from "node:crypto";
import { describe, expect, it } from "vitest";
import { AccountRole, generateKeyPairSigner, getAddressDecoder, type Address } from "@solana/kit";
import { PROGRAMS } from "../config";
import { concat, toHex } from "./bytes";
import type { CharacterAcc } from "./game";
import {
  EMPTY_ITEM,
  EQUIP_SLOTS,
  ITEM_BASES,
  bonusText,
  equipIx,
  equipmentOf,
  heroDefense,
  inventory,
  itemBonus,
  itemLabel,
  itemRequest,
  slotsFor,
  unequipIx,
  weaponText,
} from "./items";
import { coders } from "./programs";
import { Class } from "./types";

const A = (n: number) => new Uint8Array(32).fill(n);
const addrOf = (n: number): Address => getAddressDecoder().decode(A(n));
const ixDisc = (name: string) => createHash("sha256").update(`global:${name}`).digest().subarray(0, 8);

const SLOT = Object.fromEntries(EQUIP_SLOTS.map((s, i) => [s, i]));
const BASE = Object.fromEntries(ITEM_BASES.map((b, i) => [b, i]));
/** A worn-items snapshot with `on` (slot key -> base key). */
const worn = (on: Record<string, string>) => EQUIP_SLOTS.map((s) => (on[s] ? BASE[on[s]]! : EMPTY_ITEM));

describe("IDLs", () => {
  it("slots and bases come from the character IDL in the crates/types order", () => {
    expect(EQUIP_SLOTS).toEqual(["mainHand", "offHand", "helmet", "body", "pants", "boots", "gloves", "necklace", "cloak", "ring1", "ring2"]);
    expect(ITEM_BASES).toEqual(["sword", "staff", "wand", "leatherArmor", "robe", "lightLeatherArmor"]);
    expect(itemLabel("leatherArmor")).toBe("Leather armor");
    expect(itemLabel("ring2")).toBe("Ring 2");
    expect(itemLabel("mainHand")).toBe("Main hand");
  });

  it("the items program has no instructions yet (gear lives in the character)", () => {
    expect(coders.items.idl.address).toBe(PROGRAMS.items);
    expect(coders.items.idl.instructions).toEqual([]);
  });
});

describe("instructions", () => {
  it("equip: character program, item index u8 and the slot; signer, character (w), token or None", async () => {
    const key = await generateKeyPairSigner();
    const character = addrOf(8);
    const ix = await equipIx(key, character, 1, "body", addrOf(9));
    expect(ix.programAddress).toBe(PROGRAMS.character);
    expect(toHex(ix.data as Uint8Array)).toBe(toHex(concat(ixDisc("equip"), Uint8Array.of(1, SLOT.body!))));
    expect(ix.accounts!.map((a) => [a.address, a.role])).toEqual([
      [key.address, AccountRole.READONLY_SIGNER],
      [character, AccountRole.WRITABLE],
      [addrOf(9), AccountRole.READONLY],
    ]);
    const off = await unequipIx(key, character, "ring2", null);
    expect(off.programAddress).toBe(PROGRAMS.character);
    expect(toHex(off.data as Uint8Array)).toBe(toHex(concat(ixDisc("unequip"), Uint8Array.of(SLOT.ring2!))));
    // No token: Anchor's None is the program id.
    expect(off.accounts![2]).toEqual({ address: PROGRAMS.character, role: AccountRole.READONLY });
  });
});

/** A new warrior as create_character writes it: the class starter kit, worn. */
function warrior(): Uint8Array {
  const c = coders.character;
  const empty = { source: "empty", index: 0, asset: addrOf(0) };
  const layer = { spriteId: 0, materialRamp: 0, accentRamp: 0, elementRamp: 0 };
  const v = {
    core: {
      version: 2,
      owner: addrOf(2),
      index: 0,
      nft: addrOf(3),
      race: "human",
      class: "warrior",
      level: 1,
      xp: 0,
      stats: [10, 5, 5, 10],
      predisposition: "piercing",
      predispositionTier: 14,
      hpMax: 60,
      alive: true,
      inSession: null,
      appearance: Array.from({ length: 20 }, () => layer),
      stateVersion: 0,
      bump: 254,
    },
    name: new Uint8Array(32),
    unspentPoints: 0,
    nftFrozen: false,
    gear: {
      items: [
        { base: "sword", level: 1 },
        { base: "leatherArmor", level: 1 },
        { base: "sword", level: 0 },
        { base: "sword", level: 0 },
      ],
      itemCount: 2,
      equipped: EQUIP_SLOTS.map((s) => (s === "mainHand" ? { source: "soulbound", index: 0, asset: addrOf(0) } : s === "body" ? { source: "soulbound", index: 1, asset: addrOf(0) } : empty)),
    },
    classTier: 11,
  };
  const out: Uint8Array[] = [c.accountDiscriminator("Character")];
  c.encode({ defined: { name: "Character" } }, v, out);
  return concat(...out);
}

describe("Character.gear", () => {
  it("follows nft_frozen, before the class trait tier, in the crates/types ItemRecord / EquipRef layout", () => {
    const bytes = warrior();
    // 4 items x 2, item_count 1, 11 slots x (source 1, index 1, asset 32), then class_tier (1).
    const gear = bytes.length - (4 * 2 + 1 + 11 * 34) - 1;
    expect(bytes[bytes.length - 1]).toBe(11);
    expect(Array.from(bytes.subarray(gear, gear + 4))).toEqual([BASE.sword, 1, BASE.leatherArmor, 1]);
    const body = gear + 4 * 2 + 1 + SLOT.body! * 34;
    expect(Array.from(bytes.subarray(body, body + 2))).toEqual([1, 1]);
    const c = coders.character.decodeAccount<CharacterAcc>("Character", bytes);
    expect(c.core.owner).toBe(addrOf(2));
    expect(c.classTier).toBe(11);
    const inv = inventory(c.gear);
    expect(inv.items.map((i) => [i.base, i.worn])).toEqual([
      ["sword", "mainHand"],
      ["leatherArmor", "body"],
    ]);
    expect(inv.slots.filter((s) => s.item).map((s) => s.slot)).toEqual(["mainHand", "body"]);
    expect(equipmentOf(c.gear)).toEqual(worn({ mainHand: "sword", body: "leatherArmor" }));
  });
});

describe("item numbers (balance.json = deadroll_rules defaults)", () => {
  it("starter weapons keep the old class weapons, armor adds flat", () => {
    expect(weaponText("sword")).toBe("Blunt 4-8");
    expect(weaponText("staff")).toBe("Blunt 3-6");
    expect(weaponText("wand")).toBe("Blunt 2-5");
    expect(weaponText("robe")).toBeNull();
    expect(bonusText("leatherArmor")).toBe("+10 armor");
    expect(bonusText("sword")).toBe("damage Blunt 4-8");
    expect(slotsFor("sword")).toEqual(["mainHand"]);
    expect(slotsFor("lightLeatherArmor")).toEqual(["body"]);
  });

  it("item_bonus and hero defense of the starter kits", () => {
    const kit = worn({ mainHand: "sword", body: "leatherArmor" });
    expect(itemBonus(kit)).toEqual({ weapon: "sword", armor: 10, defense: 0, attack: 0, stats: [0, 0, 0, 0], hp: 0 });
    expect(itemBonus(worn({ body: "robe" })).weapon).toBeNull();
    expect(itemBonus(worn({}))).toEqual({ weapon: null, armor: 0, defense: 0, attack: 0, stats: [0, 0, 0, 0], hp: 0 });
    // 10 + Dexterity 7 / 5; class armor 20 + leather 10 = the old 30.
    expect(heroDefense(Class.Warrior, [10, 7, 5, 8], kit)).toEqual({ defense: 11, armor: 30 });
    expect(heroDefense(Class.Warrior, [10, 7, 5, 8], worn({ mainHand: "sword" })).armor).toBe(20);
    expect(heroDefense(Class.Mage, [5, 7, 13, 5], worn({ mainHand: "staff", body: "robe" })).armor).toBe(5);
    expect(heroDefense(Class.Support, [5, 8, 12, 5], worn({ mainHand: "wand", body: "lightLeatherArmor" })).armor).toBe(5);
  });

  it("an item icon is its own sprite on layer 0", () => {
    const req = itemRequest("sword")!;
    expect(req.layers[0]!.spriteId).toBe(106);
    expect(req.layers.slice(1).every((l) => l.spriteId === 0)).toBe(true);
    for (const b of ITEM_BASES) expect(itemRequest(b), b).not.toBeNull();
  });
});
