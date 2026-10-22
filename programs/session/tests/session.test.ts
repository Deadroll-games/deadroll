// deadroll_session against the shared local validator (agents/TASKS.md, decision block).
//
// Deploy the `mock-vrf` build (`anchor build --tools-version v1.54 -p deadroll_session -- --features
// mock-vrf`): the validator has no VRF oracle, so these tests sign `consume_randomness` with the
// fixed mock identity and choose the VRF value themselves.
//
// Fresh random keypairs everywhere; nothing assumes an empty ledger.
// Part 1 runs on any validator with the session program. Part 2 (the party flow on the map lane's
// demo preset) needs the registry `GameConfig` (trusting session), the character and map programs
// deployed and their IDLs built; it skips itself with the missing piece named otherwise.
import { before, describe, it } from "node:test";
import assert from "node:assert/strict";
import { createHash, randomBytes } from "node:crypto";
import { existsSync, readFileSync } from "node:fs";
// Node loads the CJS build; its getter exports are invisible to ESM named imports.
import anchor from "@anchor-lang/core";
import type { DeadrollSession } from "../../../target/types/deadroll_session.ts";
import { newSkill, partyAccounts } from "../../../tests/skill_ext.ts";
import {
  Connection,
  Keypair,
  LAMPORTS_PER_SOL,
  PublicKey,
  SystemProgram,
  Transaction,
  ComputeBudgetProgram,
  TransactionInstruction,
} from "@solana/web3.js";
import { MPL_CORE_PROGRAM, PROGRAMS, SESSION_KEYS_PROGRAM, VRF } from "@deadroll/sdk";

const { AnchorProvider, BN, EventParser, Program } = anchor;

/** Fixed VRF values: the n-th value any test draws is sha256("deadroll:session-test:n"), so
 * every run plays the same dice (no random seeds). */
let seedCounter = 0;
function nextSeed(): Buffer {
  return createHash("sha256").update(`deadroll:session-test:${seedCounter++}`).digest();
}
type BN = InstanceType<typeof BN>;

const idl = JSON.parse(
  readFileSync(new URL("../../../target/idl/deadroll_session.json", import.meta.url), "utf8"),
) as DeadrollSession;

// "confirmed" throughout: with the default "processed" the next simulation can miss a fresh
// blockhash ("Blockhash not found") on the shared validator.
const env = AnchorProvider.env();
const provider = new AnchorProvider(new Connection(env.connection.rpcEndpoint, "confirmed"), env.wallet, {
  commitment: "confirmed",
  preflightCommitment: "confirmed",
});
const connection = provider.connection;
const program = new Program<DeadrollSession>(idl, provider);
const SESSION = program.programId;
const CHARACTER = new PublicKey(PROGRAMS.character);
const MAP = new PublicKey(PROGRAMS.map);
const REGISTRY = new PublicKey(PROGRAMS.registry);
const SESSION_KEYS = new PublicKey(SESSION_KEYS_PROGRAM);
const MPL_CORE = new PublicKey(MPL_CORE_PROGRAM);
// MagicBlock VRF (ephemeral-vrf-sdk 0.17.3 consts); none of them has to exist under mock-vrf.
const VRF_PROGRAM = new PublicKey(VRF.program);
const ORACLE_QUEUE = new PublicKey(VRF.oracleQueue);
const SLOT_HASHES = new PublicKey(VRF.slotHashes);

const PRICE = new BN(LAMPORTS_PER_SOL / 10);
const MIN_PRICE = new BN(10_000_000);
const [N, E, S, W] = [0, 1, 2, 3];
const MAX_PATH = 16;
const EMPTY_UNIT = 0xff;
const BASE_ATTACK = 0xff;
const MONSTER_BASE = 4;
/** `EMPTY_ITEM`, and `ItemBase` / `EquipSlot` discriminants (crates/types). */
const EMPTY_ITEM = 0xff;
const [SWORD, STAFF, , LEATHER, ROBE] = [0, 1, 2, 3, 4];
const [MAIN_HAND, , , BODY] = [0, 1, 2, 3];
/** What a worn-items snapshot holds with only `worn` (slot -> base) on. */
const worn = (on: Record<number, number>) => Array.from({ length: 11 }, (_, i) => on[i] ?? EMPTY_ITEM);

// ---------------------------------------------------------------------------------------------
// helpers

const pda = (seeds: (Buffer | Uint8Array)[], programId: PublicKey) =>
  PublicKey.findProgramAddressSync(seeds, programId)[0];
const sha256 = (data: Buffer | string) => createHash("sha256").update(data).digest();
const sighash = (preimage: string) => sha256(preimage).subarray(0, 8);

const sessionAuthority = pda([Buffer.from("authority")], SESSION);
const config = pda([Buffer.from("config")], REGISTRY);
const bestiary = pda([Buffer.from("bestiary")], REGISTRY);
const tileset = pda([Buffer.from("tileset")], REGISTRY);
// Patch 4: the resonance tables open_session copies into the session's MonsterState.
const aspects = pda([Buffer.from("aspects")], REGISTRY);
const traits = pda([Buffer.from("traits")], REGISTRY);
const resonances = pda([Buffer.from("resonances")], REGISTRY);
// Patch 6: the skills of the bestiary pools (finalize_map checks the choice, open_session copies it).
const monsterSkills = pda([Buffer.from("monster_skills")], REGISTRY);
const programIdentity = pda([Buffer.from("identity")], SESSION);
/** `MOCK_VRF_IDENTITY` of programs/session/src/vrf.rs: a public test key, mock-vrf builds only. */
const MOCK_VRF = Keypair.fromSeed(sha256("deadroll:mock-vrf"));
const vrfAccounts = {
  programIdentity,
  oracleQueue: ORACLE_QUEUE,
  vrfProgram: VRF_PROGRAM,
  slotHashes: SLOT_HASHES,
  systemProgram: SystemProgram.programId,
};

function sessionAccounts(map: PublicKey) {
  const session = pda([Buffer.from("session"), map.toBuffer()], SESSION);
  return {
    session,
    monsters: pda([Buffer.from("monsters"), session.toBuffer()], SESSION),
    escrow: pda([Buffer.from("escrow"), session.toBuffer()], SESSION),
  };
}


function playerAccounts(session: PublicKey, character: PublicKey) {
  return {
    participation: pda(
      [Buffer.from("participation"), session.toBuffer(), character.toBuffer()],
      SESSION,
    ),
    stash: pda([Buffer.from("stash"), session.toBuffer(), character.toBuffer()], SESSION),
    skillSet: pda([Buffer.from("skills"), character.toBuffer()], CHARACTER),
    bag: pda([Buffer.from("bag"), character.toBuffer()], CHARACTER),
  };
}

async function funded(sol = 10): Promise<Keypair> {
  const kp = Keypair.generate();
  const sig = await connection.requestAirdrop(kp.publicKey, sol * LAMPORTS_PER_SOL);
  const latest = await connection.getLatestBlockhash();
  await connection.confirmTransaction({ signature: sig, ...latest }, "confirmed");
  return kp;
}

async function isProgram(id: PublicKey): Promise<boolean> {
  const info = await connection.getAccountInfo(id);
  return info?.executable ?? false;
}

/** Asserts the promise rejects with the Anchor error `code` (program or constraint error name). */
async function rejects(promise: Promise<unknown>, code: string): Promise<void> {
  try {
    await promise;
  } catch (e: any) {
    const name = e?.error?.errorCode?.code;
    const logs: string[] = e?.logs ?? e?.transactionLogs ?? [];
    const text = `${e?.message ?? e}\n${logs.join("\n")}`;
    assert.ok(
      name === code || text.includes(`Error Code: ${code}`) || text.includes(code),
      `expected ${code}, got ${name ?? text}`,
    );
    return;
  }
  assert.fail(`expected ${code}, transaction succeeded`);
}

function path(steps: number[]): [number[], number] {
  const padded = [...steps.slice(0, MAX_PATH)];
  while (padded.length < MAX_PATH) padded.push(0);
  return [padded, steps.length];
}

/** Roll `index` of a VRF value, exactly `deadroll_combat::VrfRoll`: sha256(seed ‖ index u32 LE). */
function rollAt(seed: Buffer, index: number): bigint {
  const i = Buffer.alloc(4);
  i.writeUInt32LE(index);
  return sha256(Buffer.concat([seed, i])).readBigUInt64LE(0);
}
const d20Of = (seed: Buffer) => Number(rollAt(seed, 0) % 20n) + 1;
/** First four bytes are `n` little endian. */
function fixedSeed(n: number): Buffer {
  const seed = Buffer.alloc(32);
  seed.writeUInt32LE(n, 0);
  return seed;
}
/** Natural 20: always a hit and a crit. Natural 1: always a miss. */
const CRIT = fixedSeed(9);
const MISS = fixedSeed(17);

/**
 * `rules::roll_initiative`: unit `i` (players by slot, then living monsters by index) rolls
 * d20 number `i`; higher `d20 + initiative` first, then higher initiative, then lower unit id.
 */
function initiativeOrder(seed: Buffer, units: [number, number][]): { order: number[]; d20: number[] } {
  const rolled = units.map(([unit, init], i) => ({ unit, init, d20: Number(rollAt(seed, i) % 20n) + 1 }));
  rolled.sort((a, b) => b.d20 + b.init - (a.d20 + a.init) || b.init - a.init || a.unit - b.unit);
  return { order: rolled.map((r) => r.unit), d20: rolled.map((r) => r.d20) };
}

/** Units of a started session, in roll order, from its stored initiatives. */
function initiativeUnits(s: any, monsters: any): [number, number][] {
  const units: [number, number][] = [];
  for (let slot = 0; slot < s.core.playerCount; slot++) units.push([slot, s.playerInitiative[slot]]);
  for (let i = 0; i < monsters.count; i++)
    if (monsters.monsters[i].alive) units.push([MONSTER_BASE + i, s.monsterInitiative[i]]);
  return units;
}

/** A VRF value whose initiative roll puts every player first, in slot order (keeps the flow tests
 * deterministic: player 0 acts first, then player 1, then the monsters). */
function playersFirstSeed(units: [number, number][], players: number): Buffer {
  const want = units.slice(0, players).map(([u]) => u);
  for (let n = 0; n < 200_000; n++) {
    const seed = fixedSeed(n);
    const { order } = initiativeOrder(seed, units);
    if (want.every((u, i) => order[i] === u)) return seed;
  }
  throw new Error("no players-first seed");
}

const parser = new EventParser(SESSION, program.coder);
async function eventsOf(sig: string): Promise<{ name: string; data: any }[]> {
  const tx = await connection.getTransaction(sig, {
    commitment: "confirmed",
    maxSupportedTransactionVersion: 0,
  });
  return [...parser.parseLogs(tx?.meta?.logMessages ?? [])];
}
/** The data of event `name` (any casing) emitted by `sig`. */
async function eventOf(sig: string, name: string): Promise<any> {
  const found = (await eventsOf(sig)).find((x) => x.name.toLowerCase() === name.toLowerCase());
  assert.ok(found, `no ${name} event`);
  return found.data;
}

/** MagicBlock Session Keys `create_session_v2`, signed by the session key and the main wallet. */
async function issueToken(
  signer: Keypair,
  authority: Keypair,
  opts: { target?: PublicKey; validUntil?: number } = {},
): Promise<PublicKey> {
  const target = opts.target ?? SESSION;
  const token = pda(
    [
      Buffer.from("session_token_v2"),
      target.toBuffer(),
      signer.publicKey.toBuffer(),
      authority.publicKey.toBuffer(),
    ],
    SESSION_KEYS,
  );
  const validUntil =
    opts.validUntil === undefined
      ? Buffer.from([0])
      : Buffer.concat([Buffer.from([1]), new BN(opts.validUntil).toArrayLike(Buffer, "le", 8)]);
  const data = Buffer.concat([
    sighash("global:create_session_v2"),
    Buffer.from([0]), // top_up: None
    validUntil,
    Buffer.from([0]), // lamports: None
  ]);
  const ix = new TransactionInstruction({
    programId: SESSION_KEYS,
    keys: [
      { pubkey: token, isSigner: false, isWritable: true },
      { pubkey: signer.publicKey, isSigner: true, isWritable: true },
      { pubkey: authority.publicKey, isSigner: true, isWritable: true }, // fee payer
      { pubkey: authority.publicKey, isSigner: true, isWritable: false },
      { pubkey: target, isSigner: false, isWritable: false },
      { pubkey: SystemProgram.programId, isSigner: false, isWritable: false },
    ],
    data,
  });
  await provider.sendAndConfirm(new Transaction().add(ix), [signer, authority]);
  return token;
}

async function revokeToken(token: PublicKey, authority: PublicKey): Promise<void> {
  const ix = new TransactionInstruction({
    programId: SESSION_KEYS,
    keys: [
      { pubkey: token, isSigner: false, isWritable: true },
      { pubkey: authority, isSigner: false, isWritable: true }, // fee payer gets the rent back
      { pubkey: authority, isSigner: false, isWritable: false },
      { pubkey: SystemProgram.programId, isSigner: false, isWritable: false },
    ],
    data: sighash("global:revoke_session_v2"),
  });
  await provider.sendAndConfirm(new Transaction().add(ix), []);
}

function openSession(dm: Keypair, map: PublicKey, layout: PublicKey, price: BN, dmActor: PublicKey) {
  return program.methods
    .openSession(price, dmActor)
    .accountsStrict({
      dm: dm.publicKey,
      map,
      layout,
      ...sessionAccounts(map),
      sessionAuthority,
      config,
      bestiary,
      tileset,
      mapProgram: MAP,
      systemProgram: SystemProgram.programId,
      aspects,
      traits,
      resonances,
      monsterSkills,
    })
    .signers([dm]);
}

/** The VRF callback, signed by `identity` (the mock identity on localnet). */
function consume(
  identity: Keypair,
  a: {
    session: PublicKey;
    monsters: PublicKey;
    participation: PublicKey;
    character: PublicKey;
    skillSet: PublicKey;
    /** The character's `Bag`, burned by `mark_dead` if the roll kills it. */
    bag: PublicKey;
    /** Unused since patch 5 (allies come with the party accounts); null. */
    targetParticipation?: PublicKey | null;
    /** The session's `MapLayout` (patch 4: knockback and burning cells; patch 5: line walls). */
    layout: PublicKey;
  },
  seed: Buffer,
  /** Patch 5: every other living player's Participation and Character (`partyAccounts`). */
  party: { pubkey: PublicKey; isSigner: boolean; isWritable: boolean }[] = [],
) {
  return program.methods
    .consumeRandomness([...seed])
    .accountsStrict({
      vrfProgramIdentity: identity.publicKey,
      targetParticipation: null,
      ...a,
      sessionAuthority,
      config,
      characterProgram: CHARACTER,
    })
    .remainingAccounts(party)
    .signers([identity]);
}

// ---------------------------------------------------------------------------------------------
// Part 1: runs on the shared validator today

describe("deadroll_session: deployed program", () => {
  it("is deployed", async () => {
    assert.ok(await isProgram(SESSION), "anchor deploy -p deadroll_session first");
  });

  it("open_session rejects an account that is not a deadroll map", async () => {
    const dm = await funded();
    const map = Keypair.generate().publicKey;
    await rejects(
      openSession(dm, map, Keypair.generate().publicKey, PRICE, PublicKey.default).rpc(),
      "InvalidMap",
    );
  });

  it("open_session rejects a wrong GameConfig address", async () => {
    const dm = await funded();
    const map = Keypair.generate().publicKey;
    await rejects(
      openSession(dm, map, map, PRICE, PublicKey.default)
        .accountsPartial({ config: Keypair.generate().publicKey })
        .rpc(),
      "ConstraintSeeds",
    );
  });

  it("open_session rejects registry tables at other addresses", async () => {
    const dm = await funded();
    const map = Keypair.generate().publicKey;
    for (const over of [
      { bestiary: tileset },
      { tileset: bestiary },
      { bestiary: config },
      { aspects: resonances },
      { traits: aspects },
      { resonances: traits },
    ]) {
      await rejects(
        openSession(dm, map, map, PRICE, PublicKey.default).accountsPartial(over).rpc(),
        "InvalidRegistryTable",
      );
    }
  });

  it("open_session rejects a wrong map program", async () => {
    const dm = await funded();
    const map = Keypair.generate().publicKey;
    await rejects(
      openSession(dm, map, map, PRICE, PublicKey.default)
        .accountsPartial({ mapProgram: CHARACTER })
        .rpc(),
      "ConstraintAddress",
    );
  });

  it("game instructions reject a session that was never opened", async () => {
    const signer = await funded();
    const { session, monsters, escrow } = sessionAccounts(Keypair.generate().publicKey);
    const character = Keypair.generate().publicKey;
    const { participation, stash, skillSet, bag } = playerAccounts(session, character);
    const run = (m: any) => rejects(m.signers([signer]).rpc(), "AccountNotInitialized");
    await run(
      program.methods
        .startSession()
        .accountsStrict({ dm: signer.publicKey, session, monsters, escrow, ...vrfAccounts }),
    );
    await run(
      program.methods
        .endTurn()
        .accountsStrict({ signer: signer.publicKey, session, monsters, participation: null, sessionToken: null }),
    );
    await run(
      program.methods.act(BASE_ATTACK, 0).accountsStrict({
        signer: signer.publicKey,
        session,
        participation,
        character,
        skillSet,
        bag,
        monsters,
        sessionToken: null,
        ...vrfAccounts,
        targetParticipation: null,
        layout: Keypair.generate().publicKey,
      }),
    );
    await run(
      program.methods.monsterAct(0, 0, 0).accountsStrict({
        signer: signer.publicKey,
        session,
        monsters,
        participation,
        character,
        skillSet,
        bag,
        sessionToken: null,
        ...vrfAccounts,
        layout: Keypair.generate().publicKey,
      }),
    );
    await run(
      program.methods
        .flee()
        .accountsStrict({ signer: signer.publicKey, session, monsters, participation, sessionToken: null }),
    );
    await run(
      program.methods.claimEscrow().accountsStrict({
        dm: signer.publicKey,
        session,
        escrow,
        map: Keypair.generate().publicKey,
        sessionAuthority,
        config,
        mapProgram: MAP,
        characterProgram: CHARACTER,
      }),
    );
    await rejects(
      program.methods
        .claimRewards()
        .accountsStrict({
          signer: signer.publicKey,
          session,
          participation,
          stash,
          character,
          bag,
          sessionAuthority,
          config,
          characterProgram: CHARACTER,
          sessionToken: null,
        })
        .signers([signer])
        .rpc(),
      "AccountNotInitialized",
    );
  });

  it("act rejects VRF accounts at other addresses", async () => {
    const signer = await funded();
    const { session, monsters } = sessionAccounts(Keypair.generate().publicKey);
    const character = Keypair.generate().publicKey;
    const { participation, skillSet, bag } = playerAccounts(session, character);
    const act = (over: Record<string, PublicKey>) =>
      program.methods
        .act(BASE_ATTACK, 0)
        .accountsStrict({ signer: signer.publicKey, session, participation, character, skillSet, bag, monsters, sessionToken: null, targetParticipation: null, layout: Keypair.generate().publicKey, ...vrfAccounts, ...over })
        .signers([signer])
        .rpc();
    // Session is checked first; these only prove the IDL carries the checked accounts.
    for (const key of ["oracleQueue", "vrfProgram", "slotHashes", "programIdentity"]) {
      await assert.rejects(act({ [key]: Keypair.generate().publicKey }));
    }
  });

  it("consume_randomness rejects a session that was never opened", async () => {
    // Anchor loads every account before any constraint, so the identity check (mock identity in
    // a mock-vrf build, ConstraintAddress otherwise) shows only with a real session: part 2.
    const { session, monsters } = sessionAccounts(Keypair.generate().publicKey);
    const character = Keypair.generate().publicKey;
    const { participation, skillSet, bag } = playerAccounts(session, character);
    const accounts = { session, monsters, participation, character, skillSet, bag, layout: Keypair.generate().publicKey };
    await rejects(consume(await funded(1), accounts, CRIT).rpc(), "AccountNotInitialized");
    await rejects(consume(MOCK_VRF, accounts, CRIT).rpc(), "AccountNotInitialized");
  });

  it("dice in the tests follow deadroll_combat::VrfRoll", () => {
    assert.equal(d20Of(CRIT), 20);
    assert.equal(d20Of(MISS), 1);
    // rules::tests::initiative_order: same tie-breaks (total, then initiative, then unit id).
    const units: [number, number][] = [[0, 5], [1, 0], [4, 2], [5, 2], [6, 10]];
    const seed = playersFirstSeed(units, 2);
    assert.deepEqual(initiativeOrder(seed, units).order.slice(0, 2), [0, 1]);
    // sha256([0; 32] ‖ 00 00 00 00), first 8 bytes LE (crates/combat/src/rng.rs test vector source).
    const zero = Buffer.alloc(32);
    assert.equal(rollAt(zero, 0), sha256(Buffer.alloc(36)).readBigUInt64LE(0));
  });

  it("external account discriminators", () => {
    assert.deepEqual([...sighash("account:Character")], [0x8c, 0x73, 0xa5, 0x24, 0xf1, 0x99, 0x66, 0x54]);
    assert.deepEqual([...sighash("account:Map")], [0xb6, 0x1e, 0x8e, 0x97, 0x2a, 0xf1, 0xb4, 0xf4]);
    assert.deepEqual([...sighash("account:MapLayout")], [0x8e, 0x57, 0xdf, 0xf4, 0x3b, 0x7b, 0x3e, 0xca]);
  });
});

