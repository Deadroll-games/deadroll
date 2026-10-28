// Builds, signs and sends transactions with an explicit fee payer: the session key (game moves,
// no popup), the dev keypair, or the Wallet Standard signer. Extra signers (a new Core NFT asset,
// the mock VRF identity) ride in the instructions' account metas. Returns the confirmed logs so
// callers can decode events at once (the shared validator keeps ~100 slots of history).
//
// One user action = one wallet popup: `sendBatch` signs every transaction of the action in one
// wallet call (`solana:signTransaction` takes several inputs), then sends them in order, each
// confirmed before the next, because later steps write accounts the earlier ones create.
import {
  appendTransactionMessageInstructions,
  compileTransaction,
  createTransactionMessage,
  getSignatureFromTransaction,
  getSignersFromTransactionMessage,
  isTransactionModifyingSigner,
  isTransactionPartialSigner,
  isTransactionMessageWithinSizeLimit,
  isTransactionSendingSigner,
  pipe,
  sendAndConfirmTransactionFactory,
  setTransactionMessageFeePayerSigner,
  setTransactionMessageLifetimeUsingBlockhash,
  signAndSendTransactionMessageWithSigners,
  signTransactionWithSigners,
  getBase58Decoder,
  getBase64Decoder,
  getBase64EncodedWireTransaction,
  type Blockhash,
  type Instruction,
  type Signature,
  type Transaction,
  type TransactionMessageBytesBase64,
  type TransactionSigner,
} from "@solana/kit";
import { client } from "../wallet/client";
import { errorMessage, errorName as sdkErrorName, programNameOf } from "@deadroll/sdk";
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

/** A batch stopped at step `failedAt` (0-based): `sent` landed, the rest was not sent. */
export class BatchError extends TxError {
  constructor(
    message: string,
    logs: string[],
    readonly sent: Sent[],
    readonly failedAt: number,
    /** The failed step's signature when it was signed and sent: it may still have landed. */
    readonly signature: Signature | null,
  ) {
    super(message, logs);
  }
}

/** Step `index` (0-based) of `total` is being signed (the wallet popup) or sent. */
export type BatchProgress = (phase: "sign" | "send", index: number, total: number) => void;

export async function sendTx(payer: TransactionSigner, instructions: Instruction[]): Promise<Sent> {
  try {
    return (await sendBatch(payer, [instructions]))[0]!;
  } catch (e) {
    // One step: callers see a plain TxError.
    if (e instanceof BatchError) throw new TxError(e.message, e.logs);
    throw e;
  }
}

/**
 * Signs `steps` (one transaction each) with one wallet call and sends them in order. Throws a
 * BatchError naming the first step that did not land; the caller resumes with the rest.
 */
export async function sendBatch(payer: TransactionSigner, steps: Instruction[][], onProgress?: BatchProgress): Promise<Sent[]> {
  const sent: Sent[] = [];
  const total = steps.length;
  const fail = (e: unknown, signature: Signature | null): never => {
    const logs = e instanceof TxError ? e.logs : errorLogs(e);
    const msg = e instanceof TxError ? e.message : explainFailure(e, logs, steps[sent.length] ?? []);
    throw new BatchError(total > 1 ? `step ${sent.length + 1}/${total}: ${msg}` : msg, logs, sent, sent.length, signature);
  };
  const canSign = isTransactionPartialSigner(payer) || isTransactionModifyingSigner(payer);
  if (!canSign && isTransactionSendingSigner(payer)) {
    // A wallet that only signs and sends cannot hold a batch back: one popup per step, in order.
    for (const [i, ixs] of steps.entries()) {
      onProgress?.("send", i, total);
      let signature: Signature | null = null;
      try {
        const bytes = await signAndSendTransactionMessageWithSigners(message(payer, ixs, await blockhash()));
        signature = getBase58Decoder().decode(bytes) as Signature;
        await waitConfirmed(signature);
      } catch (e) {
        fail(e, signature);
      }
      sent.push(await landed(payer, signature!));
    }
    return sent;
  }
  let signed: Transaction[] = [];
  try {
    onProgress?.("sign", 0, total);
    signed = await signAll(payer, steps, await blockhash());
  } catch (e) {
    fail(e, null);
  }
  const { rpc, rpcSubscriptions } = client;
  const sendAndConfirm = sendAndConfirmTransactionFactory({ rpc, rpcSubscriptions });
  for (const [i, tx] of signed.entries()) {
    onProgress?.("send", i, total);
    const signature = getSignatureFromTransaction(tx);
    try {
      // The factory wants a lifetime-narrowed type; every message here has a blockhash lifetime.
      await sendAndConfirm(tx as Parameters<typeof sendAndConfirm>[0], { commitment: "confirmed" });
    } catch (e) {
      fail(e, signature);
    }
    sent.push(await landed(payer, signature));
  }
  return sent;
}

/** A placeholder lifetime: the size of a message does not depend on its blockhash. */
const SIZE_LIFETIME: Lifetime = { blockhash: "11111111111111111111111111111111" as Blockhash, lastValidBlockHeight: 0n };

/**
 * Packs instruction groups, in order, into as few transactions as fit the size limit (a group
 * never splits): fewer transactions in a batch, fewer that the wallet previews before the
 * accounts they write exist.
 */
