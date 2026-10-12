// MagicBlock Session Keys (crate session-keys 3.1.1, `create_session_v2`). The player signs once;
// afterwards a burner key signs game moves and the session program checks the SessionTokenV2
// (programs/session/src/auth.rs). The burner seed lives in sessionStorage: one tab, one party.
import {
  AccountRole,
  createKeyPairSignerFromPrivateKeyBytes,
  type AccountMeta,
  type AccountSignerMeta,
  type Address,
  type Instruction,
  type KeyPairSigner,
  type TransactionSigner,
} from "@solana/kit";
import { SYSTEM_PROGRAM } from "../config";
import { concat } from "../chain/bytes";
import { IX_DISC } from "../chain/disc";
import { SESSION_KEYS_PROGRAM, sessionTokenPda } from "../chain/pda";

/** The program caps validity at one week; the demo needs a few hours. */
export const SESSION_VALIDITY_SECS = 3 * 60 * 60;
/** Lamports moved to the burner key for fees (program default is 0.01 SOL). */
export const SESSION_TOP_UP_LAMPORTS = 20_000_000n;

const STORAGE_KEY = "deadroll.sessionKey.v1";

function i64le(v: bigint): Uint8Array {
  const out = new Uint8Array(8);
  new DataView(out.buffer).setBigInt64(0, v, true);
  return out;
}
function u64le(v: bigint): Uint8Array {
  const out = new Uint8Array(8);
  new DataView(out.buffer).setBigUint64(0, v, true);
  return out;
}

/** Borsh args: top_up Option<bool>, valid_until Option<i64>, lamports Option<u64>. */
export function createSessionV2Data(validUntil: bigint, topUpLamports: bigint): Uint8Array {
  return concat(
    IX_DISC.create_session_v2,
    new Uint8Array([1, 1]),
    new Uint8Array([1]),
    i64le(validUntil),
    new Uint8Array([1]),
    u64le(topUpLamports),
  );
}

export async function createSessionV2Instruction(args: {
  authority: TransactionSigner;
  sessionSigner: TransactionSigner;
  targetProgram: Address;
  validUntil: bigint;
  topUpLamports?: bigint;
}): Promise<{ instruction: Instruction; sessionToken: Address }> {
  const sessionToken = await sessionTokenPda(args.targetProgram, args.sessionSigner.address, args.authority.address);
  // Accounts in the order of CreateSessionTokenV2: token, session_signer, fee_payer, authority, target, system.
  // The authority pays (fee_payer == authority), so it appears once with the strongest role.
  const accounts: (AccountMeta | AccountSignerMeta)[] = [
    { address: sessionToken, role: AccountRole.WRITABLE },
    { address: args.sessionSigner.address, role: AccountRole.WRITABLE_SIGNER, signer: args.sessionSigner },
    { address: args.authority.address, role: AccountRole.WRITABLE_SIGNER, signer: args.authority },
    { address: args.authority.address, role: AccountRole.WRITABLE_SIGNER, signer: args.authority },
    { address: args.targetProgram, role: AccountRole.READONLY },
    { address: SYSTEM_PROGRAM, role: AccountRole.READONLY },
  ];
  return {
    instruction: {
      programAddress: SESSION_KEYS_PROGRAM,
      accounts,
      data: createSessionV2Data(args.validUntil, args.topUpLamports ?? SESSION_TOP_UP_LAMPORTS),
    },
    sessionToken,
  };
}

export type StoredSessionKey = { seed: Uint8Array; authority: string; validUntil: number; token: string | null };

function store(): Storage | null {
  try {
    return window.sessionStorage;
  } catch {
    return null;
  }
}

export function saveSessionKey(k: StoredSessionKey): void {
  try {
    store()?.setItem(
      STORAGE_KEY,
      JSON.stringify({ seed: Array.from(k.seed), authority: k.authority, validUntil: k.validUntil, token: k.token }),
    );
  } catch {
    // storage blocked: the key lives only in memory for this page
  }
}

export function loadSessionKey(): StoredSessionKey | null {
  try {
    const raw = store()?.getItem(STORAGE_KEY);
    if (!raw) return null;
    const v = JSON.parse(raw) as { seed: number[]; authority: string; validUntil: number; token: string | null };
    if (!Array.isArray(v.seed) || v.seed.length !== 32) return null;
    if (v.validUntil * 1000 <= Date.now()) return null;
    return { seed: Uint8Array.from(v.seed), authority: v.authority, validUntil: v.validUntil, token: v.token };
  } catch {
    return null;
  }
}

export function clearSessionKey(): void {
  try {
    store()?.removeItem(STORAGE_KEY);
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
