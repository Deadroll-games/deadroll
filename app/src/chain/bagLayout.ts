// Patch 11: the bag screen (docs/ITEMS_AND_CRAFT.md 6.1, decisions of patch 11). The player lays
// out the settled cache and the bag as often as they like without a transaction; "Done" sends the
// final layout once (`close_cache`), and the chain accepts it only if
// `deadroll_rules::loot::check_bag_layout` does. `checkBagLayout` mirrors that check (fixture:
// crates/rules/tests/bag_ts_fixtures.rs); the moves below keep every layout within it.
import balance from "./balance.json";
import { REWARD_KINDS, type Reward } from "./loot";

/** `BAG_RESOURCE_SLOTS` and `BAG_CONSUMABLE_SLOTS` (crates/types). */
export const RESOURCE_SLOTS = 40;
export const CONSUMABLE_SLOTS = 5;
export const RESOURCE_STACK_MAX = balance.loot.resource_stack_max;
export const CONSUMABLE_STACK_MAX = balance.loot.consumable_stack_max;
const CONSUMABLE = REWARD_KINDS.indexOf("consumable");

export const EMPTY: Reward = { kind: 0, id: 0, qty: 0 };
/** A bag's resources and consumables (`BagLayout`), one reward per slot, `qty == 0` = empty. */
export type Bag = { resources: Reward[]; consumables: Reward[] };
export type Section = keyof Bag;
export type SlotRef = { section: Section; index: number };
/** The screen's state: the bag being laid out and what is left in the cache. */
export type BagState = { bag: Bag; cache: Reward[] };

/** `deadroll_rules::loot::BagError` names. */
export type BagError = "DirtyEmptySlot" | "WrongSection" | "StackTooBig" | "MoreThanOwned";

export const emptyBag = (): Bag => ({
  resources: Array.from({ length: RESOURCE_SLOTS }, () => ({ ...EMPTY })),
  consumables: Array.from({ length: CONSUMABLE_SLOTS }, () => ({ ...EMPTY })),
});

export const sectionOf = (kind: number): Section => (kind === CONSUMABLE ? "consumables" : "resources");
export const stackMax = (kind: number) => (kind === CONSUMABLE ? CONSUMABLE_STACK_MAX : RESOURCE_STACK_MAX);
const same = (a: Reward, b: Reward) => a.qty > 0 && b.qty > 0 && a.kind === b.kind && a.id === b.id;
const isEmpty = (r: Reward) => r.qty === 0;
const slotKey = (r: SlotRef) => `${r.section === "resources" ? "r" : "c"}:${r.index}`;

/** `check_bag_layout`: the first rule `after` breaks, or null. */
export function checkBagLayout(before: Bag, cache: Reward[], after: Bag): BagError | null {
  for (const section of ["resources", "consumables"] as const) {
    const consumable = section === "consumables";
    for (const s of after[section]) {
      if (isEmpty(s)) {
        if (s.kind !== 0 || s.id !== 0) return "DirtyEmptySlot";
        continue;
      }
      if ((s.kind === CONSUMABLE) !== consumable) return "WrongSection";
      if (s.qty > (consumable ? CONSUMABLE_STACK_MAX : RESOURCE_STACK_MAX)) return "StackTooBig";
    }
  }
  const all = (b: Bag) => [...b.resources, ...b.consumables];
  const total = (list: Reward[], r: Reward) => list.filter((x) => same(x, r)).reduce((n, x) => n + x.qty, 0);
  for (const s of all(after)) {
    if (isEmpty(s)) continue;
    if (total(all(after), s) > total(all(before), s) + total(cache, s)) return "MoreThanOwned";
  }
  return null;
}

/** The layout as `close_cache` takes it (empty slots all zero). */
export function layoutArgs(bag: Bag): Bag {
  const clean = (r: Reward) => (isEmpty(r) ? { ...EMPTY } : { ...r });
  return { resources: bag.resources.map(clean), consumables: bag.consumables.map(clean) };
}

function clone(s: BagState): BagState {
  return { bag: { resources: s.bag.resources.map((r) => ({ ...r })), consumables: s.bag.consumables.map((r) => ({ ...r })) }, cache: s.cache.map((r) => ({ ...r })) };
}

/** Adds `r` back to the cache, onto the same reward if it is there. */
function toCache(cache: Reward[], r: Reward) {
  const at = cache.find((c) => same(c, r));
  if (at) at.qty += r.qty;
  else cache.push({ ...r });
}

function dropEmpty(s: BagState): BagState {
  s.cache = s.cache.filter((c) => c.qty > 0);
  return s;
}

/**
 * Takes up to `qty` (default: all) of cache reward `ci` into slot `to` (empty, or the same
 * reward), up to a stack; `to` null: the first fitting slots, partial stacks of it first
 * (Shift + click). Slots in `locked` are skipped. Returns the new state, or null if nothing moved.
 */
