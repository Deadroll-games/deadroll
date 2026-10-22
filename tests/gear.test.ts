// The character's gear against the shared local validator (docs/CHARACTER.md 9-10,
// ITEMS_AND_CRAFT.md 10): the class starter items that create_character gives, equip and unequip
// by the owner or the game's session key (deadroll_character).
//
// Needs the character program and the registry GameConfig (tools/scripts deploy:local). The
// in-session lock (AlreadyLocked) and the dead character (CharacterDead) need a party: they run
// in programs/session/tests/session.test.ts. LevelTooLow cannot happen with the level 1 starter items; the rule is
// covered by the unit tests (programs/character/src/state/gear.rs, crates/rules/tests/items.rs).
import { describe, it } from "node:test";
import assert from "node:assert/strict";
import { createHash } from "node:crypto";
import { existsSync, readFileSync } from "node:fs";
import anchor from "@anchor-lang/core";
import { extOf } from "./skill_ext.ts";
import {
  Connection,
  Keypair,
  LAMPORTS_PER_SOL,
  PublicKey,
  SystemProgram,
  Transaction,
  TransactionInstruction,
} from "@solana/web3.js";
import { MPL_CORE_PROGRAM, PROGRAMS, SESSION_KEYS_PROGRAM } from "@deadroll/sdk";

const { AnchorProvider, BN, EventParser, Program } = anchor;

const env = AnchorProvider.env();
const provider = new AnchorProvider(new Connection(env.connection.rpcEndpoint, "confirmed"), env.wallet, {
  commitment: "confirmed",
  preflightCommitment: "confirmed",
});
const connection = provider.connection;

const ITEMS = new PublicKey(PROGRAMS.items);
const CHARACTER = new PublicKey(PROGRAMS.character);
const SESSION = new PublicKey(PROGRAMS.session);
const REGISTRY = new PublicKey(PROGRAMS.registry);
const SESSION_KEYS = new PublicKey(SESSION_KEYS_PROGRAM);
const MPL_CORE = new PublicKey(MPL_CORE_PROGRAM);

const CHARACTER_IDL = new URL("../target/idl/deadroll_character.json", import.meta.url);

const pda = (seeds: (Buffer | Uint8Array)[], programId: PublicKey) =>
  PublicKey.findProgramAddressSync(seeds, programId)[0];
const sha256 = (data: Buffer | string) => createHash("sha256").update(data).digest();
const config = pda([Buffer.from("config")], REGISTRY);

async function isProgram(id: PublicKey): Promise<boolean> {
  return (await connection.getAccountInfo(id))?.executable ?? false;
}

const missing: string[] = [];
if (!existsSync(CHARACTER_IDL)) missing.push("target/idl/deadroll_character.json");
if (!(await isProgram(CHARACTER))) missing.push("deadroll_character deployed");
if (!(await isProgram(SESSION_KEYS))) missing.push("session-keys program");
if ((await connection.getAccountInfo(config))?.owner.equals(REGISTRY) !== true) missing.push("registry GameConfig");
const SKIP = missing.length ? `run tools/scripts deploy:local first; missing: ${missing.join(", ")}` : false;

const character: any = SKIP ? null : new Program(JSON.parse(readFileSync(CHARACTER_IDL, "utf8")), provider);
const characterEvents = SKIP ? null : new EventParser(CHARACTER, character.coder);

// ---------------------------------------------------------------------------------------------
// helpers

const e = (name: string) => ({ [name]: {} });
const text = (s: string, len: number) => {
  const out = new Array(len).fill(0);
  Buffer.from(s, "ascii").forEach((b, i) => (out[i] = b));
  return out;
};
const variant = (v: any) => Object.keys(v)[0];

async function funded(sol = 5): Promise<Keypair> {
  const kp = Keypair.generate();
  const sig = await connection.requestAirdrop(kp.publicKey, sol * LAMPORTS_PER_SOL);
  const latest = await connection.getLatestBlockhash();
  await connection.confirmTransaction({ signature: sig, ...latest }, "confirmed");
  return kp;
}

