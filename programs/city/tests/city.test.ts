// City program tests on the shared local validator (docs/CITY_DESCRIPTION.md 3-4, 10;
// docs/PARTY_AND_TURNS.md 2, steps 1-3): the city, its buildings, the Tavern board and its quests.
//
// Needs the whole stack from `tools/scripts deploy:local` (the registry GameConfig lists every
// program; the city is seeded). The city is a singleton other lanes and the site read: the tests
// use the wallet as the authority, read what already exists, and skip authority-only cases if
// someone else owns it. Buildings the tests add get `TEST_PREFAB`, which the site does not draw.
//
// cd tests && ANCHOR_PROVIDER_URL=http://127.0.0.1:8899 ANCHOR_WALLET=~/.config/solana/id.json \
//   node --import tsx --test ../programs/city/tests/city.test.ts
import { describe, it } from "node:test";
import assert from "node:assert/strict";
import { randomBytes } from "node:crypto";
import { existsSync, readFileSync } from "node:fs";
import anchor from "@anchor-lang/core";
import { Connection, Keypair, LAMPORTS_PER_SOL, PublicKey, SystemProgram, Transaction } from "@solana/web3.js";
import { MPL_CORE_PROGRAM, PROGRAMS, VRF } from "@deadroll/sdk";
import { FACINGS, layoutBytes, loadMapPreset, presetPoints, presetSecrets, secretChunks } from "../../../tools/scripts/lib/demo-map.ts";
import { address as kitAddress, createKeyPairSignerFromBytes } from "@solana/kit";
import { extOf } from "../../../tests/skill_ext.ts";

const { AnchorProvider, BN, EventParser, Program } = anchor;

const env = AnchorProvider.env();
const provider = new AnchorProvider(new Connection(env.connection.rpcEndpoint, "confirmed"), env.wallet, {
  commitment: "confirmed",
  preflightCommitment: "confirmed",
});
const connection = provider.connection;

const REGISTRY = new PublicKey(PROGRAMS.registry);
const CHARACTER = new PublicKey(PROGRAMS.character);
const MAP = new PublicKey(PROGRAMS.map);
const SESSION = new PublicKey(PROGRAMS.session);
const CITY = new PublicKey(PROGRAMS.city);
const MPL_CORE = new PublicKey(MPL_CORE_PROGRAM);

const idl = (name: string) => new URL(`../../../target/idl/deadroll_${name}.json`, import.meta.url);

/** `CITY_W_MAX`, `CITY_CELLS`, `CITY_OBJECTS_MAX` (crates/types). */
const CITY_W_MAX = 48;
const CITY_CELLS = 48 * 32;
const CITY_OBJECTS_MAX = 32;
/** A prefab id no site picture has: test buildings stay invisible in the seeded city. */
const TEST_PREFAB = 0xffff;
const PRICE = new BN(LAMPORTS_PER_SOL / 10);

const pda = (seeds: (Buffer | Uint8Array)[], programId: PublicKey) => PublicKey.findProgramAddressSync(seeds, programId)[0];
const u16le = (v: number) => {
  const b = Buffer.alloc(2);
  b.writeUInt16LE(v);
  return b;
};

const config = pda([Buffer.from("config")], REGISTRY);
const cityPda = pda([Buffer.from("city")], CITY);
const buildingPda = (id: number) => pda([Buffer.from("building"), u16le(id)], CITY);
const boardPda = (building: PublicKey) => pda([Buffer.from("tavern_board"), building.toBuffer()], CITY);
const questPda = (session: PublicKey) => pda([Buffer.from("quest"), session.toBuffer()], CITY);
const registryTable = (seed: string) => pda([Buffer.from(seed)], REGISTRY);
const sessionAuthority = pda([Buffer.from("authority")], SESSION);

async function isProgram(id: PublicKey): Promise<boolean> {
  return (await connection.getAccountInfo(id))?.executable ?? false;
}

const missing: string[] = [];
for (const name of ["city", "map", "session", "character"]) if (!existsSync(idl(name))) missing.push(`target/idl/deadroll_${name}.json`);
for (const [name, id] of Object.entries({ CITY, MAP, SESSION, CHARACTER })) if (!(await isProgram(id))) missing.push(`${name} deployed`);
if ((await connection.getAccountInfo(config))?.owner.equals(REGISTRY) !== true) missing.push("registry GameConfig");
const SKIP = missing.length ? `run tools/scripts deploy:local first; missing: ${missing.join(", ")}` : false;