export function packBySize(payer: TransactionSigner, groups: readonly Instruction[][]): Instruction[][] {
  const out: Instruction[][] = [];
  for (const g of groups) {
    const last = out[out.length - 1];
    if (last && isTransactionMessageWithinSizeLimit(message(payer, [...last, ...g], SIZE_LIFETIME))) out[out.length - 1] = [...last, ...g];
    else out.push([...g]);
  }
  return out;
}

/**
 * What a batch takes from the payer, in lamports: the first step simulated (its rent, its fee),
 * plus the fee of every later step (they write accounts the first creates, so they cannot be
 * simulated before it lands).
 */
export async function batchCost(payer: TransactionSigner, steps: Instruction[][]): Promise<bigint> {
  const { rpc } = client;
  const lifetime = await blockhash();
  const [first, ...rest] = steps.map((ixs) => message(payer, ixs, lifetime));
  if (!first) return 0n;
  const before = (await rpc.getBalance(payer.address, { commitment: "confirmed" }).send()).value;
  const sim = await rpc
    .simulateTransaction(getBase64EncodedWireTransaction(compileTransaction(first)), {
      encoding: "base64",
      sigVerify: false,
      replaceRecentBlockhash: true,
      commitment: "confirmed",
      accounts: { addresses: [payer.address], encoding: "base64" },
    })
    .send();
  if (sim.value.err) throw new TxError(explainFailure(new Error(JSON.stringify(sim.value.err)), [...(sim.value.logs ?? [])], steps[0]), [...(sim.value.logs ?? [])]);
  const after = BigInt(sim.value.accounts?.[0]?.lamports ?? before);
  let fees = 0n;
  for (const m of rest) {
    const bytes = getBase64Decoder().decode(compileTransaction(m).messageBytes) as TransactionMessageBytesBase64;
    const { value } = await rpc.getFeeForMessage(bytes, { commitment: "confirmed" }).send();
    fees += BigInt(value ?? 0n);
  }
  return before - after + fees;
}

/** Did a signature land (confirmed, no error)? Read before re-signing a step that failed to confirm. */
export async function hasLanded(signature: Signature): Promise<boolean> {
  const { value } = await client.rpc.getSignatureStatuses([signature], { searchTransactionHistory: true }).send();
  const st = value[0];
  return !!st && !st.err && (st.confirmationStatus === "confirmed" || st.confirmationStatus === "finalized");
}

type Lifetime = Parameters<typeof setTransactionMessageLifetimeUsingBlockhash>[0];

async function blockhash(): Promise<Lifetime> {
  return (await client.rpc.getLatestBlockhash({ commitment: "confirmed" }).send()).value;
}

function message(payer: TransactionSigner, instructions: Instruction[], lifetime: Lifetime) {
  return pipe(
    createTransactionMessage({ version: 0 }),
    (m) => setTransactionMessageFeePayerSigner(payer, m),
    (m) => setTransactionMessageLifetimeUsingBlockhash(lifetime, m),
    (m) => appendTransactionMessageInstructions(instructions, m),
  );
}

/**
 * The wallet signs every step in one call (one popup), then each step's other signers (the dev
 * keypair, a new asset keypair) sign what the wallet returned: kit's order, modifying signers
 * first, partial signers after.
 */
export async function signAll(payer: TransactionSigner, steps: Instruction[][], lifetime: Lifetime): Promise<Transaction[]> {
  const messages = steps.map((ixs) => message(payer, ixs, lifetime));
  const wallet = isTransactionModifyingSigner(payer) ? payer : null;
  let txs: Transaction[] = messages.map((m) => compileTransaction(m));
  if (wallet) txs = [...(await wallet.modifyAndSignTransactions(txs as never))];
  return Promise.all(
    messages.map((m, i) => {
      const others = getSignersFromTransactionMessage(m).filter((s) => s !== wallet);
      return signTransactionWithSigners(others as never, txs[i]! as never);
    }),
  );
}

async function landed(payer: TransactionSigner, signature: Signature): Promise<Sent> {
  // Balance badges re-read after own transactions; no window in node (the localnet e2e test).
  if (typeof window !== "undefined") window.dispatchEvent(new CustomEvent<string>(TX_SENT_EVENT, { detail: payer.address }));
  return { signature, logs: await logsOf(signature) };
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

/** "Name: message" of a Deadroll program's custom error, or null for another program or code. */
function programError(program: string, code: number): string | null {
  const name = programNameOf(program);
  const label = name ? sdkErrorName(name, code) : null;
  if (!name || !label) return null;
  const msg = errorMessage(name, code);
  return msg ? `${label}: ${msg}` : label;
}

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
      const code = parseInt(m[2]!, 16);
      const name = programError(m[1]!, code);
      return name ?? `program ${m[1]!.slice(0, 6)}.. error ${code}`;
    }
  }
  const msg = e instanceof Error ? e.message : String(e);
  // kit without logs: "Custom program error: #6016 (instruction #1)"; instruction #n is 1-based.
  const custom = /#(\d+) \(instruction #(\d+)\)/.exec(msg);
  if (custom) {
    const program = instructions[Number(custom[2]) - 1]?.programAddress;
    const name = program ? programError(program, Number(custom[1])) : null;
    if (name) return name;
  }
  const last = logs.filter((l) => /failed|Error/i.test(l)).pop();
  return last ? `${msg} (${last})` : msg;
}