async function rejects(promise: Promise<unknown>, code: string): Promise<void> {
  try {
    await promise;
  } catch (err: any) {
    const name = err?.error?.errorCode?.code;
    const logs: string[] = err?.logs ?? err?.transactionLogs ?? [];
    const msg = `${err?.message ?? err}\n${logs.join("\n")}`;
    assert.ok(name === code || msg.includes(code), `expected ${code}, got ${name ?? msg}`);
    return;
  }
  assert.fail(`expected ${code}, transaction succeeded`);
}

async function eventsOf(sig: string) {
  const tx = await connection.getTransaction(sig, { commitment: "confirmed", maxSupportedTransactionVersion: 0 });
  return [...characterEvents!.parseLogs(tx?.meta?.logMessages ?? [])];
}
async function eventOf(sig: string, name: string) {
  const found = (await eventsOf(sig)).find((x) => x.name === name);
  assert.ok(found, `no ${name} event`);
  return found.data;
}

/** One active skill per class from `deadroll_skillcheck::presets` (enough to create). */
function skill(source: string, aspect: string, purpose = "damage", effect = "none", form = "strike") {
  return {
    category: e("active"),
    purpose: e(purpose),
    source: e(source),
    form: e(form),
    target: e("single"),
    aspects: [aspect, "fire", "fire"].map(e),
    aspectCount: 1,
    effect: e(effect),
    trigger: e("none"),
    auraRadius: 0,
  };
}
type Kind = "warrior" | "mage" | "support";
const PROFILES = {
  warrior: { race: "human", predisposition: "piercing", stats: [10, 7, 5, 8], skill: skill("combat", "piercing") },
  mage: { race: "forestElf", predisposition: "cold", stats: [5, 7, 13, 5], skill: skill("magical", "cold") },
  support: { race: "forestElf", predisposition: "lightning", stats: [5, 8, 12, 5], skill: skill("magical", "lightning", "buff", "haste", "projectile") }, // Quicken
};

/** A fresh character of `kind`; its class starter items come with it. */
async function newCharacter(kind: Kind): Promise<{ owner: Keypair; character: PublicKey; sig: string }> {
  const owner = await funded();
  const profile = pda([Buffer.from("profile"), owner.publicKey.toBuffer()], CHARACTER);
  if (!(await connection.getAccountInfo(profile))) {
    await character.methods
      .createProfile()
      .accountsStrict({ owner: owner.publicKey, profile, systemProgram: SystemProgram.programId })
      .signers([owner])
      .rpc();
  }
  const index = (await character.account.playerProfile.fetch(profile)).characterCount;
  const c = pda([Buffer.from("character"), owner.publicKey.toBuffer(), Buffer.from([index])], CHARACTER);
  const p = PROFILES[kind];
  const asset = Keypair.generate();
  const sig = await character.methods
    .createCharacter({
      name: text("Kit", 32),
      race: e(p.race),
      class: e(kind),
      predisposition: e(p.predisposition),
      stats: p.stats,
      appearance: new Array(20).fill({ spriteId: 0, materialRamp: 0, accentRamp: 0, elementRamp: 0 }),
      skills: [{ blocks: p.skill, name: text("Strike", 24), ext: extOf("Strike") }],
    })
    .accountsStrict({
      owner: owner.publicKey,
      profile,
      character: c,
      skillSet: pda([Buffer.from("skills"), c.toBuffer()], CHARACTER),
      bag: pda([Buffer.from("bag"), c.toBuffer()], CHARACTER),
      config,
      asset: asset.publicKey,
      mplCoreProgram: MPL_CORE,
      systemProgram: SystemProgram.programId,
      skillExtras: pda([Buffer.from("skill_extras"), c.toBuffer()], CHARACTER),
    })
    .signers([owner, asset])
    .rpc();
  return { owner, character: c, sig };
}

function equip(signer: Keypair, c: PublicKey, index: number, slot: string, token: PublicKey | null = null) {
  return character.methods
    .equip(index, e(slot))
    .accountsStrict({ signer: signer.publicKey, character: c, sessionToken: token })
    .signers([signer])
    .rpc();
}