export function take(state: BagState, ci: number, to: SlotRef | null, qty?: number, locked: ReadonlySet<string> = new Set()): BagState | null {
  const s = clone(state);
  const c = s.cache[ci];
  if (!c || isEmpty(c)) return null;
  const section = sectionOf(c.kind);
  const max = stackMax(c.kind);
  let left = Math.min(qty ?? c.qty, c.qty);
  const into = (index: number) => {
    const slot = s.bag[section][index]!;
    if (!isEmpty(slot) && !same(slot, c)) return;
    const n = Math.min(left, max - slot.qty);
    if (n <= 0) return;
    s.bag[section][index] = { kind: c.kind, id: c.id, qty: slot.qty + n };
    left -= n;
    c.qty -= n;
  };
  if (to) {
    if (to.section !== section || locked.has(slotKey(to))) return null;
    into(to.index);
  } else {
    const order = s.bag[section].map((_, i) => i).filter((i) => !locked.has(slotKey({ section, index: i })));
    for (const i of order.filter((i) => same(s.bag[section][i]!, c))) into(i);
    for (const i of order.filter((i) => isEmpty(s.bag[section][i]!))) if (left > 0) into(i);
  }
  return c.qty === state.cache[ci]!.qty ? null : dropEmpty(s);
}

/**
 * Click a cache reward, then a bag slot: the bag's reward goes back into the cache and the cache
 * reward takes its place (up to a stack; the rest stays in the cache). An empty slot or the same
 * reward just takes. A locked slot is never replaced.
 */
export function replace(state: BagState, ci: number, to: SlotRef, locked: ReadonlySet<string> = new Set()): BagState | null {
  const c = state.cache[ci];
  if (!c || isEmpty(c) || to.section !== sectionOf(c.kind) || locked.has(slotKey(to))) return null;
  const slot = state.bag[to.section][to.index]!;
  if (isEmpty(slot) || same(slot, c)) return take(state, ci, to, undefined, locked);
  const s = clone(state);
  const moved = s.cache[ci]!;
  const n = Math.min(moved.qty, stackMax(moved.kind));
  s.bag[to.section][to.index] = { kind: moved.kind, id: moved.id, qty: n };
  moved.qty -= n;
  toCache(s.cache, slot);
  return dropEmpty(s);
}

/** Moves slot `from` onto `to` in the same section: into an empty slot, onto the same reward up
 * to a stack, or the two swap. */
export function moveSlot(state: BagState, from: SlotRef, to: SlotRef): BagState | null {
  if (from.section !== to.section || from.index === to.index) return null;
  const s = clone(state);
  const list = s.bag[from.section];
  const a = list[from.index]!;
  const b = list[to.index]!;
  if (isEmpty(a)) return null;
  if (same(a, b)) {
    const n = Math.min(a.qty, stackMax(a.kind) - b.qty);
    if (n <= 0) return null;
    b.qty += n;
    a.qty -= n;
    if (a.qty === 0) list[from.index] = { ...EMPTY };
  } else {
    list[from.index] = b;
    list[to.index] = a;
  }
  return s;
}

/** Splits a stack in two: half (rounded down) into the first empty slot of its section. */
export function split(state: BagState, at: SlotRef): BagState | null {
  const r = state.bag[at.section][at.index]!;
  const half = Math.floor(r.qty / 2);
  const free = state.bag[at.section].findIndex(isEmpty);
  if (half === 0 || free < 0) return null;
  const s = clone(state);
  s.bag[at.section][at.index]!.qty -= half;
  s.bag[at.section][free] = { kind: r.kind, id: r.id, qty: half };
  return s;
}

/** Throws a slot away (it burns; the site asks first). A locked slot is kept. */
export function discard(state: BagState, at: SlotRef, locked: ReadonlySet<string> = new Set()): BagState | null {
  if (isEmpty(state.bag[at.section][at.index]!) || locked.has(slotKey(at))) return null;
  const s = clone(state);
  s.bag[at.section][at.index] = { ...EMPTY };
  return s;
}

export { slotKey };

// ---- slot locks: this browser only, any time (not the Ready lock of the party)

const LOCK_KEY = (character: string) => `deadroll.bagLock.v1:${character}`;

/** The locked slots of `character` in this browser (`"r:3"`, `"c:0"`). */
export function loadLocks(character: string): Set<string> {
  try {
    const raw = localStorage.getItem(LOCK_KEY(character));
    const list: unknown = raw ? JSON.parse(raw) : [];
    return new Set(Array.isArray(list) ? list.filter((x): x is string => typeof x === "string") : []);
  } catch {
    return new Set();
  }
}

export function saveLocks(character: string, locks: ReadonlySet<string>) {
  try {
    localStorage.setItem(LOCK_KEY(character), JSON.stringify([...locks]));
  } catch {
    // A private window or blocked storage: the locks last for this page only.
  }
}
