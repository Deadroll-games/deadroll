// The session key of one party in this browser (docs/GAME_COMMISSIONS.md 3, 6.2, 8): its signer,
// token and balance, and the instructions that start (at Ready, docs/PARTY_AND_TURNS.md 2.1),
// top up, replace and leave it. The main wallet signs those (one popup each); the key signs the
// game moves.
import { useCallback, useEffect, useState } from "react";
import type { Address, Instruction, KeyPairSigner, TransactionSigner } from "@solana/kit";
import { DATA_SOURCE, PROGRAMS } from "../config";
import { client } from "../wallet/client";
import { TX_SENT_EVENT } from "../chain/txEvents";
import { setSessionKeyIx } from "../chain/game";
import { transferSolIx } from "../chain/system";
import { useAppWallet } from "../wallet/AppWallet";
import { createSessionInstruction, fetchSessionToken, revokeSessionInstruction } from "./magicblock";
import {
  FEES,
  clearSessionKey,
  isLowBalance,
  loadSessionKey,
  newSeed,
  readyKeyPlan,
  saveSessionKey,
  signerFromSeed,
  topUpRoom,
  validUntilFromNow,
  type ReadyKeyPlan,
} from "./sessionKey";

/** A key made for a transaction that has not confirmed yet: `commitKey` after it did. */
export type PreparedKey = { signer: KeyPairSigner; seed: Uint8Array; token: Address; validUntil: number; instruction: Instruction };

/** A new key for `party`: `instruction` creates its token (valid for `FEES.validitySecs`) and moves
 * the prepayment to it, paid and signed by the main wallet. */
export async function prepareKey(authority: TransactionSigner, prepay: bigint = FEES.prepay): Promise<PreparedKey> {
  const seed = newSeed();
  const signer = await signerFromSeed(seed);
  const validUntil = validUntilFromNow();
  const { instruction, token } = await createSessionInstruction({
    authority,
    key: signer,
    targetProgram: PROGRAMS.session,
    validUntil: BigInt(validUntil),
    prepayLamports: prepay,
  });
  return { signer, seed, token, validUntil, instruction };
}

export function commitKey(authority: Address, party: Address, k: PreparedKey): void {
  saveSessionKey(authority, party, { seed: k.seed, token: k.token, validUntil: k.validUntil });
}

async function balanceOf(a: Address): Promise<bigint> {
  return (await client.rpc.getBalance(a, { commitment: "confirmed" }).send()).value;
}

/** Revoke the token if it is still there (its rent goes back to the wallet) and move everything
 * the key holds to the wallet. The wallet pays the fee, so the key ends at zero. */
async function returnKeyIxs(wallet: TransactionSigner, key: KeyPairSigner, token: Address): Promise<Instruction[]> {
  const out: Instruction[] = [];
  if (await fetchSessionToken(token)) out.push(await revokeSessionInstruction(token, wallet));
  const left = await balanceOf(key.address);
  if (left > 0n) out.push(transferSolIx(key, wallet.address, left));
  return out;
}

export type PartyKey = {
  /** The key saved in this browser for (wallet, party); null when there is none. */
  signer: KeyPairSigner | null;
  token: Address | null;
  validUntil: number | null;
  /** Its token is on chain and not expired. */
  live: boolean;
  balance: bigint | null;
  low: boolean;
  /** `signer` when it may sign for `bound` (the key the party has for that unit), else null. */
  signerFor: (bound: Address) => KeyPairSigner | null;
  /** Main wallet -> key, at most up to `FEES.maxBalance`; null when there is no room. */
  topUpIx: (wallet: TransactionSigner, lamports: bigint) => Instruction | null;
  /** The instructions that hand this browser's key back: revoke + refund. */
  returnIxs: (wallet: TransactionSigner) => Promise<Instruction[]>;
  /** What Ready does with the key of a participation bound to `bound` (see `readyKeyPlan`). */
  readyPlan: (bound: Address) => ReadyKeyPlan;
  /** The instructions Ready puts before `set_ready(true)`: for a new key this browser's old key
   * gives back what it holds, the new token takes the prepayment, and `set_session_key` binds it;
   * `prepared` is committed with `replaced` once the transaction confirmed. */
  readyIxs: (wallet: TransactionSigner, session: Address, character: Address, bound: Address) => Promise<{ ixs: Instruction[]; prepared: PreparedKey | null }>;
  /** After a replace or leave confirmed: drop the old key, or use the new one. */
  replaced: (next: PreparedKey | null) => void;
  refresh: () => Promise<void>;
};

