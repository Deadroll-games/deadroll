// Sends instructions with a payer and reports through toasts. One hook for every screen.
import { useCallback, useState } from "react";
import type { Instruction, TransactionSigner } from "@solana/kit";
import { PROGRAMS } from "../config";
import { useToast } from "../components/toast/Toast";
import { decodeEvents, type ChainEvent } from "./events";
import { coders } from "./programs";
import { sendTx } from "./send";

const IDS = { session: PROGRAMS.session, character: PROGRAMS.character, map: PROGRAMS.map };

export type TxResult = { signature: string; events: ChainEvent[] };

export type SendOptions = {
  /** No success toast. */
  quiet?: boolean;
  /** No error toast. */
  silent?: boolean;
  /** Sees the failure text (`explainFailure`: "ErrorName: message"); true = handled, no toast. */
  onError?: (message: string) => boolean;
};

export function eventsOfLogs(logs: readonly string[]): ChainEvent[] {
  return decodeEvents(logs, coders, IDS);
}

export function useTx() {
  const toast = useToast();
  const [busy, setBusy] = useState<string | null>(null);
  const send = useCallback(
    async (what: string, payer: TransactionSigner | null, ixs: Instruction[] | (() => Promise<Instruction[]>), opts: SendOptions = {}): Promise<TxResult | null> => {
      if (!payer) {
        toast.push({ kind: "error", text: `${what}: connect a wallet (or open the app with ?dev=1 on localnet)` });
        return null;
      }
      setBusy(what);
      try {
        const list = typeof ixs === "function" ? await ixs() : ixs;
        const { signature, logs } = await sendTx(payer, list);
        if (!opts.quiet) toast.push({ kind: "ok", text: what, signature });
        return { signature, events: eventsOfLogs(logs) };
      } catch (e) {
        if (opts.onError?.(e instanceof Error ? e.message : String(e))) return null;
        if (!opts.silent) toast.error(e, what);
        return null;
      } finally {
        setBusy(null);
      }
    },
    [toast],
  );
  return { send, busy };
}