function unequip(signer: Keypair, c: PublicKey, slot: string, token: PublicKey | null = null) {
  return character.methods
    .unequip(e(slot))
    .accountsStrict({ signer: signer.publicKey, character: c, sessionToken: token })
    .signers([signer])
    .rpc();
}

const fetchCharacter = (c: PublicKey) => character.account.character.fetch(c);
/** Slot name -> worn base name, from a fetched character. */
function wornOf(ch: any): Record<string, string> {
  const slots = character.idl.types.find((t: any) => t.name === "equipSlot").type.variants.map((v: any) => v.name);
  const out: Record<string, string> = {};
  ch.gear.equipped.forEach((r: any, i: number) => {
    if (r.source.soulbound !== undefined) out[slots[i]] = variant(ch.gear.items[r.index].base);
  });
  return out;
}

/** MagicBlock Session Keys `create_session_v2`, signed by the session key and the main wallet. */
async function issueToken(
  signer: Keypair,
  authority: Keypair,
  opts: { target?: PublicKey; validUntil?: number } = {},
): Promise<PublicKey> {
  const target = opts.target ?? SESSION;
  const token = pda(
    [Buffer.from("session_token_v2"), target.toBuffer(), signer.publicKey.toBuffer(), authority.publicKey.toBuffer()],
    SESSION_KEYS,
  );
  const validUntil =
    opts.validUntil === undefined
      ? Buffer.from([0])
      : Buffer.concat([Buffer.from([1]), new BN(opts.validUntil).toArrayLike(Buffer, "le", 8)]);
  const ix = new TransactionInstruction({
    programId: SESSION_KEYS,
    keys: [
      { pubkey: token, isSigner: false, isWritable: true },
      { pubkey: signer.publicKey, isSigner: true, isWritable: true },
      { pubkey: authority.publicKey, isSigner: true, isWritable: true },
      { pubkey: authority.publicKey, isSigner: true, isWritable: false },
      { pubkey: target, isSigner: false, isWritable: false },
      { pubkey: SystemProgram.programId, isSigner: false, isWritable: false },
    ],
    data: Buffer.concat([sha256("global:create_session_v2").subarray(0, 8), Buffer.from([0]), validUntil, Buffer.from([0])]),
  });
  await provider.sendAndConfirm(new Transaction().add(ix), [signer, authority]);
  return token;
}

// ---------------------------------------------------------------------------------------------
// tests

describe("create_character: the class starter items", { skip: SKIP }, () => {
  for (const [kind, kit] of [
    ["warrior", { mainHand: "sword", body: "leatherArmor" }],
    ["mage", { mainHand: "staff", body: "robe" }],
    ["support", { mainHand: "wand", body: "lightLeatherArmor" }],
  ] as const) {
    it(`a ${kind} gets its class starter kit, worn, in the character account`, async () => {
      const { character: c, sig } = await newCharacter(kind);
      const ev = await eventOf(sig, "characterCreated");
      assert.equal(variant(ev.class), kind);
      assert.equal(ev.itemCount, 2);
      const ch = await fetchCharacter(c);
      assert.equal(ch.core.version, 3); // 3: gear, then the class trait tier
      assert.equal(ch.gear.itemCount, 2);
      assert.deepEqual(ch.gear.items.slice(0, 2).map((r: any) => [variant(r.base), r.level]), [
        [kit.mainHand, 1],
        [kit.body, 1],
      ]);
      assert.deepEqual(wornOf(ch), kit);
    });
  }
});

