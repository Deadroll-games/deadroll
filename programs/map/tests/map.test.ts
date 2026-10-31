// Map program tests on a local validator: `anchor build --tools-version v1.54`, then run them
// the way the decision block of agents/TASKS.md says (lane L6 port: 8919).
// Instructions, accounts, PDAs and error codes come from @deadroll/sdk (generated from the IDL).
import { strict as assert } from "node:assert";
import { createHash } from "node:crypto";
import { describe, it } from "node:test";
import {
  address,
  airdropFactory,
  appendTransactionMessageInstructions,
  createSolanaRpc,
  createSolanaRpcSubscriptions,
  createTransactionMessage,
  generateKeyPairSigner,
  getAddressEncoder,
  getBase64Encoder,
  getSignatureFromTransaction,
  lamports,
  pipe,
  sendAndConfirmTransactionFactory,
  setTransactionMessageFeePayerSigner,
  setTransactionMessageLifetimeUsingBlockhash,
  signTransactionMessageWithSigners,
} from "@solana/kit";
import { PROGRAMS, SYSTEM_PROGRAM, errorCode } from "@deadroll/sdk";
import {
  findLayoutPda,
  findMapPda,
  getCreateMapInstruction,
  getFinalizeMapInstruction,
  getMapDecoder,
  getMapLayoutDecoder,
  getSetStatusInstruction,
  getSpawnEncoder,
  getWriteArenaInstruction,
  getWriteLayoutInstruction,
  getWritePointsInstruction,
  getWriteSpawnsInstruction,
  getWriteSecretsInstruction,
  type MapStatus,
} from "@deadroll/sdk/map";
import { findConfigPda, findTablePda } from "@deadroll/sdk/registry";

// `anchor test` passes the cluster; the websocket is the RPC port + 1.
const rpcUrl = new URL(process.env.ANCHOR_PROVIDER_URL ?? "http://127.0.0.1:8899");
const wsUrl = new URL(rpcUrl);
wsUrl.protocol = "ws:";
wsUrl.port = String(Number(rpcUrl.port) + 1);
const rpc = createSolanaRpc(rpcUrl.toString());
const rpcSubscriptions = createSolanaRpcSubscriptions(wsUrl.toString());
const airdrop = airdropFactory({ rpc, rpcSubscriptions });
const sendAndConfirm = sendAndConfirmTransactionFactory({ rpc, rpcSubscriptions });
const enc = getAddressEncoder();

// Shared constants (crates/types, crates/mapcheck).
const MAP_MAX = 32;
const MAP_CELLS = 1024;
const SPAWNS_MAX = 24;
// Patch 11: visible chests and reward tables (assets/registry/reward_tables.json, LootTuning).
const CHESTS_MAX = 4;
// Patch 13: the entrance, the exit and the chests are points.
const POINTS_MAX = 2 + CHESTS_MAX;
const POINT = { Chest: 0, Entrance: 1, Exit: 2 };
const NO_TABLE = 0xff;
const T1_CHEST = 0, T1_BOSS = 1, T2_CHEST = 2, T2_BOSS = 3;
/** Patch 13: tile types (`TileKind`) and cell bytes (type in bits 7-3, variant in bits 2-0). */
const KIND = { Void: 0, StoneFloor: 1, WoodenFloor: 2, Dirt: 3, Wall: 4, Door: 5, ShallowWater: 6, DeepWater: 7, Stairs: 8, Rubble: 9, Chasm: 10 };
const cell = (kind: number, variant = 0) => (kind << 3) | variant;
const FLOOR = cell(KIND.StoneFloor);
const WALL = cell(KIND.Wall);
const STATUS = { Draft: 0, Finalized: 1, InSession: 2, Finished: 3 };
/** `Biome` (crates/types): the art set of a map. */
const BIOME = { Caves: 0, Forest: 1, Crypt: 2 };
/** `Map` account size: discriminator, `MapData` (105), tier, min_players, chest_radius, settle_timeout_secs (u32), biome. */
const MAP_SIZE = 8 + 105 + 1 + 1 + 1 + 4 + 1;
const WOLF = 0, GOBLIN = 1, CHIEFTAIN = 2, WARRIOR = 3, SHAMAN = 4;
/** `MapLayoutDataV2::SIZE`: the bytes `layout_hash` covers, after the discriminator. */
const LAYOUT_CORE_SIZE = 1 + 32 + MAP_CELLS + SPAWNS_MAX * 9 + 1 + POINTS_MAX * 4 + 1 + 1 + 1;
/** Arena update: `MapArenasData` after the core (8 packs of 6 bytes, 8 arenas of 12 x 12). */
const PACKS_MAX = 8;
const ARENA_SIDE_MAX = 12;
const ARENAS_SIZE = PACKS_MAX * 6 + PACKS_MAX * ARENA_SIDE_MAX * ARENA_SIDE_MAX;
const FACING = { N: 0, NE: 1, E: 2, SE: 3, S: 4, SW: 5, W: 6, NW: 7 };
/** Patch C: `MapSecretsData` after the arenas: threats, declared counts, blob length, the blob. */
const SECRET_TYPES = 4;
const SECRETS_BLOB_MAX = 800;
/** The bytes `layout_hash` covers: the core, the arenas, the threats and the declared counts. */
const HASHED_SIZE = LAYOUT_CORE_SIZE + ARENAS_SIZE + PACKS_MAX + SECRET_TYPES;
/** `PackLayout.visible` of an ambush pack. */
const NO_SPAWN = 0xff;
/** Any nonzero root: the map program never opens the tree. */
const ROOT = new Array(32).fill(1);
/** The demo packs' threats (levels; the boss 3 times) and one declared trap (tier 1 needs one). */
const DEMO_THREAT = [0, 2, 2, 5, 0, 0, 0, 0];
const DEMO_DECLARED = [1, 0, 0, 0];
/** Offset of `points` in the layout account. */
const POINTS_AT = 8 + 1 + 32 + MAP_CELLS + SPAWNS_MAX * 9 + 1;
/** Anchor `ConstraintSeeds`. */
const CONSTRAINT_SEEDS = 2006;

const ENTRANCE = { x: 1, y: 1 };
const EXIT = { x: 14, y: 14 };

type Cell = { x: number; y: number };
/** `skills`: the chosen pool bits; `flags`: bit 0 elite (patch 6). */
type Spawn = { id: number; x: number; y: number; pack: number; level?: number; skills: number; flags?: number };

// ---------- encoding ----------

function err(name: string): number {
  const code = errorCode("map", name);
  assert.ok(code !== null, `no error ${name} in the IDL`);
  return code;
}

const u16 = (v: number) => [v & 0xff, (v >> 8) & 0xff];
/** The program's `Spawn` record (expected bytes). */
const spawnBytes = (s: Spawn) => [s.id, s.x, s.y, s.level ?? 1, ...u16(s.skills), s.flags ?? 0, 0, s.pack];
const spawnArgs = (s: Spawn) => ({ bestiaryId: s.id, x: s.x, y: s.y, level: s.level ?? 1, skills: s.skills, flags: s.flags ?? 0, aspects: 0, pack: s.pack });

// ---------- addresses ----------

const mapAddress = async (dm: string, index: number) => (await findMapPda({ dm: address(dm), index }))[0];
const layoutAddress = async (map: string) => (await findLayoutPda({ map: address(map) }))[0];
const configAddress = async () => (await findConfigPda())[0];
const tableAddress = async (seed: string) => (await findTablePda({ seed }))[0];

