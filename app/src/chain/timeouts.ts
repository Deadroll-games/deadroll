// Stuck party (docs/PARTY_AND_TURNS.md 3.2, 5.3; MAP_DESCRIPTION.md 8.5), mirror of
// `deadroll_rules::timeout` (fixture: crates/rules/tests/timeout_ts_fixtures.rs). The site shows
// "DM is missing", the countdown, "Return to the party", "I'm back", the DM's settlement,
// "Skip turn" and "Retry the roll" only when the program would accept them.
import balance from "./balance.json";
import { MOCK_VRF } from "../config";

export type TimeoutTuning = {
  revealTimeoutSecs: number;
  dmReturnConfirmSecs: number;
  turnTimeoutSecs: number;
  vrfRetrySlots: number;
};

/** The program defaults (`BalanceConfig::DEFAULT.timeouts`). */
export const TIMEOUTS_DEFAULT: TimeoutTuning = {
  revealTimeoutSecs: balance.timeouts.reveal_timeout_secs,
  dmReturnConfirmSecs: balance.timeouts.dm_return_confirm_secs,
  turnTimeoutSecs: balance.timeouts.turn_timeout_secs,
  vrfRetrySlots: balance.timeouts.vrf_retry_slots,
};

/** `TimeoutTuning::LOCALNET`: what a `mock-vrf` session build waits (localnet tests only). */
export const TIMEOUTS_LOCALNET: TimeoutTuning = { revealTimeoutSecs: 3, dmReturnConfirmSecs: 6, turnTimeoutSecs: 3, vrfRetrySlots: 5 };

/** What the deployed session program waits: the localnet numbers against a `mock-vrf` build. */
export const TIMEOUTS: TimeoutTuning = MOCK_VRF ? TIMEOUTS_LOCALNET : TIMEOUTS_DEFAULT;

/** `GameSession.dm_missing_at`, `dm_back_at` (unix seconds, 0 = not set). */
export type DmWatch = { missingAt: number; backAt: number };

export type DmPhase =
  | { kind: "present" }
  /** Marked missing; the DM may come back until `until` (inclusive). Frozen. */
  | { kind: "missing"; until: number }
  /** The DM never came back: each player claims after `endedAt + settle`. */
  | { kind: "gone"; endedAt: number }
  /** The DM is back; the living players confirm until `until` (inclusive). Frozen. */
  | { kind: "back"; until: number }
  /** Not everyone confirmed: the DM settles until `endedAt + settle`, the players claim after. */
  | { kind: "unconfirmed"; endedAt: number };

/** `DmWatch::phase`: where the party stands with its DM at `now`. */
export function dmPhase(w: DmWatch, now: number, settleTimeoutSecs: number, t: TimeoutTuning = TIMEOUTS): DmPhase {
  if (w.missingAt === 0) return { kind: "present" };
  if (w.backAt === 0) {
    const until = w.missingAt + settleTimeoutSecs;
    return now <= until ? { kind: "missing", until } : { kind: "gone", endedAt: w.missingAt };
  }
  const until = w.backAt + t.dmReturnConfirmSecs;
  return now <= until ? { kind: "back", until } : { kind: "unconfirmed", endedAt: until };
}

/** The reveal owed since `owedAt` is old enough to mark the DM as missing. */
export const revealOverdue = (owedAt: number, now: number, t: TimeoutTuning = TIMEOUTS) => now >= owedAt + t.revealTimeoutSecs;

/** The current unit did nothing since `clock` for the turn timeout: anyone may skip its turn. */
export const turnOverdue = (clock: number, now: number, t: TimeoutTuning = TIMEOUTS) => now >= clock + t.turnTimeoutSecs;

/** The VRF request sent in slot `sent` may be sent again at `slot`. */
export const vrfRetryDue = (sent: bigint | number, slot: bigint | number, t: TimeoutTuning = TIMEOUTS) => BigInt(slot) >= BigInt(sent) + BigInt(t.vrfRetrySlots);

/** Seconds left until `at`, never negative. */
export const secsUntil = (at: number, now: number) => Math.max(0, at - now);

/** `1h 02m`, `4m 05s`, `12s`. */
export function countdown(secs: number): string {
  const s = Math.max(0, Math.floor(secs));
  const h = Math.floor(s / 3600);
  const m = Math.floor((s % 3600) / 60);
  const pad = (n: number) => String(n).padStart(2, "0");
  if (h > 0) return `${h}h ${pad(m)}m`;
  if (m > 0) return `${m}m ${pad(s % 60)}s`;
  return `${s}s`;
}
