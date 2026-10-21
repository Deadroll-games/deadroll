// Adapter from the MagicBlock Session Keys client (@magicblock-labs/gum-sdk 3.0.10: web3.js v1 and
// Anchor 0.30 over the official `gpl_session` IDL) to this site's @solana/kit stack. The package
// builds `create_session_v2` / `revoke_session_v2` and reads `SessionTokenV2`; kit signs and sends
// (chain/send.ts), so the token, the prepayment and a game instruction share one transaction.
// gum-react-sdk is not used: it makes V1 tokens in a transaction of its own, one key per wallet,
// and needs React 18 and wallet-adapter.
import "./bufferGlobal";
import { AnchorProvider, BN, web3 } from "@coral-xyz/anchor";
import { SessionTokenManager } from "@magicblock-labs/gum-sdk";
import { AccountRole, address, type AccountMeta, type AccountSignerMeta, type Address, type Instruction, type TransactionSigner } from "@solana/kit";
import { RPC_URL, SYSTEM_PROGRAM } from "../config";
import { sessionTokenPda } from "../chain/pda";

type Wallet = ConstructorParameters<typeof AnchorProvider>[1];

/** The package wants a wallet for its provider; it only builds instructions and reads here. */
function readOnlyWallet(publicKey: web3.PublicKey): Wallet {
  const refuse = () => Promise.reject(new Error("the session key manager never signs; kit does"));
  return { publicKey, signTransaction: refuse, signAllTransactions: refuse } as Wallet;
}

let connection: web3.Connection | null = null;
function manager(): SessionTokenManager {
  connection ??= new web3.Connection(RPC_URL, "confirmed");
  return new SessionTokenManager(readOnlyWallet(web3.PublicKey.default), connection);
}

const pk = (a: Address) => new web3.PublicKey(a);

/**
 * A web3.js instruction as a kit one. An account whose address is one of `signers` signs with it
 * (`revoke_session_v2` lists the authority as a plain account; it must sign while the token lives).
 */
export function toKitInstruction(ix: web3.TransactionInstruction, signers: readonly TransactionSigner[]): Instruction {
  const by = new Map<string, TransactionSigner>(signers.map((s) => [s.address, s]));
  const accounts = ix.keys.map((k): AccountMeta | AccountSignerMeta => {
    const a = address(k.pubkey.toBase58());
    const signer = by.get(a);
    if (signer) return { address: a, role: k.isWritable ? AccountRole.WRITABLE_SIGNER : AccountRole.READONLY_SIGNER, signer };
    return { address: a, role: k.isWritable ? AccountRole.WRITABLE : AccountRole.READONLY };
  });
  return { programAddress: address(ix.programId.toBase58()), accounts, data: Uint8Array.from(ix.data) };
}

/**
 * `create_session_v2`: `authority` (the main wallet) lets `key` act in `targetProgram` until
 * `validUntil` (unix seconds), pays the token rent (it comes back on revoke) and moves
 * `prepayLamports` to the key for its fees.
 */
export async function createSessionInstruction(args: {
  authority: TransactionSigner;
  key: TransactionSigner;
  targetProgram: Address;
  validUntil: bigint;
  prepayLamports: bigint;
}): Promise<{ instruction: Instruction; token: Address }> {
  const token = await sessionTokenPda(args.targetProgram, args.key.address, args.authority.address);
  const ix = await manager()
    .program.methods.createSessionV2(true, new BN(args.validUntil.toString()), new BN(args.prepayLamports.toString()))
    .accountsStrict({
      sessionToken: pk(token),
      sessionSigner: pk(args.key.address),
      feePayer: pk(args.authority.address),
      authority: pk(args.authority.address),
      targetProgram: pk(args.targetProgram),
      systemProgram: pk(SYSTEM_PROGRAM),
    })
    .instruction();
  return { instruction: toKitInstruction(ix, [args.authority, args.key]), token };
}

/** `revoke_session_v2`: closes the token, its rent goes back to the main wallet. A live token
 * needs the main wallet's signature; an expired one anybody may revoke. */
export async function revokeSessionInstruction(token: Address, authority: TransactionSigner | Address): Promise<Instruction> {
  const authorityAddress = typeof authority === "string" ? authority : authority.address;
  const ix = await manager()
    .program.methods.revokeSessionV2()
    .accountsStrict({
      sessionToken: pk(token),
      feePayer: pk(authorityAddress),
      authority: pk(authorityAddress),
      systemProgram: pk(SYSTEM_PROGRAM),
    })
    .instruction();
  return toKitInstruction(ix, typeof authority === "string" ? [] : [authority]);
}

export type SessionTokenInfo = { authority: Address; targetProgram: Address; sessionSigner: Address; validUntil: number };

/** The `SessionTokenV2` account, or null when there is none (revoked, never created). */
export async function fetchSessionToken(token: Address): Promise<SessionTokenInfo | null> {
  const t = await manager().program.account.sessionTokenV2.fetchNullable(pk(token));
  if (!t) return null;
  return {
    authority: address(t.authority.toBase58()),
    targetProgram: address(t.targetProgram.toBase58()),
    sessionSigner: address(t.sessionSigner.toBase58()),
    validUntil: t.validUntil.toNumber(),
  };
}