// ---------- instructions ----------

type Signer = Awaited<ReturnType<typeof generateKeyPairSigner>>;

/** Patch 11: the DM's map settings (minimum players, chest radius, time to settle), then the biome. */
type Settings = { minPlayers?: number; chestRadius?: number; settleTimeoutSecs?: number; biome?: number };

async function createMapIx(dm: Signer, index: number, width: number, height: number, tier: number, minPlayers = 1, more: Settings = {}) {
  const map = await mapAddress(dm.address, index);
  return getCreateMapInstruction({
    dm,
    map,
    layout: await layoutAddress(map),
    index,
    width,
    height,
    tier,
    minPlayers,
    chestRadius: more.chestRadius ?? 3,
    settleTimeoutSecs: more.settleTimeoutSecs ?? 3600,
    biome: more.biome ?? BIOME.Caves,
  });
}

async function editAccounts(dm: Signer, map: string) {
  return { dm, map: address(map), layout: await layoutAddress(map) };
}

async function writeLayoutIx(dm: Signer, map: string, offset: number, cells: Uint8Array | number[]) {
  return getWriteLayoutInstruction({ ...(await editAccounts(dm, map)), offset, cells: Uint8Array.from(cells) });
}

async function writeSpawnsIx(dm: Signer, map: string, spawns: Spawn[]) {
  return getWriteSpawnsInstruction({ ...(await editAccounts(dm, map)), spawns: spawns.map(spawnArgs) });
}

type Chest = { x: number; y: number; table: number };
type Point = { kind: number; x: number; y: number; data: number };

/** The entrance, the exit (`null` leaves one out), then the chests. */
function points(entrance: Cell | null, exit: Cell | null, chests: Chest[] = []): Point[] {
  return [
    ...(entrance ? [{ kind: POINT.Entrance, ...entrance, data: 0 }] : []),
    ...(exit ? [{ kind: POINT.Exit, ...exit, data: 0 }] : []),
    ...chests.map((c) => ({ kind: POINT.Chest, x: c.x, y: c.y, data: c.table })),
  ];
}

async function writePointsIx(dm: Signer, map: string, list: Point[], bossTable: number) {
  return getWritePointsInstruction({ ...(await editAccounts(dm, map)), points: list, bossTable });
}

/** The demo entrance and exit with `chests`. */
async function writeChestsIx(dm: Signer, map: string, chests: Chest[], bossTable: number) {
  return writePointsIx(dm, map, points(ENTRANCE, EXIT, chests), bossTable);
}

/** Patch 6, 11 and 13: the registry bestiary, monster skills, rewards and tileset tables follow the
 * layout; `tables` overrides them. */
/** Patch C: a chunk of the encrypted secrets and the public part. */
async function writeSecretsIx(dm: Signer, map: string, opts: { offset?: number; bytes?: number[]; len?: number; threat?: number[]; declared?: number[] } = {}) {
  const bytes = opts.bytes ?? [1, 2, 3, 4];
  return getWriteSecretsInstruction({
    ...(await editAccounts(dm, map)),
    offset: opts.offset ?? 0,
    bytes: Uint8Array.from(bytes),
    len: opts.len ?? bytes.length,
    threat: opts.threat ?? DEMO_THREAT,
    declared: opts.declared ?? DEMO_DECLARED,
  });
}

async function finalizeIx(dm: Signer, map: string, root: number[], tables: string[] | null = null) {
  const [bestiary, skills, rewards, tileset] = tables ?? [await tableAddress("bestiary"), await tableAddress("monster_skills"), await tableAddress("rewards"), await tableAddress("tileset")];
  return getFinalizeMapInstruction({
    ...(await editAccounts(dm, map)),
    bestiary: address(bestiary),
    monsterSkills: address(skills),
    rewards: address(rewards),
    tileset: address(tileset),
    secretsRoot: Uint8Array.from(root),
  });
}

function setStatusIx(map: string, authority: Signer, config: string, status: number) {
  return getSetStatusInstruction({ map: address(map), sessionAuthority: authority, config: address(config), status: status as MapStatus });
}

// ---------- transactions ----------

async function funded(): Promise<Signer> {
  const s = await generateKeyPairSigner();
  await airdrop({ recipientAddress: s.address, lamports: lamports(5_000_000_000n), commitment: "confirmed" });
  return s;
}

/** Sends `ixs` paid by `payer`; returns compute units used. */
async function send(payer: Signer, ixs: any[]): Promise<number> {
  const { value: blockhash } = await rpc.getLatestBlockhash({ commitment: "confirmed" }).send();
  const msg = pipe(
    createTransactionMessage({ version: 0 }),
    (m) => setTransactionMessageFeePayerSigner(payer, m),
    (m) => setTransactionMessageLifetimeUsingBlockhash(blockhash, m),
    (m) => appendTransactionMessageInstructions(ixs, m),
  );
  const tx = await signTransactionMessageWithSigners(msg);
  try {
    await sendAndConfirm(tx as any, { commitment: "confirmed" });
  } catch (e: any) {
    // Keep the original as `cause` (error codes are read from it), surface the program logs.
    const logs: string[] = e?.context?.logs ?? [];
    throw new Error(`${e.message}\n${logs.join("\n")}`, { cause: e });
  }
  const sig = getSignatureFromTransaction(tx);
  const info = await rpc
    .getTransaction(sig, { commitment: "confirmed", maxSupportedTransactionVersion: 0 })
    .send();
  return Number(info?.meta?.computeUnitsConsumed ?? 0);
}

/** The custom program error code of a failed send: from the error chain, else from the logs. */
function customCode(e: any): number | undefined {
  for (let x = e; x; x = x.cause) {
    if (typeof x.context?.code === "number" && "index" in (x.context ?? {})) return x.context.code;
  }
  for (let x = e; x; x = x.cause) {
    for (const line of x.context?.logs ?? []) {
      const m = /custom program error: 0x([0-9a-f]+)/i.exec(line);
      if (m) return parseInt(m[1], 16);
    }
  }
  return undefined;
}

async function expectErr(p: Promise<unknown>, code: number) {
  try {
    await p;
  } catch (e) {
    assert.equal(customCode(e), code, `expected custom error ${code}, got: ${e}`);
    return;
  }
  assert.fail(`expected custom error ${code}, transaction succeeded`);
}

// ---------- accounts ----------

async function data(addr: string): Promise<Uint8Array> {
  const { value } = await rpc.getAccountInfo(address(addr), { encoding: "base64", commitment: "confirmed" }).send();
  assert.ok(value, `account ${addr} missing`);
  return Uint8Array.from(getBase64Encoder().encode(value.data[0]));
}

async function readMap(addr: string) {
  const { core, tier, minPlayers, chestRadius, settleTimeoutSecs, biome } = getMapDecoder().decode(await data(addr));
  return {
    version: core.version,
    dm: Uint8Array.from(enc.encode(core.dm)),
    index: core.index,
    width: core.width,
    height: core.height,
    status: core.status,
    layoutHash: Array.from(core.layoutHash),
    secretsRoot: Array.from(core.secretsRoot),
    tier,
    minPlayers,
    chestRadius,
    settleTimeoutSecs,
    biome,
  };
}

