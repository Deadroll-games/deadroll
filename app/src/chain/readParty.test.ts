// readParty request count: 2 calls on the first read, 1 call once the previous read is known.
// The mocked RPC serves real accounts, encoded with the sdk from zero values.
import { beforeEach, describe, expect, it, vi } from "vitest";
import { getAddressDecoder, getBase64Decoder, type Address, type ReadonlyUint8Array } from "@solana/kit";
import { getCharacterDecoder, getCharacterEncoder } from "@deadroll/sdk/character";
import { getMapDecoder, getMapEncoder, getMapLayoutDecoder, getMapLayoutEncoder } from "@deadroll/sdk/map";
import {
  getEscrowDecoder,
  getEscrowEncoder,
  getGameSessionDecoder,
  getGameSessionEncoder,
  getMonsterStateDecoder,
  getMonsterStateEncoder,
  getParticipationDecoder,
  getParticipationEncoder,
} from "@deadroll/sdk/session";

const calls: Address[][] = [];
/** Account data by address, base64. */
const accounts = new Map<string, string>();
vi.mock("../wallet/client", () => ({
  client: {
    rpc: {
      getMultipleAccounts: (addresses: Address[]) => {
        calls.push([...addresses]);
        return { send: async () => ({ value: addresses.map((a) => (accounts.has(a) ? { data: [accounts.get(a)!, "base64"] } : null)) }) };
      },
    },
  },
}));

import { readParty } from "./game";
import { escrowPda, monstersPda, participationPda } from "./pda";

const A = (n: number) => getAddressDecoder().decode(new Uint8Array(32).fill(n)) as Address;
const SESSION = A(1);
const b64 = getBase64Decoder();

type Codec = { encode(v: any): ReadonlyUint8Array };
type Zero = { decode(b: ReadonlyUint8Array): any; fixedSize?: number; maxSize?: number };
/** An account from zero values, edited, as base64. */
function account(encoder: Codec, decoder: Zero, edit: (v: any) => void = () => {}): string {
  const v = decoder.decode(new Uint8Array(decoder.fixedSize ?? decoder.maxSize!));
  edit(v);
  return b64.decode(encoder.encode(v));
}

async function roster(players: Address[]): Promise<void> {
  accounts.clear();
  accounts.set(
    SESSION,
    account(getGameSessionEncoder(), getGameSessionDecoder(), (s) => {
      s.layout = A(3);
      s.core.map = A(2);
      s.core.playerCount = players.length;
      players.forEach((p, i) => (s.core.players[i] = p));
    }),
  );
  accounts.set(await monstersPda(SESSION), account(getMonsterStateEncoder(), getMonsterStateDecoder()));
  accounts.set(await escrowPda(SESSION), account(getEscrowEncoder(), getEscrowDecoder()));
  accounts.set(A(2), account(getMapEncoder(), getMapDecoder()));
  accounts.set(A(3), account(getMapLayoutEncoder(), getMapLayoutDecoder()));
  for (const p of players) {
    accounts.set(await participationPda(SESSION, p), account(getParticipationEncoder(), getParticipationDecoder()));
    accounts.set(p, account(getCharacterEncoder(), getCharacterDecoder() as Zero));
  }
}

beforeEach(async () => {
  calls.length = 0;
  await roster([A(10), A(11)]);
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
    await roster([A(10), A(11), A(12)]);
    const next = (await readParty(SESSION, first.tiles, first))!;
    expect(calls).toHaveLength(2);
    expect(next.players).toHaveLength(3);
  });
});
