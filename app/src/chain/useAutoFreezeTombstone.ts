// React side of chain/tombstone.ts: watches character program logs for CharacterDied.
import { useEffect, useRef, useState } from "react";
import type { Address, TransactionSigner } from "@solana/kit";
import { DATA_SOURCE, PROGRAMS } from "../config";
import { client } from "../wallet/client";
import { rpcAccountSource } from "./accounts";
import { eventsFromLogs } from "./events";
import { decodeCharacterDied } from "./layouts";
import { sendWithPayer } from "./send";
import { handleCharacterDied, type TombstoneLog } from "./tombstone";

/**
 * Subscribes to character program logs and freezes tombstones. `payer` should be the session key
 * (no popup); falls back to the wallet signer. Does nothing in mock mode or without a payer.
 */
export function useAutoFreezeTombstone(payer: TransactionSigner | null, me: Address | null): TombstoneLog[] {
  const [log, setLog] = useState<TombstoneLog[]>([]);
  const seen = useRef(new Set<string>());

  useEffect(() => {
    if (DATA_SOURCE !== "chain" || !payer) return;
    const abort = new AbortController();
    const accounts = rpcAccountSource(client.rpc);
    const push = (entry: TombstoneLog) => setLog((l) => [entry, ...l].slice(0, 20));

    (async () => {
      const stream = await client.rpcSubscriptions
        .logsNotifications({ mentions: [PROGRAMS.character] }, { commitment: "confirmed" })
        .subscribe({ abortSignal: abort.signal });
      for await (const note of stream) {
        if (note.value.err) continue;
        for (const raw of eventsFromLogs(note.value.logs, PROGRAMS.character)) {
          const ev = decodeCharacterDied(raw);
          if (!ev || seen.current.has(ev.character)) continue;
          seen.current.add(ev.character);
          push({ character: ev.character, status: "seen", detail: "CharacterDied" });
          void handleCharacterDied({ ev, me, payer, accounts, send: sendWithPayer, signal: abort.signal }).then(push);
        }
      }
    })().catch((e: unknown) => {
      if (!abort.signal.aborted) push({ character: "" as Address, status: "failed", detail: `log subscription: ${String(e)}` });
    });
    return () => abort.abort();
  }, [payer, me]);

  return log;
}
