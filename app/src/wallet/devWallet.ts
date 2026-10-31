// Dev wallet for localnet demos (`?dev=1`): one fresh keypair per browser tab, kept in
// sessionStorage, airdropped from the local validator. DM and two players run in three tabs or
// windows side by side without a wallet extension. Never offered for a non-local RPC.
import { createKeyPairSignerFromPrivateKeyBytes, lamports, type Address, type KeyPairSigner } from "@solana/kit";
import { DEV_TOOLS, IS_LOCALNET } from "../config";

const SEED_KEY = "deadroll.devWallet.v1";
const FLAG_KEY = "deadroll.devMode.v1";

export const DEV_AIRDROP_LAMPORTS = 5_000_000_000n;
export const DEV_MIN_LAMPORTS = 1_000_000_000n;

function storage(): Storage | null {
  try {
    return window.sessionStorage;
  } catch {
    return null;
  }
}

/**
 * `?dev=1` turns dev mode on for this tab (it survives navigation); `?dev=0` turns it off.
 * Never in the devnet build, even with a localhost Custom RPC.
 */
export function devModeRequested(
  search: string = typeof location === "undefined" ? "" : location.search,
  allowed: boolean = DEV_TOOLS && IS_LOCALNET,
): boolean {
  if (!allowed) return false;
  const q = new URLSearchParams(search).get("dev");
  try {
    if (q === "1") storage()?.setItem(FLAG_KEY, "1");
    if (q === "0") storage()?.removeItem(FLAG_KEY);
    return storage()?.getItem(FLAG_KEY) === "1";
  } catch {
    return q === "1";
  }
}

/** The tab's dev keypair: loaded from sessionStorage, or created and stored. */
export async function loadDevSigner(): Promise<KeyPairSigner> {
  let seed: Uint8Array | null = null;
  try {
    const raw = storage()?.getItem(SEED_KEY);
    const arr = raw ? (JSON.parse(raw) as number[]) : null;
    if (Array.isArray(arr) && arr.length === 32) seed = Uint8Array.from(arr);
  } catch {
    seed = null;
  }
  if (!seed) {
    seed = crypto.getRandomValues(new Uint8Array(32));
    try {
      storage()?.setItem(SEED_KEY, JSON.stringify(Array.from(seed)));
    } catch {
      // storage blocked: the key lives only for this page load
    }
  }
  return createKeyPairSignerFromPrivateKeyBytes(seed);
}

type AirdropRpc = {
  getBalance(a: Address, c?: { commitment: "confirmed" }): { send(): Promise<{ value: bigint }> };
  requestAirdrop(a: Address, l: ReturnType<typeof lamports>, c?: { commitment: "confirmed" }): { send(): Promise<string> };
  getSignatureStatuses(s: string[]): { send(): Promise<{ value: ({ confirmationStatus: string | null } | null)[] }> };
};

/** Airdrops when the balance is under `DEV_MIN_LAMPORTS`; waits for confirmation. Returns the balance. */
export async function ensureDevFunds(rpc: AirdropRpc, who: Address, force = false): Promise<bigint> {
  const { value } = await rpc.getBalance(who, { commitment: "confirmed" }).send();
  if (value >= DEV_MIN_LAMPORTS && !force) return value;
  const sig = await rpc.requestAirdrop(who, lamports(DEV_AIRDROP_LAMPORTS), { commitment: "confirmed" }).send();
  for (let i = 0; i < 60; i++) {
    const { value: st } = await rpc.getSignatureStatuses([sig]).send();
    const s = st[0]?.confirmationStatus;
    if (s === "confirmed" || s === "finalized") break;
    await new Promise((r) => setTimeout(r, 500));
  }
  return (await rpc.getBalance(who, { commitment: "confirmed" }).send()).value;
}

/**
 * Patch C: the same dev wallet in another tab or browser profile (the DM opening its map secrets
 * on another device, without any local file): the tab's seed as hex, and a seed pasted from
 * another tab. Dev mode only (localnet), a throwaway key.
 */
export function exportDevSeed(): string | null {
  try {
    const raw = storage()?.getItem(SEED_KEY);
    const arr = raw ? (JSON.parse(raw) as number[]) : null;
    return Array.isArray(arr) && arr.length === 32 ? arr.map((b) => b.toString(16).padStart(2, "0")).join("") : null;
  } catch {
    return null;
  }
}

/** Replaces the tab's dev keypair with `hex` (32 bytes); false when it is not a seed. */
export function importDevSeed(hex: string): boolean {
  const clean = hex.trim().toLowerCase();
  if (!/^[0-9a-f]{64}$/.test(clean)) return false;
  const seed = Array.from({ length: 32 }, (_, i) => parseInt(clean.slice(i * 2, i * 2 + 2), 16));
  try {
    storage()?.setItem(SEED_KEY, JSON.stringify(seed));
    return true;
  } catch {
    return false;
  }
}
