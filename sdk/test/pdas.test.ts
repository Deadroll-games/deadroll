// Every generated PDA against the seeds in agents/CONTRACTS.md 2, derived by hand. Catches seeds
// the Anchor IDL describes lossily (see FIXES in scripts/generate.ts).
import assert from "node:assert/strict";
import { test } from "node:test";
import { generateKeyPairSigner, getAddressEncoder, getProgramDerivedAddress, type Address } from "@solana/kit";
import { PROGRAMS } from "../src/index.ts";
import * as character from "../src/character.ts";
import * as map from "../src/map.ts";
import * as registry from "../src/registry.ts";
import * as render from "../src/render.ts";
import * as session from "../src/session.ts";

const enc = getAddressEncoder();
const utf8 = (s: string) => new TextEncoder().encode(s);
const u16 = (v: number) => new Uint8Array([v & 0xff, v >> 8]);
const u32 = (v: number) => new Uint8Array([v & 0xff, (v >> 8) & 0xff, (v >> 16) & 0xff, v >>> 24]);
const key = (a: Address) => enc.encode(a) as Uint8Array;

async function want(program: Address, seeds: Uint8Array[]): Promise<Address> {
  return (await getProgramDerivedAddress({ programAddress: program, seeds }))[0];
}
async function same(got: Promise<readonly [Address, number]>, program: Address, seeds: (Uint8Array | string)[]) {
  assert.equal((await got)[0], await want(program, seeds.map((s) => (typeof s === "string" ? utf8(s) : s))));
}

test("generated PDAs follow CONTRACTS.md 2", async () => {
  const [a, b] = await Promise.all([generateKeyPairSigner(), generateKeyPairSigner()]).then((k) => k.map((s) => s.address));
  await same(registry.findConfigPda(), PROGRAMS.registry, ["config"]);
  for (const t of ["aspects", "resonances", "bestiary", "tileset", "prefabs", "traits", "monster_skills"]) {
    await same(registry.findTablePda({ seed: t }), PROGRAMS.registry, [t]);
  }
  await same(render.findRasterizerPda({ version: 258 }), PROGRAMS.render, ["rasterizer", u16(258)]);
  await same(render.findPalettePda({ version: 2 }), PROGRAMS.render, ["palette", u16(2)]);
  await same(render.findSpriteBookPda({ bookId: 513 }), PROGRAMS.render, ["sprites", u16(513)]);
  await same(render.findRenderAuthorityPda(), PROGRAMS.render, ["authority"]);
  await same(character.findProfilePda({ owner: a }), PROGRAMS.character, ["profile", key(a)]);
  await same(character.findSkillSetPda({ character: a }), PROGRAMS.character, ["skills", key(a)]);
  await same(character.findBagPda({ character: a }), PROGRAMS.character, ["bag", key(a)]);
  await same(character.findSkillExtrasPda({ character: a }), PROGRAMS.character, ["skill_extras", key(a)]);
  await same(map.findMapPda({ dm: a, index: 70_000 }), PROGRAMS.map, ["map", key(a), u32(70_000)]);
  await same(map.findLayoutPda({ map: a }), PROGRAMS.map, ["layout", key(a)]);
  await same(session.findSessionPda({ map: a }), PROGRAMS.session, ["session", key(a)]);
  await same(session.findMonstersPda({ session: a }), PROGRAMS.session, ["monsters", key(a)]);
  await same(session.findEscrowPda({ session: a }), PROGRAMS.session, ["escrow", key(a)]);
  await same(session.findParticipationPda({ session: a, character: b }), PROGRAMS.session, ["participation", key(a), key(b)]);
  // Patch 11: the reward cache replaces the per-party stash.
  await same(session.findRewardCachePda({ character: b }), PROGRAMS.session, ["cache", key(b)]);
  await same(session.findSessionAuthorityPda(), PROGRAMS.session, ["authority"]);
  await same(session.findProgramIdentityPda(), PROGRAMS.session, ["identity"]);
});
