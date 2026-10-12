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
import type { DeadrollSession } from "../target/types/deadroll_session.ts";
import {
  Connection,
  Keypair,
  LAMPORTS_PER_SOL,
  PublicKey,
  SystemProgram,
  Transaction,
  TransactionInstruction,
} from "@solana/web3.js";

const { AnchorProvider, BN, EventParser, Program } = anchor;
type BN = InstanceType<typeof BN>;

const idl = JSON.parse(
  readFileSync(new URL("../target/idl/deadroll_session.json", import.meta.url), "utf8"),
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
const CHARACTER = new PublicKey("9uVNvkt5kTMad65GtdqQmBb3X1JQMWwQ985qLwTCdqfL");
const MAP = new PublicKey("2YeyBfQ5xkRKD2kagxRS2yZYCQdWt5qidikaKfQhLBXH");
const REGISTRY = new PublicKey("dZ1jraMfS3bAUTkRw2TA1ddg8UDGzgUdFQz5pmBxjoG");
const SESSION_KEYS = new PublicKey("KeyspM2ssCJbqUhQ4k7sveSiY4WjnYsrXkC8oDbwde5");
const MPL_CORE = new PublicKey("CoREENxT6tW1HoK8ypY1SxRMZTcVPm7R94rH4PZNhX7d");
// MagicBlock VRF (ephemeral-vrf-sdk 0.17.3 consts); none of them has to exist under mock-vrf.
const VRF_PROGRAM = new PublicKey("Vrf1RNUjXmQGjmQrQLvJHs9SNkvDJEsRVFPkfSQUwGz");
const ORACLE_QUEUE = new PublicKey("Cuj97ggrhhidhbu39TijNVqE74xvKJ69gDervRUXAxGh");
const SLOT_HASHES = new PublicKey("SysvarS1otHashes111111111111111111111111111");

const PRICE = new BN(LAMPORTS_PER_SOL / 10);
const MIN_PRICE = new BN(10_000_000);
const [N, E, S, W] = [0, 1, 2, 3];
const MAX_PATH = 16;
const EMPTY_UNIT = 0xff;
const BASE_ATTACK = 0xff;
const MONSTER_BASE = 4;

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
function seedWhere(pred: (d20: number) => boolean): Buffer {
  for (;;) {
    const seed = randomBytes(32);
    if (pred(d20Of(seed))) return seed;
  }
}
/** Natural 20: always a hit and a crit. Natural 1: always a miss. */
const CRIT = seedWhere((d) => d === 20);
const MISS = seedWhere((d) => d === 1);

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
  for (;;) {
    const seed = randomBytes(32);
    const { order } = initiativeOrder(seed, units);
    if (want.every((u, i) => order[i] === u)) return seed;
  }
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
    })
    .signers([dm]);
}