// ---------------------------------------------------------------------------------------------
// Part 2: party flow on the demo map, needs registry GameConfig + character + map

const CHARACTER_IDL = new URL("../../../target/idl/deadroll_character.json", import.meta.url);
const MAP_IDL = new URL("../../../target/idl/deadroll_map.json", import.meta.url);
const missing: string[] = [];
if ((await connection.getAccountInfo(config))?.owner.equals(REGISTRY) !== true)
  missing.push("registry GameConfig (L4)");
for (const [name, table] of [
  ["bestiary", bestiary],
  ["tileset", tileset],
  ["aspects", aspects],
  ["traits", traits],
  ["resonances", resonances],
] as const)
  if ((await connection.getAccountInfo(table))?.owner.equals(REGISTRY) !== true)
    missing.push(`registry ${name} table seeded (tools/scripts/seed-registry.ts)`);
if (!(await isProgram(CHARACTER))) missing.push("deadroll_character deployed");
if (!(await isProgram(MAP))) missing.push("deadroll_map deployed");
if (!existsSync(CHARACTER_IDL)) missing.push("target/idl/deadroll_character.json");
if (!existsSync(MAP_IDL)) missing.push("target/idl/deadroll_map.json");
const SKIP_PARTY = missing.length ? `needs ${missing.join(", ")}` : false;

const characterProgram: any = SKIP_PARTY
  ? null
  : new Program(JSON.parse(readFileSync(CHARACTER_IDL, "utf8")), provider);
const mapProgram: any = SKIP_PARTY ? null : new Program(JSON.parse(readFileSync(MAP_IDL, "utf8")), provider);

const EQUIP_SLOT_NAMES = ["mainHand", "offHand", "helmet", "body"];
/** character `unequip` by the owner (or `signer` with `token`). */
function unequip(owner: Keypair, character: PublicKey, slot: number, signer = owner, token: PublicKey | null = null) {
  return characterProgram.methods
    .unequip(e(EQUIP_SLOT_NAMES[slot]))
    .accountsStrict({ signer: signer.publicKey, character, sessionToken: token })
    .signers([signer])
    .rpc();
}
/** character `spend_stat_points` by `signer` (the owner's main wallet only). */
function spendStatPoints(signer: Keypair, character: PublicKey, points: number[]) {
  return characterProgram.methods
    .spendStatPoints(points)
    .accountsStrict({ owner: signer.publicKey, character })
    .signers([signer])
    .rpc();
}
/** character `equip` of soulbound item `item` by the owner. */
function equip(owner: Keypair, character: PublicKey, item: number, slot: number) {
  return characterProgram.methods
    .equip(item, e(EQUIP_SLOT_NAMES[slot]))
    .accountsStrict({ signer: owner.publicKey, character, sessionToken: null })
    .signers([owner])
    .rpc();
}
/** Every skill of a starter set taken onto the map: 4 active, 1 reactive, 1 passive. */
const FULL_SELECTION = [0, 1, 2, 3, 4, 5];
const NO_SKILL = 0xff;

// The map lane's demo preset (programs/map/tests/map.test.ts): 16x16 room in a wall ring.
const TILE = { Floor: 0, Wall: 1, Entrance: 4, Exit: 5 };
const ENTRANCE = { x: 1, y: 1 };
const EXIT = { x: 14, y: 14 };
const [WOLF, GOBLIN, CHIEFTAIN, GOBLIN_WARRIOR, SHAMAN] = [0, 1, 2, 3, 4];
/** Monster index of the chieftain in `DEMO_SPAWNS`. */
const CHIEFTAIN_INDEX = 4;
/** `skills`: the chosen pool bits (assets/registry/maps/demo_16x16.json): exactly the slots of each
 * kind the template has (patch 6, finalize_map checks it). */
type SpawnDef = { id: number; x: number; y: number; pack: number; skills: number; flags?: number };
const DEMO_SPAWNS: SpawnDef[] = [
  { id: WOLF, x: 6, y: 2, pack: 1, skills: 0b0111 },
  { id: WOLF, x: 7, y: 3, pack: 1, skills: 0b1110 },
  { id: GOBLIN, x: 2, y: 10, pack: 2, skills: 0b11 },
  { id: GOBLIN, x: 3, y: 11, pack: 2, skills: 0b11 },
  { id: CHIEFTAIN, x: 12, y: 12, pack: 3, skills: 0b11111 },
  { id: GOBLIN_WARRIOR, x: 11, y: 12, pack: 3, skills: 0b11 },
  { id: SHAMAN, x: 12, y: 11, pack: 3, skills: 0b111 },
];

interface MapFixture {
  map: PublicKey;
  layout: PublicKey;
}

/** `bossLevel`: the goblin chieftain's spawn level (its XP grows with it); the rest are level 1.
 * `spawns`: another roster on the same room (patch 6 tests place packs near the entrance). */
async function createFinalizedMap(dm: Keypair, bossLevel = 1, roster: SpawnDef[] = DEMO_SPAWNS): Promise<MapFixture> {
  const index = randomBytes(4).readUInt32LE(0);
  const idx = Buffer.alloc(4);
  idx.writeUInt32LE(index);
  const map = pda([Buffer.from("map"), dm.publicKey.toBuffer(), idx], MAP);
  const layout = pda([Buffer.from("layout"), map.toBuffer()], MAP);
  const edit = { dm: dm.publicKey, map, layout };
  await mapProgram.methods
    .createMap(index, 16, 16, 1)
    .accountsStrict({ ...edit, systemProgram: SystemProgram.programId })
    .signers([dm])
    .rpc();
  const tiles = new Array(15 * 32 + 16).fill(TILE.Wall);
  for (let y = 1; y < 15; y++) for (let x = 1; x < 15; x++) tiles[y * 32 + x] = TILE.Floor;
  tiles[ENTRANCE.y * 32 + ENTRANCE.x] = TILE.Entrance;
  tiles[EXIT.y * 32 + EXIT.x] = TILE.Exit;
  await mapProgram.methods.writeLayout(0, Buffer.from(tiles)).accountsStrict(edit).signers([dm]).rpc();
  const spawns = roster.map((s) => ({
    bestiaryId: s.id,
    x: s.x,
    y: s.y,
    level: s.id === CHIEFTAIN ? bossLevel : 1,
    skills: s.skills,
    flags: s.flags ?? 0,
    aspects: 0,
    pack: s.pack,
  }));
  await mapProgram.methods.writeSpawns(ENTRANCE, EXIT, spawns).accountsStrict(edit).signers([dm]).rpc();
  // Patch 6: the builder rules read the registry bestiary and monster skills.
  await mapProgram.methods
    .finalizeMap(new Array(32).fill(7))
    .accountsStrict({ ...edit, bestiary, monsterSkills })
    .signers([dm])
    .rpc();
  return { map, layout };
}

// Same blocks as `deadroll_skillcheck::presets` (programs/character/tests/character.test.ts).
const e = (name: string) => ({ [name]: {} });
const text = (s: string, len: number) => {
  const out = new Array(len).fill(0);
  Buffer.from(s, "ascii").forEach((b, i) => (out[i] = b));
  return out;
};
function blocks(category: string, purpose: string, source: string, form: string, aspects: string[], o: any = {}) {
  return {
    category: e(category),
    purpose: e(purpose),
    source: e(source),
    form: e(form),
    target: e(o.line ? "line" : "single"),
    aspects: [...aspects, "fire", "fire"].slice(0, 3).map(e),
    aspectCount: aspects.length,
    effect: e(o.effect ?? "none"),
    trigger: e(o.trigger ?? "none"),
    auraRadius: 0,
  };
}
const WARRIOR_SKILLS: [string, any][] = [
  ["Piercing Thrust", blocks("active", "damage", "combat", "strike", ["piercing"])],
  ["Shield Bash", blocks("active", "damage", "combat", "strike", ["blunt"], { effect: "stun" })],
  ["Javelin", blocks("active", "damage", "combat", "projectile", ["piercing"])],
  ["Bleeding Cleave", blocks("active", "damage", "combat", "strike", ["blood", "piercing"], { line: true })],
  ["Riposte", blocks("reactive", "damage", "combat", "strike", ["piercing"], { trigger: "hit" })],
  ["Iron Skin", blocks("passive", "buff", "combat", "strike", ["blunt"], { effect: "protection" })],
];
const MAGE_SKILLS: [string, any][] = [
  ["Frost Bolt", blocks("active", "damage", "magical", "projectile", ["cold"])],
  ["Frost Burn", blocks("active", "damage", "magical", "projectile", ["cold", "fire"])],
  ["Ice Prison", blocks("active", "damage", "magical", "projectile", ["cold"], { effect: "freeze" })],
  ["Arc", blocks("active", "damage", "magical", "strike", ["lightning"], { line: true })],
  ["Cold Snap", blocks("reactive", "debuff", "magical", "strike", ["cold"], { effect: "slow", trigger: "enemyAdjacent" })],
  ["Cold Mind", blocks("passive", "buff", "magical", "projectile", ["cold"], { effect: "empowerment" })],
];
const SUPPORT_SKILLS: [string, any][] = [
  ["Quicken", blocks("active", "buff", "magical", "projectile", ["lightning"], { effect: "haste" })],
  ["Sap", blocks("active", "debuff", "magical", "projectile", ["blood"], { effect: "weakening" })],
  ["Expose", blocks("active", "debuff", "magical", "projectile", ["fire"], { effect: "vulnerability" })],
  ["Shelter", blocks("active", "buff", "combat", "strike", ["blunt"], { effect: "protection" })],
  ["Last Stand", { ...blocks("reactive", "buff", "combat", "strike", ["blunt"], { effect: "protection", trigger: "hpBelow" }), auraRadius: 2 }],
  ["Cold Ward", { ...blocks("passive", "buff", "magical", "strike", ["cold"], { effect: "protection" }), auraRadius: 2 }],
];
// Patch 5: no Heal in the builder (CHARACTER 8.1); Expose takes the slot Mend had.
const [QUICKEN, SAP, EXPOSE, SHELTER] = [0, 1, 2, 3];
/** Projectile skill slot per class: Javelin, Frost Bolt, Sap (a debuff, it rolls like an attack). */
const RANGED_SLOT = { warrior: 2, mage: 0, support: SAP, stormMage: 0 };
const REACTIVE_SLOT = 4;

type Kind = "warrior" | "mage" | "support" | "stormMage";
const PROFILES = {
  warrior: { name: "Bran", race: "human", predisposition: "piercing", stats: [10, 7, 5, 8], skills: WARRIOR_SKILLS },
  mage: { name: "Ilyra", race: "forestElf", predisposition: "cold", stats: [5, 7, 13, 5], skills: MAGE_SKILLS },
  support: { name: "Senna", race: "forestElf", predisposition: "lightning", stats: [5, 8, 12, 5], skills: SUPPORT_SKILLS },
  // A mage with a Lightning predisposition: its Fire and Cold get Swiftness (states jump).
  stormMage: { name: "Vael", race: "forestElf", predisposition: "lightning", stats: [5, 7, 13, 5], skills: MAGE_SKILLS },
};
const CLASS_OF: Record<Kind, string> = { warrior: "warrior", mage: "mage", support: "support", stormMage: "mage" };
/** A character of `kind`: the class starter items come with it, worn (create_character). */
async function createCharacter(owner: Keypair, kind: Kind): Promise<PublicKey> {
  const profile = pda([Buffer.from("profile"), owner.publicKey.toBuffer()], CHARACTER);
  if (!(await connection.getAccountInfo(profile))) {
    await characterProgram.methods
      .createProfile()
      .accountsStrict({ owner: owner.publicKey, profile, systemProgram: SystemProgram.programId })
      .signers([owner])
      .rpc();
  }
  const index = (await characterProgram.account.playerProfile.fetch(profile)).characterCount;
  const character = pda([Buffer.from("character"), owner.publicKey.toBuffer(), Buffer.from([index])], CHARACTER);
  const layer = { spriteId: 1, materialRamp: 2, accentRamp: 3, elementRamp: 0 };
  const p = PROFILES[kind];
  const args = {
    name: text(p.name, 32),
    race: e(p.race),
    class: e(CLASS_OF[kind]),
    predisposition: e(p.predisposition),
    stats: p.stats,
    appearance: new Array(20).fill(layer),
    skills: p.skills.map(([name, b]) => newSkill(name, b, text)),
  };
  const asset = Keypair.generate();
  await characterProgram.methods
    .createCharacter(args)
    .accountsStrict({
      owner: owner.publicKey,
      profile,
      character,
      skillSet: pda([Buffer.from("skills"), character.toBuffer()], CHARACTER),
      bag: pda([Buffer.from("bag"), character.toBuffer()], CHARACTER),
      config,
      asset: asset.publicKey,
      mplCoreProgram: MPL_CORE,
      systemProgram: SystemProgram.programId,
      skillExtras: pda([Buffer.from("skill_extras"), character.toBuffer()], CHARACTER),
    })
    .signers([owner, asset])
    .rpc();
  return character;
}

interface Player {
  kind: Kind;
  wallet: Keypair;
  actor: Keypair;
  character: PublicKey;
  participation: PublicKey;
  stash: PublicKey;
  skillSet: PublicKey;
  bag: PublicKey;
}

/** A state of `kind` (IDL name, e.g. "freeze") is on. */
const hasState = (states: any[], kind: string) => states.some((x) => x.turns > 0 && x.kind[kind] !== undefined);
/** Burning + Bleeding damage the next turn start deals. */
const dotOf = (states: any[]) =>
  states
    .filter((x) => x.turns > 0 && (x.kind.burning !== undefined || x.kind.bleeding !== undefined))
    .reduce((sum, x) => sum + x.power, 0);
const eventsNamed = (events: { name: string; data: any }[], name: string) =>
  events.filter((x) => x.name.toLowerCase() === name.toLowerCase()).map((x) => x.data);

const cheb = (a: any, b: any) => Math.max(Math.abs(a.x - b.x), Math.abs(a.y - b.y));
/** A living player slot monster `m` can strike from where it stands (shamans shoot, range 6). */
function targetInRange(s: any, m: any): number | undefined {
  const range = m.bestiaryId === SHAMAN ? 6 : 1;
  for (let slot = 0; slot < s.core.playerCount; slot++) {
    const d = cheb(m, s.playerPos[slot]);
    if (s.aliveMask & (1 << slot) && d >= 1 && d <= range) return slot;
  }
  return undefined;
}

/** Patch 6: the accounts that finalize the deaths a callback queued (`GameSession.pending_deaths`):
 * each queued slot's Character, SkillSet and Bag, writable, in slot order. */
function deathAccounts(st: any) {
  const out: { pubkey: PublicKey; isSigner: boolean; isWritable: boolean }[] = [];
  for (let slot = 0; slot < st.core.playerCount; slot++) {
    if (!(st.pendingDeaths & (1 << slot))) continue;
    const c: PublicKey = st.core.players[slot];
    const skillSet = pda([Buffer.from("skills"), c.toBuffer()], CHARACTER);
    const bag = pda([Buffer.from("bag"), c.toBuffer()], CHARACTER);
    for (const pubkey of [c, skillSet, bag]) out.push({ pubkey, isSigner: false, isWritable: true });
  }
  return out;
}
/** Compute limit of an instruction that may finalize deaths (`character.mark_dead` each). */
const FINALIZE_CU = 600_000;
const finalizeBudget = (deaths: unknown[]) => (deaths.length ? [ComputeBudgetProgram.setComputeUnitLimit({ units: FINALIZE_CU })] : []);

/** One party on a fresh demo map, with helpers for every session instruction. */
class Party {
  dm!: Keypair;
  dmActor!: Keypair;
  fixture!: MapFixture;
  acc!: ReturnType<typeof sessionAccounts>;
  players: Player[] = [];

