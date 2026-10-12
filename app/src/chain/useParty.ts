// Live view of one party: polls the accounts (session, monsters, participations, characters, map)
// and listens to session program logs for its events, so all windows (DM and players) see the
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

const POLL_MS = 1200;

export function useParty(address: Address | null) {
  const [party, setParty] = useState<Party | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [events, setEvents] = useState<PartyEvent[]>([]);
  const [live, setLive] = useState<SubStatus>({ state: "connecting" });
  const tiles = useRef<Uint8Array | null>(null);
  const seen = useRef(new Set<string>());
  const inflight = useRef(false);

  const refresh = useCallback(async () => {
    if (!address || inflight.current) return;
    inflight.current = true;
    try {
      const p = await readParty(address, tiles.current);
      if (p?.tiles) tiles.current = p.tiles;
      setParty(p);
      setError(p ? null : "No party at this address");
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    } finally {
      inflight.current = false;
    }
  }, [address]);

  /** Adds the events of a transaction (from our own send, or the log subscription) once. */
  const ingest = useCallback(
    (signature: string, evs: ChainEvent[]) => {
      if (!address || seen.current.has(signature)) return;
      seen.current.add(signature);
      const mine = evs.filter((e) => e.data?.session === address);
      if (mine.length === 0) return;
      const at = Date.now();
      setEvents((list) => [...list, ...mine.map((e) => ({ ...e, signature, at }))].slice(-200));
      void refresh();
    },
    [address, refresh],
  );

  useEffect(() => {
    tiles.current = null;
    seen.current = new Set();
    setEvents([]);
    setParty(null);
    if (DATA_SOURCE !== "chain" || !address) return;
    void refresh();
    const timer = setInterval(() => void refresh(), POLL_MS);
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
      { signal: abort.signal, onStatus: setLive, onReconnect: resync },
    );
    return () => {
      clearInterval(timer);
      abort.abort();
    };
  }, [address, refresh, ingest]);

  return { party, error, events, refresh, ingest, live };
}