async function readLayout(addr: string) {
  const raw = await data(addr);
  const { core } = getMapLayoutDecoder().decode(raw);
  const spawn = getSpawnEncoder();
  return {
    raw,
    version: core.version,
    map: Uint8Array.from(enc.encode(core.map)),
    cells: Uint8Array.from(core.cells),
    spawns: Uint8Array.from(core.spawns.flatMap((s) => Array.from(spawn.encode(s)))),
    spawnCount: core.spawnCount,
    points: core.points.slice(0, core.pointCount).map((p) => ({ kind: p.kind as number, x: p.x, y: p.y, data: p.data })),
    bossTable: core.bossTable,
  };
}

// ---------- fixtures ----------

/** A valid tier 1, 16x16 map: open room in a wall ring. Only the first 15 rows + 16 cells. */
function demoTiles(): number[] {
  const tiles = new Array(15 * MAP_MAX + 16).fill(WALL);
  for (let y = 1; y < 15; y++) for (let x = 1; x < 15; x++) tiles[y * MAP_MAX + x] = FLOOR;
  return tiles;
}

/** The skill choices of assets/registry/maps/demo_16x16.json: exactly the slots of each kind.
 * Arena update: the cells are in each pack's arena (`DEMO_PACKS`). */
const demoSpawns = (): Spawn[] => [
  { id: WOLF, x: 4, y: 4, pack: 1, skills: 0b0111 },
  { id: WOLF, x: 5, y: 4, pack: 1, skills: 0b1110 },
  { id: GOBLIN, x: 4, y: 4, pack: 2, skills: 0b11 },
  { id: GOBLIN, x: 4, y: 5, pack: 2, skills: 0b11 },
  { id: CHIEFTAIN, x: 4, y: 4, pack: 3, skills: 0b11111 },
  { id: WARRIOR, x: 3, y: 4, pack: 3, skills: 0b11 },
  { id: SHAMAN, x: 4, y: 3, pack: 3, skills: 0b111 },
];

/** Patch C: the public spawns, the visible monster of each pack (the rest is under the root). */
const publicSpawns = (): Spawn[] => {
  const all = demoSpawns();
  return [all[0], all[2], all[4]];
};

/** Arena update: a pack's record (`PackLayout`) and its arena, row by row. */
type Pack = { pack: number; visible: number; x: number; y: number; facing: number; width: number; height: number; cells: number[] };
const openArena = (w = 8, h = 8) => new Array(w * h).fill(FLOOR);
/** The demo packs: the visible monster's map cell and facing, an open 8x8 arena each. */
const DEMO_PACKS = (): Pack[] => [
  { pack: 1, visible: 0, x: 6, y: 2, facing: FACING.S, width: 8, height: 8, cells: openArena() },
  { pack: 2, visible: 1, x: 2, y: 10, facing: FACING.E, width: 8, height: 8, cells: openArena() },
  { pack: 3, visible: 2, x: 12, y: 12, facing: FACING.N, width: 8, height: 8, cells: openArena() },
];

async function writeArenaIx(dm: Signer, map: string, p: Pack) {
  const record = { visible: p.visible, x: p.x, y: p.y, facing: p.facing, width: p.width, height: p.height };
  return getWriteArenaInstruction({ ...(await editAccounts(dm, map)), pack: p.pack, record, cells: Uint8Array.from(p.cells) });
}

async function writeDemoArenas(dm: Signer, map: string) {
  for (const p of DEMO_PACKS()) await send(dm, [await writeArenaIx(dm, map, p)]);
}

/** One tier 1 chest in the room, the tier 1 boss table: 2 + 4 points, the tier 1 budget for one player. */
const DEMO_CHESTS: Chest[] = [{ x: 5, y: 5, table: T1_CHEST }];

async function create(dm: Signer, index = 0, width = 16, height = 16, tier = 1, minPlayers = 1) {
  await send(dm, [await createMapIx(dm, index, width, height, tier, minPlayers)]);
  const map = await mapAddress(dm.address, index);
  return { map, layout: await layoutAddress(map) };
}

async function demoDraft(dm: Signer, index = 0) {
  const accounts = await create(dm, index);
  await send(dm, [await writeLayoutIx(dm, accounts.map, 0, demoTiles())]);
  await send(dm, [await writeSpawnsIx(dm, accounts.map, publicSpawns())]);
  await send(dm, [await writeChestsIx(dm, accounts.map, DEMO_CHESTS, T1_BOSS)]);
  await writeDemoArenas(dm, accounts.map);
  await send(dm, [await writeSecretsIx(dm, accounts.map)]);
  return accounts;
}

async function demoFinal(dm: Signer, index = 0) {
  const accounts = await demoDraft(dm, index);
  await send(dm, [await finalizeIx(dm, accounts.map, ROOT)]);
  return accounts;
}

// ---------- tests ----------

describe("map: create_map", () => {
  it("creates a draft map and its layout", async () => {
    const dm = await funded();
    const { map, layout } = await create(dm, 7, 16, 24, 2);
    const m = await readMap(map);
    // Version 2: `biome` appended.
    assert.equal(m.version, 2);
    assert.deepEqual(m.dm, Uint8Array.from(enc.encode(dm.address)));
    assert.equal(m.index, 7);
    assert.equal(m.width, 16);
    assert.equal(m.height, 24);
    assert.equal(m.status, STATUS.Draft);
    assert.equal(m.tier, 2);
    const l = await readLayout(layout);
    // Patch 13: every cell Void (0), no points. Arena update: layout version 3.
    assert.equal(l.version, 4);
    assert.deepEqual(l.map, Uint8Array.from(enc.encode(address(map))));
    assert.equal(l.spawnCount, 0);
    assert.equal(l.points.length, 0);
    assert.ok(l.cells.every((t) => t === cell(KIND.Void)));
  });

  it("lets one DM keep many maps", async () => {
    const dm = await funded();
    const a = await create(dm, 0, 8, 8);
    const b = await create(dm, 1, 32, 32);
    assert.notEqual(a.map, b.map);
    assert.equal((await readMap(b.map)).index, 1);
  });

  it("rejects a side outside 8..=32", async () => {
    const dm = await funded();
    for (const [w, h] of [[7, 16], [16, 7], [33, 16], [16, 33]]) {
      await expectErr(send(dm, [await createMapIx(dm, 0, w, h, 1)]), err("SizeOutOfRange"));
    }
  });

  it("rejects a tier without limits", async () => {
    const dm = await funded();
    for (const tier of [0, 3]) {
      await expectErr(send(dm, [await createMapIx(dm, 0, 16, 16, tier)]), err("TierOutOfRange"));
    }
  });

  it("patch 11: stores the minimum players and rejects it outside 1..=4", async () => {
    const dm = await funded();
    const { map, layout } = await create(dm, 0, 16, 16, 1, 3);
    assert.equal((await readMap(map)).minPlayers, 3);
    assert.equal((await readLayout(layout)).bossTable, NO_TABLE);
    for (const bad of [0, 5]) {
      await expectErr(send(dm, [await createMapIx(dm, 1, 16, 16, 1, bad)]), err("MinPlayersOutOfRange"));
    }
  });

  it("patch 11: stores the DM's chest radius and time to settle, within the limits", async () => {
    const dm = await funded();
    await send(dm, [await createMapIx(dm, 0, 16, 16, 1, 1, { chestRadius: 10, settleTimeoutSecs: 36_000 })]);
    const map = await readMap(await mapAddress(dm.address, 0));
    assert.deepEqual([map.chestRadius, map.settleTimeoutSecs], [10, 36_000]);
    for (const chestRadius of [0, 11]) {
      await expectErr(send(dm, [await createMapIx(dm, 1, 16, 16, 1, 1, { chestRadius })]), err("ChestRadiusOutOfRange"));
    }
    // The map takes seconds (localnet tests); open_session refuses less than 10 minutes on devnet.
    await send(dm, [await createMapIx(dm, 2, 16, 16, 1, 1, { settleTimeoutSecs: 5 })]);
    for (const settleTimeoutSecs of [0, 36_001]) {
      await expectErr(send(dm, [await createMapIx(dm, 3, 16, 16, 1, 1, { settleTimeoutSecs })]), err("SettleTimeoutOutOfRange"));
    }
  });

  it("stores the biome the DM picks, and its size has room for it", async () => {
    const dm = await funded();
    for (const [i, biome] of [BIOME.Caves, BIOME.Forest, BIOME.Crypt].entries()) {
      const cu = await send(dm, [await createMapIx(dm, i, 16, 16, 1, 1, { biome })]);
      if (biome === BIOME.Forest) console.log(`create_map: ${cu} CU`);
      const addr = await mapAddress(dm.address, i);
      assert.equal((await readMap(addr)).biome, biome);
      assert.equal((await data(addr)).length, MAP_SIZE);
    }
  });

  it("rejects a biome past Crypt", async () => {
    const dm = await funded();
    for (const biome of [3, 255]) {
      await expectErr(send(dm, [await createMapIx(dm, 0, 16, 16, 1, 1, { biome })]), err("InvalidBiome"));
    }
  });

  it("rejects the same index twice", async () => {
    const dm = await funded();
    await create(dm, 3);
    // System program: account already in use.
    await expectErr(send(dm, [await createMapIx(dm, 3, 16, 24, 1)]), 0);
  });
});

