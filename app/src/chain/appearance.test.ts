import { describe, expect, it } from "vitest";
import { EquipSource } from "@deadroll/sdk/character";
import { LAYER, MATERIAL, TOMBSTONE, characterAppearance, isCurrentAppearance } from "./appearance";
import { EMPTY_ITEM, EQUIP_SLOTS, ITEM_BASES, ITEM_SPRITE_BASE, characterRequest, dressed, starterEquipment } from "./items";
import type { CharacterAcc } from "./game";
import { Aspect, Class, Race } from "./types";

const slot = (name: string) => EQUIP_SLOTS.indexOf(name);
const base = (name: string) => ITEM_BASES.indexOf(name);

describe("characterAppearance", () => {
  it("holds the body as one layer, its hair and the shared front fist, and no items", () => {
    for (const [race, body, hair, material] of [
      [Race.Human, 10, 11, MATERIAL.human],
      [Race.ForestElf, 12, 13, MATERIAL.elf],
    ] as const) {
      const a = characterAppearance(race, Aspect.Fire);
      expect(a[LAYER.body]).toMatchObject({ spriteId: body, materialRamp: material });
      expect(a[LAYER.hair]).toMatchObject({ spriteId: hair, materialRamp: material });
      expect(a[LAYER.fistFront]).toMatchObject({ spriteId: 14, materialRamp: material });
      for (const l of [LAYER.mainhand, LAYER.armorBody, LAYER.offhand, LAYER.helmet, LAYER.pants, LAYER.boots, LAYER.backItem]) {
        expect(a[l]!.spriteId).toBe(0);
      }
    }
  });

  it("keeps the shield over the armor and the hair, the fist over the weapon", () => {
    expect(LAYER.armorBody).toBeLessThan(LAYER.offhand);
    expect(LAYER.hair).toBeLessThan(LAYER.offhand);
    expect(LAYER.mainhand).toBeLessThan(LAYER.fistFront);
    expect(LAYER.fistFront).toBeLessThan(LAYER.gloveFront);
  });
});

describe("dressed", () => {
  it("wears each class starter kit: one sprite per item base, 100 + ItemBase", () => {
    for (const [cls, weapon, armor] of [
      [Class.Warrior, "sword", "leatherArmor"],
      [Class.Mage, "staff", "robe"],
      [Class.Support, "wand", "lightLeatherArmor"],
    ] as const) {
      const eq = starterEquipment(cls);
      expect(eq[slot("mainHand")]).toBe(base(weapon));
      expect(eq[slot("body")]).toBe(base(armor));
      const a = dressed(characterAppearance(Race.ForestElf, Aspect.Cold), eq);
      expect(a[LAYER.mainhand]!.spriteId).toBe(ITEM_SPRITE_BASE + base(weapon));
      expect(a[LAYER.armorBody]!.spriteId).toBe(ITEM_SPRITE_BASE + base(armor));
      expect(a[LAYER.body]!.spriteId).toBe(12);
      expect(a.filter((l) => l.spriteId !== 0)).toHaveLength(6);
    }
  });

  it("draws only what is worn: unequipping clears the layer, equipping fills it", () => {
    const body = characterAppearance(Race.Human, Aspect.Fire);
    const eq = starterEquipment(Class.Warrior);
    eq[slot("body")] = EMPTY_ITEM;
    const noArmor = dressed(body, eq);
    expect(noArmor[LAYER.armorBody]!.spriteId).toBe(0);
    expect(noArmor[LAYER.mainhand]!.spriteId).toBe(ITEM_SPRITE_BASE + base("sword"));
    eq[slot("body")] = base("robe");
    expect(dressed(body, eq)[LAYER.armorBody]!.spriteId).toBe(ITEM_SPRITE_BASE + base("robe"));
    const naked = dressed(body, EQUIP_SLOTS.map(() => EMPTY_ITEM));
    expect(naked.filter((l) => l.spriteId !== 0)).toHaveLength(4);
  });
});

describe("characterRequest", () => {
  const char = (alive: boolean, worn: Record<string, string>): CharacterAcc => {
    const bases = Object.values(worn);
    return {
      core: { alive, appearance: characterAppearance(Race.Human, Aspect.Fire) },
      gear: {
        items: bases.map((b) => ({ base: base(b), level: 1 })),
        itemCount: bases.length,
        equipped: EQUIP_SLOTS.map((s) => {
          const i = Object.keys(worn).indexOf(s);
          return i < 0 ? { source: EquipSource.Empty, index: 0, asset: "" } : { source: EquipSource.Soulbound, index: i, asset: "" };
        }),
      },
    } as unknown as CharacterAcc;
  };

  it("dresses a living character in its gear and shows a tombstone once dead", () => {
    const r = characterRequest(char(true, { mainHand: "wand", body: "lightLeatherArmor" }));
    expect(r.layers[LAYER.mainhand]!.spriteId).toBe(ITEM_SPRITE_BASE + base("wand"));
    expect(r.layers[LAYER.armorBody]!.spriteId).toBe(ITEM_SPRITE_BASE + base("lightLeatherArmor"));
    expect(characterRequest(char(false, { mainHand: "wand" }))).toBe(TOMBSTONE);
  });
});

describe("isCurrentAppearance", () => {
  it("accepts the bodies and refuses appearances from before the one-piece bodies", () => {
    for (const race of [Race.Human, Race.ForestElf]) {
      expect(isCurrentAppearance(characterAppearance(race, Aspect.Cold))).toBe(true);
    }
    // Patch 7 and older: arm_back at slot 2, legs 5, torso 8, head 11, hair 12, arm_front 14.
    const old = Array.from({ length: 20 }, () => ({ spriteId: 0, materialRamp: 1, accentRamp: 13, elementRamp: 0 }));
    for (const [s, id] of [[2, 2], [5, 3], [8, 4], [11, 5], [12, 6], [14, 7]] as const) old[s] = { ...old[s]!, spriteId: id };
    expect(isCurrentAppearance(old)).toBe(false);
    expect(isCurrentAppearance(old.map((l, i) => (i === 2 ? { ...l, spriteId: 20 } : l)))).toBe(false);
    expect(isCurrentAppearance([])).toBe(false);
  });
});
