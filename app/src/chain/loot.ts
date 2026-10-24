// Patch 11: chests and the reward budget of a map as the builder shows them (docs/MAP_DESCRIPTION.md
// 7.1 and 10, docs/PARTY_AND_TURNS.md 9.2). A mirror of `deadroll_mapcheck::loot::check_tables`
// over the numbers of balance.json (`LootTuning`) and the registry `rewards` table; the authority
// is crates/mapcheck inside map.finalize_map, which also checks where the chests stand.
// Fixture: crates/rules/tests/loot_ts_fixtures.rs.
import balance from "./balance.json";

/** `CHESTS_MAX` and `NO_TABLE` (crates/types). */
export const CHESTS_MAX = 4;
export const NO_TABLE = 0xff;
/** `PARTY_MAX` (crates/types): the map's minimum players is 1..=4. */
export const MIN_PLAYERS_MAX = 4;

/** Reward table record (`deadroll_rules::loot::record`): size and the builder's bytes. */
const RECORD_SIZE = 108;
const PRESENT_AT = 0;
const LEVEL_BASE_AT = 1;
const LEVEL_MAX_AT = 2;
const COST_AT = 4;

/** The top monster level of each tier (`deadroll_mapcheck::limits::tier_limits`, `level_max`). */
const TIER_LEVEL_CAP: Record<number, number> = { 1: 2, 2: 4 };

/** A reward table as the builder sees it. */
export type RewardTable = { id: number; levelBase: number; levelMax: number; cost: number };

/** The loot limits of one tier (`deadroll_rules::loot::map_rules`). */
export type TierLoot = { chests: number; budget: number[]; allowed: bigint; levelCap: number };

/** A visible chest in the builder: its cell and reward table id. */
export type Chest = { x: number; y: number; table: number };

/** `MapCheckError` names of the table rules, in the order the check runs them. */
export type TableError =
  | "MinPlayersOutOfRange"
  | "TooManyChests"
  | "UnknownTable"
  | "TableNotAllowed"
  | "BossTableMissing"
  | "BossTableOneLevel"
  | "OverBudget";

export const TABLE_ERROR_MESSAGES: Record<TableError, string> = {
  MinPlayersOutOfRange: `Minimum players must be 1 to ${MIN_PLAYERS_MAX}`,
  TooManyChests: "Too many chests for this tier",
  UnknownTable: "A reward table does not exist in the registry",
  TableNotAllowed: "A reward table is not allowed on this tier",
  BossTableMissing: "Choose the boss reward table",
  BossTableOneLevel: "The boss table needs at least two levels",
  OverBudget: "Chests and boss cost more than the budget",
};

/** The registry `rewards` body as a list of tables (empty ids left out). */
export function rewardTables(body: Uint8Array): RewardTable[] {
  const out: RewardTable[] = [];
  for (let id = 0; (id + 1) * RECORD_SIZE <= body.length; id++) {
    const r = body.subarray(id * RECORD_SIZE, (id + 1) * RECORD_SIZE);
    if (r[PRESENT_AT] !== 1) continue;
    out.push({ id, levelBase: r[LEVEL_BASE_AT]!, levelMax: r[LEVEL_MAX_AT]!, cost: r[COST_AT]! | (r[COST_AT + 1]! << 8) });
  }
  return out;
}

/** The limits of `tier`, null for a tier without numbers. */
export function tierLoot(tier: number): TierLoot | null {
  const i = tier - 1;
  const t = balance.loot;
  const levelCap = TIER_LEVEL_CAP[tier];
  if (i < 0 || i >= t.chests.length || levelCap === undefined) return null;
  return { chests: t.chests[i]!, budget: t.budget[i]!, allowed: BigInt(t.allowed_tables[i]!), levelCap };
}

/** The budget of a tier for the map's minimum players (0 when out of range). */
export function budgetOf(tier: number, minPlayers: number): number {
  return tierLoot(tier)?.budget[minPlayers - 1] ?? 0;
}

/** A table may stand on `tier`: in the tier's mask and not above its top level. */
export function tableAllowed(t: RewardTable, tier: number): boolean {
  const rules = tierLoot(tier);
  if (!rules || t.id >= 64) return false;
  return ((rules.allowed >> BigInt(t.id)) & 1n) === 1n && t.levelMax <= rules.levelCap;
}