/** The VRF callback, signed by `identity` (the mock identity on localnet). */
function consume(
  identity: Keypair,
  a: { session: PublicKey; monsters: PublicKey; participation: PublicKey; character: PublicKey; skillSet: PublicKey },
  seed: Buffer,
) {
  return program.methods
    .consumeRandomness([...seed])
    .accountsStrict({
      vrfProgramIdentity: identity.publicKey,
      ...a,
      sessionAuthority,
      config,
      characterProgram: CHARACTER,
    })
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
    for (const over of [{ bestiary: tileset }, { tileset: bestiary }, { bestiary: config }]) {
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
        monsters,
        sessionToken: null,
        ...vrfAccounts,
      }),
    );
    await run(
      program.methods.monsterAct(0, 0, 0).accountsStrict({
        signer: signer.publicKey,
        session,
        monsters,
        participation,
        character,
        sessionToken: null,
        ...vrfAccounts,
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
      }),
    );
    await rejects(
      program.methods
        .claimRewards()
        .accountsStrict({ session, participation, stash, character, bag, sessionAuthority, config, characterProgram: CHARACTER })
        .rpc(),
      "AccountNotInitialized",
    );
  });

  it("act rejects VRF accounts at other addresses", async () => {
    const signer = await funded();
    const { session, monsters } = sessionAccounts(Keypair.generate().publicKey);
    const character = Keypair.generate().publicKey;
    const { participation, skillSet } = playerAccounts(session, character);
    const act = (over: Record<string, PublicKey>) =>
      program.methods
        .act(BASE_ATTACK, 0)
        .accountsStrict({ signer: signer.publicKey, session, participation, character, skillSet, monsters, sessionToken: null, ...vrfAccounts, ...over })
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
    const { participation, skillSet } = playerAccounts(session, character);
    const accounts = { session, monsters, participation, character, skillSet };
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

const CHARACTER_IDL = new URL("../target/idl/deadroll_character.json", import.meta.url);
const MAP_IDL = new URL("../target/idl/deadroll_map.json", import.meta.url);
const missing: string[] = [];
if ((await connection.getAccountInfo(config))?.owner.equals(REGISTRY) !== true)
  missing.push("registry GameConfig (L4)");
for (const [name, table] of [["bestiary", bestiary], ["tileset", tileset]] as const)
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

// The map lane's demo preset (tests/map.test.ts): 16x16 room in a wall ring.
const TILE = { Floor: 0, Wall: 1, Entrance: 4, Exit: 5 };
const ENTRANCE = { x: 1, y: 1 };
const EXIT = { x: 14, y: 14 };
const [WOLF, GOBLIN, CHIEFTAIN, GOBLIN_WARRIOR, SHAMAN] = [0, 1, 2, 3, 4];
const DEMO_SPAWNS = [
  { id: WOLF, x: 6, y: 2, pack: 1 },
  { id: WOLF, x: 7, y: 3, pack: 1 },
  { id: GOBLIN, x: 2, y: 10, pack: 2 },
  { id: GOBLIN, x: 3, y: 11, pack: 2 },
  { id: CHIEFTAIN, x: 12, y: 12, pack: 3 },
  { id: GOBLIN_WARRIOR, x: 11, y: 12, pack: 3 },
  { id: SHAMAN, x: 12, y: 11, pack: 3 },
];

interface MapFixture {
  map: PublicKey;
  layout: PublicKey;
}

async function createFinalizedMap(dm: Keypair): Promise<MapFixture> {
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
  const spawns = DEMO_SPAWNS.map((s) => ({
    bestiaryId: s.id,
    x: s.x,
    y: s.y,
    level: 1,
    skills: 0,
    flags: 0,
    aspects: 0,
    pack: s.pack,
  }));
  await mapProgram.methods.writeSpawns(ENTRANCE, EXIT, spawns).accountsStrict(edit).signers([dm]).rpc();
  await mapProgram.methods.finalizeMap(new Array(32).fill(7)).accountsStrict(edit).signers([dm]).rpc();
  return { map, layout };
}

// Same blocks as `deadroll_skillcheck::presets` (tests/character.test.ts).
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
/** Projectile damage skill slot per class: Javelin, Frost Bolt. */
const RANGED_SLOT = { warrior: 2, mage: 0 };
const REACTIVE_SLOT = 4;

type Kind = "warrior" | "mage";
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
  const set = kind === "warrior" ? WARRIOR_SKILLS : MAGE_SKILLS;
  const args = {
    name: text(kind === "warrior" ? "Bran" : "Ilyra", 32),
    race: e(kind === "warrior" ? "human" : "forestElf"),
    class: e(kind),
    predisposition: e(kind === "warrior" ? "piercing" : "cold"),
    stats: kind === "warrior" ? [10, 7, 5, 8] : [5, 7, 13, 5],
    appearance: new Array(20).fill(layer),
    skills: set.map(([name, b]) => ({ blocks: structuredClone(b), name: text(name, 24) })),
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

/** One party on a fresh demo map, with helpers for every session instruction. */
class Party {
  dm!: Keypair;
  dmActor!: Keypair;
  fixture!: MapFixture;
  acc!: ReturnType<typeof sessionAccounts>;
  players: Player[] = [];

  static async create(kinds: Kind[]): Promise<Party> {
    const p = new Party();
    [p.dm, p.dmActor] = await Promise.all([funded(), funded(1)]);
    p.fixture = await createFinalizedMap(p.dm);
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
        sessionAuthority,
        config,
        characterProgram: CHARACTER,
        systemProgram: SystemProgram.programId,
      })
      .signers([signer])
      .rpc();
  }
  ready(who: number, signer: Keypair, value: boolean) {
    return program.methods
      .setReady(value)
      .accountsStrict({ player: signer.publicKey, session: this.acc.session, participation: this.players[who].participation })
      .signers([signer])
      .rpc();
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
  act(who: number, signer: Keypair, slot: number, target: number, token: PublicKey | null = null) {
    const pl = this.players[who];
    return program.methods
      .act(slot, target)
      .accountsStrict({
        signer: signer.publicKey,
        session: this.acc.session,
        participation: pl.participation,
        character: pl.character,
        skillSet: pl.skillSet,
        monsters: this.acc.monsters,
        sessionToken: token,
        ...vrfAccounts,
      })
      .signers([signer])
      .rpc();
  }
  monsterAct(signer: Keypair, monster: number, target: number, token: PublicKey | null = null, action = 0) {
    const pl = this.players[target];
    return program.methods
      .monsterAct(monster, action, target)
      .accountsStrict({
        signer: signer.publicKey,
        session: this.acc.session,
        monsters: this.acc.monsters,
        participation: pl.participation,
        character: pl.character,
        sessionToken: token,
        ...vrfAccounts,
      })
      .signers([signer])
      .rpc();
  }
  /** The callback for the pending action of player `who` (attacker or target). */
  consume(who: number, seed: Buffer, identity = MOCK_VRF) {
    const pl = this.players[who];
    return consume(
      identity,
      {
        session: this.acc.session,
        monsters: this.acc.monsters,
        participation: pl.participation,
        character: pl.character,
        skillSet: pl.skillSet,
      },
      seed,
    ).rpc({ commitment: "confirmed" });
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
  claimRewards(who: number) {
    const pl = this.players[who];
    return program.methods
      .claimRewards()
      .accountsStrict({
        session: this.acc.session,
        participation: pl.participation,
        stash: pl.stash,
        character: pl.character,
        bag: pl.bag,
        sessionAuthority,
        config,
        characterProgram: CHARACTER,
      })
      .rpc({ commitment: "confirmed" });
  }
  claimEscrow(signer: Keypair) {
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
      })
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
      const s = await this.state();
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
        const target = living.sort((a: any, b: any) => cheb(pl.core, a) - cheb(pl.core, b))[0];
        if (opts.passive || !target || ap < 1 || acted) {
          await this.endTurn(this.players[unit].wallet, unit);
        } else if (cheb(pl.core, target) <= 6 && cheb(pl.core, target) >= 1 && ap >= 2) {
          await this.act(unit, this.players[unit].wallet, RANGED_SLOT[this.players[unit].kind], target.i);
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

  private async stepToward(who: number | null, from: any, to: any, dirs: number[][], cheb: any, monster?: number) {
    const options = dirs
      .map(([d, dx, dy]) => ({ d, cell: { x: from.x + dx, y: from.y + dy } }))
      .filter((o) => cheb(o.cell, to) < cheb(from, to))
      .sort((a, b) => cheb(a.cell, to) - cheb(b.cell, to));
    for (const o of options) {
      try {
        if (who === null) await this.monsterMove(this.dm, monster!, [o.d]);
        else await this.move(who, this.players[who].wallet, [o.d]);
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

  it("join_session: owner only (also signs character.lock_for_session), pays only the missing part", async () => {
    const [p0, p1] = party.players;
    await rejects(party.join(0, p1.wallet, PRICE), "NotCharacterOwner");
    const before = await connection.getBalance(party.acc.escrow);
    await party.join(0, p0.wallet, PRICE.muln(3));
    assert.equal((await connection.getBalance(party.acc.escrow)) - before, PRICE.toNumber());
    await party.join(1, p1.wallet, PRICE);
    const [a, b] = await Promise.all([party.part(0), party.part(1)]);
    assert.ok(a.contribution.eq(PRICE) && b.contribution.eqn(0));
    assert.equal(a.core.hp, (await party.character(0)).core.hpMax);
    assert.ok((await party.character(0)).core.inSession.equals(party.acc.session));
  });

  it("set_ready / start_session", async () => {
    const [p0, p1] = party.players;
    await rejects(party.ready(0, p0.actor, true), "NotPlayer");
    await party.ready(0, p0.wallet, true);
    await rejects(party.start(party.dm), "NotAllReady");
    await party.ready(1, p1.wallet, true);
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
    await rejects(party.act(0, p0.wallet, REACTIVE_SLOT, 0), "InvalidSkill");
    await rejects(party.act(0, p0.wallet, 9, 0), "InvalidSkill");
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
    const before = (await party.monsters()).core.monsters[0];
    await party.act(1, p1.wallet, RANGED_SLOT.mage, 0); // Frost Bolt, wolf at distance 5
    const sig = await party.consume(1, MISS);
    const hit = await eventOf(sig, "hitResolved");
    assert.ok(hit.result.outcome.miss);
    assert.deepEqual([hit.result.d20, hit.result.damage], [1, 0]);
    assert.equal((await party.monsters()).core.monsters[0].hp, before.hp);
    await rejects(party.act(1, p1.wallet, BASE_ATTACK, 0), "AlreadyActed");
  });

  it("monster_act: DM on that monster's turn, target in range", async () => {
    const [p0, p1] = party.players;
    await rejects(party.monsterAct(party.dm, 0, 0), "NotYourTurn");
    await party.endTurn(p1.wallet, 1);
    const s = await party.state();
    const wolfTurn = s.core.turnOrder[s.core.turnIndex] - MONSTER_BASE;
    await rejects(party.monsterAct(p0.wallet, wolfTurn, 0), "Unauthorized");
    await rejects(party.monsterAct(party.dm, wolfTurn, 0, null, 7), "InvalidAction");
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
  });

  it("killing the goblin chieftain wins the party", async () => {
    await rejects(party.claimRewards(0), "NotFinished");
    await rejects(party.claimEscrow(party.dm), "NotFinished");
    const s = await party.play({ playerSeed: CRIT, monsterSeed: () => MISS, steps: 2000 });
    assert.ok(s.core.status.victory, "boss killed");
    assert.ok(s.xpPool > 0);
    assert.deepEqual(Buffer.from(s.victorySeed), Buffer.from(s.core.lastSeed));
    const m = await party.monsters();
    assert.ok(!m.core.monsters[4].alive);
  });

  it("claim_rewards: XP and boss loot for the living, nothing for the dead, once each", async () => {
    const before = await party.character(0);
    const sig = await party.claimRewards(0);
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

    const dead = await eventOf(await party.claimRewards(1), "rewardsClaimed");
    assert.equal(dead.xp, 0);
    assert.ok(!(await party.character(1)).core.alive);
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

describe("deadroll_session: flee and wipe", { skip: SKIP_PARTY }, () => {
  it("flee needs every living player; any other action resets the intents", async () => {
    const party = await Party.create(["warrior", "mage"]);
    await party.startAll();
    const [p0, p1] = party.players;
    await rejects(party.flee(1, p1.wallet), "NotYourTurn");
    await party.flee(0, p0.wallet); // spends the turn
    let s = await party.state();
    assert.deepEqual([s.fleeMask, s.core.turnIndex], [0b01, 1]);
    await party.move(1, p1.wallet, [S]); // another action: intents reset
    assert.equal((await party.state()).fleeMask, 0);
    await party.endTurn(p1.wallet, 1);
    for (let i = 0; i < DEMO_SPAWNS.length; i++) await party.endTurn(party.dm, null);
    await party.flee(0, p0.wallet);
    await party.flee(1, p1.wallet);
    s = await party.state();
    assert.ok(s.core.status.fled);
    await rejects(party.flee(0, p0.wallet), "NotActive");

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
