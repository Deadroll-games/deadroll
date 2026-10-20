import { describe, expect, it } from "vitest";
import { LAYER, MATERIAL, characterAppearance, isCurrentAppearance } from "./appearance";
import { Aspect, Class, Race } from "./types";

describe("characterAppearance", () => {
  it("draws the body as one layer with its hair and the shared front fist", () => {
    for (const [race, body, hair, material] of [
      [Race.Human, 10, 11, MATERIAL.human],
      [Race.ForestElf, 12, 13, MATERIAL.elf],
    ] as const) {
      const a = characterAppearance(race, Class.Warrior, Aspect.Fire);
      expect(a[LAYER.body]).toMatchObject({ spriteId: body, materialRamp: material });
      expect(a[LAYER.hair]).toMatchObject({ spriteId: hair, materialRamp: material });
      expect(a[LAYER.fistFront]).toMatchObject({ spriteId: 14, materialRamp: material });
    }
  });

  it("puts the warrior's shield over the armor and the hair, and the fist over the sword", () => {
    const a = characterAppearance(Race.Human, Class.Warrior, Aspect.Fire);
    expect(a[LAYER.armorBody]!.spriteId).toBe(104);
    expect(a[LAYER.offhand]!.spriteId).toBe(101);
    expect(a[LAYER.mainhand]!.spriteId).toBe(106);
    expect(LAYER.armorBody).toBeLessThan(LAYER.offhand);
    expect(LAYER.hair).toBeLessThan(LAYER.offhand);
    expect(LAYER.mainhand).toBeLessThan(LAYER.fistFront);
    expect(LAYER.fistFront).toBeLessThan(LAYER.gloveFront);
  });

  it("keeps the old body part ids out of every preset", () => {
    for (const race of [Race.Human, Race.ForestElf]) {
      for (const cls of [Class.Warrior, Class.Mage, Class.Support]) {
        const ids = characterAppearance(race, cls, Aspect.Cold).map((l) => l.spriteId);
        for (const old of [2, 3, 4, 5, 6, 7, 20, 21, 22, 23, 24, 25]) expect(ids).not.toContain(old);
      }
    }
  });
});

describe("isCurrentAppearance", () => {
  it("accepts the presets and refuses appearances from before the one-piece bodies", () => {
    for (const race of [Race.Human, Race.ForestElf]) {
      expect(isCurrentAppearance(characterAppearance(race, Class.Mage, Aspect.Cold))).toBe(true);
    }
    // Patch 7 and older: arm_back at slot 2, legs 5, torso 8, head 11, hair 12, arm_front 14.
    const old = Array.from({ length: 20 }, () => ({ spriteId: 0, materialRamp: 1, accentRamp: 13, elementRamp: 0 }));
    for (const [slot, id] of [[2, 2], [5, 3], [8, 4], [11, 5], [12, 6], [14, 7]] as const) old[slot] = { ...old[slot]!, spriteId: id };
    expect(isCurrentAppearance(old)).toBe(false);
    expect(isCurrentAppearance(old.map((l, i) => (i === 2 ? { ...l, spriteId: 20 } : l)))).toBe(false);
    expect(isCurrentAppearance([])).toBe(false);
  });
});