/** Budget points the chests and the boss table spend (unknown tables count 0). */
export function spentOf(chests: number[], boss: number, tables: RewardTable[]): number {
  const cost = (id: number) => tables.find((t) => t.id === id)?.cost ?? 0;
  return chests.reduce((sum, id) => sum + cost(id), boss === NO_TABLE ? 0 : cost(boss));
}

/** `check_tables`: the points spent, or the first broken rule. */
export function checkTables(tier: number, minPlayers: number, chests: number[], boss: number, tables: RewardTable[]): { ok: number } | { err: TableError } {
  const rules = tierLoot(tier);
  if (!rules) throw new Error(`no loot numbers for tier ${tier}`);
  if (minPlayers < 1 || minPlayers > MIN_PLAYERS_MAX) return { err: "MinPlayersOutOfRange" };
  if (chests.length > CHESTS_MAX || chests.length > rules.chests) return { err: "TooManyChests" };
  const allowed = (id: number): RewardTable | TableError => {
    const t = tables.find((x) => x.id === id);
    if (!t) return "UnknownTable";
    return tableAllowed(t, tier) ? t : "TableNotAllowed";
  };
  let spent = 0;
  for (const id of chests) {
    const t = allowed(id);
    if (typeof t === "string") return { err: t };
    spent += t.cost;
  }
  if (boss === NO_TABLE) return { err: "BossTableMissing" };
  const b = allowed(boss);
  if (typeof b === "string") return { err: b };
  if (b.levelMax <= b.levelBase) return { err: "BossTableOneLevel" };
  spent += b.cost;
  if (spent > rules.budget[minPlayers - 1]!) return { err: "OverBudget" };
  return { ok: spent };
}

/** Tooltip of a chest or a table: its level range. */
export function levelsLabel(t: RewardTable | undefined): string {
  if (!t) return "unknown table";
  return t.levelBase === t.levelMax ? `level ${t.levelBase}` : `levels ${t.levelBase}-${t.levelMax}`;
}

/** `RewardKind` (crates/types), in discriminant order. */
export const REWARD_KINDS = ["resource", "recipe", "consumable", "blank", "reagent"] as const;
export type RewardKindName = (typeof REWARD_KINDS)[number];
/** One reward in a cache or a bag slot. */
export type Reward = { kind: number; id: number; qty: number };

/** `Participation.cache`: `CacheSlot`s of 5 bytes (kind, id u16 LE, qty u16 LE), used ones only
 * (`CacheSlot::read_all`; a kind byte out of range reads as empty). */
export function cacheRewards(raw: ArrayLike<number>): Reward[] {
  const out: Reward[] = [];
  for (let at = 0; at + 5 <= raw.length; at += 5) {
    const kind = raw[at]!;
    const qty = raw[at + 3]! | (raw[at + 4]! << 8);
    if (qty > 0 && kind < REWARD_KINDS.length) out.push({ kind, id: raw[at + 1]! | (raw[at + 2]! << 8), qty });
  }
  return out;
}

/** A reward as text until the item names and icons exist (placeholder). */
export function rewardLabel(r: Reward): string {
  return `${REWARD_KINDS[r.kind] ?? "?"} #${r.id} x${r.qty}`;
}

/** The DM's deadline to settle (`settlement::deadline`): unix seconds. A `mock-vrf` localnet build
 * waits 20 s instead; the site shows the real number. */
export function settleDeadline(endedAt: bigint | number): number {
  return Number(endedAt) + balance.loot.settle_timeout_secs;
}

/** `LootTuning.chest_open_ap`: what opening a chest costs. */
export const CHEST_OPEN_AP = balance.loot.chest_open_ap;

/** The visible chests of a session (`GameSession.chests`, `chest_count` used) and which are open. */
export function chestsOf(s: { chests: { x: number; y: number; data: number }[]; chestCount: number; chestsOpened: number }) {
  return s.chests.slice(0, Math.min(s.chestCount, CHESTS_MAX)).map((c, index) => ({
    index,
    x: c.x,
    y: c.y,
    table: c.data,
    opened: (s.chestsOpened & (1 << index)) !== 0,
  }));
}
