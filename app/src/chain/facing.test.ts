import { describe, expect, it } from "vitest";
import type { ChainEvent } from "./events";
import { facesRight, lastTargets } from "./facing";
import { MONSTER_BASE } from "./game";

const hero = (slot: number, x: number, y: number, alive = true) => ({ slot, x, y, alive });
const ev = (name: string, attacker: number, target: number) => ({ name, data: { attacker, target } }) as unknown as ChainEvent;

describe("monster facing", () => {
  it("the last hero each monster attacked, from requests and hits; heroes and monster targets are ignored", () => {
    const targets = lastTargets([
      ev("ActionRequested", MONSTER_BASE, 1),
      ev("HitResolved", MONSTER_BASE, 1),
      ev("HitResolved", 2, MONSTER_BASE), // a hero attacks
      ev("Moved", MONSTER_BASE, 0),
      ev("ActionRequested", MONSTER_BASE + 2, 0),
      ev("HitResolved", MONSTER_BASE, 3),
      ev("HitResolved", MONSTER_BASE + 1, MONSTER_BASE + 2), // monster on monster
    ]);
    expect([...targets.entries()].sort()).toEqual([
      [0, 3],
      [2, 0],
    ]);
  });

  it("faces the hero it attacked last, while that hero lives", () => {
    const heroes = [hero(0, 1, 5), hero(1, 9, 9)];
    expect(facesRight({ x: 5, y: 5 }, heroes, 1)).toBe(true);
    expect(facesRight({ x: 5, y: 5 }, heroes, 0)).toBe(false);
    // The target died: the nearest living hero (slot 0, to the left).
    expect(facesRight({ x: 5, y: 5 }, [hero(0, 1, 5), hero(1, 9, 9, false)], 1)).toBe(false);
  });

  it("without a target, the nearest living hero (Chebyshev, ties to the lower slot)", () => {
    expect(facesRight({ x: 5, y: 5 }, [hero(0, 1, 5), hero(1, 7, 7)], undefined)).toBe(true);
    expect(facesRight({ x: 5, y: 5 }, [hero(2, 8, 5), hero(1, 2, 5)], undefined)).toBe(false);
    expect(facesRight({ x: 5, y: 5 }, [hero(0, 1, 5, false), hero(1, 7, 7, false)], undefined)).toBe(false);
  });

  it("a hero in the same column keeps the sprite facing left", () => {
    expect(facesRight({ x: 5, y: 5 }, [hero(0, 5, 2)], 0)).toBe(false);
  });
});
