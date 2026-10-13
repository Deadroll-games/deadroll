// Turn rules of the session program as the screens need them (PR #23, docs/PARTY_AND_TURNS.md 5.1):
// every turn begins with the permissionless start_turn, one attack or active skill per turn,
// skill cooldowns counted in own turn starts. Pure functions over decoded accounts and events.
import type { Address } from "@solana/kit";
import { MONSTER_BASE, currentUnit, status, type GameSessionAcc, type Party, type ParticipationAcc, type PartyPlayer } from "./game";

/** start_turn ran for the current turn (states ticked, cooldowns counted down, AP granted). */
export const turnStarted = (s: GameSessionAcc): boolean => s.startedSeq === s.turnSeq;

/** The current unit already used its one attack or active skill this turn. */
export const turnActed = (s: GameSessionAcc): boolean => s.actedSeq === s.turnSeq;

/** The turn start this tab has to send: `player` null for a monster turn. */
export type TurnStart = { unit: number; turnSeq: number; player: PartyPlayer | null };

/**
 * Whether `me` sends start_turn now: the tab whose player is next does it for that player, the DM
 * tab for monsters. Nothing while a roll is pending, the party is not active or the turn started.
 */
export function startTurnFor(p: Party, me: Address | null): TurnStart | null {
  const s = p.session;
  if (!me || status(s) !== "active" || s.core.pendingVrf || turnStarted(s)) return null;
  const unit = currentUnit(s);
  if (unit === null) return null;
  if (unit < MONSTER_BASE) {
    const player = p.players.find((x) => x.slot === unit);
    return player && player.part.player === me ? { unit, turnSeq: s.turnSeq, player } : null;
  }
  return s.core.dm === me ? { unit, turnSeq: s.turnSeq, player: null } : null;
}

/**
 * A player's cooldowns by SkillSet slot: own turn starts until usable (0 = usable). The
 * participation can trail the events by a refresh, so SkillUsed events of this unit after its
 * TurnTicked for `turnSeq` raise them at once (a use only ever raises a cooldown within a turn).
 */
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