describe("map: write_layout", () => {
  it("writes a chunk of cell bytes at its offset, with their variants", async () => {
    const dm = await funded();
    const { map, layout } = await create(dm);
    const chunk = [WALL, cell(KIND.ShallowWater, 3), cell(KIND.Chasm, 7)];
    await send(dm, [await writeLayoutIx(dm, map, 100, chunk)]);
    const { cells } = await readLayout(layout);
    assert.deepEqual(Array.from(cells.slice(99, 104)), [0, ...chunk, 0]);
  });

  it("writes a full 32x32 grid in two chunks", async () => {
    const dm = await funded();
    const { map, layout } = await create(dm, 0, 32, 32);
    const grid = Array.from({ length: MAP_CELLS }, (_, i) => i % 256);
    await send(dm, [await writeLayoutIx(dm, map, 0, grid.slice(0, 512))]);
    await send(dm, [await writeLayoutIx(dm, map, 512, grid.slice(512))]);
    assert.deepEqual(Array.from((await readLayout(layout)).cells), grid);
  });

  it("rejects a chunk past the end of the grid", async () => {
    const dm = await funded();
    const { map } = await create(dm);
    await expectErr(send(dm, [await writeLayoutIx(dm, map, MAP_CELLS - 10, new Array(11).fill(0))]), err("ChunkOutOfRange"));
    await expectErr(send(dm, [await writeLayoutIx(dm, map, 0xffff, [0])]), err("ChunkOutOfRange"));
  });

  it("rejects a signer that is not the DM", async () => {
    const dm = await funded();
    const other = await funded();
    const { map } = await create(dm);
    await expectErr(send(other, [await writeLayoutIx(other, map, 0, [0])]), err("NotDm"));
  });

  it("rejects writes after finalize", async () => {
    const dm = await funded();
    const { map } = await demoFinal(dm);
    await expectErr(send(dm, [await writeLayoutIx(dm, map, 0, [WALL])]), err("NotDraft"));
  });
});

describe("map: write_spawns", () => {
  it("writes the spawns", async () => {
    const dm = await funded();
    const { map, layout } = await create(dm);
    await send(dm, [await writeSpawnsIx(dm, map, demoSpawns())]);
    const l = await readLayout(layout);
    assert.equal(l.spawnCount, demoSpawns().length);
    assert.deepEqual(Array.from(l.spawns.slice(0, 9)), spawnBytes(demoSpawns()[0]));
  });

  it("clears old slots when the new list is shorter", async () => {
    const dm = await funded();
    const { map, layout } = await create(dm);
    await send(dm, [await writeSpawnsIx(dm, map, demoSpawns())]);
    const one = [{ id: WOLF, x: 5, y: 5, pack: 1, skills: 0b0111 }];
    await send(dm, [await writeSpawnsIx(dm, map, one)]);
    const l = await readLayout(layout);
    assert.equal(l.spawnCount, 1);
    assert.deepEqual(Array.from(l.spawns.slice(0, 9)), spawnBytes(one[0]));
    assert.ok(l.spawns.slice(9).every((b) => b === 0));
  });

  it("rejects more than SPAWNS_MAX spawns", async () => {
    const dm = await funded();
    const { map } = await create(dm);
    const tooMany = Array.from({ length: SPAWNS_MAX + 1 }, () => ({ id: WOLF, x: 5, y: 5, pack: 1, skills: 0b0111 }));
    await expectErr(send(dm, [await writeSpawnsIx(dm, map, tooMany)]), err("TooManySpawns"));
  });

  it("rejects a signer that is not the DM", async () => {
    const dm = await funded();
    const other = await funded();
    const { map } = await create(dm);
    await expectErr(send(other, [await writeSpawnsIx(other, map, demoSpawns())]), err("NotDm"));
  });

  it("rejects writes after finalize", async () => {
    const dm = await funded();
    const { map } = await demoFinal(dm);
    await expectErr(send(dm, [await writeSpawnsIx(dm, map, [])]), err("NotDraft"));
  });
});

