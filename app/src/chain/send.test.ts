// @vitest-environment node
import { beforeEach, describe, expect, it, vi } from "vitest";
import {
  AccountRole,
  generateKeyPairSigner,
  getSignatureFromTransaction,
  getTransactionMessageSize,
  type Address,
  type Instruction,
  type KeyPairSigner,
  type Transaction,
  type TransactionModifyingSigner,
} from "@solana/kit";

// What reached the network, in order, and which signatures failed.
const sentTxs: Transaction[] = [];
const failing = new Set<number>();
const landedSigs = new Set<string>();

vi.mock("@solana/kit", async (orig) => ({
  ...(await orig<typeof import("@solana/kit")>()),
  sendAndConfirmTransactionFactory: () => async (tx: Transaction) => {
    if (failing.has(sentTxs.length)) {
      sentTxs.push(tx);
      throw new Error("block height exceeded");
    }
    sentTxs.push(tx);
  },
}));

vi.mock("../wallet/client", () => ({
  client: {
    rpc: {
      getLatestBlockhash: () => ({ send: async () => ({ value: { blockhash: "11111111111111111111111111111111", lastValidBlockHeight: 100n } }) }),
      getTransaction: () => ({ send: async () => ({ meta: { logMessages: ["Program log: ok"] } }) }),
      getSignatureStatuses: (sigs: string[]) => ({
        send: async () => ({ value: sigs.map((s) => (landedSigs.has(s) ? { err: null, confirmationStatus: "confirmed" } : null)) }),
      }),
    },
    rpcSubscriptions: {},
  },
}));

import { BatchError, CONFIRM_POLL_MS, logsRetryMs, packBySize, sendBatch } from "./send";
import { progressText, remainingAfter } from "./useTx";

const PROGRAM = "11111111111111111111111111111111" as Address;

/** A Wallet Standard-like signer: one `modifyAndSignTransactions` call per popup. */
function wallet(kp: KeyPairSigner, log: string[]): TransactionModifyingSigner {
  const w = {
    address: kp.address,
    async modifyAndSignTransactions(txs: readonly Transaction[]) {
      log.push(`wallet x${txs.length}`);
      const dicts = await kp.signTransactions(txs as never);
      return txs.map((t, i) => Object.freeze({ ...t, signatures: Object.freeze({ ...t.signatures, ...dicts[i] }) })) as never;
    },
  };
  return w;
}

/** A partial signer (the new Core asset keypair) that records when it signs. */
function partial(kp: KeyPairSigner, log: string[]): KeyPairSigner {
  return { ...kp, signTransactions: async (txs) => (log.push("asset"), kp.signTransactions(txs)) };
}

const ix = (data: number, signer?: KeyPairSigner): Instruction =>
  ({
    programAddress: PROGRAM,
    accounts: signer ? [{ address: signer.address, role: AccountRole.WRITABLE_SIGNER, signer }] : [],
    data: new Uint8Array([data]),
  }) as unknown as Instruction;

beforeEach(() => {
  sentTxs.length = 0;
  failing.clear();
  landedSigs.clear();
});

describe("send timing", () => {
  it("polls signature status no faster than every 1.5 s", () => {
    expect(CONFIRM_POLL_MS).toBeGreaterThanOrEqual(1_500);
  });
  it("backs off the log fetch", () => {
    expect([0, 1, 2].map(logsRetryMs)).toEqual([500, 1_000, 1_500]);
  });
});

