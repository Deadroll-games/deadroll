// @vitest-environment node
import { createHash } from "node:crypto";
import { describe, expect, it } from "vitest";
import { AccountRole, getAddressDecoder, type Address } from "@solana/kit";
import { concat, toHex, u16le, u32le } from "./bytes";
import { ACCOUNT_DISC, EVENT_DISC, IX_DISC } from "./disc";
import { encodeBlob, encodeGameConfig } from "./fixtures";
import { decodeBlob, decodeBlobHeader, decodeCharacter, decodeCharacterDied, decodeGameConfig } from "./layouts";
import { gameConfigPda, rasterizerPda } from "./pda";
import { eventsFromLogs } from "./events";
import { freezeTombstoneInstruction } from "./character";
import { MPL_CORE_PROGRAM, PROGRAMS, SYSTEM_PROGRAM } from "../config";

const disc = (s: string) => createHash("sha256").update(s).digest("hex").slice(0, 16);
/** 32 bytes all equal to n, and the address they encode. */
const A = (n: number) => new Uint8Array(32).fill(n);
const addrOf = (n: number): Address => getAddressDecoder().decode(A(n));
const b64 = (b: Uint8Array) => Buffer.from(b).toString("base64");

describe("discriminators", () => {
  it("match sha256 of the Anchor names", () => {
    for (const [k, v] of Object.entries(ACCOUNT_DISC)) expect(toHex(v), k).toBe(disc(`account:${k}`));
    for (const [k, v] of Object.entries(IX_DISC)) expect(toHex(v), k).toBe(disc(`global:${k}`));
    for (const [k, v] of Object.entries(EVENT_DISC)) expect(toHex(v), k).toBe(disc(`event:${k}`));
  });
});

describe("GameConfig", () => {
  it("PDA matches tools/scripts", async () => {
    expect(await gameConfigPda()).toBe("HYGYnuyssMfrHr2gT3AC3hkQpD6xt37a5hrCc2Z97JHN");
  });

  it("decodes the fixed offsets of GameConfigData", async () => {
    const data = encodeGameConfig({
      authority: addrOf(1),
      programs: Object.values(PROGRAMS),
      rasterizerVersion: 0x0203,
      rasterizer: await rasterizerPda(0x0203),
      rasterizerSha256: A(0xab),
      palette: addrOf(2),
      uriBase: "https://deadroll.example/",
    });
    expect(data.length).toBe(8 + 420);
    // Absolute offsets: rasterizer_version at 265, rasterizer_sha256 at 299.
    expect([data[265], data[266], data[299]]).toEqual([0x03, 0x02, 0xab]);
    const c = decodeGameConfig(data);
    expect(c.version).toBe(1);
    expect(c.authority).toBe(addrOf(1));
    expect(c.programs[5]).toBe(PROGRAMS.session);
    expect(c.rasterizerVersion).toBe(0x0203);
    expect(c.rasterizer).toBe(await rasterizerPda(0x0203));
    expect(toHex(c.rasterizerSha256)).toBe("ab".repeat(32));
    expect(c.palette).toBe(addrOf(2));
    expect(c.uriBase).toBe("https://deadroll.example/");
    expect(c.bump).toBe(255);
  });

  it("rejects a wrong discriminator and short data", () => {
    expect(() => decodeGameConfig(new Uint8Array(428))).toThrow(/not a GameConfig/);
    expect(() => decodeGameConfig(concat(ACCOUNT_DISC.GameConfig, new Uint8Array(10)))).toThrow(/too short/);
  });
});

describe("render blobs", () => {
  it("reads the documented header offsets (same vector as tools/scripts)", () => {
    const data = new Uint8Array(116);
    data.fill(0xab, 0, 8);
    data.set([1, 3, 1, 255, 0x04, 0x03, 0x02, 0x01, 0x06, 0x05, 0x7e, 0x7e], 8);
    data.fill(0x11, 20, 52);
    data.fill(0x22, 52, 84);
    data.fill(0x33, 84, 116);
    const h = decodeBlobHeader(data);
    expect(h).toMatchObject({ version: 1, kind: 3, finalized: true, bump: 255, len: 0x01020304, id: 0x0506 });
    expect(h.authority).toBe(addrOf(0x11));
    expect(h.expectedSha256).toEqual(A(0x22));
    expect(h.sha256).toEqual(A(0x33));
    expect(() => decodeBlobHeader(new Uint8Array(115))).toThrow(/header needs 116/);
  });

  it("decodeBlob checks kind, id, finalized and length", () => {
    const body = new Uint8Array([1, 2, 3]);
    const sha = A(7);
    expect(decodeBlob(encodeBlob("palette", 4, body, sha), "palette", 4).body).toEqual(body);
    expect(() => decodeBlob(encodeBlob("palette", 4, body, sha), "sprite_book", 4)).toThrow(/not a sprite_book/);
    expect(() => decodeBlob(encodeBlob("palette", 4, body, sha), "palette", 5)).toThrow(/id 4/);
    expect(() => decodeBlob(encodeBlob("palette", 4, body, sha, { finalized: false }), "palette", 4)).toThrow(/not finalized/);
    expect(() => decodeBlob(concat(encodeBlob("palette", 4, body, sha), new Uint8Array(1)), "palette", 4)).toThrow(/header says/);
  });
});