  static async create(kinds: Kind[], opts: { bossLevel?: number; roster?: SpawnDef[] } = {}): Promise<Party> {
    const p = new Party();
    [p.dm, p.dmActor] = await Promise.all([funded(), funded(1)]);
    p.fixture = await createFinalizedMap(p.dm, opts.bossLevel, opts.roster);
    p.acc = sessionAccounts(p.fixture.map);
    for (const kind of kinds) {
      const [wallet, actor] = await Promise.all([funded(), funded(1)]);
      const character = await createCharacter(wallet, kind);
      p.players.push({ kind, wallet, actor, character, ...playerAccounts(p.acc.session, character) });
    }
    return p;
  }

  open() {
    return openSession(this.dm, this.fixture.map, this.fixture.layout, PRICE, this.dmActor.publicKey);
  }
  join(who: number, signer: Keypair, contribution: BN) {
    const pl = this.players[who];
    return program.methods
      .joinSession(contribution, pl.actor.publicKey)
      .accountsStrict({
        player: signer.publicKey,
        session: this.acc.session,
        character: pl.character,
        participation: pl.participation,
        stash: pl.stash,
        escrow: this.acc.escrow,
        systemProgram: SystemProgram.programId,
      })
      .signers([signer])
      .rpc();
  }
  /** Ready takes `selection` onto the map and locks the character; unready unlocks it. */
  ready(who: number, signer: Keypair, value: boolean, selection = FULL_SELECTION, over: Record<string, PublicKey> = {}) {
    const pl = this.players[who];
    return program.methods
      .setReady(value, selection)
      .accountsStrict({
        player: signer.publicKey,
        session: this.acc.session,
        participation: pl.participation,
        character: pl.character,
        skillSet: pl.skillSet,
        sessionAuthority,
        config,
        characterProgram: CHARACTER,
        skillExtras: pda([Buffer.from("skill_extras"), pl.character.toBuffer()], CHARACTER),
        ...over,
      })
      .signers([signer])
      .rpc({ commitment: "confirmed" });
  }
  start(signer: Keypair) {
    return program.methods
      .startSession()
      .accountsStrict({
        dm: signer.publicKey,
        session: this.acc.session,
        monsters: this.acc.monsters,
        escrow: this.acc.escrow,
        ...vrfAccounts,
      })
      .signers([signer])
      .rpc();
  }
  /** The initiative units of the started session (players, then living monsters). */
  async initiativeUnits() {
    return initiativeUnits(await this.state(), (await this.monsters()).core);
  }
  /** The initiative callback (player slot 0's accounts) with a seed that puts players first. */
  async rollInitiative(): Promise<{ sig: string; seed: Buffer }> {
    const seed = playersFirstSeed(await this.initiativeUnits(), this.players.length);
    return { sig: await this.consume(0, seed), seed };
  }
  /** open, everyone joins (player 0 pays the whole price), ready, start, initiative. */
  async startAll() {
    await this.open().rpc();
    for (let i = 0; i < this.players.length; i++) await this.join(i, this.players[i].wallet, i === 0 ? PRICE : new BN(0));
    for (let i = 0; i < this.players.length; i++) await this.ready(i, this.players[i].wallet, true);
    await this.start(this.dm);
    await this.rollInitiative();
    await this.begin();
  }
  /** `start_turn` with player `who`'s accounts (`null`: a monster's turn); sent by the provider
   * wallet, which is not in the party (anyone may start a turn). */
  async startTurn(who: number | null, over: Record<string, PublicKey | null> = {}, deaths: { pubkey: PublicKey; isSigner: boolean; isWritable: boolean }[] | null = null) {
    const pl = who === null ? null : this.players[who];
    // Patch 6: the deaths the last callback queued are finalized first.
    const queue = deaths ?? deathAccounts(await this.state());
    return program.methods
      .startTurn()
      .accountsStrict({
        session: this.acc.session,
        monsters: this.acc.monsters,
        participation: pl?.participation ?? null,
        character: pl?.character ?? null,
        sessionAuthority,
        config,
        characterProgram: CHARACTER,
        // Burned if over-time damage kills the player.
        skillSet: pl?.skillSet ?? null,
        bag: pl?.bag ?? null,
        // Pays the VRF fee when over-time damage triggers a reactive skill (patch 5).
        payer: provider.wallet.publicKey,
        ...vrfAccounts,
        ...over,
      })
      .remainingAccounts(queue)
      .preInstructions(finalizeBudget(queue))
      .rpc({ commitment: "confirmed" });
  }
  /** Patch 6: `start_turn` that finalizes the queued deaths (with the current player's accounts,
   * the turn may not have started yet); `null` when nothing waits. */
  async finalize() {
    const s = await this.state();
    if (!s.pendingDeaths || s.core.pendingVrf || !s.core.status.active) return null;
    const unit = s.core.turnOrder[s.core.turnIndex];
    return this.startTurn(unit < MONSTER_BASE ? unit : null);
  }
  /** `start_turn` for whoever's turn it is; a reactive roll it asks for is answered (`settle`). */
  async begin(): Promise<string> {
    const s = await this.state();
    const unit = s.core.turnOrder[s.core.turnIndex];
    const sig = await this.startTurn(unit < MONSTER_BASE ? unit : null);
    await this.settle();
    return sig;
  }
  /**
   * Patch 5: answers a pending reactive-only roll (a monster stepping next to a Cold Snap bearer,
   * over-time damage below a threshold) with `seed`, on the first bearer's accounts; nothing
   * when no such roll waits.
   */
  async settle(seed: Buffer = MISS) {
    const s = await this.state();
    if (!s.core.pendingVrf) return this.finalize();
    // Patch 6: a monster's reactive-only roll rides on the player the request named.
    if (s.pendingMonsterReactive.kind !== 0) return this.consume(this.pendingWho(s), seed);
    if (s.pendingReactive.kind === 0) return null;
    const bearer = [0, 1, 2, 3].find((i) => s.pendingReactive.bearers & (1 << i))!;
    return this.consume(bearer, seed);
  }
  /** The player whose accounts the pending callback takes (`GameSession.pending.participation`). */
  pendingWho(s: any): number {
    const who = this.players.findIndex((pl) => pl.participation.equals(s.pending.participation));
    assert.ok(who >= 0, "the pending participation is a party player's");
    return who;
  }
  /** Starts every turn and ends every unit's turn but `unit`'s, until `unit`'s turn has started. */
  async skipTo(unit: number, max = 40) {
    for (let i = 0; i < max; i++) {
      await this.settle();
      const s = await this.state();
      const cur = s.core.turnOrder[s.core.turnIndex];
      if (s.startedSeq !== s.turnSeq) await this.begin();
      else if (cur === unit) return s;
      else if (cur < MONSTER_BASE) await this.endTurn(this.players[cur].wallet, cur);
      else await this.endTurn(this.dm, null);
    }
    assert.fail(`unit ${unit} never got a turn`);
  }
  /**
   * The callback for player `who`'s pending action with the first VRF value whose simulated
   * `HitResolved` result passes `pred` (the test picks the dice; nothing is sent while trying).
   */
  async consumeWhere(who: number, pred: (result: any) => boolean, tries = 600) {
    return this.consumeBest(who, (r) => (pred(r) ? 1 : -Infinity), tries, 1);
  }
  /** Same, sending the best-scoring of `tries` simulated values (at once when one reaches
   * `enough`); `null` when every value scored `-Infinity` (nothing sent). */
  async consumeBest(who: number, score: (result: any) => number, tries = 200, enough = Infinity) {
    const pl = this.players[who];
    const a = {
      session: this.acc.session,
      monsters: this.acc.monsters,
      participation: pl.participation,
      character: pl.character,
      skillSet: pl.skillSet,
      bag: pl.bag,
      layout: this.fixture.layout,
    };
    // Simulated through the connection (no signature check): Anchor's `simulate()` re-signs with
    // the extra signer after the wallet and loses the fee payer's signature.
    const { blockhash } = await connection.getLatestBlockhash();
    let best: { seed: Buffer; score: number } | null = null;
    for (let i = 0; i < tries; i++) {
      const seed = nextSeed();
      const tx = await consume(MOCK_VRF, a, seed, await this.partyOf(who)).transaction();
      tx.feePayer = provider.wallet.publicKey;
      tx.recentBlockhash = blockhash;
      tx.partialSign(MOCK_VRF);
      const sim = await connection.simulateTransaction(tx);
      const hit = sim.value.err
        ? undefined
        : [...parser.parseLogs(sim.value.logs ?? [])].find((x) => x.name.toLowerCase() === "hitresolved");
      const value = hit ? score(hit.data.result) : -Infinity;
      if (value > (best?.score ?? -Infinity)) best = { seed, score: value };
      if (value >= enough) break;
    }
    if (!best) return null;
    const sig = await this.consume(who, best.seed);
    return { sig, result: (await eventOf(sig, "hitResolved")).result, score: best.score };
  }
  /** Starts and ends turns until `unit`'s turn is next to start (it is not started). */
  async untilStartOf(unit: number, max = 40) {
    for (let i = 0; i < max; i++) {
      await this.settle();
      const s = await this.state();
      const cur = s.core.turnOrder[s.core.turnIndex];
      if (s.startedSeq !== s.turnSeq) {
        if (cur === unit) return s;
        await this.begin();
      } else if (cur < MONSTER_BASE) await this.endTurn(this.players[cur].wallet, cur);
      else await this.endTurn(this.dm, null);
    }
    assert.fail(`unit ${unit} never got a turn`);
  }
  move(who: number, signer: Keypair, steps: number[], token: PublicKey | null = null) {
    const [padded, len] = path(steps);
    return program.methods
      .moveUnit(padded, len)
      .accountsStrict({
        signer: signer.publicKey,
        session: this.acc.session,
        participation: this.players[who].participation,
        monsters: this.acc.monsters,
        layout: this.fixture.layout,
        sessionToken: token,
        // Patch 6: stepping next to a monster can trigger its reactive skill (a VRF roll).
        ...vrfAccounts,
      })
      .signers([signer])
      .rpc();
  }
  monsterMove(signer: Keypair, monster: number, steps: number[], token: PublicKey | null = null) {
    const [padded, len] = path(steps);
    return program.methods
      .monsterMove(monster, padded, len)
      .accountsStrict({
        signer: signer.publicKey,
        session: this.acc.session,
        monsters: this.acc.monsters,
        layout: this.fixture.layout,
        sessionToken: token,
        ...vrfAccounts,
      })
      .signers([signer])
      .rpc();
  }
  endTurn(signer: Keypair, who: number | null, token: PublicKey | null = null) {
    return program.methods
      .endTurn()
      .accountsStrict({
        signer: signer.publicKey,
        session: this.acc.session,
        monsters: this.acc.monsters,
        participation: who === null ? null : this.players[who].participation,
        sessionToken: token,
      })
      .signers([signer])
      .rpc();
  }
  /** `ally`: the target's participation for a heal or buff on another player. */
  act(who: number, signer: Keypair, slot: number, target: number, token: PublicKey | null = null, ally: PublicKey | null = null) {
    const pl = this.players[who];
    return program.methods
      .act(slot, target)
      .accountsStrict({
        signer: signer.publicKey,
        session: this.acc.session,
        participation: pl.participation,
        character: pl.character,
        skillSet: pl.skillSet,
        bag: pl.bag,
        monsters: this.acc.monsters,
        sessionToken: token,
        ...vrfAccounts,
        targetParticipation: ally,
        layout: this.fixture.layout,
      })
      .signers([signer])
      .rpc();
  }
  /**
   * `action`: 0 = the natural attack, `1 + slot` = the monster's skill in that slot (patch 6).
   * `target`: a player slot, or for a heal or buff (`accounts` = the lowest living player) a
   * monster index; `accounts`: whose participation the callback takes.
   */
  monsterAct(signer: Keypair, monster: number, target: number, token: PublicKey | null = null, action = 0, accounts = target) {
    const pl = this.players[accounts];
    return program.methods
      .monsterAct(monster, action, target)
      .accountsStrict({
        signer: signer.publicKey,
        session: this.acc.session,
        monsters: this.acc.monsters,
        participation: pl.participation,
        character: pl.character,
        skillSet: pl.skillSet,
        bag: pl.bag,
        sessionToken: token,
        ...vrfAccounts,
        layout: this.fixture.layout,
      })
      .signers([signer])
      .rpc();
  }
  /** The callback for the pending action of player `who` (attacker or target); `ally`: the
   * other player of a heal or buff. */
  async consume(who: number, seed: Buffer, identity = MOCK_VRF, ally: PublicKey | null = null) {
    return (await this.consumeTx(who, seed, identity, ally)).rpc({ commitment: "confirmed" });
  }
  /** The other living players' accounts the callback of player `who`'s pending roll gets. */
  async partyOf(who: number) {
    return partyAccounts(await this.state(), this.players[who].participation);
  }
  /** `party`: override the party accounts (`null`: what the session expects). */
  async consumeTx(
    who: number,
    seed: Buffer,
    identity = MOCK_VRF,
    ally: PublicKey | null = null,
    party: { pubkey: PublicKey; isSigner: boolean; isWritable: boolean }[] | null = null,
  ) {
    const pl = this.players[who];
    return consume(
      identity,
      {
        session: this.acc.session,
        monsters: this.acc.monsters,
        participation: pl.participation,
        character: pl.character,
        skillSet: pl.skillSet,
        bag: pl.bag,
        targetParticipation: ally,
        layout: this.fixture.layout,
      },
      seed,
      party ?? (await this.partyOf(who)),
    );
  }
  /** The events the callback would emit with `seed` (simulated, nothing sent); `null` on an error. */
  async simulateConsume(who: number, seed: Buffer, ally: PublicKey | null = null) {
    const tx = await (await this.consumeTx(who, seed, MOCK_VRF, ally)).transaction();
    tx.feePayer = provider.wallet.publicKey;
    tx.recentBlockhash = (await connection.getLatestBlockhash()).blockhash;
    tx.partialSign(MOCK_VRF);
    const sim = await connection.simulateTransaction(tx);
    return sim.value.err ? null : [...parser.parseLogs(sim.value.logs ?? [])];
  }
  /** Sends the callback with the first VRF value whose simulated events pass `pred`. */
  async consumeEventsWhere(
    who: number,
    pred: (events: { name: string; data: any }[]) => boolean,
    ally: PublicKey | null = null,
    tries = 400,
  ) {
    for (let i = 0; i < tries; i++) {
      const seed = nextSeed();
      const events = await this.simulateConsume(who, seed, ally);
      if (events && pred(events)) {
        const sig = await this.consume(who, seed, MOCK_VRF, ally);
        return { sig, seed, events: await eventsOf(sig) };
      }
    }
    return assert.fail("no VRF value gave the wanted rolls");
  }
  flee(who: number, signer: Keypair) {
    return program.methods
      .flee()
      .accountsStrict({
        signer: signer.publicKey,
        session: this.acc.session,
        monsters: this.acc.monsters,
        participation: this.players[who].participation,
        sessionToken: null,
      })
      .signers([signer])
      .rpc();
  }
  /** Signed by the owner's main wallet unless another signer (and session token) is given. */
  async claimRewards(who: number, signer: Keypair = this.players[who].wallet, sessionToken: PublicKey | null = null) {
    const pl = this.players[who];
    return program.methods
      .claimRewards()
      .accountsStrict({
        signer: signer.publicKey,
        session: this.acc.session,
        participation: pl.participation,
        stash: pl.stash,
        character: pl.character,
        bag: pl.bag,
        sessionAuthority,
        config,
        characterProgram: CHARACTER,
        sessionToken,
      })
      .remainingAccounts(deathAccounts(await this.state()))
      .preInstructions(finalizeBudget(deathAccounts(await this.state())))
      .signers([signer])
      .rpc({ commitment: "confirmed" });
  }
  async claimEscrow(signer: Keypair) {
    const queue = deathAccounts(await this.state());
    return program.methods
      .claimEscrow()
      .accountsStrict({
        dm: signer.publicKey,
        session: this.acc.session,
        escrow: this.acc.escrow,
        map: this.fixture.map,
        sessionAuthority,
        config,
        mapProgram: MAP,
        characterProgram: CHARACTER,
      })
      .remainingAccounts(queue)
      .preInstructions(finalizeBudget(queue))
      .signers([signer])
      .rpc();
  }
  state() {
    return program.account.gameSession.fetch(this.acc.session) as Promise<any>;
  }
  part(who: number) {
    return program.account.participation.fetch(this.players[who].participation) as Promise<any>;
  }
  monsters() {
    return program.account.monsterState.fetch(this.acc.monsters) as Promise<any>;
  }
  character(who: number) {
    return characterProgram.account.character.fetch(this.players[who].character) as Promise<any>;
  }

  /**
   * Plays until the party ends or `steps` transactions ran. Players shoot their ranged skill at the
   * nearest living monster (walking closer when out of range) with `playerSeed`; monsters walk to
   * and strike `huntSlot` (or any living player) with `monsterSeed(slot)`; `passive` players only
   * end their turn.
   */
  async play(opts: {
    playerSeed: Buffer;
    monsterSeed: (slot: number) => Buffer;
    huntSlot?: number;
    /** Players go for this monster while it lives (else the nearest). */
    focus?: number;
    passive?: boolean;
    steps?: number;
    until?: (s: any, monsters: any[]) => boolean;
  }) {
    const dirs = [
      [N, 0, -1],
      [E, 1, 0],
      [S, 0, 1],
      [W, -1, 0],
    ];
    for (let step = 0; step < (opts.steps ?? 400); step++) {
      await this.settle();
      const s = await this.state();
      if (s.core.status.active && s.startedSeq !== s.turnSeq) {
        await this.begin();
        continue;
      }
      const all = (await this.monsters()).core;
      const ms = all.monsters.slice(0, all.count);
      if (!s.core.status.active || opts.until?.(s, ms)) return s;
      const unit = s.core.turnOrder[s.core.turnIndex];
      // One attack or active skill per turn: after it, the unit ends its turn.
      const acted = s.actedSeq === s.turnSeq;
      if (unit < MONSTER_BASE) {
        const pl = await this.part(unit);
        const ap = pl.apSeq === s.turnSeq ? pl.core.actionPoints : pl.maxAp;
        const living = ms.map((m: any, i: number) => ({ ...m, i })).filter((m: any) => m.alive);
        const target =
          living.find((m: any) => m.i === opts.focus) ??
          living.sort((a: any, b: any) => cheb(pl.core, a) - cheb(pl.core, b))[0];
        // The ranged skill when it is off cooldown, else the weapon (a strike) from next to the target.
        const ranged = RANGED_SLOT[this.players[unit].kind];
        const [slot, reach] = pl.cooldowns[ranged] === 0 ? [ranged, 6] : [BASE_ATTACK, 1];
        if (opts.passive || !target || ap < 1 || acted) {
          await this.endTurn(this.players[unit].wallet, unit);
        } else if (cheb(pl.core, target) <= reach && cheb(pl.core, target) >= 1 && ap >= 2) {
          await this.act(unit, this.players[unit].wallet, slot, target.i);
          await this.consume(unit, opts.playerSeed);
        } else if (!(await this.stepToward(unit, pl.core, target, dirs, cheb))) {
          await this.endTurn(this.players[unit].wallet, unit);
        }
      } else {
        const mi = unit - MONSTER_BASE;
        const m = ms[mi];
        const alive = this.players.map((_, i) => i).filter((i) => s.aliveMask & (1 << i));
        const slot = opts.huntSlot !== undefined && alive.includes(opts.huntSlot) ? opts.huntSlot : alive[0];
        const pos = s.playerPos[slot];
        const range = m.bestiaryId === SHAMAN ? 6 : 1;
        if (acted) {
          await this.endTurn(this.dm, null);
        } else if (cheb(m, pos) <= range && m.actionPoints >= 2) {
          await this.monsterAct(this.dm, mi, slot);
          await this.consume(slot, opts.monsterSeed(slot));
        } else if (m.actionPoints >= 1 && (await this.stepToward(null, m, pos, dirs, cheb, mi))) {
          // stepped
        } else {
          await this.endTurn(this.dm, null);
        }
      }
    }
    return this.state();
  }

