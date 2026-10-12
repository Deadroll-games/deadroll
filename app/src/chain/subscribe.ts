// Resilient RPC subscriptions: resubscribe with exponential backoff when the WebSocket stream
// throws or ends, and tell the caller after each reconnect so it can re-fetch what it missed.
import type { Address } from "@solana/kit";
import { client } from "../wallet/client";

export type SubStatus =
  | { state: "connecting" }
  | { state: "live" }
  | { state: "reconnecting"; attempt: number; inMs: number; reason: string }
  | { state: "stopped" };

export type ResilientOptions = {
  signal: AbortSignal;
  onStatus?: (s: SubStatus) => void;
  /** Called after a successful subscribe that follows a failure (not on the first connect). */
  onReconnect?: () => void;
  baseMs?: number;
  maxMs?: number;
  /** A stream that stayed up this long (or delivered an item) resets the backoff. */
  stableMs?: number;
  random?: () => number;
  now?: () => number;
  sleep?: (ms: number, signal: AbortSignal) => Promise<void>;
};

/** 1s, 2s, 4s ... capped at maxMs, with +-20% jitter. */
export function backoffMs(attempt: number, baseMs = 1_000, maxMs = 30_000, random: () => number = Math.random): number {
  const raw = Math.min(maxMs, baseMs * 2 ** Math.min(attempt, 30));
  return Math.round(raw * (0.8 + 0.4 * random()));
}

function abortableSleep(ms: number, signal: AbortSignal): Promise<void> {
  return new Promise((resolve) => {
    if (signal.aborted) return resolve();
    const done = () => {
      clearTimeout(t);
      signal.removeEventListener("abort", done);
      resolve();
    };
    const t = setTimeout(done, ms);
    signal.addEventListener("abort", done);
  });
}

const reasonOf = (e: unknown): string => (e == null ? "stream closed" : e instanceof Error ? e.message : String(e));

/** Runs until `signal` aborts. `open` creates one subscription stream; `onItem` errors never stop the loop. */
export async function subscribeResilient<T>(
  open: (signal: AbortSignal) => Promise<AsyncIterable<T>>,
  onItem: (item: T) => void,
  opts: ResilientOptions,
): Promise<void> {
  const { signal, onStatus, onReconnect, baseMs = 1_000, maxMs = 30_000, stableMs = 15_000 } = opts;
  const random = opts.random ?? Math.random;
  const now = opts.now ?? Date.now;
  const sleep = opts.sleep ?? abortableSleep;
  let attempt = 0;
  let failed = false;
  onStatus?.({ state: "connecting" });
  while (!signal.aborted) {
    let startedAt: number | null = null;
    let delivered = false;
    let reason: unknown = null;
    try {
      const stream = await open(signal);
      if (signal.aborted) break;
      startedAt = now();
      onStatus?.({ state: "live" });
      if (failed) onReconnect?.();
      for await (const item of stream) {
        delivered = true;
        try {
          onItem(item);
        } catch (e) {
          console.error("subscription handler", e);
        }
      }
    } catch (e) {
      reason = e;
    }
    if (signal.aborted) break;
    failed = true;
    if (delivered || (startedAt != null && now() - startedAt >= stableMs)) attempt = 0;
    const inMs = backoffMs(attempt, baseMs, maxMs, random);
    attempt += 1;
    onStatus?.({ state: "reconnecting", attempt, inMs, reason: reasonOf(reason) });
    await sleep(inMs, signal);
  }
  onStatus?.({ state: "stopped" });
}

export type LogNote = { signature: string; err: unknown; logs: readonly string[] };

/** Opens a `logsNotifications` stream for transactions that mention `program`. */
export function openProgramLogs(program: Address) {
  return async (signal: AbortSignal): Promise<AsyncIterable<LogNote>> => {
    const stream = await client.rpcSubscriptions
      .logsNotifications({ mentions: [program] }, { commitment: "confirmed" })
      .subscribe({ abortSignal: signal });
    return (async function* () {
      for await (const note of stream) yield note.value as LogNote;
    })();
  };
}

/** Logs of the latest successful transactions that touch `address`, oldest first (resync after a gap). */
export async function recentLogs(address: Address, limit = 20): Promise<{ signature: string; logs: readonly string[] }[]> {
  const sigs = await client.rpc.getSignaturesForAddress(address, { limit, commitment: "confirmed" }).send();
  const out: { signature: string; logs: readonly string[] }[] = [];
  for (const s of [...sigs].reverse()) {
    if (s.err) continue;
    const tx = await client.rpc
      .getTransaction(s.signature, { commitment: "confirmed", maxSupportedTransactionVersion: 0, encoding: "json" })
      .send();
    if (tx?.meta?.logMessages) out.push({ signature: s.signature, logs: tx.meta.logMessages });
  }
  return out;
}

export function statusLabel(s: SubStatus): string {
  if (s.state === "live") return "live";
  if (s.state === "connecting") return "connecting";
  if (s.state === "stopped") return "stopped";
  return `reconnecting in ${Math.ceil(s.inMs / 1000)}s (attempt ${s.attempt})`;
}
