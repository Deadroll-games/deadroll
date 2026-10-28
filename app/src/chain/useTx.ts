// Sends instructions with a payer and reports through toasts. One hook for every screen.
import { useCallback, useState } from "react";
import type { Instruction, TransactionSigner } from "@solana/kit";
import { DEV_TOOLS } from "../config";
import { useToast } from "../components/toast/Toast";
import { decodeEvents, type ChainEvent } from "./events";
import { BatchError, hasLanded, sendBatch, sendTx, type Sent } from "./send";

export type TxResult = { signature: string; events: ChainEvent[] };

export type SendOptions = {
  /** No success toast. */
  quiet?: boolean;
  /** No error toast. */
  silent?: boolean;
  /** Sees the failure text (`explainFailure`: "ErrorName: message"); true = handled, no toast. */
  onError?: (message: string) => boolean;
};

/** A multi-step action that stopped: `retry` re-signs the steps left in one popup. */
export type Stalled = { what: string; step: number; total: number; retry: () => Promise<TxResult[] | null> };

export function eventsOfLogs(logs: readonly string[]): ChainEvent[] {
  return decodeEvents(logs);
}

const resultOf = ({ signature, logs }: Sent): TxResult => ({ signature, events: eventsOfLogs(logs) });

/** "what 2/5" while step 2 of 5 is sent; "what: confirm in the wallet" while the popup is open. */
export function progressText(what: string, phase: "sign" | "send", index: number, total: number): string {
  if (phase === "sign") return `${what}: confirm in the wallet`;
  return total > 1 ? `${what} ${index + 1}/${total}` : what;
}

/**
 * The steps left after a BatchError: a failed step that landed anyway (its confirmation timed
 * out) counts as sent, so a retry never repeats it.
 */
export async function remainingAfter(e: BatchError, steps: number): Promise<{ landed: Sent | null; from: number }> {
  if (e.signature && (await hasLanded(e.signature))) {
    return { landed: { signature: e.signature, logs: [] }, from: Math.min(steps, e.failedAt + 1) };
  }
  return { landed: null, from: e.failedAt };
}

export function useTx() {
  const toast = useToast();
  const [busy, setBusy] = useState<string | null>(null);
  const [stalled, setStalled] = useState<Stalled | null>(null);
  const send = useCallback(
    async (what: string, payer: TransactionSigner | null, ixs: Instruction[] | (() => Promise<Instruction[]>), opts: SendOptions = {}): Promise<TxResult | null> => {
      if (!payer) {
        toast.push({ kind: "error", text: `${what}: connect a wallet${DEV_TOOLS ? " (or open the app with ?dev=1 on localnet)" : ""}` });
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

  /**
   * One user action of several transactions: one wallet popup for all of them, sent in order.
   * Returns every step's result, or null when a step failed (then `stalled` offers the retry).
   */
  const sendSteps = useCallback(
    async (what: string, payer: TransactionSigner | null, steps: Instruction[][]): Promise<TxResult[] | null> => {
      if (!payer) {
        toast.push({ kind: "error", text: `${what}: connect a wallet${DEV_TOOLS ? " (or open the app with ?dev=1 on localnet)" : ""}` });
        return null;
      }
      const run = async (done: TxResult[], left: Instruction[][]): Promise<TxResult[] | null> => {
        const offset = done.length;
        const total = offset + left.length;
        setStalled(null);
        setBusy(progressText(what, "sign", offset, total));
        try {
          const sent = await sendBatch(payer, left, (phase, i) => setBusy(progressText(what, phase, offset + i, total)));
          const all = [...done, ...sent.map(resultOf)];
          toast.push({ kind: "ok", text: what, signature: all[all.length - 1]!.signature });
          return all;
        } catch (e) {
          if (!(e instanceof BatchError)) {
            toast.error(e, what);
            return null;
          }
          const { landed, from } = await remainingAfter(e, left.length).catch(() => ({ landed: null, from: e.failedAt }));
          const now = [...done, ...e.sent.map(resultOf), ...(landed ? [resultOf(landed)] : [])];
          if (from >= left.length) {
            // The last step landed after all: the action is complete.
            toast.push({ kind: "ok", text: what, signature: now[now.length - 1]!.signature });
            return now;
          }
          toast.error(e, what);
          const rest = left.slice(from);
          setStalled({ what, step: now.length + 1, total, retry: () => run(now, rest) });
          return null;
        } finally {
          setBusy(null);
        }
      };
      return run([], steps);
    },
    [toast],
  );

  return { send, sendSteps, stalled, dismissStalled: useCallback(() => setStalled(null), []), busy };
}