  async stepToward(who: number | null, from: any, to: any, dirs: number[][], cheb: any, monster?: number) {
    // Grid steps lower the Manhattan distance; a diagonal approach never lowers Chebyshev.
    const manhattan = (a: any, b: any) => Math.abs(a.x - b.x) + Math.abs(a.y - b.y);
    const options = dirs
      .map(([d, dx, dy]) => ({ d, cell: { x: from.x + dx, y: from.y + dy } }))
      .filter((o) => manhattan(o.cell, to) < manhattan(from, to))
      .sort((a, b) => cheb(a.cell, to) - cheb(b.cell, to));
    for (const o of options) {
      try {
        if (who === null) await this.monsterMove(this.dm, monster!, [o.d]);
        else await this.move(who, this.players[who].wallet, [o.d]);
        await this.settle();
        return true;
      } catch {
        // blocked or occupied: try the next direction
      }
    }
    return false;
  }
}

describe("deadroll_session: lobby, auth and movement on the demo map", { skip: SKIP_PARTY }, () => {
  let party: Party, stranger: Keypair;
  let p0Token: PublicKey, p1Token: PublicKey;

  before(async () => {
    party = await Party.create(["warrior", "mage"]);
    stranger = await funded(1);
  });

  it("open_session: DM only, minimum price, then opens and spawns monsters", async () => {
    const { dm, dmActor, fixture } = party;
    await rejects(openSession(party.players[0].wallet, fixture.map, fixture.layout, PRICE, dmActor.publicKey).rpc(), "NotDm");
    await rejects(openSession(dm, fixture.map, fixture.layout, MIN_PRICE.subn(1), dmActor.publicKey).rpc(), "PriceTooLow");
    await party.open().rpc({ commitment: "confirmed" });
    const s = await party.state();
    assert.ok(s.core.status.lobby);
    assert.deepEqual([s.entrance.x, s.entrance.y], [ENTRANCE.x, ENTRANCE.y]);
    const m = await party.monsters();
    assert.equal(m.core.count, DEMO_SPAWNS.length);
    assert.deepEqual([m.core.monsters[4].x, m.core.monsters[4].y, m.core.monsters[4].bestiaryId], [12, 12, CHIEFTAIN]);
    await rejects(party.open().rpc(), "already in use");
  });

  it("join_session: owner only, pays only the missing part; the character is not locked yet", async () => {
    const [p0, p1] = party.players;
    await rejects(party.join(0, p1.wallet, PRICE), "NotCharacterOwner");
    const before = await connection.getBalance(party.acc.escrow);
    await party.join(0, p0.wallet, PRICE.muln(3));
    assert.equal((await connection.getBalance(party.acc.escrow)) - before, PRICE.toNumber());
    await party.join(1, p1.wallet, PRICE);
    const [a, b] = await Promise.all([party.part(0), party.part(1)]);
    assert.ok(a.contribution.eq(PRICE) && b.contribution.eqn(0));
    // HP, gear and skills come with Ready.
    assert.equal(a.core.hp, 0);
    assert.deepEqual(a.equipment, worn({}));
    assert.deepEqual(a.skills, new Array(6).fill(NO_SKILL));
    assert.equal((await party.character(0)).core.inSession, null);
    // Until Ready the gear can still change.
    await unequip(p0.wallet, p0.character, BODY);
    await equip(p0.wallet, p0.character, 1, BODY);
  });

  it("set_ready: checks the map selection, locks the character, copies HP, gear and skills; unready unlocks", async () => {
    const [p0, p1] = party.players;
    await rejects(party.ready(0, p0.actor, true), "NotPlayer");
    await rejects(party.ready(0, p0.wallet, false), "ReadyUnchanged");
    // docs/SKILL_BUILDER.md 13: every category full, own skills only, no repeats.
    for (const bad of [
      [0, 1, 2, NO_SKILL, 4, 5],
      [0, 1, 2, 3, NO_SKILL, 5],
      [0, 1, 2, 3, 5, 4],
      [0, 1, 2, 2, 4, 5],
      [0, 1, 2, 9, 4, 5],
    ]) {
      await rejects(party.ready(0, p0.wallet, true, bad), "InvalidSkillSelection");
    }
    // The SkillSet of another character: the character program checks its seeds.
    await rejects(party.ready(0, p0.wallet, true, FULL_SELECTION, { skillSet: p1.skillSet }), "ConstraintSeeds");
    await rejects(party.ready(0, p0.wallet, true, FULL_SELECTION, { character: p1.character }), "InvalidCharacter");

    const order = [3, 1, 0, 2, 4, 5];
    const sig = await party.ready(0, p0.wallet, true, order);
    const ev = await eventOf(sig, "playerReady");
    assert.deepEqual([ev.slot, ev.ready, ev.skills], [0, true, order]);
    const a = await party.part(0);
    const c = await party.character(0);
    assert.deepEqual(a.skills, order);
    assert.equal(a.core.hp, c.core.hpMax);
    assert.equal(a.hpMax, c.core.hpMax);
    assert.ok(a.maxAp >= 1);
    // The worn starter kits, copied from the characters.
    assert.deepEqual(a.equipment, worn({ [MAIN_HAND]: SWORD, [BODY]: LEATHER }));
    assert.ok(c.core.inSession.equals(party.acc.session));
    // From here until the party ends the bag and the gear are frozen.
    await rejects(unequip(p0.wallet, p0.character, BODY), "AlreadyLocked");
    await rejects(party.ready(0, p0.wallet, true), "ReadyUnchanged");

    // Unready: unlocked again, the selection is gone.
    await party.ready(0, p0.wallet, false);
    assert.equal((await party.character(0)).core.inSession, null);
    assert.deepEqual((await party.part(0)).skills, new Array(6).fill(NO_SKILL));
    await unequip(p0.wallet, p0.character, BODY);
    await equip(p0.wallet, p0.character, 1, BODY);
  });

  it("start_session", async () => {
    const [p0, p1] = party.players;
    await party.ready(0, p0.wallet, true);
    await rejects(party.start(party.dm), "NotAllReady");
    await party.ready(1, p1.wallet, true);
    assert.deepEqual((await party.part(1)).equipment, worn({ [MAIN_HAND]: STAFF, [BODY]: ROBE }));
    await rejects(party.start(party.dmActor), "NotDm");
    await party.start(party.dm);
    let s = await party.state();
    assert.ok(s.core.status.active && s.core.pendingVrf);
    assert.equal(s.turnOrderLen, 0);
    assert.deepEqual([s.pending.attacker, s.pending.target], [EMPTY_UNIT, EMPTY_UNIT]);
    assert.ok(s.pending.participation.equals(party.players[0].participation));
    // Registry snapshot: speed and initiative per monster, tile costs (Wall impassable).
    assert.ok(s.monsterSpeed.slice(0, DEMO_SPAWNS.length).every((v: number) => v >= 1));
    assert.equal(s.tileCost[TILE.Wall], 0);
    // Nobody acts before the initiative roll.
    await rejects(party.move(0, party.players[0].wallet, [S]), "RollPending");
    await rejects(party.start(party.dm), "NotInLobby");
  });

  it("initiative: one VRF value orders every unit; InitiativeRolled shows the dice", async () => {
    await rejects(party.consume(0, CRIT, await funded(1)), "ConstraintAddress");
    await rejects(party.consume(1, CRIT), "InvalidPendingAccounts");
    const units = await party.initiativeUnits();
    assert.equal(units.length, 2 + DEMO_SPAWNS.length);
    const { sig, seed } = await party.rollInitiative();
    const expected = initiativeOrder(seed, units);
    const ev = await eventOf(sig, "initiativeRolled");
    assert.equal(ev.len, units.length);
    assert.equal(ev.actionCounter, 0);
    assert.deepEqual(Buffer.from(ev.seed), seed);
    assert.deepEqual(ev.order.slice(0, ev.len), expected.order);
    assert.deepEqual(ev.d20.slice(0, ev.len), expected.d20);
    assert.equal((await eventOf(sig, "turnStarted")).unit, 0);
    const s = await party.state();
    assert.ok(!s.core.pendingVrf);
    assert.equal(s.turnOrderLen, units.length);
    assert.deepEqual(s.core.turnOrder.slice(0, units.length), expected.order);
    assert.equal(s.core.turnOrder[units.length], EMPTY_UNIT);
    assert.deepEqual([s.core.turnIndex, s.core.round, s.turnSeq, s.core.actionCounter], [0, 1, 1, 1]);
    await rejects(party.consume(0, seed), "NoRollPending");
  });

  it("start_turn: every turn starts first, sent by anyone with the current unit's accounts", async () => {
    const [p0, p1] = party.players;
    // Nothing else runs on a turn that has not started (its over-time damage cannot be dodged).
    await rejects(party.move(0, p0.wallet, [S]), "TurnNotStarted");
    await rejects(party.act(0, p0.wallet, BASE_ATTACK, 0), "TurnNotStarted");
    await rejects(party.endTurn(p0.wallet, 0), "TurnNotStarted");
    await rejects(party.flee(0, p0.wallet), "TurnNotStarted");
    await rejects(party.startTurn(null), "ParticipationMissing");
    await rejects(party.startTurn(1), "NotYourTurn");
    await rejects(party.startTurn(0, { character: p1.character }), "InvalidCharacter");
    await rejects(party.startTurn(0, { character: null }), "InvalidCharacter");
    await rejects(party.startTurn(0, { sessionAuthority: stranger.publicKey }), "ConstraintAddress");
    await rejects(party.startTurn(0, { config: stranger.publicKey }), "ConstraintAddress");
    await rejects(party.startTurn(0, { characterProgram: MAP }), "ConstraintAddress");
    // The provider wallet sends it: not in the party, and that is fine.
    const sig = await party.startTurn(0);
    const ev = await eventOf(sig, "turnTicked");
    assert.deepEqual(
      [ev.unit, ev.turnSeq, ev.dotDamage, ev.died, ev.skipped, ev.expired],
      [0, 1, 0, false, false, 0],
    );
    const p = await party.part(0);
    assert.equal(ev.hp, p.core.hp);
    assert.ok(ev.cooldowns.every((c: number) => c === 0));
    assert.deepEqual(ev.states, p.core.states);
    const s = await party.state();
    assert.equal(s.startedSeq, s.turnSeq);
    // Action points are granted at the start.
    assert.deepEqual([p.apSeq, p.core.actionPoints], [s.turnSeq, p.maxAp]);
    await rejects(party.startTurn(0), "TurnAlreadyStarted");
  });

  it("auth: main wallet or bound session key with a live token from that wallet", async () => {
    const [p0, p1] = party.players;
    await rejects(party.move(0, stranger, [S]), "Unauthorized");
    await rejects(party.move(0, p0.actor, [S]), "SessionTokenMissing");
    const foreign = await issueToken(p0.actor, p1.wallet);
    await rejects(party.move(0, p0.actor, [S], foreign), "InvalidSessionToken");
    const now = (await connection.getBlockTime(await connection.getSlot()))!;
    const expired = await issueToken(p0.actor, p0.wallet, { validUntil: now - 60 });
    await rejects(party.move(0, p0.actor, [S], expired), "SessionTokenExpired");
    await revokeToken(expired, p0.wallet.publicKey);
    p0Token = await issueToken(p0.actor, p0.wallet);
    p1Token = await issueToken(p1.actor, p1.wallet);
    await rejects(party.move(0, p1.actor, [S], p1Token), "Unauthorized");
  });

  it("move_unit: walls, occupancy and action points", async () => {
    const key = party.players[0].actor;
    await rejects(party.move(1, party.players[1].wallet, [S]), "NotYourTurn");
    await rejects(party.move(0, key, [N], p0Token), "CellBlocked"); // wall ring
    await rejects(party.move(0, key, [E, W], p0Token), "CellOccupied"); // player 1 on the entrance
    await rejects(party.move(0, key, [E, E, E, E, E, E, E, E], p0Token), "NotEnoughActionPoints");
    await party.move(0, key, [S], p0Token);
    const p = await party.part(0);
    assert.deepEqual([p.core.x, p.core.y, p.core.actionPoints], [1, 2, p.maxAp - 1]);
  });
});

