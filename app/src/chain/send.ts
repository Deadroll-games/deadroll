// Builds, signs and sends one transaction with an explicit fee payer: the session key (game moves,
// no popup), the dev keypair, or the Wallet Standard signer. Extra signers (a new Core NFT asset,
// the mock VRF identity) ride in the instructions' account metas. Returns the confirmed logs so
// callers can decode events at once (the shared validator keeps ~100 slots of history).
import {
  appendTransactionMessageInstructions,
  createTransactionMessage,
  getSignatureFromTransaction,
  isTransactionModifyingSigner,
  isTransactionPartialSigner,
  isTransactionSendingSigner,
  pipe,
  sendAndConfirmTransactionFactory,
  setTransactionMessageFeePayerSigner,
  setTransactionMessageLifetimeUsingBlockhash,
  signAndSendTransactionMessageWithSigners,
  signTransactionMessageWithSigners,
  getBase58Decoder,
  type Instruction,
  type Signature,
  type TransactionSigner,
} from "@solana/kit";
import { client } from "../wallet/client";
import { coders } from "./programs";
import { PROGRAMS } from "../config";
import { TX_SENT_EVENT } from "./txEvents";

export type Sent = { signature: Signature; logs: string[] };

export class TxError extends Error {
  constructor(
    message: string,
    readonly logs: string[],
  ) {
    super(message);
  }
}

export async function sendTx(payer: TransactionSigner, instructions: Instruction[]): Promise<Sent> {
  const { rpc, rpcSubscriptions } = client;
  try {
    const { value: blockhash } = await rpc.getLatestBlockhash({ commitment: "confirmed" }).send();
    const message = pipe(
      createTransactionMessage({ version: 0 }),
      (m) => setTransactionMessageFeePayerSigner(payer, m),
      (m) => setTransactionMessageLifetimeUsingBlockhash(blockhash, m),
      (m) => appendTransactionMessageInstructions(instructions, m),
    );
    let signature: Signature;
    const canSign = isTransactionPartialSigner(payer) || isTransactionModifyingSigner(payer);
    if (!canSign && isTransactionSendingSigner(payer)) {
      // A wallet that only signs and sends.
      const bytes = await signAndSendTransactionMessageWithSigners(message);
      signature = getBase58Decoder().decode(bytes) as Signature;
      await waitConfirmed(signature);
    } else {
      const tx = await signTransactionMessageWithSigners(message);
      const sendAndConfirm = sendAndConfirmTransactionFactory({ rpc, rpcSubscriptions });
      // The factory wants a lifetime-narrowed type; the message above has a blockhash lifetime.
      await sendAndConfirm(tx as Parameters<typeof sendAndConfirm>[0], { commitment: "confirmed" });
      signature = getSignatureFromTransaction(tx);
    }
    window.dispatchEvent(new CustomEvent<string>(TX_SENT_EVENT, { detail: payer.address }));
    return { signature, logs: await logsOf(signature) };
  } catch (e) {
    if (e instanceof TxError) throw e;
    const logs = errorLogs(e);
    throw new TxError(explainFailure(e, logs, instructions), logs);
  }
}

/** Compatibility with the tombstone watcher: send and return the signature. */
export async function sendWithPayer(payer: TransactionSigner, instructions: Instruction[]): Promise<Signature> {
  return (await sendTx(payer, instructions)).signature;
}

/** Signature-status poll for send-only wallets: 1.5 s apart, 40 tries (about a minute). */
export const CONFIRM_POLL_MS = 1_500;
/** Wait before the n-th getTransaction retry: 500 ms, 1 s, 1.5 s ... (a confirmed tx is usually served at once). */
export const logsRetryMs = (attempt: number): number => 500 * (attempt + 1);

async function waitConfirmed(signature: Signature): Promise<void> {
  for (let i = 0; i < 40; i++) {
    const { value } = await client.rpc.getSignatureStatuses([signature]).send();
    const st = value[0];
    if (st?.err) throw new Error(`transaction failed: ${JSON.stringify(st.err)}`);
    if (st?.confirmationStatus === "confirmed" || st?.confirmationStatus === "finalized") return;
    await new Promise((r) => setTimeout(r, CONFIRM_POLL_MS));
  }
  throw new Error(`not confirmed: ${signature}`);
}

export async function logsOf(signature: Signature): Promise<string[]> {
  for (let i = 0; i < 6; i++) {
    const tx = await client.rpc
      .getTransaction(signature, { commitment: "confirmed", maxSupportedTransactionVersion: 0, encoding: "json" })
      .send();
    if (tx) return [...(tx.meta?.logMessages ?? [])];
    await new Promise((r) => setTimeout(r, logsRetryMs(i)));
  }
  return [];
}

function errorLogs(e: unknown): string[] {
  // kit's SolanaError carries simulation logs in context (preflight failure) or in its cause.
  for (let cur: any = e, depth = 0; cur && depth < 5; cur = cur.cause, depth++) {
    const logs = cur?.context?.logs ?? cur?.logs;
    if (Array.isArray(logs)) return logs as string[];
  }
  return [];
}

const PROGRAM_CODERS: Record<string, keyof typeof coders> = {
  [PROGRAMS.session]: "session",
  [PROGRAMS.character]: "character",
  [PROGRAMS.map]: "map",
};

/** The Anchor error name at the start of an `explainFailure` text ("AlreadyActed: ..."), or null. */
export function errorName(message: string): string | null {
  return /^([A-Z]\w*):/.exec(message)?.[1] ?? null;
}

/** The failing program's Anchor error name from logs (or the failing instruction), or the error message. */
export function explainFailure(e: unknown, logs: string[], instructions: readonly Instruction[] = []): string {
  for (const line of [...logs].reverse()) {
    const anchor = /Error Code: (\w+)\. Error Number: \d+\. Error Message: (.+)$/.exec(line);
    if (anchor) return `${anchor[1]}: ${anchor[2]}`;
  }
  for (const line of [...logs].reverse()) {
    const m = /^Program (\w+) failed: custom program error: 0x([0-9a-f]+)/i.exec(line);
    if (m) {
      const coder = PROGRAM_CODERS[m[1]!];
      const code = parseInt(m[2]!, 16);
      const name = coder ? coders[coder].errorName(code) : null;
      return name ?? `program ${m[1]!.slice(0, 6)}.. error ${code}`;
    }
  }
  const msg = e instanceof Error ? e.message : String(e);
  // kit without logs: "Custom program error: #6016 (instruction #1)"; instruction #n is 1-based.
  const custom = /#(\d+) \(instruction #(\d+)\)/.exec(msg);
  if (custom) {
    const program = instructions[Number(custom[2]) - 1]?.programAddress;
    const coder = program ? PROGRAM_CODERS[program] : undefined;
    const name = coder ? coders[coder].errorName(Number(custom[1])) : null;
    if (name) return name;
  }
  const last = logs.filter((l) => /failed|Error/i.test(l)).pop();
  return last ? `${msg} (${last})` : msg;
}