const load = (name: string): any => (SKIP ? null : new Program(JSON.parse(readFileSync(idl(name), "utf8")), provider));
const city = load("city");
const mapProgram = load("map");
const session = load("session");
const character = load("character");
const cityEvents = SKIP ? null : new EventParser(CITY, city.coder);

// ---------------------------------------------------------------------------------------------
// helpers

const e = (name: string) => ({ [name]: {} });
const text = (s: string, len: number) => {
  const out = new Array(len).fill(0);
  Buffer.from(s, "ascii").forEach((b, i) => (out[i] = b));
  return out;
};

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

async function eventOf(sig: string, name: string) {
  const tx = await connection.getTransaction(sig, { commitment: "confirmed", maxSupportedTransactionVersion: 0 });
  const found = [...cityEvents!.parseLogs(tx?.meta?.logMessages ?? [])].find((x) => x.name === name);
  assert.ok(found, `no ${name} event`);
  return found.data;
}

/** Compute units of a confirmed transaction, for the report. */
async function cu(what: string, sig: string): Promise<string> {
  const tx = await connection.getTransaction(sig, { commitment: "confirmed", maxSupportedTransactionVersion: 0 });
  console.log(`${what}: ${tx?.meta?.computeUnitsConsumed ?? "?"} CU`);
  return sig;
}

const authorityWallet = (provider.wallet as any).payer as Keypair;

/** The registry GameConfig authority is this wallet: authority cases can run. */
async function weAreAuthority(): Promise<boolean> {
  if (SKIP) return false;
  const data = (await connection.getAccountInfo(config))!.data;
  return new PublicKey(data.subarray(9, 41)).equals(authorityWallet.publicKey);
}
const AUTHORITY = (await weAreAuthority()) ? false : "the GameConfig authority is another wallet";

function buildingArgs(o: Partial<Record<string, any>> = {}) {
  return {
    kind: e("storage"),
    prefabId: TEST_PREFAB,
    x: 0,
    y: 0,
    door: { x: 0, y: 0 },
    program: PublicKey.default,
    target: PublicKey.default,
    interiorId: 0,
    status: e("closed"),
    requirement: e("none"),
    ...o,
  };
}

async function cityState(): Promise<any> {
  return city.account.city.fetch(cityPda);
}

async function ensureCity(): Promise<void> {
  if (await connection.getAccountInfo(cityPda)) return;
  // Not seeded yet: someone else's init is refused, then the wallet creates it.
  const stranger = await funded();
  await rejects(
    city.methods
      .initCity(20, 12, { x: 1, y: 1 })
      .accountsStrict({ authority: stranger.publicKey, config, city: cityPda, systemProgram: SystemProgram.programId })
      .signers([stranger])
      .rpc(),
    "Unauthorized",
  );
  await rejects(
    city.methods
      .initCity(CITY_W_MAX + 1, 12, { x: 1, y: 1 })
      .accountsStrict({ authority: authorityWallet.publicKey, config, city: cityPda, systemProgram: SystemProgram.programId })
      .rpc(),
    "SizeOutOfRange",
  );
  const sig = await city.methods
    .initCity(20, 12, { x: 1, y: 1 })
    .accountsStrict({ authority: authorityWallet.publicKey, config, city: cityPda, systemProgram: SystemProgram.programId })
    .rpc()
    .then((s: string) => cu("init_city", s));
  assert.equal((await eventOf(sig, "cityInitialized")).width, 20);
}

/** add_building with the next id; returns its address. */
async function addBuilding(args: any, signer: Keypair = authorityWallet): Promise<PublicKey> {
  const id = (await cityState()).core.buildingCount;
  const building = buildingPda(id);
  const call = city.methods
    .addBuilding(args)
    .accountsStrict({ authority: signer.publicKey, config, city: cityPda, building, systemProgram: SystemProgram.programId });
  await (signer === authorityWallet ? call : call.signers([signer])).rpc().then((s: string) => cu("add_building", s));
  return building;
}

/** A Tavern test building (open, its board as the target) and its board. */
async function testTavern(): Promise<{ building: PublicKey; board: PublicKey }> {
  const id = (await cityState()).core.buildingCount;
  const building = buildingPda(id);
  const board = boardPda(building);
  await addBuilding(buildingArgs({ kind: e("tavern"), program: CITY, target: board, status: e("active") }));
  await city.methods
    .initTavernBoard()
    .accountsStrict({ authority: authorityWallet.publicKey, city: cityPda, building, board, systemProgram: SystemProgram.programId })
    .rpc()
    .then((s: string) => cu("init_tavern_board", s));
  return { building, board };
}

