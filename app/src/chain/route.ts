// Step paths for move_unit / monster_move, as tests/demo.test.ts `route` and `walk`: cheapest path
// over passable cells not taken by a living unit or a closed chest, step costs from the Tileset
// (`GameSession.tile_cost` by tile type, a cell byte's bits 7-3).
import { MAP_MAX, Tile, cellByte, cellKind, type Cell } from "./types";

const WALL = cellByte(Tile.Wall);

export const DIR = { N: 0, E: 1, S: 2, W: 3 } as const;
export const MAX_PATH = 16;
const DIRS: [number, number, number][] = [
  [DIR.N, 0, -1],
  [DIR.E, 1, 0],
  [DIR.S, 0, 1],
  [DIR.W, -1, 0],
];

export type Step = { dir: number; cost: number; to: Cell };

export const cheb = (a: Cell, b: Cell) => Math.max(Math.abs(a.x - b.x), Math.abs(a.y - b.y));

/**
 * Cheapest steps from `from` to the first cell where `goal` holds. `tileCost[kind]` is the AP cost
 * of a tile type (0 = blocked, `GameSession.tile_cost`).
 */
export function route(opts: {
  from: Cell;
  goal: (x: number, y: number) => boolean;
  tiles: Uint8Array;
  width: number;
  height: number;
  tileCost: number[];
  taken: Set<number>;
}): Step[] | null {
  const { from, goal, tiles, width, height, tileCost, taken } = opts;
  const cost = (x: number, y: number) => {
    if (x < 0 || y < 0 || x >= width || y >= height) return 0;
    if (taken.has(y * MAP_MAX + x)) return 0;
    return tileCost[cellKind(tiles[y * MAP_MAX + x] ?? WALL)] ?? 0;
  };
  const start = from.y * MAP_MAX + from.x;
  const dist = new Map<number, number>([[start, 0]]);
  const prev = new Map<number, [number, number, number]>();
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
        const [, dir, c] = prev.get(k)!;
        steps.unshift({ dir, cost: c, to: { x: k % MAP_MAX, y: Math.floor(k / MAP_MAX) } });
      }
      return steps;
    }
    for (const [dir, dx, dy] of DIRS) {
      const c = cost(x + dx, y + dy);
      if (!c) continue;
      const k = (y + dy) * MAP_MAX + x + dx;
      const d = dist.get(best)! + c;
      if (!dist.has(k) || d < dist.get(k)!) {
        dist.set(k, d);
        prev.set(k, [best, dir, c]);
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

/** `move_unit` / `monster_move` args: 16 direction bytes, zero padded, and the used length. */
export function pathArgs(steps: Step[]): { steps: number[]; len: number } {
  const dirs = steps.slice(0, MAX_PATH).map((s) => s.dir);
  const len = dirs.length;
  while (dirs.length < MAX_PATH) dirs.push(0);
  return { steps: dirs, len };
}

/** Cells reachable from `from` with at most `ap` action points (for the move highlight). */
export function reachable(opts: {
  from: Cell;
  ap: number;
  tiles: Uint8Array;
  width: number;
  height: number;
  tileCost: number[];
  taken: Set<number>;
}): Cell[] {
  const { from, ap, tiles, width, height, tileCost, taken } = opts;
  const dist = new Map<number, number>([[from.y * MAP_MAX + from.x, 0]]);
  const queue = [from.y * MAP_MAX + from.x];
  while (queue.length) {
    queue.sort((a, b) => dist.get(a)! - dist.get(b)!);
    const k = queue.shift()!;
    const x = k % MAP_MAX;
    const y = Math.floor(k / MAP_MAX);
    for (const [, dx, dy] of DIRS) {
      const nx = x + dx;
      const ny = y + dy;
      if (nx < 0 || ny < 0 || nx >= width || ny >= height) continue;
      const nk = ny * MAP_MAX + nx;
      const c = taken.has(nk) ? 0 : (tileCost[cellKind(tiles[nk] ?? WALL)] ?? 0);
      if (!c) continue;
      const d = dist.get(k)! + c;
      if (d > ap || (dist.has(nk) && dist.get(nk)! <= d)) continue;
      dist.set(nk, d);
      queue.push(nk);
    }
  }
  dist.delete(from.y * MAP_MAX + from.x);
  return [...dist.keys()].map((k) => ({ x: k % MAP_MAX, y: Math.floor(k / MAP_MAX) }));
}
