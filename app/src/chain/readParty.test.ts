// readParty request count: 2 calls on the first read, 1 call once the previous read is known.
import { beforeEach, describe, expect, it, vi } from "vitest";
import { getAddressDecoder, type Address } from "@solana/kit";

const calls: Address[][] = [];
vi.mock("../wallet/client", () => ({
  client: {
    rpc: {
      getMultipleAccounts: (addresses: Address[]) => {
        calls.push([...addresses]);
        return { send: async () => ({ value: addresses.map(() => ({ data: ["AAAA", "base64"] })) }) };
      },
    },
  },
}));

import { coders } from "./programs";
import { readParty } from "./game";

const A = (n: number) => getAddressDecoder().decode(new Uint8Array(32).fill(n)) as Address;
const SESSION = A(1);
let roster: Address[];

beforeEach(() => {
  calls.length = 0;
  roster = [A(10), A(11)];
  vi.spyOn(coders.session, "decodeAccount").mockImplementation(((name: string) => {
    if (name === "GameSession") return { layout: A(3), core: { players: [...roster, A(99)], playerCount: roster.length, map: A(2) } };
    if (name === "MonsterState") return { core: { monsters: [], count: 0 } };
    return { collected: 0n, claimed: false };
  }) as never);
  vi.spyOn(coders.character, "decodeAccount").mockReturnValue({} as never);
  vi.spyOn(coders.map, "decodeAccount").mockReturnValue({ core: { tiles: new Uint8Array(4) } } as never);
});

describe("readParty", () => {
  it("makes two getMultipleAccounts calls on the first read and one afterwards", async () => {
    const first = (await readParty(SESSION))!;
    expect(calls).toHaveLength(2);
    calls.length = 0;
    const next = (await readParty(SESSION, first.tiles, first))!;
    expect(calls).toHaveLength(1);
    expect(next.players).toHaveLength(2);
  });

  it("falls back to a second call when the roster changed", async () => {
    const first = (await readParty(SESSION))!;
    calls.length = 0;
    roster = [...roster, A(12)];
    const next = (await readParty(SESSION, first.tiles, first))!;
    expect(calls).toHaveLength(2);
    expect(next.players).toHaveLength(3);
  });
});
