// Which way a monster is drawn. Monster sprites face left (toward the heroes, ART-3); the grid
// mirrors one to face the hero it attacked last, else the nearest living hero. Picture only.
import type { ChainEvent } from "./events";
import { MONSTER_BASE } from "./game";
import type { Cell } from "./types";

export type FacingHero = Cell & { slot: number; alive: boolean };

/**
 * Per monster index, the hero slot it attacked last in `events` (oldest first): the target of its
 * latest `ActionRequested` or `HitResolved` that names a hero.
 */
export function lastTargets(events: readonly ChainEvent[]): Map<number, number> {
  const out = new Map<number, number>();
  for (const e of events) {
    if (e.name !== "ActionRequested" && e.name !== "HitResolved") continue;
    const { attacker, target } = e.data as { attacker: number; target: number };
    if (attacker >= MONSTER_BASE && target < MONSTER_BASE) out.set(attacker - MONSTER_BASE, target);
  }
  return out;
}

/**
 * True when the monster at `monster` should be mirrored to face right: the hero it attacked last
 * (`lastTarget`, while alive), else the nearest living hero (Chebyshev, ties to the lower slot),
 * stands to its right. A hero in the same column, or no living hero, keeps the sprite facing left.
 */
export function facesRight(monster: Cell, heroes: readonly FacingHero[], lastTarget: number | undefined): boolean {
  const alive = heroes.filter((h) => h.alive);
  const dist = (h: Cell) => Math.max(Math.abs(h.x - monster.x), Math.abs(h.y - monster.y));
  const target =
    alive.find((h) => h.slot === lastTarget) ??
    alive.reduce<FacingHero | undefined>((best, h) => (!best || dist(h) < dist(best) || (dist(h) === dist(best) && h.slot < best.slot) ? h : best), undefined);
  return target !== undefined && target.x > monster.x;
}
