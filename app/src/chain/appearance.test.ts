import { describe, expect, it } from "vitest";
import { LAYER, MATERIAL, characterAppearance } from "./appearance";
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

  it("puts the warrior's shield over the armor and the fist over the sword", () => {
    const a = characterAppearance(Race.Human, Class.Warrior, Aspect.Fire);
    expect(a[LAYER.armorBody]!.spriteId).toBe(104);
    expect(a[LAYER.offhand]!.spriteId).toBe(101);
    expect(a[LAYER.mainhand]!.spriteId).toBe(106);
    expect(LAYER.armorBody).toBeLessThan(LAYER.offhand);
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