function characterBytes(opts: { nft: boolean; alive: boolean; frozen: boolean }): Uint8Array {
  const layer = concat(u16le(101), new Uint8Array([2, 3, 0]));
  return concat(
    ACCOUNT_DISC.Character,
    new Uint8Array([1]),
    A(9),
    new Uint8Array([0]),
    opts.nft ? concat(new Uint8Array([1]), A(8)) : new Uint8Array([0]),
    new Uint8Array([1, 1]), // ForestElf, Mage
    u16le(3),
    u32le(1234),
    concat(u16le(5), u16le(6), u16le(9), u16le(7)),
    new Uint8Array([1, 9]), // Cold, tier 9
    u32le(40),
    new Uint8Array([opts.alive ? 1 : 0]),
    new Uint8Array([0]), // in_session None
    ...Array.from({ length: 20 }, () => layer),
    u32le(42),
    new Uint8Array([253]),
    concat(new TextEncoder().encode("Aria"), new Uint8Array(28)),
    new Uint8Array([2, opts.frozen ? 1 : 0]),
  );
}

describe("Character", () => {
  it("decodes core and lane fields with and without nft", () => {
    const c = decodeCharacter(characterBytes({ nft: true, alive: false, frozen: true }));
    expect(c).toMatchObject({ owner: addrOf(9), race: 1, class: 1, level: 3, xp: 1234, stats: [5, 6, 9, 7] });
    expect(c).toMatchObject({ predisposition: 1, predispositionTier: 9, hpMax: 40, alive: false, inSession: null });
    expect(c).toMatchObject({ stateVersion: 42, bump: 253, name: "Aria", unspentPoints: 2, nftFrozen: true });
    expect(c.nft).toBe(addrOf(8));
    expect(c.appearance[19]).toEqual({ spriteId: 101, materialRamp: 2, accentRamp: 3, elementRamp: 0 });
    const n = decodeCharacter(characterBytes({ nft: false, alive: true, frozen: false }));
    expect(n.nft).toBeNull();
    expect(n.name).toBe("Aria");
    expect(n.nftFrozen).toBe(false);
  });
});

function diedEvent(nft: boolean): Uint8Array {
  return concat(EVENT_DISC.CharacterDied, A(1), A(2), A(3), nft ? concat(new Uint8Array([1]), A(4)) : new Uint8Array([0]), u32le(7));
}

describe("events", () => {
  it("decodes CharacterDied", () => {
    expect(decodeCharacterDied(diedEvent(true))).toEqual({
      character: addrOf(1),
      owner: addrOf(2),
      session: addrOf(3),
      nft: addrOf(4),
      stateVersion: 7,
    });
    expect(decodeCharacterDied(diedEvent(false))?.nft).toBeNull();
    expect(decodeCharacterDied(concat(EVENT_DISC.TombstoneFrozen, A(1), A(2)))).toBeNull();
  });

  it("attributes Program data to the innermost program", () => {
    const logs = [
      `Program ${PROGRAMS.session} invoke [1]`,
      "Program data: " + b64(new Uint8Array([1, 2, 3])),
      `Program ${PROGRAMS.character} invoke [2]`,
      "Program log: Instruction: MarkDead",
      "Program data: " + b64(diedEvent(true)),
      `Program ${PROGRAMS.character} consumed 5000 of 200000 compute units`,
      `Program ${PROGRAMS.character} success`,
      "Program data: " + b64(new Uint8Array([9])),
      `Program ${PROGRAMS.session} success`,
    ];
    const ch = eventsFromLogs(logs, PROGRAMS.character);
    expect(ch).toHaveLength(1);
    expect(decodeCharacterDied(ch[0]!)?.stateVersion).toBe(7);
    expect(eventsFromLogs(logs, PROGRAMS.session)).toEqual([new Uint8Array([1, 2, 3]), new Uint8Array([9])]);
  });
});

describe("freeze_tombstone", () => {
  it("has the discriminator only and the account order of the program", () => {
    const payer = { address: addrOf(5), signTransactions: async () => [] } as never;
    const ix = freezeTombstoneInstruction({ character: addrOf(1), asset: addrOf(4), payer });
    expect(ix.programAddress).toBe(PROGRAMS.character);
    expect(toHex(ix.data as Uint8Array)).toBe("9f253aa1ccda438d");
    expect(ix.accounts?.map((a) => [a.address, a.role])).toEqual([
      [addrOf(1), AccountRole.WRITABLE],
      [addrOf(4), AccountRole.WRITABLE],
      [addrOf(5), AccountRole.WRITABLE_SIGNER],
      [MPL_CORE_PROGRAM, AccountRole.READONLY],
      [SYSTEM_PROGRAM, AccountRole.READONLY],
    ]);
  });
});
