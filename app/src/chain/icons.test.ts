import { describe, expect, it } from "vitest";
import { ICON, iconRequest } from "./icons";
import { REWARD_KINDS } from "./loot";

describe("iconRequest", () => {
  it("gives every resource id 0..7 and every non-resource kind an icon sprite in 600..799", () => {
    const sprite = (kind: number, id: number) => iconRequest({ kind, id, qty: 1 })?.layers[0]?.spriteId;
    for (let id = 0; id < 8; id++) {
      const s = sprite(0, id);
      expect(s, `resource ${id}`).toBeGreaterThanOrEqual(600);
      expect(s).toBeLessThan(800);
    }
    for (let kind = 1; kind < REWARD_KINDS.length; kind++) expect(sprite(kind, 0), REWARD_KINDS[kind]).toBeGreaterThanOrEqual(600);
    expect(sprite(1, 0)).toBe(ICON.recipe);
    expect(sprite(2, 3)).toBe(ICON.potion);
  });

  it("has no icon for an unknown resource or kind", () => {
    expect(iconRequest({ kind: 0, id: 99, qty: 1 })).toBeNull();
    expect(iconRequest({ kind: 9, id: 0, qty: 1 })).toBeNull();
  });
});
