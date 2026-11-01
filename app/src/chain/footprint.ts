// Patch B: the cells a unit stands on, mirror of crates/mapcheck/src/footprint.rs and
// `deadroll_rules::line::line_ends` (fixture: crates/rules/tests/arena_ts_fixtures.rs).
// A unit's cell is the top-left one of its square, `size` cells per side (monsters 1 or 2).
import type { Cell } from "./types";

export interface Footprint {
  x: number;
  y: number;
  size: number;
}

/** A unit of `size` cells per side on `at` (0 counts as 1). */
export function footprint(at: Cell, size = 1): Footprint {
  return { x: at.x, y: at.y, size: size > 0 ? size : 1 };
}

function side(f: Footprint): number {
  return f.size > 0 ? f.size : 1;
}

/** The gap between two spans `[a, a + an)` and `[b, b + bn)`: 0 when they touch or overlap. */
function gap(a: number, an: number, b: number, bn: number): number {
  const aEnd = a + an - 1;
  const bEnd = b + bn - 1;
  return Math.max(0, b - aEnd) + Math.max(0, a - bEnd);
}

/** Chebyshev distance between the nearest cells of the two (0 when they overlap). */
export function squareDistance(a: Footprint, b: Footprint): number {
  return Math.max(gap(a.x, side(a), b.x, side(b)), gap(a.y, side(a), b.y, side(b)));
}

/** `c` is one of its cells. */
export function covers(f: Footprint, c: Cell): boolean {
  return gap(f.x, side(f), c.x, 1) === 0 && gap(f.y, side(f), c.y, 1) === 0;
}

/** Its cells, row by row. */
export function cellsOf(f: Footprint): Cell[] {
  const out: Cell[] = [];
  for (let dy = 0; dy < side(f); dy++) for (let dx = 0; dx < side(f); dx++) out.push({ x: f.x + dx, y: f.y + dy });
  return out;
}

/** Its cell nearest to `c` (`c` itself when covered). */
export function nearest(f: Footprint, c: Cell): Cell {
  const n = side(f) - 1;
  const clamp = (v: number, lo: number) => Math.min(Math.max(v, lo), Math.min(lo + n, 255));
  return { x: clamp(c.x, f.x), y: clamp(c.y, f.y) };
}

/** The ends of a line from a caster on `caster` at a unit on `target` (`line_ends`). */
export function lineEnds(caster: Footprint, target: Footprint): { from: Cell; through: Cell } {
  const through = nearest(target, nearest(caster, { x: target.x, y: target.y }));
  return { from: nearest(caster, through), through };
}
