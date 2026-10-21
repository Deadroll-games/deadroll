// The temporary key of one party (docs/GAME_COMMISSIONS.md, 3-8). The main wallet signs once to
// create its MagicBlock token and prepayment (magicblock.ts) together with join_session or
// open_session; afterwards the key signs game moves and the session program checks the token and
// that the key is the one bound to this party. The key lives in this browser's local storage, one
// per (wallet, party), so a closed tab gets it back; another browser replaces it.
import { createKeyPairSignerFromPrivateKeyBytes, type Address, type KeyPairSigner } from "@solana/kit";
import balance from "../chain/balance.json";

/** `deadroll_rules::FeeTuning` (BalanceConfig.fees), lamports and seconds. */
export const FEES = {
  prepay: BigInt(balance.fees.prepay_lamports),
  lowBalance: BigInt(balance.fees.low_balance_lamports),
  maxBalance: BigInt(balance.fees.max_key_lamports),
  validitySecs: balance.fees.key_validity_secs,
};

export type StoredSessionKey = { seed: Uint8Array; token: Address; validUntil: number };

const PREFIX = "deadroll.sessionKey.v2";
const storageKey = (authority: Address, party: Address) => `${PREFIX}:${authority}:${party}`;

function store(): Storage | null {
  try {
    return window.localStorage;
  } catch {
    return null;
  }
}

export function saveSessionKey(authority: Address, party: Address, k: StoredSessionKey): void {
  try {
    store()?.setItem(storageKey(authority, party), JSON.stringify({ seed: Array.from(k.seed), token: k.token, validUntil: k.validUntil }));
  } catch {
    // storage blocked: the key lives only in memory for this page
  }
}

/** The key saved for this wallet and party, expired or not (an expired token is still revoked). */
export function loadSessionKey(authority: Address, party: Address): StoredSessionKey | null {
  try {
    const raw = store()?.getItem(storageKey(authority, party));
    if (!raw) return null;
    const v = JSON.parse(raw) as { seed: number[]; token: string; validUntil: number };
    if (!Array.isArray(v.seed) || v.seed.length !== 32 || typeof v.token !== "string") return null;
    return { seed: Uint8Array.from(v.seed), token: v.token as Address, validUntil: v.validUntil };
  } catch {
    return null;
  }
}

export function clearSessionKey(authority: Address, party: Address): void {
  try {
    store()?.removeItem(storageKey(authority, party));
  } catch {
    // ignore
  }
}

export function newSeed(): Uint8Array {
  return crypto.getRandomValues(new Uint8Array(32));
}

export function signerFromSeed(seed: Uint8Array): Promise<KeyPairSigner> {
  return createKeyPairSignerFromPrivateKeyBytes(seed);
}

/** `valid_until` of a token created now. */
export const validUntilFromNow = (nowMs = Date.now()): number => Math.floor(nowMs / 1000) + FEES.validitySecs;

/** How much a top-up may add without passing the key's maximum balance (0 = none). */
export const topUpRoom = (balanceLamports: bigint): bigint => (balanceLamports >= FEES.maxBalance ? 0n : FEES.maxBalance - balanceLamports);

export const isLowBalance = (balanceLamports: bigint): boolean => balanceLamports < FEES.lowBalance;
