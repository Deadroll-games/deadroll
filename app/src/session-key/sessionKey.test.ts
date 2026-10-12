// @vitest-environment node
import { describe, expect, it } from "vitest";
import { AccountRole, getAddressDecoder, getAddressEncoder, getProgramDerivedAddress } from "@solana/kit";
import { toHex } from "../chain/bytes";
import { SESSION_KEYS_PROGRAM } from "../chain/pda";
import { PROGRAMS, SYSTEM_PROGRAM } from "../config";
import { createSessionV2Data, createSessionV2Instruction, newSeed, signerFromSeed } from "./sessionKey";

const addrOf = (n: number) => getAddressDecoder().decode(new Uint8Array(32).fill(n));

describe("create_session_v2", () => {
  it("encodes Some(true), Some(valid_until i64), Some(lamports u64)", () => {
    const hex = toHex(createSessionV2Data(0x0102030405n, 20_000_000n));
    expect(hex).toBe("dfe96c0741c2eb26" + "0101" + "01" + "0504030201000000" + "01" + "002d310100000000");
  });

  it("derives the SessionTokenV2 PDA and orders accounts like CreateSessionTokenV2", async () => {
    const authority = { address: addrOf(1) } as never;
    const burner = await signerFromSeed(newSeed());
    const { instruction, sessionToken } = await createSessionV2Instruction({
      authority,
      sessionSigner: burner,
      targetProgram: PROGRAMS.session,
      validUntil: 1n,
    });
    const enc = getAddressEncoder();
    const [expected] = await getProgramDerivedAddress({
      programAddress: SESSION_KEYS_PROGRAM,
      seeds: [new TextEncoder().encode("session_token"), enc.encode(PROGRAMS.session), enc.encode(burner.address), enc.encode(addrOf(1))],
    });
    expect(sessionToken).toBe(expected);
    expect(instruction.programAddress).toBe("KeyspM2ssCJbqUhQ4k7sveSiY4WjnYsrXkC8oDbwde5");
    expect(instruction.accounts?.map((a) => [a.address, a.role])).toEqual([
      [expected, AccountRole.WRITABLE],
      [burner.address, AccountRole.WRITABLE_SIGNER],
      [addrOf(1), AccountRole.WRITABLE_SIGNER],
      [addrOf(1), AccountRole.WRITABLE_SIGNER],
      [PROGRAMS.session, AccountRole.READONLY],
      [SYSTEM_PROGRAM, AccountRole.READONLY],
    ]);
  });

  it("restores the same burner from its seed", async () => {
    const seed = newSeed();
    expect((await signerFromSeed(seed)).address).toBe((await signerFromSeed(seed)).address);
  });
});