describe("map: finalize_map", () => {
  it("finalizes the demo map and stores the layout hash", async () => {
    const dm = await funded();
    const { map, layout } = await demoDraft(dm);
    const cu = await send(dm, [await finalizeIx(dm, map, new Array(32).fill(9))]);
    console.log(`finalize 16x16: ${cu} CU`);
    const m = await readMap(map);
    assert.equal(m.status, STATUS.Finalized);
    assert.deepEqual(m.secretsRoot, new Array(32).fill(9));
    // Anyone can check the hash against the layout account bytes.
    const raw = (await readLayout(layout)).raw;
    // Arena update: the hash covers the arenas too. Patch C: and the public part of the secrets,
    // not the encrypted copy.
    const expected = createHash("sha256").update(raw.slice(8, 8 + HASHED_SIZE)).digest();
    assert.deepEqual(m.layoutHash, Array.from(expected));
  });

  it("finalizes a 32x32 map within the default compute budget", async () => {
    const dm = await funded();
    const { map } = await create(dm, 0, 32, 32);
    // Every starter floor with every variant: the BFS reads each cell's type from the tileset.
    const floors = [KIND.StoneFloor, KIND.WoodenFloor, KIND.Dirt, KIND.ShallowWater, KIND.Stairs, KIND.Rubble];
    const grid = Array.from({ length: MAP_CELLS }, (_, i) => cell(floors[i % floors.length], i % 8));
    await send(dm, [await writeLayoutIx(dm, map, 0, grid.slice(0, 512))]);
    await send(dm, [await writeLayoutIx(dm, map, 512, grid.slice(512))]);
    await send(dm, [await writeSpawnsIx(dm, map, publicSpawns())]);
    await send(dm, [await writePointsIx(dm, map, points({ x: 0, y: 0 }, { x: 31, y: 31 }, DEMO_CHESTS), T1_BOSS)]);
    await writeDemoArenas(dm, map);
    await send(dm, [await writeSecretsIx(dm, map)]);
    const cu = await send(dm, [await finalizeIx(dm, map, ROOT)]);
    console.log(`finalize 32x32: ${cu} CU`);
    assert.ok(cu < 200_000, `${cu} CU`);
    assert.equal((await readMap(map)).status, STATUS.Finalized);
  });

  it("rejects a map without a path from the entrance to the exit", async () => {
    const dm = await funded();
    const { map } = await demoDraft(dm);
    // A wall across row 13 cuts the exit off.
    await send(dm, [await writeLayoutIx(dm, map, 13 * MAP_MAX + 1, new Array(14).fill(WALL))]);
    await expectErr(send(dm, [await finalizeIx(dm, map, ROOT)]), err("ExitUnreachable"));
    assert.equal((await readMap(map)).status, STATUS.Draft);
    // Deep water and a chasm cut it off too; rubble and shallow water do not.
    for (const [kind, ok] of [[KIND.DeepWater, false], [KIND.Chasm, false], [KIND.Rubble, true], [KIND.ShallowWater, true]] as const) {
      await send(dm, [await writeLayoutIx(dm, map, 13 * MAP_MAX + 1, new Array(14).fill(cell(kind, 2)))]);
      const run = send(dm, [await finalizeIx(dm, map, ROOT)]);
      if (ok) await run;
      else await expectErr(run, err("ExitUnreachable"));
      if (ok) break;
    }
    assert.equal((await readMap(map)).status, STATUS.Finalized);
  });

  it("patch 13: a diagonal never cuts a corner", async () => {
    const dm = await funded();
    const { map } = await demoDraft(dm);
    // Rows 8 and 9 are walls but (7, 8) and (8, 9): the diagonal between them passes the walls
    // (8, 8) and (7, 9), touching at the corner, so the halves stay apart.
    const row = (y: number, open: number[]) => writeLayoutIx(dm, map, y * MAP_MAX + 1, Array.from({ length: 14 }, (_, i) => (open.includes(i + 1) ? FLOOR : WALL)));
    await send(dm, [await row(8, [7])]);
    await send(dm, [await row(9, [8])]);
    await expectErr(send(dm, [await finalizeIx(dm, map, ROOT)]), err("ExitUnreachable"));
    // A single impassable cell at the corner still blocks: (8, 8) deep water, (7, 9) a wall.
    await send(dm, [await writeLayoutIx(dm, map, 8 * MAP_MAX + 8, [cell(KIND.DeepWater)])]);
    await expectErr(send(dm, [await finalizeIx(dm, map, ROOT)]), err("ExitUnreachable"));
    // Both corners passable: the way is open.
    await send(dm, [await writeLayoutIx(dm, map, 8 * MAP_MAX + 8, [cell(KIND.Rubble)])]);
    await send(dm, [await finalizeIx(dm, map, ROOT)]);
    assert.equal((await readMap(map)).status, STATUS.Finalized);
  });

  it("patch C: only the visible monsters are public; the threats and the declared counts", async () => {
    const dm = await funded();
    const { map } = await demoDraft(dm);
    // A hidden monster left in the public list.
    await send(dm, [await writeSpawnsIx(dm, map, [...publicSpawns(), demoSpawns()[1]])]);
    await expectErr(send(dm, [await finalizeIx(dm, map, ROOT)]), err("SpawnNotVisible"));
    await send(dm, [await writeSpawnsIx(dm, map, publicSpawns())]);
    // A used pack with threat 0, an unused one with a threat.
    for (const [t, name] of [[[0, 0, 2, 5, 0, 0, 0, 0], "ThreatOutOfRange"], [[1, 2, 2, 5, 0, 0, 0, 0], "ThreatOutOfRange"]] as const) {
      await send(dm, [await writeSecretsIx(dm, map, { threat: [...t] })]);
      await expectErr(send(dm, [await finalizeIx(dm, map, ROOT)]), err(name));
    }
    // Tier 1: at least one trap, at most two (ambush traps included), one stash, one ambush per
    // ambush pack (none here).
    for (const [d, name] of [[[0, 0, 0, 0], "TooFewTraps"], [[3, 0, 0, 0], "TooManySecrets"], [[1, 0, 2, 0], "TooManySecrets"], [[1, 1, 0, 0], "AmbushUnlinked"]] as const) {
      await send(dm, [await writeSecretsIx(dm, map, { declared: [...d] })]);
      await expectErr(send(dm, [await finalizeIx(dm, map, ROOT)]), err(name));
    }
    await send(dm, [await writeSecretsIx(dm, map)]);
    // No root.
    await expectErr(send(dm, [await finalizeIx(dm, map, new Array(32).fill(0))]), err("SecretsMissing"));
    await send(dm, [await finalizeIx(dm, map, ROOT)]);
    assert.equal((await readMap(map)).status, STATUS.Finalized);
  });

  it("patch C: an ambush pack shows no monster and needs its declared ambush", async () => {
    const dm = await funded();
    const { map } = await demoDraft(dm);
    // Pack 2 becomes an ambush pack: its goblin leaves the public list.
    const ambush = { ...DEMO_PACKS()[1], visible: NO_SPAWN, x: 0, y: 0 };
    await send(dm, [await writeArenaIx(dm, map, ambush)]);
    const spawns = publicSpawns();
    await send(dm, [await writeSpawnsIx(dm, map, [spawns[0], spawns[2]])]);
    const pack3 = { ...DEMO_PACKS()[2], visible: 1 };
    await send(dm, [await writeArenaIx(dm, map, pack3)]);
    await expectErr(send(dm, [await finalizeIx(dm, map, ROOT)]), err("AmbushUnlinked"));
    await send(dm, [await writeSecretsIx(dm, map, { declared: [0, 1, 0, 0] })]);
    await send(dm, [await finalizeIx(dm, map, ROOT)]);
    assert.equal((await readMap(map)).status, STATUS.Finalized);
  });

  it("patch 6: the spawns' skills, elite flags and the registry tables", async () => {
    const dm = await funded();
    const { map } = await demoDraft(dm);
    const finalizeWith = async (edit: (s: Spawn[]) => void) => {
      const spawns = publicSpawns();
      edit(spawns);
      await send(dm, [await writeSpawnsIx(dm, map, spawns)]);
      return send(dm, [await finalizeIx(dm, map, ROOT)]);
    };
    // The wolf's pool has 4 skills: bit 4 is past it.
    await expectErr(finalizeWith((s) => (s[0].skills = 0b10111)), err("MonsterSkillUnknown"));
    // 3 active slots: one chosen is too few, all 4 too many.
    await expectErr(finalizeWith((s) => (s[0].skills = 0b1)), err("MonsterSkillCount"));
    await expectErr(finalizeWith((s) => (s[0].skills = 0b1111)), err("MonsterSkillCount"));
    // Elite: one more active slot; never a boss; one per tier-1 map.
    await expectErr(finalizeWith((s) => (s[0].flags = 1)), err("MonsterSkillCount"));
    await expectErr(finalizeWith((s) => (s[2].flags = 1)), err("EliteBoss"));
    // Patch C: the elites over the map are counted when the packs are revealed (the session).
    // Other accounts in place of the registry tables.
    await send(dm, [await writeSpawnsIx(dm, map, publicSpawns())]);
    const tileset = await tableAddress("tileset");
    const skills = await tableAddress("monster_skills");
    const rewards = await tableAddress("rewards");
    await expectErr(send(dm, [await finalizeIx(dm, map, ROOT, [tileset, skills, rewards, tileset])]), err("InvalidRegistryTable"));
    // Patch 13: another table in place of the tileset.
    await expectErr(send(dm, [await finalizeIx(dm, map, ROOT, [await tableAddress("bestiary"), skills, rewards, rewards])]), err("InvalidRegistryTable"));
    // An elite wolf with all 4 skills finalizes.
    const cu = await finalizeWith((s) => {
      s[0].flags = 1;
      s[0].skills = 0b1111;
    });
    console.log(`finalize 16x16 with an elite: ${cu} CU`);
    assert.equal((await readMap(map)).status, STATUS.Finalized);
  });

  it("rejects an empty draft: void cells, no entrance", async () => {
    const dm = await funded();
    const { map } = await create(dm);
    await expectErr(send(dm, [await finalizeIx(dm, map, ROOT)]), err("EntranceCount"));
  });

  it("patch 13: the entrance and the exit points", async () => {
    const dm = await funded();
    const { map } = await demoDraft(dm);
    const root = ROOT;
    const finalizeWith = async (list: Point[]) => {
      await send(dm, [await writePointsIx(dm, map, list, T1_BOSS)]);
      return send(dm, [await finalizeIx(dm, map, root)]);
    };
    await expectErr(finalizeWith(points(null, EXIT, DEMO_CHESTS)), err("EntranceCount"));
    await expectErr(finalizeWith(points(ENTRANCE, null, DEMO_CHESTS)), err("ExitCount"));
    await expectErr(finalizeWith([...points(ENTRANCE, EXIT, DEMO_CHESTS), ...points({ x: 2, y: 1 }, null)]), err("EntranceCount"));
    await expectErr(finalizeWith([...points(ENTRANCE, EXIT, DEMO_CHESTS), ...points(null, { x: 13, y: 14 })]), err("ExitCount"));
    await expectErr(finalizeWith(points({ x: 16, y: 1 }, EXIT, DEMO_CHESTS)), err("PointOutOfBounds"));
    await expectErr(finalizeWith(points({ x: 0, y: 1 }, EXIT, DEMO_CHESTS)), err("PointNotPassable"));
    await expectErr(finalizeWith(points(ENTRANCE, ENTRANCE, DEMO_CHESTS)), err("PointOverlap"));
    // Arena update: the exit on a pack's map cell.
    await expectErr(finalizeWith(points(ENTRANCE, { x: 6, y: 2 }, DEMO_CHESTS)), err("PackOverlap"));
    // The exit on a chasm, then on stairs; any order of the points.
    await send(dm, [await writeLayoutIx(dm, map, EXIT.y * MAP_MAX + EXIT.x, [cell(KIND.Chasm)])]);
    await expectErr(finalizeWith(points(ENTRANCE, EXIT, DEMO_CHESTS)), err("PointNotPassable"));
    await send(dm, [await writeLayoutIx(dm, map, EXIT.y * MAP_MAX + EXIT.x, [cell(KIND.Stairs, 4)])]);
    await finalizeWith(points(ENTRANCE, EXIT, DEMO_CHESTS).reverse());
    assert.equal((await readMap(map)).status, STATUS.Finalized);
  });

  it("patch 13: a tile type the tileset does not have", async () => {
    const dm = await funded();
    const { map } = await demoDraft(dm);
    await send(dm, [await writeLayoutIx(dm, map, 5 * MAP_MAX + 9, [cell(11), cell(31, 7)])]);
    await expectErr(send(dm, [await finalizeIx(dm, map, ROOT)]), err("InvalidTile"));
    // Outside the map any byte is ignored.
    await send(dm, [await writeLayoutIx(dm, map, 5 * MAP_MAX + 9, [FLOOR, FLOOR])]);
    await send(dm, [await writeLayoutIx(dm, map, 5 * MAP_MAX + 20, [0xff])]);
    await send(dm, [await finalizeIx(dm, map, ROOT)]);
    assert.equal((await readMap(map)).status, STATUS.Finalized);
  });

  it("rejects a signer that is not the DM", async () => {
    const dm = await funded();
    const other = await funded();
    const { map } = await demoDraft(dm);
    await expectErr(send(other, [await finalizeIx(other, map, new Array(32).fill(0))]), err("NotDm"));
  });

  it("rejects finalizing twice", async () => {
    const dm = await funded();
    const { map } = await demoFinal(dm);
    await expectErr(send(dm, [await finalizeIx(dm, map, new Array(32).fill(1))]), err("NotDraft"));
    assert.deepEqual((await readMap(map)).secretsRoot, ROOT);
  });
});

