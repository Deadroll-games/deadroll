// @vitest-environment node
// Level-up stat points: the spend_stat_points instruction bytes, who may spend them, and the
// skill record encoding the combat module reads.
import { createHash } from "node:crypto";
import { describe, expect, it } from "vitest";
import { AccountRole, generateKeyPairSigner, getAddressDecoder, type Address } from "@solana/kit";
import { PROGRAMS } from "../config";
import { spendLock } from "../routes/CharacterDetail";
import { concat, toHex, u16le } from "./bytes";
import fixtures from "./__fixtures__/combat-wasm.json";
import { skillInputOf } from "./combatWasm";
import { spendStatPointsIx, type CharacterAcc, type SkillRecordAcc } from "./game";

const addrOf = (n: number): Address => getAddressDecoder().decode(new Uint8Array(32).fill(n));
const ixDisc = (name: string) => createHash("sha256").update(`global:${name}`).digest().subarray(0, 8);
const e = (name: string) => ({ [name]: {} });

describe("spend_stat_points", () => {
  it("character program, points as four u16 in Stat order; owner (signer), character (w)", async () => {
    const owner = await generateKeyPairSigner();
    const ix = spendStatPointsIx(owner, addrOf(8), [1, 0, 2, 300]);
    expect(ix.programAddress).toBe(PROGRAMS.character);
    expect(toHex(ix.data as Uint8Array)).toBe(toHex(concat(ixDisc("spend_stat_points"), u16le(1), u16le(0), u16le(2), u16le(300))));
    expect(ix.accounts!.map((a) => [a.address, a.role])).toEqual([
      [owner.address, AccountRole.READONLY_SIGNER],
      [addrOf(8), AccountRole.WRITABLE],
    ]);
  });

  it("only the owner, a living character, outside a party", () => {
    const owner = addrOf(1);
    const c = { core: { owner, alive: true, inSession: null } } as unknown as CharacterAcc;
    expect(spendLock(c, owner)).toBeNull();
    expect(spendLock(c, addrOf(2))).toMatch(/owner/);
    expect(spendLock(c, null)).toMatch(/owner/);
    expect(spendLock({ core: { ...c.core, inSession: addrOf(3) } } as unknown as CharacterAcc, owner)).toMatch(/party/);
    expect(spendLock({ core: { ...c.core, alive: false } } as unknown as CharacterAcc, owner)).toMatch(/tombstone/i);
  });
});

describe("skillInputOf", () => {
  it("writes a SkillRecord's blocks as the fixture's discriminants", () => {
    // Bleeding Cleave: active damage, martial strike, line, Blood + Piercing.
    const k: SkillRecordAcc = {
      blocks: {
        category: e("active"),
        purpose: e("damage"),
        source: e("combat"),
        form: e("strike"),
        target: e("line"),
        aspects: [e("blood"), e("piercing"), e("fire")],
        aspectCount: 2,
        effect: e("none"),
        trigger: e("none"),
        auraRadius: 0,
      },
      sharesBp: [6_500, 3_500, 0],
      powerBp: 6_666,
      fusion: 0,
      cooldown: 2,
      name: new Uint8Array(32),
    };
    const s = skillInputOf(k);
    expect(s.blocks).toEqual([0, 0, 1, 0, 1, 3, 5, 0, 2, 0, 0, 0]);
    expect([s.sharesBp, s.powerBp]).toEqual([[6_500, 3_500, 0], 6_666]);
    // Same encoding as the Rust fixtures: some fixture skill uses these exact blocks.
    const blocks = fixtures.attacks.map((a) => (a.input.skill as { blocks: number[] } | null)?.blocks).filter(Boolean);
    expect(blocks).toContainEqual([0, 0, 1, 0, 1, 3, 5, 0, 2, 0, 0, 0]);
  });
});
