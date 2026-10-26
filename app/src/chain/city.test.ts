// @vitest-environment node
// Byte-level tests of the city builders and reads (chain/city.ts): Anchor's discriminator rule,
// account lists in the order of the program's Accounts structs, hand-built account bytes.
import { createHash } from "node:crypto";
import { describe, expect, it } from "vitest";
import { AccountRole, generateKeyPairSigner, getAddressEncoder, getProgramDerivedAddress, type Address } from "@solana/kit";
import { PROGRAMS, SYSTEM_PROGRAM } from "../config";
import { boardPda, buildingPda, cityPda, closeQuestIx, postQuestIx, questPda, questState, type QuestView } from "./city";
import { decodeAccount, sessionAccounts } from "./game";
import { gameConfigPda } from "./pda";
import { mockCity } from "../game/city/model";

const sha = (s: string) => createHash("sha256").update(s).digest().subarray(0, 8);
const enc = getAddressEncoder();
const pda = async (seeds: (string | Uint8Array)[]) => (await getProgramDerivedAddress({ programAddress: PROGRAMS.city, seeds })).at(0) as Address;

describe("city PDAs (agents/CONTRACTS.md 25)", () => {
  it("follow the seeds", async () => {
    expect(await cityPda()).toBe(await pda(["city"]));
    expect(await buildingPda(258)).toBe(await pda(["building", new Uint8Array([2, 1])]));
    const b = await buildingPda(0);
    expect(await boardPda(b)).toBe(await pda(["tavern_board", Uint8Array.from(enc.encode(b))]));
    const s = (await generateKeyPairSigner()).address;
    expect(await questPda(s)).toBe(await pda(["quest", Uint8Array.from(enc.encode(s))]));
  });
});

describe("post_quest and close_quest", () => {
  it("post_quest: the DM's accounts in Accounts order, no args", async () => {
    const dm = await generateKeyPairSigner();
    const building = await buildingPda(0);
    const map = (await generateKeyPairSigner()).address;
    const ix = await postQuestIx(dm, building, map);
    const { session } = await sessionAccounts(map);
    expect(ix.programAddress).toBe(PROGRAMS.city);
    expect(Buffer.from(ix.data!)).toEqual(sha("global:post_quest"));
    expect(ix.accounts!.map((a) => [a.address, a.role])).toEqual([
      [dm.address, AccountRole.WRITABLE_SIGNER],
      [await gameConfigPda(), AccountRole.READONLY],
      [building, AccountRole.READONLY],
      [await boardPda(building), AccountRole.WRITABLE],
      [map, AccountRole.READONLY],
      [session, AccountRole.READONLY],
      [await questPda(session), AccountRole.WRITABLE],
      [SYSTEM_PROGRAM, AccountRole.READONLY],
    ]);
  });

  it("close_quest: the DM gets the rent back", async () => {
    const dm = await generateKeyPairSigner();
    const board = await boardPda(await buildingPda(0));
    const session = (await generateKeyPairSigner()).address;
    const ix = await closeQuestIx(dm, board, session);
    expect(Buffer.from(ix.data!)).toEqual(sha("global:close_quest"));
    expect(ix.accounts!.map((a) => [a.address, a.role])).toEqual([
      [dm.address, AccountRole.WRITABLE_SIGNER],
      [await gameConfigPda(), AccountRole.READONLY],
      [board, AccountRole.WRITABLE],
      [await questPda(session), AccountRole.WRITABLE],
      [session, AccountRole.READONLY],
    ]);
  });
});

describe("city accounts", () => {
  it("decodes a Quest from its bytes", async () => {
    const keys = await Promise.all([0, 1, 2, 3].map(async () => (await generateKeyPairSigner()).address));
    const bytes = Buffer.concat([
      sha("account:Quest"),
      Buffer.from([1]),
      ...keys.map((k) => Buffer.from(enc.encode(k))),
      Buffer.from([2, 1, 3]), // tier, biome, min_players
      Buffer.from(new BigUint64Array([100_000_000n]).buffer), // price_lamports
      Buffer.from(new BigInt64Array([1_700_000_000n]).buffer), // posted_at
      Buffer.from([254]),
    ]);
    const q = decodeAccount("Quest", bytes).core;
    expect([q.board, q.session, q.map, q.dm]).toEqual(keys);
    expect([q.tier, q.biome, q.minPlayers, q.priceLamports, q.postedAt, q.bump]).toEqual([2, 1, 3, 100_000_000n, 1_700_000_000n, 254]);
    expect(() => decodeAccount("Building", bytes)).toThrow(/not a Building/);
  });

  it("the board shows a party by its session status", () => {
    const view = (status: number | null) => ({ session: status === null ? null : { core: { status } } }) as unknown as QuestView;
    expect([view(0), view(1), view(2), view(5), view(null)].map(questState)).toEqual(["lobby", "active", "ended", "ended", "gone"]);
  });

  it("mock mode draws the seeded city of assets/city/city.json", () => {
    const c = mockCity();
    expect([c.width, c.height, c.buildings.length]).toEqual([16, 11, 3]);
    // The Tavern is open, the Artisans' Guild and the Storage are closed.
    expect(c.buildings.map((b) => [b.kind, b.status, b.prefabId])).toEqual([
      [0, 0, 1],
      [1, 1, 2],
      [2, 1, 3],
    ]);
    // Corner posts of the city wall (sprite 2008).
    expect(c.cells[0]).toBe(8);
  });
});
