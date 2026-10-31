// Turn rules of the session program as the screens need them (PR #23, docs/PARTY_AND_TURNS.md 5.1):
// every turn begins with the permissionless start_turn, one attack or active skill per turn,
// skill cooldowns counted in own turn starts. Pure functions over decoded accounts and events.
import { arenaMode } from "./game";
import type { Address } from "@solana/kit";
import { ACTIVE_MAX, NO_SKILL } from "./skillcheck";
import { StateKind } from "@deadroll/sdk/session";
import balance from "./balance.json";
import { MONSTER_BASE, currentUnit, status, variantKey, type GameSessionAcc, type Party, type ParticipationAcc, type PartyPlayer } from "./game";

/** start_turn ran for the current turn (states ticked, cooldowns counted down, AP granted). */
export const turnStarted = (s: GameSessionAcc): boolean => s.startedSeq === s.turnSeq;

/** The current unit already used its one attack or active skill this turn. */
export const turnActed = (s: GameSessionAcc): boolean => s.actedSeq === s.turnSeq;

/** The turn start this tab has to send: `player` null for a monster turn. */
/** `finalize` (patch 6): players went down in the last callback; this start_turn finalizes them
 * first, also when the turn already started. */
export type TurnStart = { unit: number; turnSeq: number; player: PartyPlayer | null; finalize: boolean };

/**
 * Whether `me` sends start_turn now: the tab whose player is next does it for that player, the DM
 * tab for monsters. Nothing while a roll is pending, the party is not active or the turn started.
 * Patch 6: while deaths wait to be finalized, any tab of the party sends it (anyone may).
 */
export function startTurnFor(p: Party, me: Address | null): TurnStart | null {
  const s = p.session;
  if (!me || status(s) !== "active" || s.core.pendingVrf) return null;
  // Arena update: between the map and an arena only the deaths are finalized.
  const between = arenaMode(s) === "engaged" || arenaMode(s) === "won" || arenaMode(s) === "retreated";
  if (between && !s.pendingDeaths) return null;
  const unit = currentUnit(s);
  if (unit === null) return null;
  const current = unit < MONSTER_BASE ? (p.players.find((x) => x.slot === unit) ?? null) : null;
  if (s.pendingDeaths) {
    const inParty = s.core.dm === me || p.players.some((x) => x.part.player === me);
    return inParty ? { unit, turnSeq: s.turnSeq, player: current, finalize: true } : null;
  }
  if (turnStarted(s)) return null;
  if (unit < MONSTER_BASE) {
    return current && current.part.player === me ? { unit, turnSeq: s.turnSeq, player: current, finalize: false } : null;
  }
  return s.core.dm === me ? { unit, turnSeq: s.turnSeq, player: null, finalize: false } : null;
}

/**
 * A player's cooldowns by SkillSet slot: own turn starts until usable (0 = usable). The
 * participation can trail the events by a refresh, so SkillUsed events of this unit after its
 * TurnTicked for `turnSeq` raise them at once (a use only ever raises a cooldown within a turn).
 */
/** Arena update: the step between the map and an arena this tab sends (any party member's): enter
 * the engaged pack's arena, or leave a won one (patch B: or one the party retreated from) once its
 * deaths are final. */
export function arenaStepFor(p: Party, me: Address | null): "enter" | "leave" | null {
  const s = p.session;
  if (!me || status(s) !== "active" || s.core.pendingVrf || s.pendingDeaths) return null;
  const inParty = s.core.dm === me || p.players.some((x) => x.part.player === me);
  if (!inParty) return null;
  const mode = arenaMode(s);
  // Patch C: into an arena once the DM revealed the pack and every cell the step uncovered.
  if (mode === "engaged") return s.obligationCount === 0 && s.packsRevealed & (1 << s.arenaPack) ? "enter" : null;
  return mode === "won" || mode === "retreated" ? "leave" : null;
}

/** Patch C: the next reveal the DM's client sends by itself (docs/MAP_DESCRIPTION.md 8.5): a cell
 * a step or a look uncovered (`by`: the player's slot, a trap hits them), then the engaged pack. */
export type RevealStep = { kind: "cell"; leaf: number; by: number } | { kind: "pack"; pack: number };
export function revealStepFor(p: Party, me: Address | null): RevealStep | null {
  const s = p.session;
  if (!me || s.core.dm !== me || status(s) !== "active" || s.core.pendingVrf) return null;
  if (s.obligationCount > 0) return { kind: "cell", leaf: s.obligations[0]!, by: s.obligedBy };
  if (arenaMode(s) === "engaged" && !(s.packsRevealed & (1 << s.arenaPack))) return { kind: "pack", pack: s.arenaPack };
  return null;
}

/** Patch C: the DM's final audit (docs/PARTY_AND_TURNS.md 3.2): the secret leaves and the packs not
 * revealed yet, after the party ended. `leaves`: every secret cell of the DM's copy; `packs`: the
 * used pack numbers. */