// The happy path (session authority PDA via CPI, trusted in GameConfig) needs the registry
// and session programs: covered by the cross-program tests (L7 open_session, L8.2). The trust
// check itself is unit-tested in programs/map/src/trust.rs.
describe("map: set_status", () => {
  it("rejects a wallet signing as the session authority", async () => {
    const dm = await funded();
    const { map } = await demoFinal(dm);
    const config = await configAddress();
    // GameConfig is a shared singleton: read it if it exists instead of assuming either way.
    const { value } = await rpc.getAccountInfo(config, { encoding: "base64", commitment: "confirmed" }).send();
    const registryConfig = value?.owner === PROGRAMS.registry;
    const ix = setStatusIx(map, dm, config, STATUS.InSession);
    await expectErr(send(dm, [ix]), err(registryConfig ? "UntrustedCaller" : "InvalidConfig"));
    assert.equal((await readMap(map)).status, STATUS.Finalized);
  });

  it("rejects a config at another address", async () => {
    const dm = await funded();
    const { map } = await demoFinal(dm);
    const ix = setStatusIx(map, dm, SYSTEM_PROGRAM, STATUS.InSession);
    await expectErr(send(dm, [ix]), CONSTRAINT_SEEDS);
  });
});

describe("map: write_points (patch 13)", () => {
  it("writes the points and the boss table, then clears old slots", async () => {
    const dm = await funded();
    const { map, layout } = await create(dm);
    const two = [{ x: 5, y: 5, table: T1_CHEST }, { x: 6, y: 6, table: T1_BOSS }];
    await send(dm, [await writeChestsIx(dm, map, two, T1_BOSS)]);
    let l = await readLayout(layout);
    assert.deepEqual(l.points, points(ENTRANCE, EXIT, two));
    assert.equal(l.bossTable, T1_BOSS);
    await send(dm, [await writePointsIx(dm, map, points(null, EXIT, two.slice(1)), NO_TABLE)]);
    l = await readLayout(layout);
    assert.deepEqual(l.points, points(null, EXIT, two.slice(1)));
    assert.equal(l.bossTable, NO_TABLE);
    // The bytes after the used points are zero.
    assert.ok(l.raw.slice(POINTS_AT + 2 * 4, POINTS_AT + POINTS_MAX * 4).every((b) => b === 0));
  });

  it("rejects more than POINTS_MAX points", async () => {
    const dm = await funded();
    const { map } = await create(dm);
    const many = Array.from({ length: CHESTS_MAX + 1 }, (_, i) => ({ x: 2 + i, y: 2, table: T1_CHEST }));
    await expectErr(send(dm, [await writeChestsIx(dm, map, many, T1_BOSS)]), err("TooManyPoints"));
  });

  it("rejects a signer that is not the DM", async () => {
    const dm = await funded();
    const other = await funded();
    const { map } = await create(dm);
    await expectErr(send(other, [await writeChestsIx(other, map, DEMO_CHESTS, T1_BOSS)]), err("NotDm"));
  });

  it("rejects writes after finalize", async () => {
    const dm = await funded();
    const { map } = await demoFinal(dm);
    await expectErr(send(dm, [await writeChestsIx(dm, map, [], T1_BOSS)]), err("NotDraft"));
  });
});