describe("sendBatch: one wallet popup per action", () => {
  it("signs every step in one wallet call, then the asset keypair, and sends them in order", async () => {
    const log: string[] = [];
    const w = wallet(await generateKeyPairSigner(), log);
    const asset = partial(await generateKeyPairSigner(), log);
    const progress: string[] = [];
    const sent = await sendBatch(w, [[ix(1, asset)], [ix(2)], [ix(3)]], (phase, i, total) => progress.push(`${phase} ${i}/${total}`));
    // One popup for three transactions. Modifying signers first, partial signers on what the wallet returned (kit's order).
    expect(log).toEqual(["wallet x3", "asset"]);
    expect(sentTxs.map((t) => Object.keys(t.signatures).length)).toEqual([2, 1, 1]);
    expect(sent.map((s) => s.signature)).toEqual(sentTxs.map(getSignatureFromTransaction));
    expect(sent[0]!.logs).toEqual(["Program log: ok"]);
    expect(progress).toEqual(["sign 0/3", "send 0/3", "send 1/3", "send 2/3"]);
  });

  it("stops at a failed step: nothing after it is sent, the error names the step", async () => {
    const w = wallet(await generateKeyPairSigner(), []);
    failing.add(1);
    const err = await sendBatch(w, [[ix(1)], [ix(2)], [ix(3)]]).catch((e: unknown) => e);
    expect(err).toBeInstanceOf(BatchError);
    const b = err as BatchError;
    expect([b.failedAt, b.sent.length, sentTxs.length]).toEqual([1, 1, 2]);
    expect(b.signature).toBe(getSignatureFromTransaction(sentTxs[1]!));
    expect(b.message).toMatch(/^step 2\/3: /);
  });

  it("a refused popup sends nothing", async () => {
    const kp = await generateKeyPairSigner();
    const w: TransactionModifyingSigner = {
      address: kp.address,
      modifyAndSignTransactions: async () => {
        throw new Error("User rejected the request");
      },
    };
    const b = (await sendBatch(w, [[ix(1)], [ix(2)]]).catch((e: unknown) => e)) as BatchError;
    expect([b.failedAt, b.signature, sentTxs.length]).toEqual([0, null, 0]);
  });

  it("a keypair payer (dev wallet, session key) signs without a modifying signer", async () => {
    const kp = await generateKeyPairSigner();
    const sent = await sendBatch(kp, [[ix(1)], [ix(2)]]);
    expect(sent).toHaveLength(2);
    expect(sentTxs).toHaveLength(2);
  });
});

describe("retry", () => {
  it("skips the failed step when it landed after all", async () => {
    const e = new BatchError("x", [], [], 1, "sig1" as never);
    landedSigs.add("sig1");
    expect(await remainingAfter(e, 3)).toEqual({ landed: { signature: "sig1", logs: [] }, from: 2 });
  });
  it("resends the failed step when it did not land", async () => {
    expect(await remainingAfter(new BatchError("x", [], [], 1, "sig2" as never), 3)).toEqual({ landed: null, from: 1 });
    expect(await remainingAfter(new BatchError("x", [], [], 0, null), 3)).toEqual({ landed: null, from: 0 });
  });
  it("shows the step being sent", () => {
    expect(progressText("Publish the map", "sign", 0, 5)).toBe("Publish the map: confirm in the wallet");
    expect(progressText("Publish the map", "send", 1, 5)).toBe("Publish the map 2/5");
    expect(progressText("Post", "send", 0, 1)).toBe("Post");
  });
});

describe("packBySize", () => {
  const big = (n: number): Instruction => ({ programAddress: PROGRAM, accounts: [], data: new Uint8Array(400).fill(n) }) as Instruction;
  it("packs groups in order into as few transactions as fit", async () => {
    const payer = await generateKeyPairSigner();
    const groups = [1, 2, 3, 4, 5, 6].map((n) => [big(n)]);
    const packed = packBySize(payer, groups);
    expect(packed.length).toBeLessThan(groups.length);
    expect(packed.flat()).toEqual(groups.flat());
    for (const p of packed) {
      const size = getTransactionMessageSize({
        version: 0,
        feePayer: payer,
        lifetimeConstraint: { blockhash: PROGRAM, lastValidBlockHeight: 0n },
        instructions: p,
      } as never);
      expect(size).toBeLessThanOrEqual(1232);
    }
  });
  it("never splits a group", async () => {
    const payer = await generateKeyPairSigner();
    const packed = packBySize(payer, [[ix(1), ix(2)], [ix(3)]]);
    expect(packed).toEqual([[ix(1), ix(2), ix(3)]]);
  });
});