describe("deadroll_session: combat, death, victory and payouts", { skip: SKIP_PARTY }, () => {
  let party: Party;

  before(async () => {
    party = await Party.create(["warrior", "mage"]);
    await party.startAll();
  });

  it("act: turn, range, skill and target checks", async () => {
    const [p0, p1] = party.players;
    await rejects(party.act(1, p1.wallet, RANGED_SLOT.mage, 0), "NotYourTurn");
    await rejects(party.act(0, p1.wallet, RANGED_SLOT.warrior, 0), "Unauthorized");
    await rejects(party.act(0, p0.wallet, 0, 0), "OutOfRange"); // Piercing Thrust is a strike
    await rejects(party.act(0, p0.wallet, BASE_ATTACK, 0), "OutOfRange");
    // Only the active skills taken onto the map (set_ready); a reactive one is not an action.
    await rejects(party.act(0, p0.wallet, REACTIVE_SLOT, 0), "SkillNotSelected");
    await rejects(party.act(0, p0.wallet, 9, 0), "SkillNotSelected");
    await rejects(party.act(0, p0.wallet, RANGED_SLOT.warrior, 50), "InvalidTarget");
  });

  it("act requests a roll; nothing moves until the VRF callback resolves the hit", async () => {
    const p0 = party.players[0];
    const before = (await party.monsters()).core.monsters[0].hp;
    await party.act(0, p0.wallet, RANGED_SLOT.warrior, 0); // wolf at (6,2), distance 5
    let s = await party.state();
    assert.ok(s.core.pendingVrf);
    assert.deepEqual([s.pending.attacker, s.pending.target, s.pending.skillSlot], [0, MONSTER_BASE, RANGED_SLOT.warrior]);
    assert.equal((await party.part(0)).core.actionPoints, (await party.part(0)).maxAp - 2);
    await rejects(party.move(0, p0.wallet, [S]), "RollPending");
    await rejects(party.endTurn(p0.wallet, 0), "RollPending");
    await rejects(party.act(0, p0.wallet, RANGED_SLOT.warrior, 0), "RollPending");
    await rejects(party.flee(0, p0.wallet), "RollPending");
    await rejects(party.startTurn(0), "RollPending");

    const impostor = await funded(1);
    await rejects(party.consume(0, CRIT, impostor), "ConstraintAddress");
    await rejects(party.consume(1, CRIT), "InvalidPendingAccounts");

    const sig = await party.consume(0, CRIT);
    const hit = await eventOf(sig, "hitResolved");
    // The app's shape: session, attacker, target, seed, actionCounter, result (HitResult).
    assert.ok(hit.session.equals(party.acc.session));
    assert.deepEqual([hit.attacker, hit.target, hit.actionCounter], [0, MONSTER_BASE, 1]);
    assert.deepEqual(Buffer.from(hit.seed), CRIT);
    const r = hit.result;
    assert.ok(r.outcome.hit);
    assert.equal(r.d20, 20);
    assert.ok(r.crit);
    assert.ok(r.damage >= 1);
    assert.ok(r.packetCount >= 1 && r.rollsUsed >= 2);
    // What the worn items put into the hit: the warrior's sword, nothing on the wolf.
    assert.ok(hit.items.attacker.weapon.sword !== undefined);
    assert.equal(hit.items.defender.weapon, null);
    assert.equal(hit.items.defender.armor, 0);
    const wolf = (await party.monsters()).core.monsters[0];
    assert.equal(wolf.hp, Math.max(0, before - r.damage));
    s = await party.state();
    assert.ok(!s.core.pendingVrf);
    assert.equal(s.core.actionCounter, 2);
    assert.deepEqual(Buffer.from(s.core.lastSeed), CRIT);
    await rejects(party.consume(0, CRIT), "NoRollPending");
  });

  it("one attack or active skill per turn; moving spends the AP left", async () => {
    const p0 = party.players[0];
    await rejects(party.act(0, p0.wallet, RANGED_SLOT.warrior, 0), "AlreadyActed");
    await rejects(party.act(0, p0.wallet, BASE_ATTACK, 0), "AlreadyActed");
    await rejects(party.act(0, p0.wallet, 0, 0), "AlreadyActed");
    await party.move(0, p0.wallet, [S]);
    const p = await party.part(0);
    assert.deepEqual([p.core.x, p.core.y, p.core.actionPoints], [1, 2, p.maxAp - 3]);
    assert.equal((await party.state()).actedSeq, (await party.state()).turnSeq);
    await party.endTurn(p0.wallet, 0);
  });

  it("a miss deals nothing; the next unit may act again", async () => {
    const p1 = party.players[1];
    await party.begin();
    // Wolf 0 (distance 5) may have died of the crit; wolf 1 is at distance 6.
    const ms = (await party.monsters()).core.monsters;
    const wolf = [0, 1].find((i) => ms[i].alive)!;
    await party.act(1, p1.wallet, RANGED_SLOT.mage, wolf); // Frost Bolt
    const sig = await party.consume(1, MISS);
    const hit = await eventOf(sig, "hitResolved");
    assert.ok(hit.result.outcome.miss);
    assert.deepEqual([hit.result.d20, hit.result.damage], [1, 0]);
    assert.equal((await party.monsters()).core.monsters[wolf].hp, ms[wolf].hp);
    await rejects(party.act(1, p1.wallet, BASE_ATTACK, wolf), "AlreadyActed");
  });

  it("monster_act: DM on that monster's turn, target in range", async () => {
    const [p0, p1] = party.players;
    await rejects(party.monsterAct(party.dm, 0, 0), "NotYourTurn");
    await party.endTurn(p1.wallet, 1);
    const s = await party.state();
    const wolfTurn = s.core.turnOrder[s.core.turnIndex] - MONSTER_BASE;
    // A monster's turn starts first too (no participation needed).
    await rejects(party.monsterMove(party.dm, wolfTurn, [S]), "TurnNotStarted");
    await rejects(party.monsterAct(party.dm, wolfTurn, 0), "TurnNotStarted");
    await rejects(party.endTurn(party.dm, null), "TurnNotStarted");
    await rejects(party.startTurn(0), "NotYourTurn");
    const ev = await eventOf(await party.startTurn(null), "turnTicked");
    assert.deepEqual([ev.unit, ev.skipped], [MONSTER_BASE + wolfTurn, false]);
    await rejects(party.monsterAct(p0.wallet, wolfTurn, 0), "Unauthorized");
    // Patch 6: action 1 + slot; 8 is past the 7 slots, 7 is the passive slot (not an action).
    await rejects(party.monsterAct(party.dm, wolfTurn, 0, null, 8), "InvalidAction");
    await rejects(party.monsterAct(party.dm, wolfTurn, 0, null, 7), "MonsterSkillMissing");
    await rejects(party.monsterAct(party.dm, wolfTurn, 0), "OutOfRange");
  });

  it("a monster attacks once per turn too", async () => {
    const s = await party.play({
      playerSeed: MISS,
      monsterSeed: () => MISS,
      passive: true,
      until: (st, ms) => {
        const unit = st.core.turnOrder[st.core.turnIndex];
        if (unit < MONSTER_BASE || st.actedSeq === st.turnSeq) return false;
        const m = ms[unit - MONSTER_BASE];
        return m.actionPoints >= 2 && targetInRange(st, m) !== undefined;
      },
    });
    const mi = s.core.turnOrder[s.core.turnIndex] - MONSTER_BASE;
    const slot = targetInRange(s, (await party.monsters()).core.monsters[mi])!;
    await party.monsterAct(party.dm, mi, slot);
    await party.consume(slot, MISS);
    await rejects(party.monsterAct(party.dm, mi, slot), "AlreadyActed");
    await party.endTurn(party.dm, null);
  });

  it("monsters kill the mage: tombstone through character.mark_dead", async () => {
    const s = await party.play({ playerSeed: MISS, monsterSeed: (slot) => (slot === 1 ? CRIT : MISS), huntSlot: 1, passive: true, steps: 400, until: (st) => (st.aliveMask & 0b10) === 0 });
    assert.ok(s.core.status.active, "the warrior still lives");
    assert.equal(s.aliveMask & 0b10, 0);
    assert.equal((await party.part(1)).core.hp, 0);
    const c = await party.character(1);
    assert.ok(!c.core.alive);
    assert.ok(c.core.inSession.equals(party.acc.session), "a tombstone stays locked to its last session");
    assert.equal(c.core.stateVersion, 1);
    // docs/CHARACTER.md 11: the soulbound starter items vanish with the character.
    assert.equal(c.gear.itemCount, 0);
    assert.ok(c.gear.equipped.every((r: any) => r.source.empty !== undefined));
    // The skills burn: no mechanics left, the names stay as the tombstone's memorial.
    const mage = party.players[1];
    const set = await characterProgram.account.skillSet.fetch(mage.skillSet);
    assert.equal(set.count, MAGE_SKILLS.length);
    for (const [i, k] of set.skills.slice(0, set.count).entries()) {
      assert.equal(k.powerBp, 0);
      assert.equal(k.blocks.aspectCount, 0);
      assert.equal(Buffer.from(k.name).toString("ascii").replace(/\0+$/, ""), MAGE_SKILLS[i]![0]);
    }
    // The bag burns.
    const bag = await characterProgram.account.bag.fetch(mage.bag);
    assert.ok(bag.resources.every((r: number) => r === 0));
    assert.equal(bag.itemCount, 0);
    await rejects(unequip(mage.wallet, mage.character, BODY), "CharacterDead");
  });

  it("killing the goblin chieftain wins the party", async () => {
    await rejects(party.claimRewards(0), "NotFinished");
    await rejects(party.claimEscrow(party.dm), "NotFinished");
    const s = await party.play({ playerSeed: CRIT, monsterSeed: () => MISS, focus: CHIEFTAIN_INDEX, steps: 2000 });
    assert.ok(s.core.status.victory, "boss killed");
    assert.ok(s.xpPool > 0);
    assert.deepEqual(Buffer.from(s.victorySeed), Buffer.from(s.core.lastSeed));
    const m = await party.monsters();
    assert.ok(!m.core.monsters[4].alive);
  });

  let p1ClaimToken: PublicKey;
  it("claim_rewards: only the owner or their session key", async () => {
    const [p0, p1] = party.players;
    await rejects(party.claimRewards(0, p1.wallet), "NotRewardOwner");
    await rejects(party.claimRewards(0, party.dm), "NotRewardOwner");
    p1ClaimToken = await issueToken(p1.actor, p1.wallet);
    await rejects(party.claimRewards(0, p1.actor, p1ClaimToken), "NotRewardOwner");
    await rejects(party.claimRewards(0, p0.actor), "SessionTokenMissing");
    await rejects(party.claimRewards(0, p0.actor, await issueToken(p0.actor, p1.wallet)), "InvalidSessionToken");
    assert.equal((await party.state()).settledMask, 0, "nothing settled by a rejected claim");
  });

  it("claim_rewards: XP and boss loot for the living, nothing for the dead, once each", async () => {
    const before = await party.character(0);
    const sig = await party.claimRewards(0); // the owner's main wallet
    const ev = await eventOf(sig, "rewardsClaimed");
    const s = await party.state();
    assert.equal(ev.xp, s.xpPool);
    assert.ok(ev.resources.reduce((a: number, b: number) => a + b, 0) >= 2);
    const after = await party.character(0);
    assert.equal(after.core.xp, before.core.xp + s.xpPool);
    assert.equal(after.core.inSession, null);
    const bag = await characterProgram.account.bag.fetch(party.players[0].bag);
    assert.deepEqual(bag.resources, ev.resources);
    const stash = (await program.account.tempStash.fetch(party.players[0].stash)) as any;
    assert.equal(stash.xp, 0);
    await rejects(party.claimRewards(0), "AlreadySettled");

    const p1 = party.players[1];
    const dead = await eventOf(await party.claimRewards(1, p1.actor, p1ClaimToken), "rewardsClaimed"); // the owner's session key
    assert.equal(dead.xp, 0);
    assert.ok(!(await party.character(1)).core.alive);
    // A tombstone spends nothing.
    await rejects(spendStatPoints(p1.wallet, p1.character, [1, 0, 0, 0]), "CharacterDead");
  });

  it("claim_escrow: DM main wallet only, once; the map is finished", async () => {
    await rejects(party.claimEscrow(party.players[0].wallet), "NotDm");
    await rejects(party.claimEscrow(party.dmActor), "NotDm");
    const before = await connection.getBalance(party.dm.publicKey);
    await party.claimEscrow(party.dm);
    const gained = (await connection.getBalance(party.dm.publicKey)) - before;
    assert.ok(gained > PRICE.toNumber() - 10_000, `DM got ${gained}`);
    const map = await mapProgram.account.map.fetch(party.fixture.map);
    assert.ok(map.core.status.finished);
    await rejects(party.claimEscrow(party.dm), "AlreadyClaimed");
  });
});

describe("character.spend_stat_points after a level-up", { skip: SKIP_PARTY }, () => {
  // A level 2 chieftain is worth 100 XP: exactly level 2 (PlayerTuning.level_xp), 2 stat points.
  let party: Party;

  before(async () => {
    party = await Party.create(["warrior", "mage"], { bossLevel: 2 });
    await party.startAll();
  });

  it("not while the character is locked in the party", async () => {
    const p0 = party.players[0];
    await rejects(spendStatPoints(p0.wallet, p0.character, [1, 0, 0, 0]), "AlreadyLocked");
  });

  it("the victory levels the survivors up: points to spend and the level's HP", async () => {
    const s = await party.play({ playerSeed: CRIT, monsterSeed: () => MISS, focus: CHIEFTAIN_INDEX, steps: 2000 });
    assert.ok(s.core.status.victory, "boss killed");
    assert.ok(s.xpPool >= 100 && s.xpPool < 300, `xp ${s.xpPool}`);
    await party.claimRewards(0);
    const c = await party.character(0);
    assert.deepEqual([c.core.level, c.unspentPoints], [2, 2]);
    // Bran: warrior base 30 + Endurance 8 x 2 + one level x 6.
    assert.equal(c.core.hpMax, 30 + 8 * 2 + 6);
  });

  it("only the owner's main wallet; not more than unspent; not nothing", async () => {
    const [p0, p1] = party.players;
    await rejects(spendStatPoints(p1.wallet, p0.character, [1, 0, 0, 0]), "NotOwner");
    await rejects(spendStatPoints(p0.actor, p0.character, [1, 0, 0, 0]), "NotOwner");
    await rejects(spendStatPoints(p0.wallet, p0.character, [2, 1, 0, 0]), "NotEnoughStatPoints");
    await rejects(spendStatPoints(p0.wallet, p0.character, [0, 0, 0, 0]), "NoPointsSpent");
  });

  it("the owner spends every point at once; Endurance raises the stored HP", async () => {
    const p0 = party.players[0];
    const before = await party.character(0);
    const sig = await spendStatPoints(p0.wallet, p0.character, [1, 0, 0, 1]);
    const tx = await connection.getTransaction(sig, { commitment: "confirmed", maxSupportedTransactionVersion: 0 });
    const events = [...new EventParser(characterProgram.programId, characterProgram.coder).parseLogs(tx?.meta?.logMessages ?? [])];
    const ev = events.find((x) => x.name.toLowerCase() === "statpointsspent")?.data as any;
    assert.ok(ev, "no statPointsSpent event");
    const c = await party.character(0);
    assert.deepEqual(c.core.stats, [11, 7, 5, 9]);
    assert.equal(c.unspentPoints, 0);
    assert.equal(c.core.hpMax, 30 + 9 * 2 + 6);
    assert.equal(c.core.stateVersion, before.core.stateVersion + 1);
    assert.deepEqual([ev.points, ev.stats, ev.unspentPoints, ev.hpMax], [[1, 0, 0, 1], [11, 7, 5, 9], 0, c.core.hpMax]);
    await rejects(spendStatPoints(p0.wallet, p0.character, [1, 0, 0, 0]), "NotEnoughStatPoints");
  });
});

describe("deadroll_session: flee and wipe", { skip: SKIP_PARTY }, () => {
  it("flee needs every living player; any other action resets the intents", async () => {
    const party = await Party.create(["warrior", "mage"]);
    await party.startAll();
    const [p0, p1] = party.players;
    await rejects(party.flee(1, p1.wallet), "NotYourTurn");
    await party.flee(0, p0.wallet); // spends the turn
    let s = await party.state();
    assert.deepEqual([s.fleeMask, s.core.turnIndex], [0b01, 1]);
    await party.begin(); // not an action: the intent stays
    assert.equal((await party.state()).fleeMask, 0b01);
    await party.move(1, p1.wallet, [S]); // another action: intents reset
    assert.equal((await party.state()).fleeMask, 0);
    await party.endTurn(p1.wallet, 1);
    for (let i = 0; i < DEMO_SPAWNS.length; i++) {
      await party.begin();
      await party.endTurn(party.dm, null);
    }
    await party.begin();
    await party.flee(0, p0.wallet);
    await party.begin();
    await party.flee(1, p1.wallet);
    s = await party.state();
    assert.ok(s.core.status.fled);
    await rejects(party.flee(0, p0.wallet), "NotActive");
    await rejects(party.startTurn(0), "NotActive");

    const ev = await eventOf(await party.claimRewards(0), "rewardsClaimed");
    assert.equal(ev.xp, 0);
    assert.equal((await party.character(0)).core.inSession, null, "unlocked, the cache is abandoned");
    await party.claimEscrow(party.dm);
  });

  it("a wiped party ends; the DM is still paid", async () => {
    const party = await Party.create(["mage"]);
    await party.startAll();
    const s = await party.play({ playerSeed: MISS, monsterSeed: () => CRIT, passive: true, steps: 300 });
    assert.ok(s.core.status.wiped);
    assert.equal(s.aliveMask, 0);
    await party.claimRewards(0);
    assert.ok((await party.character(0)).core.inSession !== null);
    await party.claimEscrow(party.dm);
  });
});

describe("deadroll_session: turn start, cooldowns and over-time damage", { skip: SKIP_PARTY }, () => {
  const DIRS = [
    [N, 0, -1],
    [E, 1, 0],
    [S, 0, 1],
    [W, -1, 0],
  ];
  /** `deadroll_combat::states::state_bit(StateKind::Freeze)`. */
  const FREEZE_BIT = 1 << 2;
  const [FROST_BOLT, FROST_BURN, ICE_PRISON] = [0, 1, 2];
  const MAGE = 1;
  const WOLF0 = 0;
  /** Dice that leave the next turn start's over-time damage lethal score this; else the lower
   * the HP left the better, with a DoT on first; a kill by the hit itself is refused. */
  const LETHAL_DOT = 1e9;
  const dotKillScore = (hp: number) => (r: any) => {
    const left = hp - r.damage;
    if (left <= 0) return -Infinity;
    const dot = dotOf(r.states);
    if (dot >= left) return LETHAL_DOT;
    return dot > 0 ? 1e6 - left : -left;
  };

  it("cooldowns: SkillUsed; a skill misses exactly its next own turns, other slots and the weapon never", async () => {
    const party = await Party.create(["warrior", "mage"]);
    await party.startAll();
    const p0 = party.players[0];
    const javelin = RANGED_SLOT.warrior;
    const sig = await party.act(0, p0.wallet, javelin, WOLF0); // cooldown 1
    const used = await eventOf(sig, "skillUsed");
    assert.deepEqual([used.unit, used.skillSlot, used.cooldown], [0, javelin, 1]);
    assert.equal(used.cooldowns[javelin], 2);
    assert.equal(used.cooldowns.filter((c: number) => c > 0).length, 1);
    await party.consume(0, MISS);
    assert.equal((await party.part(0)).cooldowns[javelin], 2);

    // Next own turn: counted down to 1, blocked; the other slots and the weapon are not.
    await party.untilStartOf(0);
    let tick = await eventOf(await party.begin(), "turnTicked");
    assert.equal(tick.cooldowns[javelin], 1);
    await rejects(party.act(0, p0.wallet, javelin, WOLF0), "SkillOnCooldown");
    await rejects(party.act(0, p0.wallet, 0, WOLF0), "OutOfRange"); // Piercing Thrust: a strike
    await rejects(party.act(0, p0.wallet, BASE_ATTACK, WOLF0), "OutOfRange");

    // The turn after: usable again.
    await party.untilStartOf(0);
    tick = await eventOf(await party.begin(), "turnTicked");
    assert.equal(tick.cooldowns[javelin], 0);
    await party.act(0, p0.wallet, javelin, WOLF0);
    await party.consume(0, MISS);
  });

  describe("hard control and over-time damage on a monster", () => {
    let party: Party;
    before(async () => {
      party = await Party.create(["warrior", "mage"]);
      await party.startAll();
      await party.endTurn(party.players[0].wallet, 0);
      await party.begin();
    });

    it("Freeze skips the wolf's turn on its own; its Burning ticks at the start; durations run out", async () => {
      const p1 = party.players[MAGE];
      let wolf = (await party.monsters()).core.monsters[WOLF0];
      await party.act(MAGE, p1.wallet, FROST_BOLT, WOLF0);
      // The lightest chill that still lands: two harder hits leave the wolf too low for the
      // freezing Frost Burn to spare it.
      const light = (ok: boolean, damage: number) => (ok ? 10_000 - damage : -Infinity);
      const chilled = await party.consumeBest(
        MAGE,
        (r) => light(r.damage < wolf.hp && hasState(r.states, "chill"), r.damage),
        400,
        10_000 - 2,
      );
      assert.ok(chilled, "no VRF value chills the wolf");

      await party.untilStartOf(MAGE);
      await party.begin();
      wolf = (await party.monsters()).core.monsters[WOLF0];
      assert.ok(hasState(wolf.states, "chill"), "Chill lasts 3 of the wolf's turns");
      await rejects(party.act(MAGE, p1.wallet, FROST_BOLT, WOLF0), "SkillOnCooldown");
      // Patch 9: the wolf's Cold resistance cuts the damage only, so one Chill is a full
      // buildup (about 7000) under the 10000 freeze threshold and the next one freezes. The bolt
      // is on cooldown this turn; Frost Burn is not.
      const chillPower = (states: any[]) =>
        states.find((x) => x.turns > 0 && x.kind.chill !== undefined)?.power ?? 0;
      assert.ok(chillPower(wolf.states) < 10_000, "one Chill does not freeze");
      await party.act(MAGE, p1.wallet, FROST_BURN, WOLF0); // Cold + Fire: the second Chill freezes
      const frozen = await party.consumeWhere(
        MAGE,
        (r) =>
          hasState(r.states, "freeze") && hasState(r.states, "burning") && wolf.hp - r.damage > dotOf(r.states),
        1500,
      );
      assert.ok(frozen, "no VRF value freezes and burns the wolf");
      const hp = (await party.monsters()).core.monsters[WOLF0].hp;
      const burn = dotOf(frozen.result.states);

      await party.untilStartOf(MONSTER_BASE + WOLF0);
      const evs = await eventsOf(await party.begin());
      const [tick] = eventsNamed(evs, "turnTicked");
      assert.deepEqual(
        [tick.unit, tick.skipped, tick.died, tick.dotDamage, tick.hp],
        [MONSTER_BASE + WOLF0, true, false, burn, hp - burn],
      );
      assert.ok(tick.expired & FREEZE_BIT, "Freeze lasts one turn");
      assert.ok(!hasState(tick.states, "freeze") && hasState(tick.states, "burning"));
      const [skip] = eventsNamed(evs, "turnSkipped");
      assert.ok(skip.state.freeze !== undefined);
      assert.deepEqual([skip.unit, skip.turnSeq], [MONSTER_BASE + WOLF0, tick.turnSeq]);
      // The turn ended on its own: the next unit's turn began and waits for its own start.
      const [next] = eventsNamed(evs, "turnStarted");
      assert.ok(next && next.unit !== MONSTER_BASE + WOLF0);
      assert.equal(next.turnSeq, tick.turnSeq + 1);
      const s = await party.state();
      assert.equal(s.core.turnOrder[s.core.turnIndex], next.unit);
      assert.notEqual(s.startedSeq, s.turnSeq);
      const after = (await party.monsters()).core.monsters[WOLF0];
      assert.deepEqual([after.hp, after.alive], [hp - burn, true]);
      await rejects(party.monsterAct(party.dm, WOLF0, 0), "NotYourTurn");
    });

    it("over-time damage kills a monster like a hit: MonsterKilled, XP to the pool, the turn moves on", async () => {
      const p1 = party.players[MAGE];
      for (let round = 0; round < 20; round++) {
        await party.untilStartOf(MAGE);
        await party.begin();
        const p = await party.part(MAGE);
        const wolf = (await party.monsters()).core.monsters[WOLF0];
        assert.ok(wolf.alive);
        const slot = [FROST_BURN, FROST_BOLT, ICE_PRISON].find((i) => p.cooldowns[i] === 0);
        if (slot === undefined) continue;
        await party.act(MAGE, p1.wallet, slot, WOLF0);
        // Stats raise the mage's hits, so fewer values leave the wolf alive with enough Burning.
        const sent = await party.consumeBest(MAGE, dotKillScore(wolf.hp), 600, LETHAL_DOT);
        assert.ok(sent, "every VRF value kills the wolf outright");
        if (sent.score < LETHAL_DOT) continue;

        const left = wolf.hp - sent.result.damage;
        await party.untilStartOf(MONSTER_BASE + WOLF0);
        const xp = (await party.state()).xpPool;
        const evs = await eventsOf(await party.begin());
        const [tick] = eventsNamed(evs, "turnTicked");
        assert.deepEqual([tick.died, tick.skipped, tick.hp], [true, false, 0]);
        assert.ok(tick.dotDamage >= left);
        const [killed] = eventsNamed(evs, "monsterKilled");
        assert.deepEqual([killed.monster, killed.by, killed.boss], [WOLF0, MAGE, false]);
        assert.ok(killed.xp > 0);
        assert.equal((await party.state()).xpPool, xp + killed.xp);
        assert.equal(eventsNamed(evs, "turnStarted").length, 1, "the dead wolf's turn ended");
        const s = await party.state();
        assert.ok(s.core.status.active);
        assert.notEqual(s.core.turnOrder[s.core.turnIndex], MONSTER_BASE + WOLF0);
        assert.ok(!(await party.monsters()).core.monsters[WOLF0].alive);
        return;
      }
      assert.fail("no lethal over-time damage in 20 rounds");
    });
  });

  it("over-time damage kills a player like a hit: PlayerDied, tombstone, the turn moves on", async () => {
    /**
     * The wolf's Ragged Bite on the mage (patch 6: a monster skill; Blood bleeds), scored for a
     * death by over-time damage: a hit whose Burning
     * or Bleeding ticks (power each own turn start, `turns` times) cover the HP it leaves is the
     * one wanted; else a DoT hit that keeps more HP than the ticks take (never 1 HP without a DoT:
     * every later hit would kill outright); else a miss over a plain hit.
     */
    const dotOverTime = (states: any[]) =>
      states
        .filter((x) => x.turns > 0 && (x.kind.burning !== undefined || x.kind.bleeding !== undefined))
        .reduce((sum, x) => sum + x.power * x.turns, 0);
    const playerDotScore = (hp: number) => (r: any) => {
      const left = hp - r.damage;
      if (left <= 0) return -Infinity;
      const total = dotOverTime(r.states);
      if (total >= left) return LETHAL_DOT;
      if (total > 0 && left - total >= 2) return 1e6 - left;
      return -r.damage;
    };
    const party = await Party.create(["warrior", "mage"]);
    await party.startAll();
    // Wolf 0 chose Lunge, Hamstring, Ragged Bite (slots 0, 1, 2).
    const BITER = 0;
    const RAGGED_BITE = 2;
    const biterUnit = MONSTER_BASE + BITER;
    for (let step = 0; step < 800; step++) {
      await party.settle();
      const s = await party.state();
      assert.ok(s.core.status.active);
      const cur = s.core.turnOrder[s.core.turnIndex];
      if (s.startedSeq !== s.turnSeq) {
        const hp = cur === MAGE ? (await party.part(MAGE)).core.hp : 0;
        const evs = await eventsOf(await party.begin());
        const [tick] = eventsNamed(evs, "turnTicked");
        if (cur !== MAGE || !tick.died) continue;
        assert.deepEqual([tick.unit, tick.hp, tick.skipped], [MAGE, 0, false]);
        assert.ok(tick.dotDamage >= hp);
        const [died] = eventsNamed(evs, "playerDied");
        assert.deepEqual([died.slot, died.by], [MAGE, biterUnit]);
        assert.ok(died.character.equals(party.players[MAGE].character));
        const after = await party.state();
        assert.equal(after.aliveMask & (1 << MAGE), 0);
        assert.ok(after.core.status.active, "the warrior still lives");
        assert.notEqual(after.core.turnOrder[after.core.turnIndex], MAGE);
        assert.ok(!(await party.character(MAGE)).core.alive, "tombstone through character.mark_dead");
        return;
      }
      if (cur < MONSTER_BASE) {
        await party.endTurn(party.players[cur].wallet, cur);
        continue;
      }
      // While the mage bleeds the wolf holds its bite: the ticks finish the job.
      const bleeding = dotOverTime((await party.part(MAGE)).core.states) > 0;
      const m = await party.monsters();
      if (cur !== biterUnit || s.actedSeq === s.turnSeq || bleeding) {
        await party.endTurn(party.dm, null);
        continue;
      }
      const wolf = m.core.monsters[BITER];
      const pos = s.playerPos[MAGE];
      if (cheb(wolf, pos) > 1 || wolf.actionPoints < 2 || m.skillCooldowns[BITER][RAGGED_BITE] > 0) {
        const moved =
          cheb(wolf, pos) > 1 &&
          wolf.actionPoints >= 1 &&
          (await party.stepToward(null, wolf, pos, DIRS, cheb, BITER));
        if (!moved) await party.endTurn(party.dm, null);
        continue;
      }
      await party.monsterAct(party.dm, BITER, MAGE, null, skillAction(RAGGED_BITE));
      const hp = (await party.part(MAGE)).core.hp;
      assert.ok(await party.consumeBest(MAGE, playerDotScore(hp), 400, LETHAL_DOT), "every VRF value kills the mage");
    }
    assert.fail("the mage never died of over-time damage");
  });
});