describe("map: finalize_map chests and rewards (patch 11)", () => {
  const root = ROOT;

  it("checks the chest rules of the builder", async () => {
    const dm = await funded();
    const { map } = await demoDraft(dm);
    const finalizeWith = async (chests: Chest[], boss: number) => {
      await send(dm, [await writeChestsIx(dm, map, chests, boss)]);
      return send(dm, [await finalizeIx(dm, map, root)]);
    };
    // Tier 1 allows one chest.
    await expectErr(finalizeWith([...DEMO_CHESTS, { x: 6, y: 6, table: T1_CHEST }], T1_BOSS), err("TooManyChests"));
    await expectErr(finalizeWith([{ x: 4, y: 4, table: T1_CHEST }, ...DEMO_CHESTS, { x: 6, y: 6, table: T1_CHEST }, { x: 7, y: 7, table: T1_CHEST }], T1_BOSS), err("TooManyChests"));
    await expectErr(finalizeWith([{ x: 0, y: 5, table: T1_CHEST }], T1_BOSS), err("ChestNotPassable"));
    await send(dm, [await writeLayoutIx(dm, map, 5 * MAP_MAX + 9, [cell(KIND.DeepWater)])]);
    await expectErr(finalizeWith([{ x: 9, y: 5, table: T1_CHEST }], T1_BOSS), err("ChestNotPassable"));
    await send(dm, [await writeLayoutIx(dm, map, 5 * MAP_MAX + 9, [FLOOR])]);
    await expectErr(finalizeWith([{ x: 20, y: 5, table: T1_CHEST }], T1_BOSS), err("ChestOutOfBounds"));
    await expectErr(finalizeWith([{ ...ENTRANCE, table: T1_CHEST }], T1_BOSS), err("ChestOverlap"));
    // Arena update: a chest on a pack's map cell is the pack's overlap.
    await expectErr(finalizeWith([{ x: 6, y: 2, table: T1_CHEST }], T1_BOSS), err("PackOverlap"));
    await expectErr(finalizeWith([{ x: 5, y: 5, table: 31 }], T1_BOSS), err("UnknownTable"));
    // A tier 2 table on a tier 1 map.
    await expectErr(finalizeWith([{ x: 5, y: 5, table: T2_CHEST }], T1_BOSS), err("TableNotAllowed"));
    await expectErr(finalizeWith(DEMO_CHESTS, NO_TABLE), err("BossTableMissing"));
    // A body table is not on the allowed list of any tier. (A one-level boss table, BossTableOneLevel,
    // needs a one-level table on the list: crates/mapcheck unit tests.)
    await expectErr(finalizeWith(DEMO_CHESTS, 4), err("TableNotAllowed"));
    // Other accounts in place of the rewards table.
    await send(dm, [await writeChestsIx(dm, map, DEMO_CHESTS, T1_BOSS)]);
    const bestiary = await tableAddress("bestiary");
    const skills = await tableAddress("monster_skills");
    await expectErr(send(dm, [await finalizeIx(dm, map, root, [bestiary, skills, bestiary, await tableAddress("tileset")])]), err("InvalidRegistryTable"));
    const cu = await finalizeWith(DEMO_CHESTS, T1_BOSS);
    console.log(`finalize 16x16 with a chest: ${cu} CU`);
    assert.equal((await readMap(map)).status, STATUS.Finalized);
  });

  it("a closed chest blocks the way and needs a reachable neighbor", async () => {
    const dm = await funded();
    const { map } = await demoDraft(dm);
    // A wall at x = 13 with one gap at (13, 7) between the room and the exit.
    for (let y = 1; y < 15; y++) await send(dm, [await writeLayoutIx(dm, map, y * MAP_MAX + 13, [WALL])]);
    await send(dm, [await writeLayoutIx(dm, map, 7 * MAP_MAX + 13, [FLOOR])]);
    await send(dm, [await writeChestsIx(dm, map, [{ x: 13, y: 7, table: T1_CHEST }], T1_BOSS)]);
    await expectErr(send(dm, [await finalizeIx(dm, map, root)]), err("ExitUnreachable"));
    // A chest walled in on every side.
    await send(dm, [await writeLayoutIx(dm, map, 7 * MAP_MAX + 13, [WALL])]);
    await send(dm, [await writeLayoutIx(dm, map, 14 * MAP_MAX + 13, [FLOOR])]);
    for (const [x, y] of [[4, 4], [5, 4], [6, 4], [4, 5], [6, 5], [4, 6], [5, 6], [6, 6]]) {
      await send(dm, [await writeLayoutIx(dm, map, y * MAP_MAX + x, [WALL])]);
    }
    await send(dm, [await writeChestsIx(dm, map, DEMO_CHESTS, T1_BOSS)]);
    await expectErr(send(dm, [await finalizeIx(dm, map, root)]), err("ChestUnreachable"));
  });

  it("the budget grows with the map's minimum players", async () => {
    // Tier 2: two chests (4 + 4) and the boss (6) cost 14; the budget is 10 for one player and 16
    // for three (LootTuning::DEFAULT).
    const spawns = publicSpawns().map((s) => ({ ...s, level: 3 }));
    const chests = [{ x: 5, y: 5, table: T2_CHEST }, { x: 8, y: 8, table: T2_CHEST }];
    for (const [min, ok] of [[1, false], [3, true]] as const) {
      const dm = await funded();
      const { map } = await create(dm, 0, 16, 16, 2, min);
      await send(dm, [await writeLayoutIx(dm, map, 0, demoTiles())]);
      await send(dm, [await writeSpawnsIx(dm, map, spawns)]);
      await send(dm, [await writeChestsIx(dm, map, chests, T2_BOSS)]);
      await writeDemoArenas(dm, map);
      await send(dm, [await writeSecretsIx(dm, map, { threat: [0, 6, 6, 15, 0, 0, 0, 0], declared: [0, 0, 0, 0] })]);
      const run = send(dm, [await finalizeIx(dm, map, root)]);
      if (ok) {
        await run;
        assert.equal((await readMap(map)).status, STATUS.Finalized);
      } else {
        await expectErr(run, err("OverBudget"));
      }
    }
  });
});