export function auditLeft(s: GameSessionAcc, leaves: number[], packs: number[]): { cells: number[]; packs: number[] } {
  const revealed = (leaf: number) => (s.revealed[leaf >> 3]! & (1 << (leaf & 7))) !== 0;
  return { cells: leaves.filter((l) => !revealed(l)), packs: packs.filter((n) => !(s.packsRevealed & (1 << n))) };
}

/** Patch B: the Retreat or Escape proposal as the site shows it (docs/PARTY_AND_TURNS.md 7.5). */
export interface ExitVoteView {
  /** The proposal on the table, `null` = none. */
  kind: "retreat" | "escape" | null;
  /** Player slots that voted for it. */
  voted: number[];
  /** Living player slots still to vote. */
  waiting: number[];
  /** The vote passed: the monsters take their last moves. */
  leaving: boolean;
  /** Retreats the party has left (`ArenaTuning::retreats_max`). */
  retreatsLeft: number;
}
export function exitVoteView(s: GameSessionAcc): ExitVoteView {
  const kind = s.exitKind === 1 ? "retreat" : s.exitKind === 2 ? "escape" : null;
  const slots = Array.from({ length: s.core.playerCount }, (_, i) => i);
  const alive = slots.filter((i) => s.aliveMask & (1 << i));
  const voted = kind ? alive.filter((i) => s.fleeMask & (1 << i)) : [];
  return {
    kind,
    voted,
    waiting: kind ? alive.filter((i) => !(s.fleeMask & (1 << i))) : [],
    leaving: s.lastMoves !== 0,
    retreatsLeft: Math.max(0, balance.arena.retreats_max - s.retreats),
  };
}

export function skillCooldowns(part: ParticipationAcc, events: readonly { name: string; data: any }[], unit: number, turnSeq: number): number[] {
  const out = Array.from(part.cooldowns);
  let since = -1;
  events.forEach((e, i) => {
    if (e.name === "TurnTicked" && e.data.unit === unit && e.data.turnSeq === turnSeq) since = i;
  });
  if (since < 0) return out;
  for (const e of events.slice(since + 1)) {
    if (e.name !== "SkillUsed" || e.data.unit !== unit) continue;
    Array.from(e.data.cooldowns as Uint8Array).forEach((n, i) => (out[i] = Math.max(out[i] ?? 0, n)));
  }
  return out;
}

/** The value is already "own turn starts until usable": no off-by-one. */
export const cooldownLabel = (n: number): string => `ready in ${n} turn${n === 1 ? "" : "s"}`;

// ---- battle log texts for the turn-rule events

/** StateKind variants in on-chain order; `expired` bit i is `1 << i` (deadroll_combat::states::state_bit). */
const STATE_KEYS = ["burning", "chill", "freeze", "bleeding", "shock", "stun", "penetration"] as const;
const DOT_KEYS: readonly string[] = ["burning", "bleeding"];
const kindOf = (e: number): string => variantKey(StateKind, e);

/** TurnTicked as a log line; null when the start changed nothing worth a line (no damage, nothing wore off). */
export function tickText(d: { dotDamage: number; hp: number; died: boolean; expired: number; states: { kind: number; turns: number }[] }, unit: string): string | null {
  const expired = STATE_KEYS.filter((_, i) => d.expired & (1 << i));
  const parts: string[] = [];
  if (d.dotDamage > 0) {
    // The over-time states that dealt it: still on the unit, or ended with this tick.
    const live = d.states.filter((x) => x.turns > 0).map((x) => kindOf(x.kind));
    const kinds = DOT_KEYS.filter((k) => live.includes(k) || expired.includes(k as (typeof STATE_KEYS)[number]));
    parts.push(`${unit} takes ${d.dotDamage} ${kinds.join(" + ") || "over-time"} damage (${d.hp} hp)${d.died ? " and dies" : ""}`);
  }
  if (expired.length) parts.push(`${parts.length ? "" : `${unit}: `}${expired.join(", ")} wore off`);
  return parts.length ? parts.join("; ") : null;
}

export function skipText(d: { state: number }, unit: string): string {
  return `${unit} is ${kindOf(d.state) === "freeze" ? "frozen" : "stunned"}: turn skipped`;
}

export function skillUsedText(d: { cooldown: number }, unit: string, skill: string): string {
  return d.cooldown > 0 ? `${unit} used ${skill}: cooldown ${d.cooldown} turn${d.cooldown === 1 ? "" : "s"}` : `${unit} used ${skill}`;
}

/** Whether SkillSet slot `slot` is one of the active skills the player took onto the map (set_ready). */
export function takenOntoMap(p: ParticipationAcc, slot: number): boolean {
  return slot !== NO_SKILL && Array.from(p.skills.slice(0, ACTIVE_MAX)).includes(slot);
}
