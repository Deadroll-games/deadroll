// @vitest-environment jsdom
import { beforeEach, describe, expect, it } from "vitest";
import { getAddressDecoder, type Address } from "@solana/kit";
import balance from "../chain/balance.json";
import { toHex } from "../chain/bytes";
import { transferData } from "../chain/system";
import { FEES, clearSessionKey, isLowBalance, loadSessionKey, newSeed, saveSessionKey, signerFromSeed, topUpRoom, validUntilFromNow } from "./sessionKey";

const addrOf = (n: number) => getAddressDecoder().decode(new Uint8Array(32).fill(n)) as Address;

describe("fees (BalanceConfig.fees)", () => {
  it("come from balance.json: 0.01 SOL prepayment, warning below 0.002, top-ups up to 0.05, 3 h tokens", () => {
    expect(FEES.prepay).toBe(BigInt(balance.fees.prepay_lamports));
    expect([FEES.prepay, FEES.lowBalance, FEES.maxBalance]).toEqual([10_000_000n, 2_000_000n, 50_000_000n]);
    expect(FEES.validitySecs).toBe(3 * 60 * 60);
    expect(validUntilFromNow(1_000_000)).toBe(1_000 + FEES.validitySecs);
  });

  it("a top-up never passes the maximum; low balance is below the warning line", () => {
    expect(topUpRoom(0n)).toBe(FEES.maxBalance);
    expect(topUpRoom(FEES.maxBalance - 1n)).toBe(1n);
    expect(topUpRoom(FEES.maxBalance)).toBe(0n);
    expect(topUpRoom(FEES.maxBalance + 5n)).toBe(0n);
    expect([isLowBalance(FEES.lowBalance - 1n), isLowBalance(FEES.lowBalance)]).toEqual([true, false]);
  });
});

describe("one key per wallet and party", () => {
  beforeEach(() => window.localStorage.clear());

  it("saves, restores and forgets a key by wallet and party", async () => {
    const seed = newSeed();
    saveSessionKey(addrOf(1), addrOf(2), { seed, token: addrOf(3), validUntil: 42 });
    const got = loadSessionKey(addrOf(1), addrOf(2))!;
    expect(got.token).toBe(addrOf(3));
    expect(got.validUntil).toBe(42);
    expect((await signerFromSeed(got.seed)).address).toBe((await signerFromSeed(seed)).address);
    // Another party, another wallet: no key.
    expect(loadSessionKey(addrOf(1), addrOf(4))).toBeNull();
    expect(loadSessionKey(addrOf(5), addrOf(2))).toBeNull();
    clearSessionKey(addrOf(1), addrOf(2));
    expect(loadSessionKey(addrOf(1), addrOf(2))).toBeNull();
  });

  it("ignores a broken entry", () => {
    window.localStorage.setItem(`deadroll.sessionKey.v2:${addrOf(1)}:${addrOf(2)}`, '{"seed":[1,2]}');
    expect(loadSessionKey(addrOf(1), addrOf(2))).toBeNull();
  });
});

describe("system transfer", () => {
  it("encodes Transfer (2, u32) and the lamports (u64), little endian", () => {
    expect(toHex(transferData(10_000_000n))).toBe("02000000" + "8096980000000000");
  });
});