describe("map: write_arena (arena update)", () => {
  const root = ROOT;

  it("writes a pack's record and arena rows, then replaces and clears them", async () => {
    const dm = await funded();
    const { map, layout } = await demoDraft(dm);
    const cells = Array.from({ length: 6 * 7 }, (_, i) => cell(KIND.StoneFloor, i % 8));
    const cu = await send(dm, [await writeArenaIx(dm, map, { pack: 7, visible: 1, x: 9, y: 3, facing: FACING.NW, width: 6, height: 7, cells })]);
    console.log(`write_arena 6x7: ${cu} CU`);
    const { arenas } = getMapLayoutDecoder().decode(await data(layout));
    assert.deepEqual(arenas.packs[7], { visible: 1, x: 9, y: 3, facing: FACING.NW, width: 6, height: 7 });
    // Rows are stored with ARENA_SIDE_MAX cells, the rest zero.
    for (let y = 0; y < 7; y++) {
      assert.deepEqual(Array.from(arenas.cells[7].slice(y * ARENA_SIDE_MAX, y * ARENA_SIDE_MAX + 6)), cells.slice(y * 6, y * 6 + 6));
      assert.equal(arenas.cells[7][y * ARENA_SIDE_MAX + 6], 0);
    }
    // A width of 0 clears the pack.
    await send(dm, [await writeArenaIx(dm, map, { pack: 7, visible: 0, x: 0, y: 0, facing: 0, width: 0, height: 0, cells: [] })]);
    const after = getMapLayoutDecoder().decode(await data(layout)).arenas;
    assert.equal(after.packs[7].width, 0);
    assert.ok(after.cells[7].every((b) => b === 0));
    // The largest arena fits one transaction.
    const side = ARENA_SIDE_MAX;
    await send(dm, [await writeArenaIx(dm, map, { pack: 6, visible: 0, x: 1, y: 1, facing: 0, width: side, height: side, cells: openArena(side, side) })]);
  });

  it("rejects a pack number past PACKS_MAX and cells that do not match the size", async () => {
    const dm = await funded();
    const { map } = await demoDraft(dm);
    const p = DEMO_PACKS()[0];
    await expectErr(send(dm, [await writeArenaIx(dm, map, { ...p, pack: PACKS_MAX })]), err("PackOutOfRange"));
    await expectErr(send(dm, [await writeArenaIx(dm, map, { ...p, cells: openArena(8, 7) })]), err("ArenaCellsMismatch"));
    await expectErr(send(dm, [await writeArenaIx(dm, map, { ...p, width: 13, height: 1, cells: openArena(13, 1) })]), err("ArenaCellsMismatch"));
  });

  it("rejects a signer that is not the DM, and writes after finalize", async () => {
    const dm = await funded();
    const stranger = await funded();
    const { map } = await demoDraft(dm);
    const ix = await writeArenaIx(stranger, map, DEMO_PACKS()[0]);
    await expectErr(send(stranger, [ix]), err("NotDm"));
    await send(dm, [await finalizeIx(dm, map, root)]);
    await expectErr(send(dm, [await writeArenaIx(dm, map, DEMO_PACKS()[0])]), err("NotDraft"));
  });

  it("finalize_map checks the packs and their arenas", async () => {
    const dm = await funded();
    const { map } = await demoDraft(dm);
    const finalizeWith = async (p: Pack) => {
      await writeDemoArenas(dm, map);
      await send(dm, [await writeArenaIx(dm, map, p)]);
      return send(dm, [await finalizeIx(dm, map, root)]);
    };
    const [wolves] = DEMO_PACKS();
    await expectErr(finalizeWith({ ...wolves, width: 0, height: 0, cells: [] }), err("PackWithoutArena"));
    await expectErr(finalizeWith({ ...wolves, width: 4, height: 8, cells: openArena(4, 8) }), err("SpawnOutOfBounds"));
    await expectErr(finalizeWith({ ...wolves, width: 5, height: 8, cells: openArena(5, 8) }), err("ArenaSizeOutOfRange"));
    await expectErr(finalizeWith({ ...wolves, width: 8, height: 5, cells: openArena(8, 5) }), err("ArenaSizeOutOfRange"));
    // Patch C: the public list holds only visible monsters, so another one is not this pack's.
    await expectErr(finalizeWith({ ...wolves, visible: 2 }), err("SpawnNotVisible"));
    await expectErr(finalizeWith({ ...wolves, x: 2, y: 2 }), err("PackAtEntrance"));
    await expectErr(finalizeWith({ ...wolves, x: 2, y: 10 }), err("PackOverlap"));
    await expectErr(finalizeWith({ ...wolves, x: 0, y: 5 }), err("PackNotPassable"));
    const walled = openArena();
    // A wall ring around the wolves at (4, 4) and (5, 4).
    for (const [x, y] of [[3, 3], [4, 3], [5, 3], [6, 3], [3, 4], [6, 4], [3, 5], [4, 5], [5, 5], [6, 5]]) walled[y * 8 + x] = WALL;
    await expectErr(finalizeWith({ ...wolves, cells: walled }), err("ArenaUnreachable"));
    const cu = await finalizeWith(wolves);
    console.log(`finalize 16x16 with 3 arenas: ${cu} CU`);
    assert.equal((await readMap(map)).status, STATUS.Finalized);
  });
});

describe("map: write_secrets (patch C)", () => {
  it("writes the encrypted copy in chunks with the public part", async () => {
    const dm = await funded();
    const { map, layout } = await demoDraft(dm);
    const blob = Array.from({ length: 700 }, (_, i) => (i * 7) & 0xff);
    const cu = await send(dm, [await writeSecretsIx(dm, map, { offset: 0, bytes: blob.slice(0, 600), len: 700, declared: [2, 0, 1, 1] })]);
    console.log(`write_secrets 600 bytes: ${cu} CU`);
    await send(dm, [await writeSecretsIx(dm, map, { offset: 600, bytes: blob.slice(600), len: 700, declared: [2, 0, 1, 1] })]);
    const { secrets } = getMapLayoutDecoder().decode(await data(layout));
    assert.deepEqual(Array.from(secrets.threat), DEMO_THREAT);
    assert.deepEqual(Array.from(secrets.declared), [2, 0, 1, 1]);
    assert.equal(secrets.blobLen, 700);
    assert.deepEqual(Array.from(secrets.blob.slice(0, 700)), blob);
  });

  it("rejects a chunk past the blob, a signer that is not the DM, and writes after finalize", async () => {
    const dm = await funded();
    const stranger = await funded();
    const { map } = await demoDraft(dm);
    await expectErr(send(dm, [await writeSecretsIx(dm, map, { offset: 3, bytes: [1, 2], len: 4 })]), err("SecretsOutOfRange"));
    await expectErr(send(dm, [await writeSecretsIx(dm, map, { bytes: [], len: SECRETS_BLOB_MAX + 1 })]), err("SecretsOutOfRange"));
    await expectErr(send(stranger, [await writeSecretsIx(stranger, map)]), err("NotDm"));
    await send(dm, [await finalizeIx(dm, map, ROOT)]);
    await expectErr(send(dm, [await writeSecretsIx(dm, map)]), err("NotDraft"));
  });

  it("finalize_map needs the encrypted copy", async () => {
    const dm = await funded();
    const { map } = await create(dm);
    await send(dm, [await writeLayoutIx(dm, map, 0, demoTiles())]);
    await send(dm, [await writeSpawnsIx(dm, map, publicSpawns())]);
    await send(dm, [await writeChestsIx(dm, map, DEMO_CHESTS, T1_BOSS)]);
    await writeDemoArenas(dm, map);
    // The public part without the encrypted copy.
    await send(dm, [await writeSecretsIx(dm, map, { bytes: [], len: 0 })]);
    await expectErr(send(dm, [await finalizeIx(dm, map, ROOT)]), err("SecretsMissing"));
  });
});