// --- a party to post: the DM's finalized demo map, opened as a session

const preset = loadMapPreset();

async function finalizedMap(dm: Keypair): Promise<{ map: PublicKey; layout: PublicKey }> {
  const index = randomBytes(4).readUInt32LE(0);
  const idx = Buffer.alloc(4);
  idx.writeUInt32LE(index);
  const map = pda([Buffer.from("map"), dm.publicKey.toBuffer(), idx], MAP);
  const layout = pda([Buffer.from("layout"), map.toBuffer()], MAP);
  const edit = { dm: dm.publicKey, map, layout };
  const settings = { minPlayers: 1, chestRadius: preset.chestRadius, settleTimeoutSecs: preset.settleTimeoutSecs, biome: 2 };
  await mapProgram.methods
    .createMap(index, preset.width, preset.height, preset.tier, settings)
    .accountsStrict({ ...edit, systemProgram: SystemProgram.programId })
    .signers([dm])
    .rpc();
  await mapProgram.methods.writeLayout(0, Buffer.from(layoutBytes(preset))).accountsStrict(edit).signers([dm]).rpc();
  // Patch C: the visible monsters in the open, the rest under the root (arena update: each pack's
  // arena).
  const { design, commitment, blob } = await presetSecrets(preset, kitAddress(map.toBase58()), await createKeyPairSignerFromBytes(dm.secretKey));
  await mapProgram.methods.writeSpawns(design.spawns).accountsStrict(edit).signers([dm]).rpc();
  const kinds = [e("chest"), e("entrance"), e("exit")];
  const points = presetPoints(preset).map((p) => ({ ...p, kind: kinds[p.kind] }));
  await mapProgram.methods.writePoints(points, preset.bossTable).accountsStrict(edit).signers([dm]).rpc();
  for (const pk of design.packs) {
    const record = { visible: pk.visible, x: pk.x, y: pk.y, facing: e(FACINGS[pk.facing].toLowerCase()), width: pk.width, height: pk.height };
    await mapProgram.methods.writeArena(pk.pack, record, Buffer.from(pk.cells)).accountsStrict(edit).signers([dm]).rpc();
  }
  for (const c of secretChunks(blob, design)) {
    await mapProgram.methods.writeSecrets(c.offset, Buffer.from(c.bytes), c.len, c.threat, c.declared).accountsStrict(edit).signers([dm]).rpc();
  }
  await mapProgram.methods
    .finalizeMap(Array.from(commitment.root))
    .accountsStrict({
      ...edit,
      bestiary: registryTable("bestiary"),
      monsterSkills: registryTable("monster_skills"),
      rewards: registryTable("rewards"),
      tileset: registryTable("tileset"),
    })
    .signers([dm])
    .rpc();
  return { map, layout };
}

function partyOf(map: PublicKey) {
  const s = pda([Buffer.from("session"), map.toBuffer()], SESSION);
  return {
    session: s,
    monsters: pda([Buffer.from("monsters"), s.toBuffer()], SESSION),
    escrow: pda([Buffer.from("escrow"), s.toBuffer()], SESSION),
  };
}

function openSessionIx(dm: Keypair, map: PublicKey, layout: PublicKey) {
  return session.methods
    .openSession(PRICE, PublicKey.default)
    .accountsStrict({
      dm: dm.publicKey,
      map,
      layout,
      ...partyOf(map),
      sessionAuthority,
      config,
      bestiary: registryTable("bestiary"),
      tileset: registryTable("tileset"),
      mapProgram: MAP,
      systemProgram: SystemProgram.programId,
      aspects: registryTable("aspects"),
      traits: registryTable("traits"),
      resonances: registryTable("resonances"),
      monsterSkills: registryTable("monster_skills"),
      rewards: registryTable("rewards"),
      triples: registryTable("triples"),
    })
    .instruction();
}

function postQuestIx(dm: Keypair, tavern: { building: PublicKey; board: PublicKey }, map: PublicKey, sessionKey: PublicKey) {
  return city.methods
    .postQuest()
    .accountsStrict({
      dm: dm.publicKey,
      config,
      building: tavern.building,
      board: tavern.board,
      map,
      session: sessionKey,
      quest: questPda(sessionKey),
      systemProgram: SystemProgram.programId,
    })
    .signers([dm]);
}

