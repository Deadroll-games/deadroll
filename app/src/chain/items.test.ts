// @vitest-environment node
// Items client: slot and base lists from the IDL, byte-level instructions, the Loadout decoder,
// and the item numbers against the program defaults (balance.json).
import { createHash } from "node:crypto";
import { describe, expect, it } from "vitest";
import { AccountRole, generateKeyPairSigner, getAddressDecoder, getAddressEncoder, type Address } from "@solana/kit";
import { PROGRAMS, SYSTEM_PROGRAM } from "../config";
import { concat, toHex } from "./bytes";
import {
  EMPTY_ITEM,
  EQUIP_SLOTS,
  ITEM_BASES,
  bonusText,
  equipIx,
  equipmentOf,
  heroDefense,
  initLoadoutIx,
  inventory,
  itemBonus,
  itemLabel,
  itemRequest,
  slotsFor,
  unequipIx,
  weaponText,
  type LoadoutAcc,
} from "./items";
import { loadoutPda } from "./pda";
import { coders } from "./programs";
import { Class } from "./types";

const A = (n: number) => new Uint8Array(32).fill(n);
const addrOf = (n: number): Address => getAddressDecoder().decode(A(n));
const pk = (a: Address) => getAddressEncoder().encode(a) as Uint8Array;
const ixDisc = (name: string) => createHash("sha256").update(`global:${name}`).digest().subarray(0, 8);

const SLOT = Object.fromEntries(EQUIP_SLOTS.map((s, i) => [s, i]));
const BASE = Object.fromEntries(ITEM_BASES.map((b, i) => [b, i]));
/** A worn-items snapshot with `on` (slot key -> base key). */
const worn = (on: Record<string, string>) => EQUIP_SLOTS.map((s) => (on[s] ? BASE[on[s]]! : EMPTY_ITEM));

describe("items IDL", () => {
  it("is for the items program id", () => {
    expect(coders.items.idl.address).toBe(PROGRAMS.items);
  });

  it("lists slots and bases in the crates/types order", () => {
    expect(EQUIP_SLOTS).toEqual(["mainHand", "offHand", "helmet", "body", "pants", "boots", "gloves", "necklace", "cloak", "ring1", "ring2"]);
    expect(ITEM_BASES).toEqual(["sword", "staff", "wand", "leatherArmor", "robe", "lightLeatherArmor"]);
    expect(itemLabel("leatherArmor")).toBe("Leather armor");
    expect(itemLabel("ring2")).toBe("Ring 2");
    expect(itemLabel("mainHand")).toBe("Main hand");
  });
});

describe("instructions", () => {
  it("init_loadout: owner (payer), character, the loadout PDA, system program", async () => {
    const owner = await generateKeyPairSigner();
    const character = addrOf(7);
    const ix = await initLoadoutIx(owner, character);
    expect(ix.programAddress).toBe(PROGRAMS.items);
    expect(toHex(ix.data as Uint8Array)).toBe(toHex(ixDisc("init_loadout")));
    expect(ix.accounts!.map((a) => [a.address, a.role])).toEqual([
      [owner.address, AccountRole.WRITABLE_SIGNER],
      [character, AccountRole.READONLY],
      [await loadoutPda(character), AccountRole.WRITABLE],
      [SYSTEM_PROGRAM, AccountRole.READONLY],
    ]);
  });

  it("equip: item index u8 and the slot; the session token or None", async () => {
    const key = await generateKeyPairSigner();
    const character = addrOf(8);
    const ix = await equipIx(key, character, 1, "body", addrOf(9));
    expect(toHex(ix.data as Uint8Array)).toBe(toHex(concat(ixDisc("equip"), Uint8Array.of(1, SLOT.body!))));
    expect(ix.accounts![0]).toMatchObject({ address: key.address, role: AccountRole.READONLY_SIGNER });
    expect(ix.accounts![2]).toEqual({ address: await loadoutPda(character), role: AccountRole.WRITABLE });
    expect(ix.accounts![3]).toEqual({ address: addrOf(9), role: AccountRole.READONLY });
    const owner = await unequipIx(key, character, "ring2", null);
    expect(toHex(owner.data as Uint8Array)).toBe(toHex(concat(ixDisc("unequip"), Uint8Array.of(SLOT.ring2!))));
    // No token: Anchor's None is the program id.
    expect(owner.accounts![3]).toEqual({ address: PROGRAMS.items, role: AccountRole.READONLY });
  });

  it("the loadout PDA is [\"loadout\", character] under items", async () => {
    const character = addrOf(5);
    const { getProgramDerivedAddress } = await import("@solana/kit");
    const [want] = await getProgramDerivedAddress({ programAddress: PROGRAMS.items, seeds: [new TextEncoder().encode("loadout"), pk(character)] });
    expect(await loadoutPda(character)).toBe(want);
  });
});

/** A warrior's loadout after init_loadout, as the program writes it. */
function warriorLoadout(): Uint8Array {
  const c = coders.items;
  const empty = { source: "empty", index: 0, asset: addrOf(0) };
  const v = {
    core: {
      version: 1,
      character: addrOf(1),
      owner: addrOf(2),
      items: [
        { base: "sword", level: 1 },
        { base: "leatherArmor", level: 1 },
        { base: "sword", level: 0 },
        { base: "sword", level: 0 },
      ],
      itemCount: 2,
      equipped: EQUIP_SLOTS.map((s) => (s === "mainHand" ? { source: "soulbound", index: 0, asset: addrOf(0) } : s === "body" ? { source: "soulbound", index: 1, asset: addrOf(0) } : empty)),
      bump: 254,
    },
  };
  const out: Uint8Array[] = [c.accountDiscriminator("Loadout")];
  c.encode({ defined: { name: "Loadout" } }, v, out);
  return concat(...out);
}

describe("Loadout", () => {
  it("decodes at the crates/types LoadoutData offsets", () => {
    const bytes = warriorLoadout();
    // discriminator 8, version 1, character 32, owner 32, 4 items x 2, item_count 1,
    // 11 slots x (source 1, index 1, asset 32), bump 1.
    expect(bytes.length).toBe(8 + 1 + 32 + 32 + 4 * 2 + 1 + 11 * 34 + 1);
    expect(Array.from(bytes.subarray(73, 77))).toEqual([BASE.sword, 1, BASE.leatherArmor, 1]);
    const body = 8 + 1 + 64 + 8 + 1 + SLOT.body! * 34;
    expect(Array.from(bytes.subarray(body, body + 2))).toEqual([1, 1]);
    const l = coders.items.decodeAccount<LoadoutAcc>("Loadout", bytes);
    expect(l.core.owner).toBe(addrOf(2));
    const inv = inventory(l);
    expect(inv.items.map((i) => [i.base, i.worn])).toEqual([
      ["sword", "mainHand"],
      ["leatherArmor", "body"],
    ]);
    expect(inv.slots.filter((s) => s.item).map((s) => s.slot)).toEqual(["mainHand", "body"]);
    expect(equipmentOf(l)).toEqual(worn({ mainHand: "sword", body: "leatherArmor" }));
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
