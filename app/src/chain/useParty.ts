// Live view of one party (session, monsters, participations, characters, map). The session program log
// stream is the main refresh path, with a slow fallback poll; it also carries the party events, so all windows (DM and players) see the
// same moves, rolls and deaths. The log subscription reconnects with backoff; after a gap the
// party is re-read and recent party transactions are replayed when the node still has them
// (the local validator keeps only ~100 slots of history).
import { useCallback, useEffect, useRef, useState } from "react";
import type { Address } from "@solana/kit";
import { DATA_SOURCE, PROGRAMS } from "../config";
import type { ChainEvent } from "./events";
import { readParty, type Party } from "./game";
import { openProgramLogs, recentLogs, subscribeResilient, type SubStatus } from "./subscribe";
import { eventsOfLogs } from "./useTx";

export type PartyEvent = ChainEvent & { signature: string; at: number };

/**
 * The log subscription is the main refresh path: every ingested party event triggers a refresh.
 * The poll is only a safety net, slow while the stream is live and 3 s while it is down.
 */
export const POLL_LIVE_MS = 10_000;
export const POLL_FALLBACK_MS = 3_000;
const MIN_GAP_MS = 400;

/** Time between two fallback polls; null while the tab is hidden (a refresh runs when it shows again). */
export function fallbackPollMs(live: SubStatus["state"], hidden: boolean): number | null {
  if (hidden) return null;
  return live === "live" ? POLL_LIVE_MS : POLL_FALLBACK_MS;
}

/**
 * A party read failure as text. Accounts written before the session program grew (turn rules:
 * appended fields) are too short for the current layout and cannot be played any more.
 */
export function partyReadError(e: unknown): string {
  const msg = e instanceof Error ? e.message : String(e);
  if (msg.startsWith("borsh: read past end")) return "This party was opened before the session program upgrade and cannot be read. Open a new party.";
  return msg;
}

export function useParty(address: Address | null) {
  const [party, setParty] = useState<Party | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [events, setEvents] = useState<PartyEvent[]>([]);
  const [live, setLive] = useState<SubStatus>({ state: "connecting" });
  const tiles = useRef<Uint8Array | null>(null);
  const seen = useRef(new Set<string>());
  const inflight = useRef(false);
  const latest = useRef<Party | null>(null);
  const liveRef = useRef<SubStatus["state"]>("connecting");
  const lastRun = useRef(0);
  const trailing = useRef<ReturnType<typeof setTimeout> | null>(null);

  const refresh = useCallback(async () => {
    if (!address || inflight.current) return;
    inflight.current = true;
    lastRun.current = Date.now();
    try {
      const p = await readParty(address, tiles.current, latest.current);
      latest.current = p;
      if (p?.tiles) tiles.current = p.tiles;
      setParty(p);
      setError(p ? null : "No party at this address");
    } catch (e) {
      setError(partyReadError(e));
    } finally {
      inflight.current = false;
    }
  }, [address]);

  /** Refresh for stream events: bursts of events (one action, several logs) share one read. */
  const refreshSoon = useCallback(() => {
    if (trailing.current) return;
    const wait = Math.max(0, lastRun.current + MIN_GAP_MS - Date.now());
    trailing.current = setTimeout(() => {
      trailing.current = null;
      void refresh();
    }, wait);
  }, [refresh]);

  /** Adds the events of a transaction (from our own send, or the log subscription) once. */
  const ingest = useCallback(
    (signature: string, evs: ChainEvent[]) => {
      if (!address || seen.current.has(signature)) return;
      seen.current.add(signature);
      const mine = evs.filter((e) => e.data?.session === address);
      if (mine.length === 0) return;
      const at = Date.now();
      setEvents((list) => [...list, ...mine.map((e) => ({ ...e, signature, at }))].slice(-200));
      refreshSoon();
    },
    [address, refreshSoon],
  );

  useEffect(() => {
    tiles.current = null;
    latest.current = null;
    seen.current = new Set();
    setEvents([]);
    setParty(null);
    if (DATA_SOURCE !== "chain" || !address) return;
    void refresh();
    // Fallback poll: a self-timed loop, so the delay follows the stream state and the tab visibility.
    let timer: ReturnType<typeof setTimeout> | null = null;
    const schedule = () => {
      const ms = fallbackPollMs(liveRef.current, document.hidden);
      timer = ms == null ? null : setTimeout(() => {
        void refresh();
        schedule();
      }, ms);
    };
    const onVisible = () => {
      if (!document.hidden) {
        void refresh();
        if (timer) clearTimeout(timer);
        schedule();
      }
    };
    document.addEventListener("visibilitychange", onVisible);
    schedule();
    const abort = new AbortController();
    // After a WebSocket gap: re-read the party and replay the events of recent party transactions
    // (ingest skips the ones already seen). Best effort: a node without history only gets the refresh.
    const resync = () => {
      void refresh();
      void recentLogs(address).then(
        (txs) => txs.forEach((t) => ingest(t.signature, eventsOfLogs(t.logs))),
        () => undefined,
      );
    };
    void subscribeResilient(
      openProgramLogs(PROGRAMS.session),
      (note) => {
        if (!note.err) ingest(note.signature, eventsOfLogs(note.logs));
      },
      {
        signal: abort.signal,
        onStatus: (st) => {
          liveRef.current = st.state;
          setLive(st);
        },
        onReconnect: resync,
      },
    );
    return () => {
      if (timer) clearTimeout(timer);
      if (trailing.current) clearTimeout(trailing.current);
      trailing.current = null;
      document.removeEventListener("visibilitychange", onVisible);
      abort.abort();
    };
  }, [address, refresh, ingest]);

  return { party, error, events, refresh, ingest, live };
}
