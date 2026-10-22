// @vitest-environment node
// The MagicBlock package's instructions after the adapter, byte for byte against the session-keys
// 3.1.1 crate: `create_session_v2(top_up, valid_until, lamports)` with `CreateSessionTokenV2`,
// `revoke_session_v2` with `RevokeSessionTokenV2`.
import { describe, expect, it } from "vitest";
import { createHash } from "node:crypto";
import { AccountRole, getAddressDecoder, getAddressEncoder, getProgramDerivedAddress, type Address, type TransactionSigner } from "@solana/kit";
import { toHex } from "../chain/bytes";
import { SESSION_KEYS_PROGRAM } from "../chain/pda";
import { PROGRAMS, SYSTEM_PROGRAM } from "../config";
import { createSessionInstruction, revokeSessionInstruction } from "./magicblock";
import { newSeed, signerFromSeed } from "./sessionKey";

const addrOf = (n: number) => getAddressDecoder().decode(new Uint8Array(32).fill(n)) as Address;
const sha = (s: string) => createHash("sha256").update(s).digest("hex").slice(0, 16);
const wallet = { address: addrOf(1) } as TransactionSigner;

describe("MagicBlock Session Keys through the adapter", () => {
  it("create_session_v2: Some(true), Some(valid_until), Some(prepayment); accounts as CreateSessionTokenV2", async () => {
    const key = await signerFromSeed(newSeed());
    const { instruction, token } = await createSessionInstruction({
      authority: wallet,
      key,
      targetProgram: PROGRAMS.session,
      validUntil: 0x0102030405n,
      prepayLamports: 10_000_000n,
    });
    expect(instruction.programAddress).toBe(SESSION_KEYS_PROGRAM);
    expect(toHex(Uint8Array.from(instruction.data!))).toBe(sha("global:create_session_v2") + "0101" + "01" + "0504030201000000" + "01" + "8096980000000000");
    const enc = getAddressEncoder();
    const [expected] = await getProgramDerivedAddress({
      programAddress: SESSION_KEYS_PROGRAM,
      seeds: [new TextEncoder().encode("session_token_v2"), enc.encode(PROGRAMS.session), enc.encode(key.address), enc.encode(wallet.address)],
    });
    expect(token).toBe(expected);
    // token, session_signer, fee_payer, authority, target_program, system_program; the wallet
    // pays and authorizes, the key signs.
    expect(instruction.accounts?.map((a) => [a.address, a.role])).toEqual([
      [expected, AccountRole.WRITABLE],
      [key.address, AccountRole.WRITABLE_SIGNER],
      [wallet.address, AccountRole.WRITABLE_SIGNER],
      [wallet.address, AccountRole.READONLY_SIGNER],
      [PROGRAMS.session, AccountRole.READONLY],
      [SYSTEM_PROGRAM, AccountRole.READONLY],
    ]);
    expect(instruction.accounts?.[1]).toHaveProperty("signer", key);
  });

  it("revoke_session_v2: the rent goes to the wallet; it signs a live token's revoke", async () => {
    const token = addrOf(9);
    const signed = await revokeSessionInstruction(token, wallet);
    expect(toHex(Uint8Array.from(signed.data!))).toBe(sha("global:revoke_session_v2"));
    expect(signed.accounts?.map((a) => [a.address, a.role])).toEqual([
      [token, AccountRole.WRITABLE],
      [wallet.address, AccountRole.WRITABLE_SIGNER],
      [wallet.address, AccountRole.READONLY_SIGNER],
      [SYSTEM_PROGRAM, AccountRole.READONLY],
    ]);
    // An expired token: anyone may revoke, nobody signs.
    const unsigned = await revokeSessionInstruction(token, wallet.address);
    expect(unsigned.accounts?.map((a) => a.role)).toEqual([AccountRole.WRITABLE, AccountRole.WRITABLE, AccountRole.READONLY, AccountRole.READONLY]);
  });
});
