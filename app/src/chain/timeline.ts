// Patch B: the arena's initiative timeline (docs/PARTY_AND_TURNS.md 6), mirror of
// `deadroll_rules::arena` (fixture: crates/rules/tests/arena_ts_fixtures.rs). The program keeps
// each unit's time until its turn (`GameSession.unit_time`); the turn bar shows the next turns as
// `upcoming` forecasts them.
import balance from "./balance.json";
import { ModKind } from "./types";
import { currentUnit, MONSTER_BASE, type Party, type TimedModAcc } from "./game";

export type ArenaTuning = {
  timeScale: number;
  initiativeBase: number;
  roundTime: number;
  slowInitiativeBp: number;
};

/** The program defaults (`BalanceConfig::DEFAULT.arena`). */
export const ARENA: ArenaTuning = {
  timeScale: balance.arena.time_scale,
  initiativeBase: balance.arena.initiative_base,
  roundTime: balance.arena.round_time,
  slowInitiativeBp: balance.arena.slow_initiative_bp,
};

const BP = 10_000;

/** `time_scale / (initiative_base + initiative)`, at least 1. */
export function delay(initiative: number, t: ArenaTuning = ARENA): number {
  const div = Math.max(1, t.initiativeBase + initiative);
  return Math.max(1, Math.floor(t.timeScale / div));
}

/** Initiative under a Slow of `slowBp`: a counterweight of `slow_initiative_bp` of its power. */
export function effectiveInitiative(initiative: number, slowBp: number, t: ArenaTuning = ARENA): number {
  const slow = Math.min(Math.floor((slowBp * t.slowInitiativeBp) / BP), 0xffff_ffff);
  return Number((BigInt(initiative) * BigInt(BP)) / BigInt(BP + slow));
}

/** The delay of a turn taken under a Slow of `slowBp`. */
export function turnDelay(initiative: number, slowBp: number, t: ArenaTuning = ARENA): number {
  return delay(effectiveInitiative(initiative, slowBp, t), t);
}

/** The Slow on a unit (0 without one). */
export function slowOf(mods: readonly TimedModAcc[] | undefined): number {
  const m = mods?.find((x) => x.turns > 0 && x.kind === ModKind.Slow);
  return m ? m.powerBp : 0;
}

export type TimelineUnit = { id: number; time: number; tie: number; alive: boolean };

/** The living unit with the lowest time; a tie to the higher `tie`, then the lower id. */
export function nextUnit(units: readonly TimelineUnit[]): number | null {
  let best: number | null = null;
  for (let i = 0; i < units.length; i++) {
    const u = units[i]!;
    if (!u.alive) continue;
    const b = best === null ? null : units[best]!;
    if (b === null || u.time < b.time || (u.time === b.time && (u.tie > b.tie || (u.tie === b.tie && u.id < b.id)))) best = i;
  }
  return best;
}

/** The next `n` unit ids `nextUnit` picks, each unit's later turns `delays[i]` apart. */
export function upcoming(units: readonly TimelineUnit[], delays: readonly number[], n: number): number[] {
  const line = units.map((u) => ({ ...u }));
  const out: number[] = [];
  while (out.length < n) {
    const i = nextUnit(line);
    if (i === null) break;
    out.push(line[i]!.id);
    line[i]!.time += Math.max(1, delays[i] ?? 1);
  }
  return out;
}

/** One entry of the turn bar. */
export type BarTurn = { unit: number; now: boolean };

/**
 * The arena's turn bar: the unit acting now, then the next `n - 1` turns the timeline forecasts.
 * After `start_turn` the current unit's time already holds its next turn; before it, its time is
 * the current one, so the forecast's first pick is the current unit itself.
 */
export function turnBar(party: Party, n: number): BarTurn[] {
  const s = party.session;
  const len = Math.min(s.turnOrderLen, s.core.turnOrder.length);
  const now = currentUnit(s);
  const units: TimelineUnit[] = [];
  const delays: number[] = [];
  for (let i = 0; i < len; i++) {
    const id = s.core.turnOrder[i]!;
    if (id < MONSTER_BASE) {
      const p = party.players.find((x) => x.slot === id);
      const alive = (s.aliveMask & (1 << id)) !== 0;
      units.push({ id, time: s.unitTime[id] ?? 0, tie: s.playerDex[id] ?? 0, alive });
      delays.push(turnDelay(s.playerInitiative[id] ?? 0, slowOf(p?.part.mods)));
    } else {
      const m = id - MONSTER_BASE;
      const initiative = s.monsterInitiative[m] ?? 0;
      units.push({ id, time: s.unitTime[id] ?? 0, tie: initiative, alive: party.monsters[m]?.alive ?? false });
      delays.push(turnDelay(initiative, slowOf(party.monsterMods[m])));
    }
  }
  const started = s.startedSeq === s.turnSeq;
  const next = upcoming(units, delays, n);
  if (now === null) return next.map((unit) => ({ unit, now: false }));
  const rest = started ? next : next.slice(1);
  return [{ unit: now, now: true }, ...rest.slice(0, n - 1).map((unit) => ({ unit, now: false }))];
}