describe("deadroll_session: support and control skills", { skip: SKIP_PARTY }, () => {
  const DIRS = [
    [N, 0, -1],
    [E, 1, 0],
    [S, 0, 1],
    [W, -1, 0],
  ];
  const [WARRIOR, MAGE, SUPPORT] = [0, 1, 2];
  const ICE_PRISON = 2;
  const BP = 10_000;
  /** `deadroll_combat::support::mod_bit(ModKind::Haste)`. */
  const HASTE_BIT = 1 << 0;
  let party: Party;

  const modOf = (mods: any[], kind: string) => mods.find((m) => m.turns > 0 && m.kind[kind] !== undefined);
  const effectOf = (events: { name: string; data: any }[]) => eventsNamed(events, "effectResolved")[0];
  /** A living monster `kind` (or any) in projectile reach of player `who`. */
  async function monsterInReach(who: number, kind?: number) {
    const pos = (await party.part(who)).core;
    const all = (await party.monsters()).core;
    const i = all.monsters
      .slice(0, all.count)
      .findIndex((m: any) => m.alive && (kind === undefined || m.bestiaryId === kind) && cheb(m, pos) >= 1 && cheb(m, pos) <= 6);
    assert.ok(i >= 0, "no monster in reach");
    return i;
  }

  before(async () => {
    party = await Party.create(["warrior", "mage", "support"]);
    await party.startAll();
  });

  it("act: heals and buffs take allies, debuffs and attacks take enemies", async () => {
    await party.skipTo(SUPPORT);
    const [p0, p1, p2] = party.players;
    const w = p2.wallet;
    // Slot 3 never joined; ally accounts must belong to the target slot.
    await rejects(party.act(SUPPORT, w, QUICKEN, 3), "InvalidAllyTarget");
    await rejects(party.act(SUPPORT, w, QUICKEN, WARRIOR), "AllyAccountMissing");
    await rejects(party.act(SUPPORT, w, QUICKEN, WARRIOR, null, p1.participation), "InvalidAllyTarget");
    // A self-cast takes no ally account, enemy actions never do.
    await rejects(party.act(SUPPORT, w, QUICKEN, SUPPORT, null, p2.participation), "UnexpectedAllyAccount");
    await rejects(party.act(SUPPORT, w, SAP, 0, null, p0.participation), "UnexpectedAllyAccount");
    await rejects(party.act(SUPPORT, w, BASE_ATTACK, 0, null, p0.participation), "UnexpectedAllyAccount");
    await rejects(party.act(SUPPORT, w, SAP, 50), "InvalidTarget");
    // Reactive and passive skills are not actions.
    await rejects(party.act(SUPPORT, w, 4, SUPPORT), "SkillNotSelected");
    const s = await party.state();
    assert.equal(s.actedSeq === s.turnSeq, false, "nothing was used");
  });

  it("Haste on the caster: more AP on its next 3 own turns (2 + 1 from Endurance), then it runs out", async () => {
    const before = await party.part(SUPPORT);
    await party.act(SUPPORT, party.players[SUPPORT].wallet, QUICKEN, SUPPORT);
    assert.ok((await party.state()).pendingTarget.equals(PublicKey.default), "a self-cast has no ally account");
    const sig = await party.consume(SUPPORT, nextSeed());
    const fx = await eventOf(sig, "effectResolved");
    assert.deepEqual([fx.caster, fx.target, fx.rider], [SUPPORT, SUPPORT, false]);
    assert.ok(fx.effect.haste !== undefined);
    // Buffs do not roll to land: no d20, a crit roll and the d100 point in the skill's range.
    assert.ok(fx.roll.outcome.hit !== undefined && fx.roll.d20 === 0);
    assert.ok(fx.applied.added !== undefined);
    const amount = fx.roll.amount;
    assert.ok(amount >= 3_000 && amount <= 9_000, `haste ${amount}`);
    const haste = modOf((await party.part(SUPPORT)).mods, "haste");
    // Own buff on self: Senna's Endurance 5 / PlayerTuning.self_buff_end_div 5 = 1 more own turn, so
    // mod_turns(2 + 1) = 4 (the running turn plus three).
    assert.deepEqual([haste.turns, haste.powerBp, haste.source], [4, amount, SUPPORT]);
    assert.ok(!(await party.state()).core.pendingVrf);

    const hasted = Math.min(255, Math.max(1, Math.floor((before.maxAp * (BP + amount)) / BP)));
    assert.ok(hasted > before.maxAp);
    for (const [turnsLeft, ap, expired] of [
      [3, hasted, 0],
      [2, hasted, 0],
      [1, hasted, 0],
      [0, before.maxAp, HASTE_BIT],
    ]) {
      await party.endTurn(party.players[SUPPORT].wallet, SUPPORT);
      await party.untilStartOf(SUPPORT);
      const ticked = await eventOf(await party.begin(), "modsTicked");
      assert.deepEqual([ticked.unit, ticked.actionPoints, ticked.expired], [SUPPORT, ap, expired]);
      const p = await party.part(SUPPORT);
      assert.equal(p.core.actionPoints, ap);
      assert.equal(modOf(p.mods, "haste")?.turns ?? 0, turnsLeft);
    }
  });

  it("Shelter protects an ally on the same cell; the ally comes with the party accounts", async () => {
    await party.skipTo(SUPPORT);
    const ally = party.players[WARRIOR].participation;
    await party.act(SUPPORT, party.players[SUPPORT].wallet, SHELTER, WARRIOR, null, ally);
    // Patch 5: no `pending_target`, the ally is one of the party accounts after the fixed list.
    assert.ok((await party.state()).pendingTarget.equals(PublicKey.default));
    const seed = nextSeed();
    await rejects((await party.consumeTx(SUPPORT, seed, MOCK_VRF, null, [])).rpc(), "InvalidPartyAccounts");
    const party3 = await party.partyOf(SUPPORT);
    // In the wrong order, or read-only: refused too.
    await rejects((await party.consumeTx(SUPPORT, seed, MOCK_VRF, null, [...party3.slice(2), ...party3.slice(0, 2)])).rpc(), "InvalidPartyAccounts");
    const readOnly = party3.map((a) => ({ ...a, isWritable: false }));
    await rejects((await party.consumeTx(SUPPORT, seed, MOCK_VRF, null, readOnly)).rpc(), "InvalidPartyAccounts");
    const fx = await eventOf(await party.consume(SUPPORT, seed), "effectResolved");
    assert.deepEqual([fx.caster, fx.target], [SUPPORT, WARRIOR]);
    assert.ok(fx.effect.protection !== undefined && fx.applied.added !== undefined);
    const shelter = modOf((await party.part(WARRIOR)).mods, "protection");
    assert.deepEqual([shelter.powerBp, shelter.source], [fx.roll.amount, SUPPORT]);
    await party.endTurn(party.players[SUPPORT].wallet, SUPPORT);
  });

  it("Sap rolls to land: a natural 1 misses; Weakening counterweighs the wolf's next hit", async () => {
    await party.skipTo(SUPPORT);
    const wolf = await monsterInReach(SUPPORT, WOLF);
    await party.act(SUPPORT, party.players[SUPPORT].wallet, SAP, wolf);
    const miss = effectOf((await party.simulateConsume(SUPPORT, MISS))!);
    assert.ok(miss.roll.outcome.miss !== undefined && miss.roll.d20 === 1);
    assert.ok(miss.applied.none !== undefined && miss.roll.amount === 0);
    assert.equal(miss.roll.rollsUsed, 1, "a miss stops after the d20");

    const fx = await eventOf(await party.consume(SUPPORT, CRIT), "effectResolved");
    assert.ok(fx.effect.weakening !== undefined && fx.roll.d20 === 20 && fx.roll.crit);
    assert.ok(fx.applied.added !== undefined);
    const weak = modOf((await party.monsters()).mods[wolf], "weakening");
    assert.deepEqual([weak.turns, weak.powerBp, weak.source], [3, fx.roll.amount, SUPPORT]);
    assert.ok(modOf(fx.mods, "weakening"), "the event shows the wolf's modifiers");

    // The wolf's own turn: it walks up to a player and bites; the hit carries the Weakening.
    await party.endTurn(party.players[SUPPORT].wallet, SUPPORT);
    await party.untilStartOf(MONSTER_BASE + wolf);
    await party.begin();
    for (let i = 0; i < 8; i++) {
      const m = (await party.monsters()).core.monsters[wolf];
      const s = await party.state();
      const slot = targetInRange(s, m);
      if (slot !== undefined) {
        await party.monsterAct(party.dm, wolf, slot);
        const hit = await eventOf(await party.consume(slot, CRIT), "hitResolved");
        assert.equal(hit.attacker, MONSTER_BASE + wolf);
        assert.equal(hit.result.packets[0].weakenBp, fx.roll.amount);
        return;
      }
      const moved = await party.stepToward(null, m, s.playerPos[WARRIOR], DIRS, cheb, wolf);
      assert.ok(moved, "the wolf cannot reach the party");
    }
    assert.fail("the wolf never got in reach");
  });

  it("Ice Prison: the skill's Freeze rolls after the hit on the same seed and takes the target's turn", async () => {
    await party.skipTo(MAGE);
    const target = await monsterInReach(MAGE);
    await party.act(MAGE, party.players[MAGE].wallet, ICE_PRISON, target);
    const { events } = await party.consumeEventsWhere(MAGE, (ev) => {
      const fx = effectOf(ev);
      return fx?.rider && fx.applied.froze !== undefined;
    });
    const hit = eventsNamed(events, "hitResolved")[0];
    const fx = effectOf(events);
    assert.ok(hit.result.outcome.hit !== undefined);
    assert.equal(fx.actionCounter, hit.actionCounter, "one action, one VRF value");
    assert.ok(fx.seed.every((b: number, i: number) => b === hit.seed[i]));
    assert.ok(fx.effect.freeze !== undefined && fx.rider);
    // The effect's dice follow the hit's: a chance roll unless the hit crit.
    assert.equal(fx.roll.chanceRoll === null, hit.result.crit);
    assert.ok(hasState(fx.states, "freeze"));
    assert.ok(hasState((await party.monsters()).core.monsters[target].states, "freeze"));

    await party.endTurn(party.players[MAGE].wallet, MAGE);
    await party.untilStartOf(MONSTER_BASE + target);
    const skipped = await eventOf(await party.begin(), "turnSkipped");
    assert.equal(skipped.unit, MONSTER_BASE + target);
    assert.ok(skipped.state.freeze !== undefined);
  });

  it("a dead ally cannot be healed or buffed", async () => {
    const s = await party.play({
      playerSeed: MISS,
      monsterSeed: (slot) => (slot === MAGE ? CRIT : MISS),
      huntSlot: MAGE,
      passive: true,
      steps: 400,
      until: (st) => (st.aliveMask & (1 << MAGE)) === 0,
    });
    assert.equal(s.aliveMask & (1 << MAGE), 0, "the mage died");
    await party.skipTo(SUPPORT);
    const cooldowns = (await party.part(SUPPORT)).cooldowns;
    const buff = [QUICKEN, SHELTER].find((slot) => cooldowns[slot] === 0);
    assert.ok(buff !== undefined);
    const dead = party.players[MAGE].participation;
    await rejects(party.act(SUPPORT, party.players[SUPPORT].wallet, buff, MAGE, null, dead), "InvalidAllyTarget");
    await rejects(party.act(SUPPORT, party.players[SUPPORT].wallet, buff, MAGE), "InvalidAllyTarget");
  });
});

describe("deadroll_session: equipment from the character's gear", { skip: SKIP_PARTY }, () => {
  let party: Party;
  const [ARMORLESS, BARE] = [0, 1];

  before(async () => {
    party = await Party.create(["warrior", "warrior"]);
    const p = party.players[ARMORLESS];
    await unequip(p.wallet, p.character, BODY);
    const b = party.players[BARE];
    await unequip(b.wallet, b.character, BODY);
    await unequip(b.wallet, b.character, MAIN_HAND);
    await party.startAll();
  });

  it("set_ready copies what is worn; nothing worn means bare hands", async () => {
    const [a, b] = await Promise.all([party.part(ARMORLESS), party.part(BARE)]);
    assert.deepEqual(a.equipment, worn({ [MAIN_HAND]: SWORD }));
    assert.deepEqual(b.equipment, worn({}));
    // The starter kit adds no HP: both start at the stored maximum.
    assert.equal(a.hpMax, (await party.character(ARMORLESS)).core.hpMax);
    assert.equal(b.hpMax, (await party.character(BARE)).core.hpMax);
  });

  it("hits carry the items of both sides", async () => {
    await party.skipTo(BARE);
    const p = party.players[BARE];
    await party.act(BARE, p.wallet, RANGED_SLOT.warrior, 0);
    const hit = await eventOf(await party.consume(BARE, CRIT), "hitResolved");
    assert.equal(hit.attacker, BARE);
    assert.equal(hit.items.attacker.weapon, null, "bare hands");
    assert.equal(hit.items.attacker.armor, 0);
  });
});

