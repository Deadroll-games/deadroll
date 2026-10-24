// Paths for move_path (patch 13), as tests/demo.test.ts `route`: cells in order over the 8
// neighbors (a diagonal is one step), passable cells not taken by a living unit or a closed chest,
// step costs from the Tileset (`GameSession.tile_cost` by tile type, a cell byte's bits 7-3). The
// chain checks the path itself (deadroll_mapcheck::walk); this only plans it.
import { MAP_MAX, Tile, cellByte, cellKind, type Cell } from "./types";

export const MAX_PATH = 16;
const WALL = cellByte(Tile.Wall);
const DIRS: [number, number][] = [
  [0, -1],
  [1, -1],
  [1, 0],
  [1, 1],
  [0, 1],
  [-1, 1],
  [-1, 0],
  [-1, -1],
];

/** One cell of a path and what stepping onto it costs. */
export type Step = { cost: number; to: Cell };

export type Ground = {
  tiles: Uint8Array;
  width: number;
  height: number;
  tileCost: ArrayLike<number>;
  /** Cells held by a living unit (not the mover) or a closed chest, as `y * 32 + x`. */
  taken: Set<number>;
};

export const cheb = (a: Cell, b: Cell) => Math.max(Math.abs(a.x - b.x), Math.abs(a.y - b.y));
/** `b` is one of the 8 neighbors of `a` (`deadroll_mapcheck::walk::adjacent`). */
export const adjacent = (a: Cell, b: Cell) => cheb(a, b) === 1;

/** Action points to step onto `(x, y)`, 0 = cannot. */
export function costAt(g: Ground, x: number, y: number): number {
  if (x < 0 || y < 0 || x >= g.width || y >= g.height) return 0;
  if (g.taken.has(y * MAP_MAX + x)) return 0;
  return g.tileCost[cellKind(g.tiles[y * MAP_MAX + x] ?? WALL)] ?? 0;
}

/** Cheapest steps from `from` to the first cell where `goal` holds. */
export function route(opts: Ground & { from: Cell; goal: (x: number, y: number) => boolean }): Step[] | null {
  const { from, goal } = opts;
  const start = from.y * MAP_MAX + from.x;
  const dist = new Map<number, number>([[start, 0]]);
  const prev = new Map<number, [number, number]>();
  const done = new Set<number>();
  for (;;) {
    let best = -1;
    for (const [k, d] of dist) if (!done.has(k) && (best < 0 || d < dist.get(best)!)) best = k;
    if (best < 0) return null;
    done.add(best);
    const x = best % MAP_MAX;
    const y = Math.floor(best / MAP_MAX);
    if (goal(x, y)) {
      const steps: Step[] = [];
      for (let k = best; prev.has(k); k = prev.get(k)![0]) {
        steps.unshift({ cost: prev.get(k)![1], to: { x: k % MAP_MAX, y: Math.floor(k / MAP_MAX) } });
      }
      return steps;
    }
    for (const [dx, dy] of DIRS) {
      const c = costAt(opts, x + dx, y + dy);
      if (!c) continue;
      const k = (y + dy) * MAP_MAX + x + dx;
      const d = dist.get(best)! + c;
      if (!dist.has(k) || d < dist.get(k)!) {
        dist.set(k, d);
        prev.set(k, [best, c]);
      }
    }
  }
}

/** The prefix of `steps` affordable with `ap - keep` action points, at most MAX_PATH steps. */
export function affordable(steps: Step[], ap: number, keep = 0): Step[] {
  const take: Step[] = [];
  let spent = 0;
  for (const st of steps) {
    if (spent + st.cost > ap - keep || take.length === MAX_PATH) break;
    take.push(st);
    spent += st.cost;
  }
  return take;
}

export const pathCost = (steps: Step[]) => steps.reduce((n, s) => n + s.cost, 0);

/**
 * A path built cell by cell (the player marks the cells): `to` next to the path's end is one more
 * step; a farther cell appends the cheapest way there. Null when it cannot be added (blocked, out of
 * action points, past MAX_PATH): the path stays as it was.
 */
export function extendPath(g: Ground, from: Cell, path: Step[], to: Cell, ap: number): Step[] | null {
  const end = path.at(-1)?.to ?? from;
  if (end.x === to.x && end.y === to.y) return null;
  // A cell already on the path cuts it back there.
  const back = path.findIndex((s) => s.to.x === to.x && s.to.y === to.y);
  if (back >= 0) return path.slice(0, back + 1);
  if (to.x === from.x && to.y === from.y) return [];
  let more: Step[] | null;
  if (adjacent(end, to)) {
    const cost = costAt(g, to.x, to.y);
    more = cost ? [{ cost, to }] : null;
  } else {
    more = route({ ...g, from: end, goal: (x, y) => x === to.x && y === to.y });
  }
  if (!more || more.length === 0) return null;
  const next = [...path, ...more];
  return next.length <= MAX_PATH && pathCost(next) <= ap ? next : null;
}

/** Cells reachable from `from` with at most `ap` action points (for the move highlight). */
export function reachable(opts: Ground & { from: Cell; ap: number }): Cell[] {
  const { from, ap } = opts;
  const dist = new Map<number, number>([[from.y * MAP_MAX + from.x, 0]]);
  const queue = [from.y * MAP_MAX + from.x];
  while (queue.length) {
    queue.sort((a, b) => dist.get(a)! - dist.get(b)!);
    const k = queue.shift()!;
    const x = k % MAP_MAX;
    const y = Math.floor(k / MAP_MAX);
    for (const [dx, dy] of DIRS) {
      const c = costAt(opts, x + dx, y + dy);
      if (!c) continue;
      const nk = (y + dy) * MAP_MAX + x + dx;
      const d = dist.get(k)! + c;
      if (d > ap || (dist.has(nk) && dist.get(nk)! <= d)) continue;
      dist.set(nk, d);
      queue.push(nk);
    }
  }
  dist.delete(from.y * MAP_MAX + from.x);
  return [...dist.keys()].map((k) => ({ x: k % MAP_MAX, y: Math.floor(k / MAP_MAX) }));
}
