// @vitest-environment node
import { describe, expect, it, vi } from "vitest";
import { getAddressDecoder, type Address } from "@solana/kit";
import { mapAccountSource } from "./accounts";
import { concat, u16le, u32le } from "./bytes";
import { ACCOUNT_DISC } from "./disc";
import type { CharacterDied } from "./layouts";
import { NON_OWNER_DELAY_MS, decideFreeze, handleCharacterDied } from "./tombstone";

const A = (n: number) => new Uint8Array(32).fill(n);
const addrOf = (n: number): Address => getAddressDecoder().decode(A(n));

/** Minimal Character account: only alive and nft_frozen matter here. */
function character(alive: boolean, frozen: boolean): Uint8Array {
  const layer = concat(u16le(0), new Uint8Array(3));
  return concat(
    ACCOUNT_DISC.Character,
    new Uint8Array([1]),
    A(2),
    new Uint8Array([0, 1]),
    A(4),
    new Uint8Array([0, 0]),
    u16le(1),
    u32le(0),
    new Uint8Array(8),
    new Uint8Array([5, 1]),
    u32le(30),
    new Uint8Array([alive ? 1 : 0, 0]),
    ...Array.from({ length: 20 }, () => layer),
    u32le(1),
    new Uint8Array([255]),
    new Uint8Array(32),
    new Uint8Array([0, frozen ? 1 : 0]),
  );
}

const ev: CharacterDied = { character: addrOf(1), owner: addrOf(2), session: addrOf(3), nft: addrOf(4), stateVersion: 1 };
const payer = { address: addrOf(9) } as never;

function run(opts: { me: Address | null; account: Uint8Array | null; nft?: Address | null; sendFails?: boolean }) {
  const send = vi.fn(async () => {
    if (opts.sendFails) throw new Error("AlreadyFrozen");
    return "sig1";
  });
  const sleep = vi.fn(async () => {});
  const accounts = mapAccountSource(new Map(opts.account ? [[ev.character as string, opts.account]] : []));
  const e = { ...ev, nft: opts.nft === undefined ? ev.nft : opts.nft };
  return { send, sleep, result: handleCharacterDied({ ev: e, me: opts.me, payer, accounts, send, sleep }) };
}

describe("decideFreeze", () => {
  it("covers every branch", () => {
    expect(decideFreeze({ ...ev, nft: null }, null)).toEqual({ kind: "skip", reason: "no-nft" });
    expect(decideFreeze(ev, null)).toEqual({ kind: "skip", reason: "missing" });
    expect(decideFreeze(ev, { alive: true, nftFrozen: false, nft: ev.nft })).toEqual({ kind: "skip", reason: "alive" });
    expect(decideFreeze(ev, { alive: false, nftFrozen: true, nft: ev.nft })).toEqual({ kind: "skip", reason: "frozen" });
    expect(decideFreeze(ev, { alive: false, nftFrozen: false, nft: ev.nft })).toEqual({ kind: "send" });
  });
});

describe("handleCharacterDied", () => {
  it("owner sends freeze_tombstone at once", async () => {
    const { send, sleep, result } = run({ me: ev.owner, account: character(false, false) });
    expect(await result).toMatchObject({ status: "sent", detail: "sig1" });
    expect(sleep).not.toHaveBeenCalled();
    expect(send).toHaveBeenCalledTimes(1);
    const ixs = (send.mock.calls[0] as unknown as [unknown, { accounts: { address: Address }[] }[]])[1];
    expect(ixs[0]!.accounts.map((a) => a.address).slice(0, 3)).toEqual([ev.character, ev.nft, addrOf(9)]);
  });

  it("non-owner waits, then sends only if still unfrozen", async () => {
    const a = run({ me: addrOf(7), account: character(false, false) });
    expect(await a.result).toMatchObject({ status: "sent" });
    expect(a.sleep).toHaveBeenCalledWith(NON_OWNER_DELAY_MS);
    const b = run({ me: addrOf(7), account: character(false, true) });
    expect(await b.result).toMatchObject({ status: "skipped", detail: "frozen" });
    expect(b.send).not.toHaveBeenCalled();
  });

  it("skips without an NFT (fallback) and for a missing or living character", async () => {
    const noNft = run({ me: ev.owner, account: character(false, false), nft: null });
    expect(await noNft.result).toMatchObject({ status: "skipped" });
    expect(noNft.send).not.toHaveBeenCalled();
    expect(await run({ me: ev.owner, account: null }).result).toMatchObject({ status: "skipped", detail: "missing" });
    expect(await run({ me: ev.owner, account: character(true, false) }).result).toMatchObject({ status: "skipped", detail: "alive" });
  });

  it("reports a failed send", async () => {
    expect(await run({ me: ev.owner, account: character(false, false), sendFails: true }).result).toMatchObject({
      status: "failed",
      detail: "AlreadyFrozen",
    });
  });
});