const POLL_MS = 20_000;

export function usePartyKey(party: Address | null): PartyKey {
  const wallet = useAppWallet();
  const authority = wallet.address;
  const [signer, setSigner] = useState<KeyPairSigner | null>(null);
  const [token, setToken] = useState<Address | null>(null);
  const [validUntil, setValidUntil] = useState<number | null>(null);
  const [live, setLive] = useState(false);
  const [balance, setBalance] = useState<bigint | null>(null);

  const load = useCallback(async () => {
    if (DATA_SOURCE !== "chain" || !authority || !party) return void setSigner(null);
    const saved = loadSessionKey(authority, party);
    if (!saved) {
      setSigner(null);
      setToken(null);
      setValidUntil(null);
      setLive(false);
      setBalance(null);
      return;
    }
    const s = await signerFromSeed(saved.seed);
    setSigner(s);
    setToken(saved.token);
    setValidUntil(saved.validUntil);
    try {
      const [t, b] = await Promise.all([fetchSessionToken(saved.token), balanceOf(s.address)]);
      setLive(!!t && t.sessionSigner === s.address && t.validUntil * 1000 > Date.now());
      setBalance(b);
    } catch {
      // keep the last values
    }
  }, [authority, party]);

  useEffect(() => {
    void load();
    const t = setInterval(() => {
      if (!document.hidden) void load();
    }, POLL_MS);
    // The key pays the moves: re-read its balance after every transaction it sends.
    const onSent = () => void load();
    window.addEventListener(TX_SENT_EVENT, onSent);
    return () => {
      clearInterval(t);
      window.removeEventListener(TX_SENT_EVENT, onSent);
    };
  }, [load]);

  const signerFor = useCallback((bound: Address) => (signer && live && signer.address === bound ? signer : null), [signer, live]);

  const topUpIx = useCallback(
    (from: TransactionSigner, lamports: bigint) => {
      if (!signer) return null;
      const room = balance === null ? lamports : topUpRoom(balance);
      const amount = lamports < room ? lamports : room;
      return amount > 0n ? transferSolIx(from, signer.address, amount) : null;
    },
    [signer, balance],
  );

  const returnIxs = useCallback(
    async (from: TransactionSigner) => (signer && token ? returnKeyIxs(from, signer, token) : []),
    [signer, token],
  );

  const readyPlan = useCallback((bound: Address) => readyKeyPlan(signer?.address ?? null, live, bound), [signer, live]);

  const readyIxs = useCallback(
    async (from: TransactionSigner, session: Address, character: Address, bound: Address) => {
      const plan = readyPlan(bound);
      if (plan === "reuse") return { ixs: [], prepared: null };
      if (plan === "bind") return { ixs: [await setSessionKeyIx(from, session, character, signer!.address)], prepared: null };
      const prepared = await prepareKey(from);
      const ixs = signer && token ? await returnKeyIxs(from, signer, token) : [];
      ixs.push(prepared.instruction, await setSessionKeyIx(from, session, character, prepared.signer.address));
      return { ixs, prepared };
    },
    [readyPlan, signer, token],
  );

  const replaced = useCallback(
    (next: PreparedKey | null) => {
      if (!authority || !party) return;
      if (next) commitKey(authority, party, next);
      else clearSessionKey(authority, party);
      void load();
    },
    [authority, party, load],
  );

  return {
    signer,
    token,
    validUntil,
    live,
    balance,
    low: balance !== null && isLowBalance(balance),
    signerFor,
    topUpIx,
    returnIxs,
    readyPlan,
    readyIxs,
    replaced,
    refresh: load,
  };
}