// ---------------------------------------------------------------------------------------------
// Patch 4: resonances that reach beyond their target (docs/DAMAGE_TYPES_AND_MIXES.md 6.4)

/** Compute units the transaction `sig` used (the VRF callback's budget is the default 200k). */
async function unitsOf(sig: string): Promise<number> {
  const tx = await connection.getTransaction(sig, { commitment: "confirmed", maxSupportedTransactionVersion: 0 });
  return tx?.meta?.computeUnitsConsumed ?? 0;
}

describe("deadroll_session: resonances on the map", { skip: SKIP_PARTY }, () => {
  const FROST_BURN = 1;
  const MAGE = 1;
  const [WOLF0, WOLF1] = [0, 1];
  const N = 0;

  it("Blasting Flame (Fire with the staff's Blunt tint) knocks the wolf back and sets the cells around it on fire", async () => {
    const party = await Party.create(["warrior", "mage"]);
    await party.startAll();
    await party.endTurn(party.players[0].wallet, 0);
    await party.begin();
    const mage = party.players[MAGE];
    // The second wolf at (7, 3): seen from the entrance, the cell behind it is free.
    const before = (await party.monsters()).core.monsters[WOLF1];
    await party.act(MAGE, mage.wallet, FROST_BURN, WOLF1);

    // The callback must get the session's own layout.
    const other = await createFinalizedMap(party.dm);
    const a = {
      session: party.acc.session,
      monsters: party.acc.monsters,
      participation: mage.participation,
      character: mage.character,
      skillSet: mage.skillSet,
      bag: mage.bag,
      layout: other.layout,
    };
    await rejects(consume(MOCK_VRF, a, CRIT).rpc(), "InvalidLayout");

    const done = await party.consumeWhere(MAGE, (r) => r.knockback > 0 && r.ignitePower > 0 && r.damage < before.hp, 2_000);
    assert.ok(done, "no VRF value triggers Blasting Flame");
    const spread = await eventOf(done.sig, "resonanceSpread");
    assert.equal(spread.moved, true, "the wolf is pushed one cell away");
    const after = (await party.monsters()).core.monsters[WOLF1];
    assert.deepEqual([after.x, after.y], [spread.movedTo.x, spread.movedTo.y]);
    assert.equal(cheb(after, before), 1);
    assert.ok(spread.ignitedCount > 0);
    const state = await party.monsters();
    const burning = state.burningCells.filter((c: any) => c.power > 0);
    assert.equal(burning.length, spread.ignitedCount);
    console.log(`CU consume_randomness, Blasting Flame (knockback + ${spread.ignitedCount} burning cells): ${await unitsOf(done.sig)}`);

    // The wolf steps out on its turn and catches fire.
    await party.untilStartOf(MONSTER_BASE + WOLF1);
    await party.begin();
    const sig = await party.monsterMove(party.dm, WOLF1, [N]);
    const burned = (await eventsOf(sig)).find((x) => x.name.toLowerCase() === "cellburned");
    assert.ok(burned, "stepping into a burning cell writes CellBurned");
    const wolf = (await party.monsters()).core.monsters[WOLF1];
    assert.ok(hasState(wolf.states, "burning"));
  });

  it("Stormfire (Fire with the race's Lightning tint) passes Burning to the adjacent wolf", async () => {
    const party = await Party.create(["warrior", "stormMage"]);
    await party.startAll();
    await party.endTurn(party.players[0].wallet, 0);
    await party.begin();
    const mage = party.players[MAGE];
    const wolf = (await party.monsters()).core.monsters[WOLF0];
    await party.act(MAGE, mage.wallet, FROST_BURN, WOLF0);
    const done = await party.consumeWhere(MAGE, (r) => r.jumpCount > 0 && r.damage < wolf.hp, 3_000);
    assert.ok(done, "no VRF value makes a state jump");
    const spread = await eventOf(done.sig, "resonanceSpread");
    assert.ok(spread.hitCount > 0);
    const hit = spread.hits[0];
    assert.equal(hit.monster, WOLF1, "the wolf next to the target");
    const other = (await party.monsters()).core.monsters[WOLF1];
    assert.ok(other.states.some((x: any) => x.turns > 0 && Object.keys(x.kind)[0] === Object.keys(hit.kind)[0]));
    console.log(`CU consume_randomness, a jump (${Object.keys(hit.kind)[0]} to wolf ${hit.monster}): ${await unitsOf(done.sig)}`);
  });
});

// ---------------------------------------------------------------------------------------------
// Patch 5: reactive and passive skills, auras, the line target, AP costs (docs/SKILL_BUILDER.md)

describe("deadroll_session: reactive and passive skills, auras", { skip: SKIP_PARTY }, () => {
  // A full party: three party accounts come with every callback.
  const [WARRIOR, MAGE, SUPPORT, STORM] = [0, 1, 2, 3];
  const WOLF0 = 0;
  let party: Party;

  before(async () => {
    party = await Party.create(["warrior", "mage", "support", "stormMage"]);
    await party.startAll();
  });

  it("set_ready takes the passive skills as auras and the reactive skills into the session", async () => {
    const s = await party.state();
    const [iron, mind, ward] = [WARRIOR, MAGE, SUPPORT].map((slot) => s.auras[slot][0]);
    // Iron Skin and Cold Mind work on their bearer only; Cold Ward is a Protection aura, radius 2.
    assert.ok(iron.kind.protection !== undefined && iron.selfOnly && iron.passive && iron.turns === 255);
    assert.ok(mind.kind.empowerment !== undefined && mind.selfOnly);
    assert.ok(ward.kind.protection !== undefined && !ward.selfOnly && !ward.hostile && ward.radius === 2);
    assert.ok(iron.powerBp > 0 && ward.powerBp > 0, "a fixed point of the effect's range");
    // Nothing temporary yet.
    assert.ok(s.auras.every((a: any[]) => a[1].turns === 0));
    // Riposte (hit), Cold Snap (an enemy next to it), Last Stand (HP below 40%).
    assert.equal(s.reactives[WARRIOR].skillSlot, 4);
    assert.ok(s.reactives[WARRIOR].skill.blocks.trigger.hit !== undefined);
    assert.ok(s.reactives[MAGE].skill.blocks.trigger.enemyAdjacent !== undefined);
    assert.equal(s.reactives[SUPPORT].skill.ext.hpBelowPct, 40);
    assert.ok(s.reactives.slice(0, 3).every((r: any) => r.cooldownLeft === 0));
    // The participation keeps the map skills with their range and AP cost.
    const p = await party.part(WARRIOR);
    assert.deepEqual(
      p.mapSkills.slice(0, 4).map((m: any) => [Object.keys(m.ext.range)[0], m.apCost]),
      [
        ["melee", 2],
        ["melee", 3],
        ["ranged", 2],
        ["ranged", 3],
      ],
    );
    assert.equal(p.mapSkills[4].apCost, 0, "a reactive skill is not an action");
  });

  it("Cold Snap: a wolf stepping next to the mage stops there and the mage's reactive rolls", async () => {
    await party.untilStartOf(MONSTER_BASE + WOLF0);
    await party.begin();
    const before = (await party.monsters()).core.monsters[WOLF0];
    // From (6, 2) west: (2, 2) is the first cell next to the party on the entrance (1, 1).
    const sig = await party.monsterMove(party.dm, WOLF0, [W, W, W, W, W]);
    const moved = (await party.monsters()).core.monsters[WOLF0];
    assert.deepEqual([moved.x, moved.y], [2, 2], "the walk stops on the cell next to the mage");
    assert.equal(moved.actionPoints, before.actionPoints - 4, "the steps not taken cost nothing");
    const s = await party.state();
    assert.ok(s.core.pendingVrf);
    assert.equal(s.pendingReactive.kind, 1, "an enemy entered");
    // Both mages carry Cold Snap; the others' reactives answer something else.
    assert.equal(s.pendingReactive.bearers, (1 << MAGE) | (1 << STORM));
    assert.equal(s.pendingReactive.source, MONSTER_BASE + WOLF0);
    assert.ok((await eventsOf(sig)).some((x) => x.name.toLowerCase() === "actionrequested"));
    await rejects(party.monsterAct(party.dm, WOLF0, WARRIOR), "RollPending");

    // The callback for a reactive-only roll takes the first bearer's accounts.
    await rejects(party.consume(WARRIOR, CRIT), "InvalidPendingAccounts");
    const done = await party.consume(MAGE, CRIT);
    console.log(`CU consume_randomness, a reactive-only roll (Cold Snap on an entering wolf): ${await unitsOf(done)}`);
    const events = await eventsOf(done);
    const reactions = eventsNamed(events, "reactiveTriggered");
    assert.deepEqual(reactions.map((x) => x.bearer), [MAGE, STORM], "in slot order");
    const r = reactions[0];
    assert.deepEqual([r.bearer, r.skillSlot, r.target], [MAGE, 4, MONSTER_BASE + WOLF0]);
    assert.ok(r.trigger.enemyAdjacent !== undefined && r.reaction.debuff !== undefined);
    const fx = eventsNamed(events, "effectResolved")[0];
    assert.deepEqual([fx.caster, fx.target], [MAGE, MONSTER_BASE + WOLF0]);
    assert.ok(fx.effect.slow !== undefined && fx.roll.d20 === 20 && fx.applied.added !== undefined);
    const after = await party.state();
    assert.ok(!after.core.pendingVrf && after.pendingReactive.kind === 0);
    assert.equal(after.reactives[MAGE].cooldownLeft, 3, "cooldown 2: its next 2 own turns");
    assert.ok((await party.monsters()).mods[WOLF0].some((m: any) => m.turns > 0 && m.kind.slow !== undefined));
  });

  it("Riposte: the wolf's hit on the warrior triggers a counterattack on the same dice", async () => {
    // The wolf still has its turn and the AP for its bite.
    await party.monsterAct(party.dm, WOLF0, WARRIOR);
    const wolfHp = (await party.monsters()).core.monsters[WOLF0].hp;
    // A bite that lands and a counterattack that leaves the wolf alive (the next test needs it).
    const { sig: done } = await party.consumeEventsWhere(WARRIOR, (ev) => {
      const h = eventsNamed(ev, "hitResolved");
      return h.length === 2 && h[1].result.damage < wolfHp;
    });
    console.log(`CU consume_randomness, a hit + Riposte, 4 players (3 party accounts), Cold Ward aura: ${await unitsOf(done)}`);
    const events = await eventsOf(done);
    const hits = eventsNamed(events, "hitResolved");
    assert.equal(hits.length, 2, "the bite and the counterattack");
    assert.deepEqual([hits[0].attacker, hits[0].target], [MONSTER_BASE + WOLF0, WARRIOR]);
    assert.deepEqual([hits[1].attacker, hits[1].target], [WARRIOR, MONSTER_BASE + WOLF0]);
    assert.equal(hits[0].actionCounter, hits[1].actionCounter, "one action, one VRF value");
    // The bite sees the auras: Iron Skin and Cold Ward are both Protection, the stronger acts.
    const ward = (await party.state()).auras;
    const protection = Math.max(ward[WARRIOR][0].powerBp, ward[SUPPORT][0].powerBp);
    assert.ok(protection > 0 && hits[0].result.packets[0].resistBp > 0, "Protection is a resistance on every aspect");
    const r = eventsNamed(events, "reactiveTriggered")[0];
    assert.ok(r.trigger.hit !== undefined && r.reaction.strike !== undefined && r.bearer === WARRIOR);
    assert.equal((await party.state()).reactives[WARRIOR].cooldownLeft, 3);
  });

  it("Last Stand: the support's HP below 40% starts a temporary Protection aura", async () => {
    for (let i = 0; i < 40; i++) {
      const s = await party.state();
      const supp = await party.part(SUPPORT);
      if (s.auras[SUPPORT][1].turns > 0) break;
      assert.ok(supp.core.hp > 0, "the support died before its threshold");
      await party.untilStartOf(MONSTER_BASE + WOLF0);
      await party.begin();
      const m = (await party.monsters()).core.monsters[WOLF0];
      if (!m.alive) assert.fail("the wolf died first");
      const st = await party.state();
      if (cheb(m, st.playerPos[SUPPORT]) > 1) {
        await party.endTurn(party.dm, null);
        continue;
      }
      await party.monsterAct(party.dm, WOLF0, SUPPORT);
      const sig = await party.consume(SUPPORT, CRIT);
      const events = await eventsOf(sig);
      const r = eventsNamed(events, "reactiveTriggered").find((x) => x.bearer === SUPPORT);
      if (!r) continue;
      console.log(`CU consume_randomness, a hit + Last Stand aura: ${await unitsOf(sig)}`);
      assert.ok(r.trigger.hpBelow !== undefined && r.reaction.aura !== undefined && r.target === SUPPORT);
      const changed = eventsNamed(events, "auraChanged")[0];
      assert.deepEqual([changed.bearer, changed.auraSlot], [SUPPORT, 1]);
      assert.ok(changed.aura.kind.protection !== undefined && !changed.aura.passive && changed.aura.radius === 2);
      const hpMax = (await party.part(SUPPORT)).hpMax;
      assert.ok((await party.part(SUPPORT)).core.hp * 100 < 40 * hpMax);
      const aura = (await party.state()).auras[SUPPORT][1];
      assert.equal(aura.turns, 3, "Protection lasts 2 of its own turns");
      // It ends with its bearer's turns.
      for (const left of [2, 1, 0]) {
        await party.untilStartOf(SUPPORT);
        await party.begin();
        assert.equal((await party.state()).auras[SUPPORT][1].turns, left);
        await party.endTurn(party.players[SUPPORT].wallet, SUPPORT);
      }
      return;
    }
    assert.fail("Last Stand never triggered");
  });
});

describe("deadroll_session: the line target and AP costs", { skip: SKIP_PARTY }, () => {
  const [WARRIOR, MAGE] = [0, 1];
  const [FROST_BOLT, ICE_PRISON, ARC] = [0, 2, 3];
  const [WOLF0, WOLF1] = [0, 1];
  let party: Party;

  before(async () => {
    party = await Party.create(["warrior", "mage"]);
    await party.startAll();
    await party.skipTo(MAGE);
  });

  it("a skill costs its own AP: Ice Prison 3, Frost Bolt 2", async () => {
    const mage = party.players[MAGE];
    // Two steps east: the elf mage has 4 AP, 2 left.
    await party.move(MAGE, mage.wallet, [E, E]);
    assert.equal((await party.part(MAGE)).core.actionPoints, 2);
    await rejects(party.act(MAGE, mage.wallet, ICE_PRISON, WOLF0), "NotEnoughActionPoints");
    // Two more steps: (5, 1), diagonal to both wolves; nothing left for this turn.
    await party.move(MAGE, mage.wallet, [E, E]);
    await rejects(party.act(MAGE, mage.wallet, FROST_BOLT, WOLF0), "NotEnoughActionPoints");
    await party.endTurn(mage.wallet, MAGE);
  });

  it("Arc hits every wolf on the line through its target, in line order", async () => {
    await party.untilStartOf(MAGE);
    await party.begin();
    const mage = party.players[MAGE];
    const s = await party.state();
    assert.deepEqual([s.playerPos[MAGE].x, s.playerPos[MAGE].y], [5, 1]);
    const ms = (await party.monsters()).core.monsters;
    // (5, 1) -> (6, 2) -> (7, 3): both wolves on the diagonal.
    assert.deepEqual([ms[WOLF0].x, ms[WOLF0].y, ms[WOLF1].x, ms[WOLF1].y], [6, 2, 7, 3]);
    await party.act(MAGE, mage.wallet, ARC, WOLF0);
    assert.equal((await party.part(MAGE)).core.actionPoints, 1, "Arc costs 3 AP (a line)");
    const done = await party.consume(MAGE, CRIT);
    console.log(`CU consume_randomness, Arc on a line of 2 wolves: ${await unitsOf(done)}`);
    const hits = eventsNamed(await eventsOf(done), "hitResolved");
    assert.deepEqual(
      hits.map((h) => h.target),
      [MONSTER_BASE + WOLF0, MONSTER_BASE + WOLF1],
    );
    assert.ok(hits.every((h) => h.attacker === MAGE));
    // Each unit on the line rolls its own hit on the same VRF value: the second starts where the
    // first stopped.
    assert.ok(hits[1].result.rollsUsed > 0);
  });
});

// ---------------------------------------------------------------------------------------------
// Patch 6: monster skills from the registry (docs/BESTIARY.md 3, 5, 8.1, 12)

/** The registry table body (8-byte discriminator, 8-byte header, then the records). */
async function tableBody(address: PublicKey): Promise<Buffer> {
  const info = await connection.getAccountInfo(address);
  assert.ok(info, "the registry table is seeded");
  return info.data.subarray(16);
}

/** The demo roster with the boss pack next to the entrance (1, 1), on its diagonal: a line from
 * the party through the chieftain runs through the goblin warrior and the shaman. One goblin is
 * elite. Monster indices: wolves 0, 1; goblins 2 (elite), 3; chieftain 4; warrior 5; shaman 6. */
const CAMP: SpawnDef[] = [
  { id: WOLF, x: 10, y: 3, pack: 1, skills: 0b0111 },
  { id: WOLF, x: 11, y: 3, pack: 1, skills: 0b1110 },
  { id: GOBLIN, x: 8, y: 8, pack: 2, skills: 0b11, flags: 1 },
  { id: GOBLIN, x: 9, y: 8, pack: 2, skills: 0b11 },
  { id: CHIEFTAIN, x: 2, y: 2, pack: 3, skills: 0b11111 },
  { id: GOBLIN_WARRIOR, x: 3, y: 3, pack: 3, skills: 0b11 },
  { id: SHAMAN, x: 4, y: 4, pack: 3, skills: 0b111 },
];
const [C_WOLF0, C_ELITE, C_GOBLIN, C_CHIEF, C_WARRIOR, C_SHAMAN] = [0, 2, 3, 4, 5, 6];
/** `monster_act` action of a monster skill slot (0 = the natural attack). */
const skillAction = (slot: number) => 1 + slot;
/** Monster slots (`deadroll_rules::monster`): 0..5 active, 5 reactive, 6 passive. */
const [M_REACTIVE, M_PASSIVE] = [5, 6];
/** Registry monster_skills indices (assets/registry/monster_skills.json). */
const MS = { netThrow: 6, spinStrike: 7, daggerThrow: 8, leader: 9, battleCry: 10, spark: 11, mending: 12, quicken: 13 };