function closeQuestIx(dm: Keypair, board: PublicKey, sessionKey: PublicKey) {
  return city.methods
    .closeQuest()
    .accountsStrict({ dm: dm.publicKey, config, board, quest: questPda(sessionKey), session: sessionKey })
    .signers([dm]);
}

/** A fresh warrior to join with. */
async function newCharacter(owner: Keypair): Promise<PublicKey> {
  const profile = pda([Buffer.from("profile"), owner.publicKey.toBuffer()], CHARACTER);
  await character.methods
    .createProfile()
    .accountsStrict({ owner: owner.publicKey, profile, systemProgram: SystemProgram.programId })
    .signers([owner])
    .rpc();
  const c = pda([Buffer.from("character"), owner.publicKey.toBuffer(), Buffer.from([0])], CHARACTER);
  const asset = Keypair.generate();
  const skill = {
    category: e("active"),
    purpose: e("damage"),
    source: e("combat"),
    form: e("strike"),
    target: e("single"),
    aspects: ["piercing", "fire", "fire"].map(e),
    aspectCount: 1,
    effect: e("none"),
    trigger: e("none"),
    auraRadius: 0,
  };
  await character.methods
    .createCharacter({
      name: text("Kit", 32),
      race: e("human"),
      class: e("warrior"),
      predisposition: e("piercing"),
      stats: [10, 7, 5, 8],
      appearance: new Array(20).fill({ spriteId: 0, materialRamp: 0, accentRamp: 0, elementRamp: 0 }),
      skills: [{ blocks: skill, name: text("Strike", 24), ext: extOf("Strike") }],
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
  return c;
}

function joinSession(player: Keypair, c: PublicKey, sessionKey: PublicKey, contribution: InstanceType<typeof BN>) {
  return session.methods
    .joinSession(contribution, PublicKey.default)
    .accountsStrict({
      player: player.publicKey,
      session: sessionKey,
      character: c,
      participation: pda([Buffer.from("participation"), sessionKey.toBuffer(), c.toBuffer()], SESSION),
      rewardCache: pda([Buffer.from("cache"), c.toBuffer()], SESSION),
      escrow: pda([Buffer.from("escrow"), sessionKey.toBuffer()], SESSION),
      systemProgram: SystemProgram.programId,
    })
    .signers([player])
    .rpc();
}

const vrfAccounts = {
  programIdentity: pda([Buffer.from("identity")], SESSION),
  oracleQueue: new PublicKey(VRF.oracleQueue),
  vrfProgram: new PublicKey(VRF.program),
  slotHashes: new PublicKey(VRF.slotHashes),
  systemProgram: SystemProgram.programId,
};

/** The player marks ready (its one skill on the map) and the DM starts: the party leaves the lobby. */
async function readyAndStart(dm: Keypair, player: Keypair, c: PublicKey, sessionKey: PublicKey) {
  await session.methods
    .setReady(true, [0, 0xff, 0xff, 0xff, 0xff, 0xff])
    .accountsStrict({
      player: player.publicKey,
      session: sessionKey,
      participation: pda([Buffer.from("participation"), sessionKey.toBuffer(), c.toBuffer()], SESSION),
      character: c,
      skillSet: pda([Buffer.from("skills"), c.toBuffer()], CHARACTER),
      sessionAuthority,
      config,
      characterProgram: CHARACTER,
      skillExtras: pda([Buffer.from("skill_extras"), c.toBuffer()], CHARACTER),
    })
    .signers([player])
    .rpc();
  await session.methods
    .startSession()
    .accountsStrict({
      dm: dm.publicKey,
      session: sessionKey,
      monsters: pda([Buffer.from("monsters"), sessionKey.toBuffer()], SESSION),
      escrow: pda([Buffer.from("escrow"), sessionKey.toBuffer()], SESSION),
      ...vrfAccounts,
    })
    .signers([dm])
    .rpc();
  assert.ok((await session.account.gameSession.fetch(sessionKey)).core.status.active);
}

// ---------------------------------------------------------------------------------------------

describe("city and buildings (docs/CITY_DESCRIPTION.md 3, 10)", { skip: SKIP || AUTHORITY }, () => {
  it("the city exists once; a second init_city is refused", async () => {
    await ensureCity();
    await rejects(
      city.methods
        .initCity(20, 12, { x: 1, y: 1 })
        .accountsStrict({ authority: authorityWallet.publicKey, config, city: cityPda, systemProgram: SystemProgram.programId })
        .rpc(),
      "already in use",
    );
  });

  it("write_city and set_city_objects: authority only, inside the city", async () => {
    const before = await cityState();
    const { width, height } = before.core;
    const stranger = await funded();
    const write = (signer: Keypair, offset: number, cells: number[]) =>
      city.methods.writeCity(offset, Buffer.from(cells)).accountsStrict({ authority: signer.publicKey, city: cityPda }).signers([signer]).rpc();
    await rejects(write(stranger, 0, [1]), "Unauthorized");
    await rejects(write(authorityWallet, CITY_CELLS - 1, [1, 1]), "OutOfCity");
    // Rewrite the first cell with what it holds: the seeded city is unchanged.
    const sig = await write(authorityWallet, 0, [before.core.cells[0]]).then((s: string) => cu("write_city (1 cell)", s));
    assert.deepEqual(await eventOf(sig, "cityWritten"), { offset: 0, len: 1 });

    const objects = before.core.objects.slice(0, before.core.objectCount);
    const set = (signer: Keypair, objs: any[], spawn: any) =>
      city.methods.setCityObjects(objs, spawn).accountsStrict({ authority: signer.publicKey, city: cityPda }).signers([signer]).rpc();
    await rejects(set(stranger, objects, before.core.spawn), "Unauthorized");
    await rejects(set(authorityWallet, objects, { x: width, y: 0 }), "OutOfCity");
    await rejects(set(authorityWallet, [{ sprite: 1, x: 0, y: height }], before.core.spawn), "OutOfCity");
    const tooMany = new Array(CITY_OBJECTS_MAX + 1).fill({ sprite: 1, x: 0, y: 0 });
    await rejects(set(authorityWallet, tooMany, before.core.spawn), "TooManyObjects");
    await set(authorityWallet, objects, before.core.spawn).then((s: string) => cu(`set_city_objects (${objects.length})`, s));
    const after = await cityState();
    assert.deepEqual(after.core.objects, before.core.objects);
    assert.deepEqual(after.core.spawn, before.core.spawn);
  });

  it("add_building: authority only, a game program or none when closed, an open Tavern is its board", async () => {
    const stranger = await funded();
    await rejects(addBuilding(buildingArgs(), stranger), "Unauthorized");
    // Not a game program: refused, even for a closed building.
    await rejects(addBuilding(buildingArgs({ program: Keypair.generate().publicKey })), "ProgramNotAllowed");
    // No program and open: refused.
    await rejects(addBuilding(buildingArgs({ status: e("active") })), "ProgramNotAllowed");
    // An open Tavern of another program, or with another target.
    await rejects(addBuilding(buildingArgs({ kind: e("tavern"), status: e("active"), program: SESSION })), "BadTarget");
    await rejects(
      addBuilding(buildingArgs({ kind: e("tavern"), status: e("active"), program: CITY, target: Keypair.generate().publicKey })),
      "BadTarget",
    );
    const width = (await cityState()).core.width;
    await rejects(addBuilding(buildingArgs({ x: width })), "OutOfCity");
    await rejects(addBuilding(buildingArgs({ door: { x: 0, y: 255 } })), "OutOfCity");

    const count = (await cityState()).core.buildingCount;
    const building = await addBuilding(buildingArgs({ kind: e("mastersGuild"), program: SESSION }));
    const b = await city.account.building.fetch(building);
    assert.equal(b.core.id, count);
    assert.ok(b.core.status.closed && b.core.kind.mastersGuild);
    assert.equal((await cityState()).core.buildingCount, count + 1);
  });

  it("update_building and close_building: authority only; a program cannot be swapped for an outsider", async () => {
    const building = await addBuilding(buildingArgs());
    const stranger = await funded();
    const update = (signer: Keypair, args: any) =>
      city.methods.updateBuilding(args).accountsStrict({ authority: signer.publicKey, config, city: cityPda, building }).signers([signer]).rpc();
    const close = (signer: Keypair) =>
      city.methods.closeBuilding().accountsStrict({ authority: signer.publicKey, city: cityPda, building }).signers([signer]).rpc();
    // docs/CITY_DESCRIPTION.md 10: a non-authority cannot change a building or its program.
    await rejects(update(stranger, buildingArgs({ program: CITY })), "Unauthorized");
    await rejects(update(authorityWallet, buildingArgs({ program: Keypair.generate().publicKey })), "ProgramNotAllowed");
    await update(authorityWallet, buildingArgs({ program: CITY, status: e("active"), interiorId: 7 })).then((s: string) => cu("update_building", s));
    const b = await city.account.building.fetch(building);
    assert.ok(b.core.status.active);
    assert.equal(b.core.interiorId, 7);
    assert.ok(b.core.program.equals(CITY));

    await rejects(close(stranger), "Unauthorized");
    await close(authorityWallet).then((s: string) => cu("close_building", s));
    assert.ok((await city.account.building.fetch(building)).core.status.closed);
    await rejects(close(authorityWallet), "BuildingClosed");
  });

  it("init_tavern_board: authority only, Taverns only", async () => {
    const storage = await addBuilding(buildingArgs());
    const init = (signer: Keypair, building: PublicKey) =>
      city.methods
        .initTavernBoard()
        .accountsStrict({ authority: signer.publicKey, city: cityPda, building, board: boardPda(building), systemProgram: SystemProgram.programId })
        .signers([signer])
        .rpc();
    await rejects(init(authorityWallet, storage), "NotATavern");
    const id = (await cityState()).core.buildingCount;
    const tavern = buildingPda(id);
    await addBuilding(buildingArgs({ kind: e("tavern"), program: CITY, target: boardPda(tavern), status: e("active") }));
    await rejects(init(await funded(), tavern), "Unauthorized");
    await init(authorityWallet, tavern);
    const board = await city.account.tavernBoard.fetch(boardPda(tavern));
    assert.ok(board.core.building.equals(tavern));
    assert.equal(board.core.open, 0);
  });
});

describe("Tavern quests (docs/PARTY_AND_TURNS.md 2, steps 1-3)", { skip: SKIP || AUTHORITY }, () => {
  it("the DM opens a party and posts it in one transaction; players take it through join_session", async () => {
    await ensureCity();
    const tavern = await testTavern();
    const dm = await funded(10);
    const { map, layout } = await finalizedMap(dm);
    const { session: sessionKey } = partyOf(map);

    // One main wallet transaction: open_session (session -> map.set_status), then post_quest (reads only).
    const tx = new Transaction().add(await openSessionIx(dm, map, layout), await postQuestIx(dm, tavern, map, sessionKey).instruction());
    const sig = await provider.sendAndConfirm(tx, [dm]);
    const size = tx.serialize({ requireAllSignatures: false }).length;
    await cu(`open_session + post_quest (one transaction, ${size} bytes)`, sig);
    const posted = await eventOf(sig, "questPosted");
    assert.ok(posted.session.equals(sessionKey));
    assert.equal(posted.priceLamports.toString(), PRICE.toString());

    const quest = await city.account.quest.fetch(questPda(sessionKey));
    assert.ok(quest.core.map.equals(map) && quest.core.dm.equals(dm.publicKey) && quest.core.board.equals(tavern.board));
    assert.equal(quest.core.tier, preset.tier);
    assert.equal(quest.core.biome, 2);
    assert.equal(quest.core.minPlayers, 1);
    const board = await city.account.tavernBoard.fetch(tavern.board);
    assert.deepEqual([board.core.posted, board.core.open], [1, 1]);

    // A player takes the quest: the contribution goes to the party escrow (session program).
    const player = await funded();
    const c = await newCharacter(player);
    await joinSession(player, c, sessionKey, PRICE);
    // The party is gathering: the DM cannot take the quest off the board.
    await rejects(closeQuestIx(dm, tavern.board, sessionKey).rpc(), "QuestStillOpen");

    // Ready and start through the board: the party left the lobby, the DM takes the quest down.
    await readyAndStart(dm, player, c, sessionKey);
    await closeQuestIx(dm, tavern.board, sessionKey).rpc().then((s: string) => cu("close_quest (party started)", s));
    assert.equal(await connection.getAccountInfo(questPda(sessionKey)), null);
  });

  it("post_quest refusals: not the DM, closed or other building, wrong board, map and party checks", async () => {
    await ensureCity();
    const tavern = await testTavern();
    const dm = await funded(10);
    const { map, layout } = await finalizedMap(dm);
    const { session: sessionKey } = partyOf(map);

    // The map is finalized but no party is open yet: the session account does not exist.
    await rejects(postQuestIx(dm, tavern, map, sessionKey).rpc(), "MapNotInSession");
    await provider.sendAndConfirm(new Transaction().add(await openSessionIx(dm, map, layout)), [dm]);

    const stranger = await funded();
    await rejects(postQuestIx(stranger, tavern, map, sessionKey).rpc(), "NotTheDm");

    // Another DM's party on this map's place: a party of another map.
    const other = await finalizedMap(dm);
    await provider.sendAndConfirm(new Transaction().add(await openSessionIx(dm, other.map, other.layout)), [dm]);
    const otherSession = partyOf(other.map).session;
    await rejects(postQuestIx(dm, tavern, map, otherSession).rpc(), "SessionMapMismatch");
    // Accounts of the wrong owner: a map passed as the party, a city account passed as the map.
    await rejects(postQuestIx(dm, tavern, map, map).rpc(), "InvalidSession");
    await rejects(postQuestIx(dm, tavern, cityPda, sessionKey).rpc(), "InvalidMap");

    // Not a Tavern, a closed Tavern, a board of another building.
    const storage = await addBuilding(buildingArgs());
    await rejects(postQuestIx(dm, { building: storage, board: tavern.board }, map, sessionKey).rpc(), "ConstraintSeeds");
    const other2 = await testTavern();
    await rejects(postQuestIx(dm, { building: tavern.building, board: other2.board }, map, sessionKey).rpc(), "ConstraintSeeds");
    await city.methods.closeBuilding().accountsStrict({ authority: authorityWallet.publicKey, city: cityPda, building: other2.building }).rpc();
    await rejects(postQuestIx(dm, other2, map, sessionKey).rpc(), "BuildingClosed");
    // A closed-kind check: a Storage whose own board exists cannot get quests.
    await city.methods
      .updateBuilding(buildingArgs({ kind: e("storage"), program: CITY, status: e("active") }))
      .accountsStrict({ authority: authorityWallet.publicKey, config, city: cityPda, building: other2.building })
      .rpc();
    await rejects(postQuestIx(dm, other2, map, sessionKey).rpc(), "NotATavern");

    // Happy path, then once only.
    await postQuestIx(dm, tavern, map, sessionKey).rpc().then((s: string) => cu("post_quest", s));
    await rejects(postQuestIx(dm, tavern, map, sessionKey).rpc(), "already in use");
  });

  it("post_quest needs the party in the lobby; close_quest by the DM once nobody joined or the party left the lobby", async () => {
    await ensureCity();
    const tavern = await testTavern();
    const dm = await funded(10);
    const { map, layout } = await finalizedMap(dm);
    const { session: sessionKey } = partyOf(map);
    await provider.sendAndConfirm(new Transaction().add(await openSessionIx(dm, map, layout), await postQuestIx(dm, tavern, map, sessionKey).instruction()), [dm]);

    const stranger = await funded();
    await rejects(closeQuestIx(stranger, tavern.board, sessionKey).rpc(), "NotTheDm");
    const other = await testTavern();
    await rejects(closeQuestIx(dm, other.board, sessionKey).rpc(), "BadBoard");
    const before = await connection.getBalance(dm.publicKey);
    const sig = await closeQuestIx(dm, tavern.board, sessionKey).rpc();
    await cu("close_quest", sig);
    assert.ok((await eventOf(sig, "questClosed")).session.equals(sessionKey));
    assert.equal(await connection.getAccountInfo(questPda(sessionKey)), null);
    assert.ok((await connection.getBalance(dm.publicKey)) > before - 10_000, "the rent goes back to the DM");
    const board = await city.account.tavernBoard.fetch(tavern.board);
    assert.deepEqual([board.core.posted, board.core.open], [1, 0]);

    // A party that already started cannot be posted.
    const late = await finalizedMap(dm);
    const lateSession = partyOf(late.map).session;
    await provider.sendAndConfirm(new Transaction().add(await openSessionIx(dm, late.map, late.layout)), [dm]);
    const player = await funded();
    const c = await newCharacter(player);
    await joinSession(player, c, lateSession, PRICE);
    await readyAndStart(dm, player, c, lateSession);
    await rejects(postQuestIx(dm, tavern, late.map, lateSession).rpc(), "SessionNotInLobby");
  });
});