describe("equip and unequip", { skip: SKIP }, () => {
  it("the owner takes items off and puts them back on", async () => {
    const { owner, character: c } = await newCharacter("warrior");

    const off = await eventOf(await unequip(owner, c, "body"), "itemUnequipped");
    assert.deepEqual([variant(off.slot), variant(off.base)], ["body", "leatherArmor"]);
    assert.deepEqual(wornOf(await fetchCharacter(c)), { mainHand: "sword" });

    const on = await eventOf(await equip(owner, c, 1, "body"), "itemEquipped");
    assert.deepEqual([variant(on.slot), on.itemIndex, variant(on.base), on.replaced], ["body", 1, "leatherArmor", null]);
    assert.deepEqual(wornOf(await fetchCharacter(c)), { mainHand: "sword", body: "leatherArmor" });

    // Wearing what is already worn reports it as replaced.
    const again = await eventOf(await equip(owner, c, 0, "mainHand"), "itemEquipped");
    assert.equal(variant(again.replaced), "sword");
  });

  it("every equip and unequip is a new state version of the NFT image; a rejected one is not", async () => {
    const { owner, character: c } = await newCharacter("warrior");
    const version = async () => (await fetchCharacter(c)).core.stateVersion;
    const v0 = await version();
    await unequip(owner, c, "body");
    assert.equal(await version(), v0 + 1);
    await equip(owner, c, 1, "body");
    assert.equal(await version(), v0 + 2);
    await rejects(unequip(owner, c, "helmet"), "SlotEmpty");
    await rejects(equip(owner, c, 0, "body"), "WrongSlot");
    assert.equal(await version(), v0 + 2);
  });

  it("rejects a slot the item is not made for", async () => {
    const { owner, character: c } = await newCharacter("mage");
    await rejects(equip(owner, c, 0, "body"), "WrongSlot");
    await rejects(equip(owner, c, 1, "mainHand"), "WrongSlot");
  });

  it("rejects an item the character does not have", async () => {
    const { owner, character: c } = await newCharacter("support");
    await rejects(equip(owner, c, 2, "ring1"), "NoSuchItem");
    await rejects(equip(owner, c, 255, "mainHand"), "NoSuchItem");
  });

  it("rejects taking off what is not worn", async () => {
    const { owner, character: c } = await newCharacter("warrior");
    await rejects(unequip(owner, c, "helmet"), "SlotEmpty");
    await unequip(owner, c, "mainHand");
    await rejects(unequip(owner, c, "mainHand"), "SlotEmpty");
  });

  it("rejects an account that is not a character", async () => {
    const owner = await funded(1);
    const profile = pda([Buffer.from("profile"), owner.publicKey.toBuffer()], CHARACTER);
    await character.methods
      .createProfile()
      .accountsStrict({ owner: owner.publicKey, profile, systemProgram: SystemProgram.programId })
      .signers([owner])
      .rpc();
    await rejects(equip(owner, profile, 0, "mainHand"), "AccountDiscriminatorMismatch");
  });
});

describe("equip and unequip: the session key", { skip: SKIP }, () => {
  it("equips with the owner's game token (issued for the session program)", async () => {
    const { owner, character: c } = await newCharacter("warrior");
    const key = await funded(1);
    const token = await issueToken(key, owner);
    await unequip(key, c, "body", token);
    await equip(key, c, 1, "body", token);
    assert.deepEqual(wornOf(await fetchCharacter(c)), { mainHand: "sword", body: "leatherArmor" });
  });

  it("rejects anyone else without a token", async () => {
    const { character: c } = await newCharacter("warrior");
    await rejects(unequip(await funded(1), c, "body"), "Unauthorized");
  });

  it("rejects a token of another wallet, for another program, or for another key", async () => {
    const { owner, character: c } = await newCharacter("warrior");
    const key = await funded(1);
    const stranger = await funded(1);
    await rejects(unequip(key, c, "body", await issueToken(key, stranger)), "InvalidToken");
    await rejects(unequip(key, c, "body", await issueToken(key, owner, { target: ITEMS })), "InvalidToken");
    const other = await funded(1);
    await rejects(unequip(key, c, "body", await issueToken(other, owner)), "InvalidToken");
    // An account that is not a session token at all (Anchor checks its owner).
    await rejects(unequip(key, c, "body", c), "AccountOwnedByWrongProgram");
  });

  it("rejects an expired token", async () => {
    const { owner, character: c } = await newCharacter("warrior");
    const key = await funded(1);
    const now = (await connection.getBlockTime(await connection.getSlot())) ?? Math.floor(Date.now() / 1000);
    const token = await issueToken(key, owner, { validUntil: now - 60 });
    await rejects(unequip(key, c, "body", token), "InvalidToken");
  });
});