describe("deadroll_session: monster skills, the shaman and the chieftain (patch 6)", { skip: SKIP_PARTY }, () => {
  const [WARRIOR, MAGE] = [0, 1];
  const [FROST_BOLT, BLEEDING_CLEAVE] = [0, 3];
  let party: Party;

  before(async () => {
    party = await Party.create(["warrior", "mage"], { roster: CAMP });
    await party.startAll();
  });

  it("open_session copies the records and the chosen skills; Leader is an aura from the start; elite has more HP", async () => {
    const m = await party.monsters();
    const bestiaryBody = await tableBody(bestiary);
    CAMP.forEach((sp, i) => assert.deepEqual([...m.records[i]], [...bestiaryBody.subarray(sp.id * 48, sp.id * 48 + 48)], `record of monster ${i}`));
    // Wolves 0..4, goblins 4..6 (the warrior shares their pool), chieftain 6..11, shaman 11..14.
    assert.equal(m.skillCount, 14, "each chosen registry skill once");
    const skills = await tableBody(monsterSkills);
    const copyOf = (i: number) => [...skills.subarray(i * 32 + 1, i * 32 + 17)];
    const slots = m.skills[C_CHIEF];
    assert.deepEqual(
      [0, 1, 2, M_REACTIVE, M_PASSIVE].map((k) => [...m.skillTable[slots[k]]]),
      [MS.netThrow, MS.spinStrike, MS.daggerThrow, MS.battleCry, MS.leader].map(copyOf),
    );
    assert.deepEqual([slots[3], slots[4]], [NO_SKILL, NO_SKILL], "a regular boss slot count: 3 active of 5");
    const leader = m.auras[C_CHIEF][0];
    assert.ok(leader.kinOnly && leader.aura.kind.empowerment !== undefined && leader.aura.passive && leader.aura.turns === 255 && leader.aura.radius === 2);
    assert.equal(m.auras[C_CHIEF][1].aura.turns, 0, "Battle Cry waits for its trigger");
    // Elite: +50% HP (BalanceConfig elite_hp_bp), one more active slot (the goblin pool has 2).
    const [elite, plain] = [m.core.monsters[C_ELITE], m.core.monsters[C_GOBLIN]];
    assert.equal(elite.flags & 1, 1);
    assert.equal(elite.hpMax, plain.hpMax + plain.hpMax / 2);
  });

  it("Leader: the goblin warrior's Thrown Stone carries the chieftain's Empowerment", async () => {
    await party.skipTo(MONSTER_BASE + C_WARRIOR);
    const [DIRTY_STRIKE, THROWN_STONE] = [0, 1];
    // Dirty Strike is melee; the party stands 2 cells away.
    await rejects(party.monsterAct(party.dm, C_WARRIOR, WARRIOR, null, skillAction(DIRTY_STRIKE)), "OutOfRange");
    // The passive slot is not an action.
    await rejects(party.monsterAct(party.dm, C_WARRIOR, WARRIOR, null, skillAction(M_PASSIVE)), "MonsterSkillMissing");
    const sig = await party.monsterAct(party.dm, C_WARRIOR, WARRIOR, null, skillAction(THROWN_STONE));
    const used = await eventOf(sig, "skillUsed");
    assert.deepEqual([used.unit, used.skillSlot], [MONSTER_BASE + C_WARRIOR, THROWN_STONE]);
    const req = await eventOf(sig, "actionRequested");
    assert.equal(req.skillSlot, skillAction(THROWN_STONE));
    const done = await party.consumeWhere(WARRIOR, (r) => r.outcome.hit !== undefined || r.outcome.crit !== undefined);
    assert.ok(done, "no VRF value lands the stone");
    console.log(`CU consume_randomness, a monster skill hit (Thrown Stone, Leader aura): ${await unitsOf(done.sig)}`);
    const hit = await eventOf(done.sig, "hitResolved");
    assert.deepEqual([hit.attacker, hit.target], [MONSTER_BASE + C_WARRIOR, WARRIOR]);
    assert.ok(done.result.packets[0].increasedBp > 0, "Empowerment from the chieftain 1 cell away");
    assert.ok((await party.monsters()).skillCooldowns[C_WARRIOR][THROWN_STONE] > 0, "the skill is on cooldown");
    await party.endTurn(party.dm, null);
  });

  it("Battle Cry: a player line through three goblins takes the chieftain below 50%; a Protection aura for its kin", async () => {
    const monster = async (i: number) => (await party.monsters()).core.monsters[i];
    // Frost Bolts (Cold: the chieftain resists only Piercing) bring it close to its threshold
    // without crossing it, then the warrior's Bleeding Cleave crosses it.
    for (let i = 0; i < 30; i++) {
      const chief = await monster(C_CHIEF);
      if (chief.hp * 100 < 75 * chief.hpMax) break;
      await party.skipTo(MAGE);
      const pl = await party.part(MAGE);
      if (pl.cooldowns[FROST_BOLT] > 0) {
        await party.endTurn(party.players[MAGE].wallet, MAGE);
        continue;
      }
      await party.act(MAGE, party.players[MAGE].wallet, FROST_BOLT, C_CHIEF);
      const done = await party.consumeWhere(MAGE, (r) => r.damage > 0 && (chief.hp - r.damage) * 100 >= 55 * chief.hpMax);
      if (!done) await party.consume(MAGE, MISS);
      await party.endTurn(party.players[MAGE].wallet, MAGE);
    }
    for (let i = 0; i < 30; i++) {
      await party.skipTo(WARRIOR);
      const pl = await party.part(WARRIOR);
      if (pl.cooldowns[BLEEDING_CLEAVE] > 0) {
        await party.endTurn(party.players[WARRIOR].wallet, WARRIOR);
        continue;
      }
      await party.act(WARRIOR, party.players[WARRIOR].wallet, BLEEDING_CLEAVE, C_CHIEF);
      const warriorHp = (await monster(C_WARRIOR)).hp;
      const found = await party
        .consumeEventsWhere(WARRIOR, (ev) => {
          const hits = eventsNamed(ev, "hitResolved");
          return (
            eventsNamed(ev, "monsterReactiveTriggered").length > 0 &&
            hits.length === 3 &&
            hits[1].result.damage > 0 &&
            hits[1].result.damage < warriorHp &&
            eventsNamed(ev, "monsterKilled").length === 0
          );
        })
        .catch(() => null);
      if (!found) {
        await party.consume(WARRIOR, MISS);
        await party.endTurn(party.players[WARRIOR].wallet, WARRIOR);
        continue;
      }
      console.log(`CU consume_randomness, a player line on 3 goblins + Battle Cry + Leader: ${await unitsOf(found.sig)}`);
      const hits = eventsNamed(found.events, "hitResolved");
      assert.deepEqual(
        hits.map((h) => h.target),
        [C_CHIEF, C_WARRIOR, C_SHAMAN].map((x) => MONSTER_BASE + x),
        "in line order",
      );
      const r = eventsNamed(found.events, "monsterReactiveTriggered")[0];
      assert.deepEqual([r.bearer, r.skillSlot, r.target], [C_CHIEF, M_REACTIVE, MONSTER_BASE + C_CHIEF]);
      assert.ok(r.trigger.hpBelow !== undefined && r.reaction.aura !== undefined);
      const changed = eventsNamed(found.events, "monsterAuraChanged")[0];
      assert.deepEqual([changed.bearer, changed.auraSlot, changed.kinOnly], [C_CHIEF, 1, true]);
      assert.ok(changed.aura.kind.protection !== undefined && !changed.aura.passive && changed.aura.radius === 2);
      const m = await party.monsters();
      assert.ok(m.auras[C_CHIEF][1].aura.turns > 0 && m.auras[C_CHIEF][0].aura.turns === 255);
      const chief = m.core.monsters[C_CHIEF];
      assert.ok(chief.hp * 100 < 50 * chief.hpMax);
      return;
    }
    assert.fail("Battle Cry never triggered");
  });

  it("the shaman heals and hastes a goblin; a wolf (another kin) and a player cannot take them", async () => {
    const [SPARK, MENDING, QUICKEN] = [0, 1, 2];
    await party.skipTo(MONSTER_BASE + C_SHAMAN);
    const before = (await party.monsters()).core.monsters[C_WARRIOR];
    assert.ok(before.hp < before.hpMax, "the cleave wounded the goblin warrior");
    // Same kin only: a wolf is another kin.
    await rejects(party.monsterAct(party.dm, C_SHAMAN, C_WOLF0, null, skillAction(MENDING), WARRIOR), "NotSameKin");
    // The callback of a heal on a monster takes the lowest living player's accounts.
    await rejects(party.monsterAct(party.dm, C_SHAMAN, C_WARRIOR, null, skillAction(MENDING), MAGE), "InvalidPendingAccounts");
    // A damage skill takes a player: monster index 5 is no player slot.
    await rejects(party.monsterAct(party.dm, C_SHAMAN, C_WARRIOR, null, skillAction(SPARK), WARRIOR), "InvalidTarget");
    await rejects(party.monsterAct(party.dm, C_SHAMAN, 9, null, skillAction(MENDING), WARRIOR), "InvalidAllyTarget");
    await party.monsterAct(party.dm, C_SHAMAN, C_WARRIOR, null, skillAction(MENDING), WARRIOR);
    const sig = await party.consume(WARRIOR, CRIT);
    console.log(`CU consume_randomness, the shaman's Mending on a goblin: ${await unitsOf(sig)}`);
    const fx = await eventOf(sig, "effectResolved");
    assert.deepEqual([fx.caster, fx.target, fx.skillSlot], [MONSTER_BASE + C_SHAMAN, MONSTER_BASE + C_WARRIOR, MENDING]);
    assert.ok(fx.effect.heal !== undefined);
    const after = (await party.monsters()).core.monsters[C_WARRIOR];
    assert.ok(after.hp > before.hp && after.hp <= after.hpMax && fx.hp === after.hp, "healed, up to its maximum");
    await party.endTurn(party.dm, null);

    // Next own turn: Quicken on the warrior (Mending may still be on cooldown).
    await party.skipTo(MONSTER_BASE + C_SHAMAN);
    const cd = (await party.monsters()).skillCooldowns[C_SHAMAN];
    if (cd[MENDING] > 0) await rejects(party.monsterAct(party.dm, C_SHAMAN, C_WARRIOR, null, skillAction(MENDING), WARRIOR), "SkillOnCooldown");
    await party.monsterAct(party.dm, C_SHAMAN, C_WARRIOR, null, skillAction(QUICKEN), WARRIOR);
    const hasted = await party.consume(WARRIOR, CRIT);
    const fx2 = await eventOf(hasted, "effectResolved");
    assert.ok(fx2.effect.haste !== undefined && fx2.applied.added !== undefined);
    const mods = (await party.monsters()).mods[C_WARRIOR];
    assert.ok(mods.some((x: any) => x.turns > 0 && x.kind.haste !== undefined), "Haste on the goblin warrior");
    await party.endTurn(party.dm, null);
  });
});

describe("deadroll_session: a monster line puts several players down; start_turn finalizes them (patch 6)", { skip: SKIP_PARTY }, () => {
  // Mages on the entrance cell: the chieftain's Spin Strike (a melee line, 1 cell) from the cell
  // next to them hits them all.
  const SPIN_STRIKE = 1;

  /**
   * Chieftain turns until one Spin Strike puts exactly `want` players down (natural attacks and
   * strikes that put nobody down in between wear them down). Returns the callback's signature.
   */
  async function lineDown(party: Party, want: number) {
    for (let i = 0; i < 120; i++) {
      await party.skipTo(MONSTER_BASE + C_CHIEF, 60);
      const s = await party.state();
      const cd = (await party.monsters()).skillCooldowns[C_CHIEF];
      const target = party.players.findIndex((_, x) => s.aliveMask & (1 << x));
      const action = cd[SPIN_STRIKE] === 0 ? skillAction(SPIN_STRIKE) : 0;
      await party.monsterAct(party.dm, C_CHIEF, target, null, action);
      let pick: Buffer | null = null;
      let none: Buffer | null = null;
      for (let k = 0; k < (action ? 160 : 30) && !pick; k++) {
        const seed = nextSeed();
        const ev = await party.simulateConsume(target, seed);
        if (!ev) continue;
        const down = eventsNamed(ev, "playerDied").length;
        if (action && down === want) pick = seed;
        else if (down === 0 && !none) none = seed;
      }
      if (!pick) {
        await party.consume(target, none ?? MISS);
        await party.endTurn(party.dm, null);
        continue;
      }
      return party.consume(target, pick);
    }
    return assert.fail(`no Spin Strike put ${want} players down`);
  }

  /** After the callback: down at once, alive on the character account, nothing else goes. */
  async function assertPending(party: Party, sig: string, want: number) {
    const events = await eventsOf(sig);
    const hits = eventsNamed(events, "hitResolved");
    assert.ok(hits.length >= want && hits.every((h) => h.attacker === MONSTER_BASE + C_CHIEF));
    const down = eventsNamed(events, "playerDied").map((d) => d.slot);
    assert.equal(down.length, want);
    assert.equal(eventsNamed(events, "characterDied").length, 0, "no mark_dead in the callback");
    const s = await party.state();
    for (const slot of down) {
      assert.equal(s.aliveMask & (1 << slot), 0, "out of the battle at once");
      assert.ok(s.pendingDeaths & (1 << slot), "queued");
      assert.ok((await party.character(slot)).core.alive, "not finalized yet");
    }
    return down;
  }

  it("3 players, 2 down: nothing but start_turn goes until the deaths are final", async () => {
    const party = await Party.create(["mage", "mage", "mage"], { roster: CAMP });
    await party.startAll();
    const sig = await lineDown(party, 2);
    console.log(`CU consume_randomness, a monster line on 3 players, 2 down: ${await unitsOf(sig)}`);
    const down = await assertPending(party, sig, 2);
    const living = [0, 1, 2].find((x) => !down.includes(x))!;
    const pl = party.players[living];
    // Every action waits for the queue.
    await rejects(party.monsterAct(party.dm, C_CHIEF, living), "DeathsPending");
    await rejects(party.monsterMove(party.dm, C_CHIEF, [S]), "DeathsPending");
    await rejects(party.endTurn(party.dm, null), "DeathsPending");
    await rejects(party.act(living, pl.wallet, 0, C_CHIEF), "DeathsPending");
    await rejects(party.move(living, pl.wallet, [E]), "DeathsPending");
    await rejects(party.flee(living, pl.wallet), "DeathsPending");
    // Missing, swapped or foreign accounts are refused; the queue stays.
    const queue = deathAccounts(await party.state());
    await rejects(party.startTurn(null, {}, queue.slice(0, 3)), "InvalidDeathAccounts");
    const swapped = [...queue.slice(3), ...queue.slice(0, 3)];
    await rejects(party.startTurn(null, {}, swapped), "InvalidDeathAccounts");
    const foreign = [...queue];
    foreign[2] = { ...foreign[2], pubkey: pda([Buffer.from("bag"), pl.character.toBuffer()], CHARACTER) };
    await rejects(party.startTurn(null, {}, foreign), "InvalidDeathAccounts");
    // Anyone sends start_turn: the turn already started (the chieftain acted), so it only
    // finalizes; the chieftain's turn goes on.
    const fin = await party.startTurn(null);
    console.log(`CU start_turn finalizing 2 deaths: ${await unitsOf(fin)}`);
    const events = await eventsOf(fin);
    assert.deepEqual(eventsNamed(events, "deathFinalized").map((d) => d.slot), down);
    assert.equal(eventsNamed(events, "turnTicked").length, 0, "no new turn");
    const s = await party.state();
    assert.equal(s.pendingDeaths, 0);
    for (const slot of down) assert.ok(!(await party.character(slot)).core.alive, "tombstone through character.mark_dead");
    assert.equal(s.core.turnOrder[s.core.turnIndex], MONSTER_BASE + C_CHIEF);
    // Now the turn ends as usual; the down players get no turn and can't be targeted.
    await party.endTurn(party.dm, null);
    let targeted = false;
    const order = (await party.state()).core.turnOrder;
    for (let i = 0; i < 60 && !targeted; i++) {
      const st = await party.state();
      const unit = st.core.turnOrder[st.core.turnIndex];
      assert.ok(!down.includes(unit), "a down player has no turn");
      if (st.startedSeq !== st.turnSeq) await party.begin();
      else if (unit < MONSTER_BASE) await party.endTurn(party.players[unit].wallet, unit);
      else {
        if (unit === MONSTER_BASE + C_CHIEF && !targeted) {
          await rejects(party.monsterAct(party.dm, C_CHIEF, down[0]), "InvalidTarget");
          targeted = true;
        }
        await party.endTurn(party.dm, null);
      }
    }
    assert.ok(order.length > 0 && targeted);
  });

  it("4 players, 3 down at once; start_turn finalizes all three; a wipe is finalized by claim_rewards", async () => {
    const party = await Party.create(["mage", "mage", "mage", "mage"], { roster: CAMP });
    await party.startAll();
    const sig = await lineDown(party, 3);
    console.log(`CU consume_randomness, a monster line on 4 players, 3 down: ${await unitsOf(sig)}`);
    const down = await assertPending(party, sig, 3);
    const fin = await party.startTurn(null);
    console.log(`CU start_turn finalizing 3 deaths: ${await unitsOf(fin)}`);
    assert.equal(eventsNamed(await eventsOf(fin), "deathFinalized").length, 3);
    for (const slot of down) assert.ok(!(await party.character(slot)).core.alive);
    assert.equal((await party.state()).pendingDeaths, 0);
    await party.endTurn(party.dm, null);
    // The last one: down in a callback, the party wiped; the claims finalize it first.
    const last = [0, 1, 2, 3].find((x) => !down.includes(x))!;
    for (let i = 0; i < 60; i++) {
      await party.skipTo(MONSTER_BASE + C_CHIEF, 60);
      await party.monsterAct(party.dm, C_CHIEF, last);
      // Any landing hit; once its HP is low enough, one kills it.
      const hurt = (ev: any[]) => eventsNamed(ev, "hitResolved").some((h) => h.result.damage > 0);
      const { events: got } = await party.consumeEventsWhere(last, (ev) => hurt(ev));
      if (eventsNamed(got, "playerDied").length === 0) {
        await party.endTurn(party.dm, null);
        continue;
      }
      const s = await party.state();
      assert.ok(s.core.status.wiped !== undefined && s.pendingDeaths === 1 << last);
      assert.ok((await party.character(last)).core.alive, "not finalized yet");
      await rejects(party.startTurn(null), "NotActive");
      const claim = await party.claimRewards(last);
      console.log(`CU claim_rewards finalizing 1 death: ${await unitsOf(claim)}`);
      assert.equal(eventsNamed(await eventsOf(claim), "deathFinalized")[0].slot, last);
      assert.ok(!(await party.character(last)).core.alive);
      assert.equal((await party.state()).pendingDeaths, 0);
      // Nothing left for claim_escrow to finalize.
      const escrow = await party.claimEscrow(party.dm);
      assert.equal(eventsNamed(await eventsOf(escrow), "deathFinalized").length, 0);
      return;
    }
    assert.fail("the last mage never went down");
  });
});
