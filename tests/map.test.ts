// Map program tests on a local validator:
//   anchor build --tools-version v1.54
//   anchor test --validator legacy --skip-build
// Instruction discriminators and error codes come from target/idl/deadroll_map.json.
import { strict as assert } from "node:assert";
import { createHash } from "node:crypto";
import { readFileSync } from "node:fs";
import { describe, it } from "node:test";
import {
  AccountRole,
  address,
  airdropFactory,
  appendTransactionMessageInstructions,
  createSolanaRpc,
  createSolanaRpcSubscriptions,
  createTransactionMessage,
  generateKeyPairSigner,
  getAddressEncoder,
  getBase64Encoder,
  getProgramDerivedAddress,
  getSignatureFromTransaction,
  lamports,
  pipe,
  sendAndConfirmTransactionFactory,
  setTransactionMessageFeePayerSigner,
  setTransactionMessageLifetimeUsingBlockhash,
  signTransactionMessageWithSigners,
} from "@solana/kit";

const idl = JSON.parse(readFileSync(new URL("../target/idl/deadroll_map.json", import.meta.url), "utf8"));
const MAP_PROGRAM = address(idl.address);
const REGISTRY_PROGRAM = address("dZ1jraMfS3bAUTkRw2TA1ddg8UDGzgUdFQz5pmBxjoG");
const SYSTEM_PROGRAM = address("11111111111111111111111111111111");

const rpc = createSolanaRpc("http://127.0.0.1:8899");
const rpcSubscriptions = createSolanaRpcSubscriptions("ws://127.0.0.1:8900");
const airdrop = airdropFactory({ rpc, rpcSubscriptions });
const sendAndConfirm = sendAndConfirmTransactionFactory({ rpc, rpcSubscriptions });
const enc = getAddressEncoder();

// Shared constants (crates/types, crates/mapcheck).
const MAP_MAX = 32;
const MAP_CELLS = 1024;
const SPAWNS_MAX = 24;
const TILE = { Floor: 0, Wall: 1, Water: 2, Door: 3, Entrance: 4, Exit: 5, Trap: 6 };
const STATUS = { Draft: 0, Finalized: 1, InSession: 2, Finished: 3 };
const WOLF = 0, GOBLIN = 1, CHIEFTAIN = 2, WARRIOR = 3, SHAMAN = 4;
/** `MapLayoutData::SIZE`: the bytes `layout_hash` covers, after the discriminator. */
const LAYOUT_CORE_SIZE = 1 + 32 + MAP_CELLS + SPAWNS_MAX * 9 + 1 + 2 + 2 + 1;
/** Anchor `ConstraintSeeds`. */
const CONSTRAINT_SEEDS = 2006;

const ENTRANCE = { x: 1, y: 1 };
const EXIT = { x: 14, y: 14 };

type Cell = { x: number; y: number };
type Spawn = { id: number; x: number; y: number; pack: number; level?: number };

// ---------- encoding ----------

function err(name: string): number {
  const e = idl.errors.find((x: { name: string }) => x.name === name);
  assert.ok(e, `no error ${name} in the IDL`);
  return e.code;
}

function disc(ix: string): Uint8Array {
  const i = idl.instructions.find((x: { name: string }) => x.name === ix);
  assert.ok(i, `no instruction ${ix} in the IDL`);
  return Uint8Array.from(i.discriminator);
}

function concat(...parts: (Uint8Array | number[])[]): Uint8Array {
  const out = new Uint8Array(parts.reduce((n, p) => n + p.length, 0));
  let o = 0;
  for (const p of parts) {
    out.set(p, o);
    o += p.length;
  }
  return out;
}

const u16 = (v: number) => [v & 0xff, (v >> 8) & 0xff];
const u32 = (v: number) => [v & 0xff, (v >> 8) & 0xff, (v >> 16) & 0xff, (v >>> 24) & 0xff];
const vec = (bytes: Uint8Array | number[]) => concat(u32(bytes.length), bytes);
const cell = (c: Cell) => [c.x, c.y];
const spawnBytes = (s: Spawn) => [s.id, s.x, s.y, s.level ?? 1, ...u16(0), 0, 0, s.pack];

// ---------- addresses ----------

async function pda(seeds: (string | Uint8Array | number[])[], program = MAP_PROGRAM) {
  const bytes = seeds.map((s) => (typeof s === "string" ? new TextEncoder().encode(s) : Uint8Array.from(s)));
  const [addr] = await getProgramDerivedAddress({ programAddress: program, seeds: bytes });
  return addr;
}

const mapAddress = (dm: string, index: number) => pda(["map", enc.encode(address(dm)), u32(index)]);
const layoutAddress = (map: string) => pda(["layout", enc.encode(address(map))]);
const configAddress = () => pda(["config"], REGISTRY_PROGRAM);

// ---------- instructions ----------

type Signer = Awaited<ReturnType<typeof generateKeyPairSigner>>;

const signerMeta = (s: Signer, writable = false) => ({
  address: s.address,
  role: writable ? AccountRole.WRITABLE_SIGNER : AccountRole.READONLY_SIGNER,
  signer: s,
});
const meta = (a: string, writable = false) => ({
  address: address(a),
  role: writable ? AccountRole.WRITABLE : AccountRole.READONLY,
});

async function createMapIx(dm: Signer, index: number, width: number, height: number, tier: number) {
  const map = await mapAddress(dm.address, index);
  return {
    programAddress: MAP_PROGRAM,
    accounts: [signerMeta(dm, true), meta(map, true), meta(await layoutAddress(map), true), meta(SYSTEM_PROGRAM)],
    data: concat(disc("create_map"), u32(index), [width, height, tier]),
  };
}

async function editAccounts(dm: Signer, map: string) {
  return [signerMeta(dm), meta(map), meta(await layoutAddress(map), true)];
}

async function writeLayoutIx(dm: Signer, map: string, offset: number, tiles: Uint8Array | number[]) {
  return {
    programAddress: MAP_PROGRAM,
    accounts: await editAccounts(dm, map),
    data: concat(disc("write_layout"), u16(offset), vec(tiles)),
  };
}

async function writeSpawnsIx(dm: Signer, map: string, entrance: Cell, exit: Cell, spawns: Spawn[]) {
  return {
    programAddress: MAP_PROGRAM,
    accounts: await editAccounts(dm, map),
    data: concat(disc("write_spawns"), cell(entrance), cell(exit), u32(spawns.length), ...spawns.map(spawnBytes)),
  };
}

async function finalizeIx(dm: Signer, map: string, root: number[]) {
  return {
    programAddress: MAP_PROGRAM,
    accounts: [signerMeta(dm), meta(map, true), meta(await layoutAddress(map))],
    data: concat(disc("finalize_map"), root),
  };
}

function setStatusIx(map: string, authority: Signer, config: string, status: number) {
  return {
    programAddress: MAP_PROGRAM,
    accounts: [meta(map, true), signerMeta(authority), meta(config)],
    data: concat(disc("set_status"), [status]),
  };
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
  await sendAndConfirm(tx as any, { commitment: "confirmed" });
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
  const d = await data(addr);
  return {
    version: d[8],
    dm: d.slice(9, 41),
    index: d[41] | (d[42] << 8) | (d[43] << 16) | (d[44] << 24),
    width: d[45],
    height: d[46],
    status: d[47],
    layoutHash: Array.from(d.slice(48, 80)),
    secretsRoot: Array.from(d.slice(80, 112)),
    tier: d[113],
  };
}

async function readLayout(addr: string) {
  const d = await data(addr);
  const spawnsAt = 41 + MAP_CELLS;
  const countAt = spawnsAt + SPAWNS_MAX * 9;
  return {
    raw: d,
    version: d[8],
    map: d.slice(9, 41),
    tiles: d.slice(41, 41 + MAP_CELLS),
    spawns: d.slice(spawnsAt, countAt),
    spawnCount: d[countAt],
    entrance: { x: d[countAt + 1], y: d[countAt + 2] },
    exit: { x: d[countAt + 3], y: d[countAt + 4] },
  };
}

// ---------- fixtures ----------

/** A valid tier 1, 16x16 map: open room in a wall ring. Only the first 15 rows + 16 cells. */
function demoTiles(): number[] {
  const tiles = new Array(15 * MAP_MAX + 16).fill(TILE.Wall);
  for (let y = 1; y < 15; y++) for (let x = 1; x < 15; x++) tiles[y * MAP_MAX + x] = TILE.Floor;
  tiles[ENTRANCE.y * MAP_MAX + ENTRANCE.x] = TILE.Entrance;
  tiles[EXIT.y * MAP_MAX + EXIT.x] = TILE.Exit;
  return tiles;
}

const demoSpawns = (): Spawn[] => [
  { id: WOLF, x: 6, y: 2, pack: 1 },
  { id: WOLF, x: 7, y: 3, pack: 1 },
  { id: GOBLIN, x: 2, y: 10, pack: 2 },
  { id: GOBLIN, x: 3, y: 11, pack: 2 },
  { id: CHIEFTAIN, x: 12, y: 12, pack: 3 },
  { id: WARRIOR, x: 11, y: 12, pack: 3 },
  { id: SHAMAN, x: 12, y: 11, pack: 3 },
];

async function create(dm: Signer, index = 0, width = 16, height = 16, tier = 1) {
  await send(dm, [await createMapIx(dm, index, width, height, tier)]);
  const map = await mapAddress(dm.address, index);
  return { map, layout: await layoutAddress(map) };
}

async function demoDraft(dm: Signer, index = 0) {
  const accounts = await create(dm, index);
  await send(dm, [await writeLayoutIx(dm, accounts.map, 0, demoTiles())]);
  await send(dm, [await writeSpawnsIx(dm, accounts.map, ENTRANCE, EXIT, demoSpawns())]);
  return accounts;
}

async function demoFinal(dm: Signer, index = 0) {
  const accounts = await demoDraft(dm, index);
  await send(dm, [await finalizeIx(dm, accounts.map, new Array(32).fill(7))]);
  return accounts;
}

// ---------- tests ----------

describe("map: create_map", () => {
  it("creates a draft map and its layout", async () => {
    const dm = await funded();
    const { map, layout } = await create(dm, 7, 16, 24, 2);
    const m = await readMap(map);
    assert.equal(m.version, 1);
    assert.deepEqual(m.dm, Uint8Array.from(enc.encode(dm.address)));
    assert.equal(m.index, 7);
    assert.equal(m.width, 16);
    assert.equal(m.height, 24);
    assert.equal(m.status, STATUS.Draft);
    assert.equal(m.tier, 2);
    const l = await readLayout(layout);
    assert.equal(l.version, 1);
    assert.deepEqual(l.map, Uint8Array.from(enc.encode(address(map))));
    assert.equal(l.spawnCount, 0);
    assert.ok(l.tiles.every((t) => t === TILE.Floor));
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

  it("rejects the same index twice", async () => {
    const dm = await funded();
    await create(dm, 3);
    // System program: account already in use.
    await expectErr(send(dm, [await createMapIx(dm, 3, 16, 24, 1)]), 0);
  });
});

describe("map: write_layout", () => {
  it("writes a chunk at its offset", async () => {
    const dm = await funded();
    const { map, layout } = await create(dm);
    await send(dm, [await writeLayoutIx(dm, map, 100, [TILE.Wall, TILE.Water, TILE.Trap])]);
    const { tiles } = await readLayout(layout);
    assert.deepEqual(Array.from(tiles.slice(99, 104)), [TILE.Floor, TILE.Wall, TILE.Water, TILE.Trap, TILE.Floor]);
  });

  it("writes a full 32x32 grid in two chunks", async () => {
    const dm = await funded();
    const { map, layout } = await create(dm, 0, 32, 32);
    const grid = Array.from({ length: MAP_CELLS }, (_, i) => i % 7);
    await send(dm, [await writeLayoutIx(dm, map, 0, grid.slice(0, 512))]);
    await send(dm, [await writeLayoutIx(dm, map, 512, grid.slice(512))]);
    assert.deepEqual(Array.from((await readLayout(layout)).tiles), grid);
  });

  it("rejects a chunk past the end of the grid", async () => {
    const dm = await funded();
    const { map } = await create(dm);
    await expectErr(send(dm, [await writeLayoutIx(dm, map, MAP_CELLS - 10, new Array(11).fill(0))]), err("ChunkOutOfRange"));
    await expectErr(send(dm, [await writeLayoutIx(dm, map, 0xffff, [0])]), err("ChunkOutOfRange"));
  });

  it("rejects an unknown tile byte", async () => {
    const dm = await funded();
    const { map } = await create(dm);
    await expectErr(send(dm, [await writeLayoutIx(dm, map, 0, [0, 1, 7])]), err("InvalidTile"));
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
    await expectErr(send(dm, [await writeLayoutIx(dm, map, 0, [TILE.Wall])]), err("NotDraft"));
  });
});

describe("map: write_spawns", () => {
  it("writes the entrance, the exit and the spawns", async () => {
    const dm = await funded();
    const { map, layout } = await create(dm);
    await send(dm, [await writeSpawnsIx(dm, map, ENTRANCE, EXIT, demoSpawns())]);
    const l = await readLayout(layout);
    assert.deepEqual(l.entrance, ENTRANCE);
    assert.deepEqual(l.exit, EXIT);
    assert.equal(l.spawnCount, demoSpawns().length);
    assert.deepEqual(Array.from(l.spawns.slice(0, 9)), spawnBytes(demoSpawns()[0]));
  });

  it("clears old slots when the new list is shorter", async () => {
    const dm = await funded();
    const { map, layout } = await create(dm);
    await send(dm, [await writeSpawnsIx(dm, map, ENTRANCE, EXIT, demoSpawns())]);
    const one = [{ id: WOLF, x: 5, y: 5, pack: 1 }];
    await send(dm, [await writeSpawnsIx(dm, map, { x: 2, y: 2 }, EXIT, one)]);
    const l = await readLayout(layout);
    assert.deepEqual(l.entrance, { x: 2, y: 2 });
    assert.equal(l.spawnCount, 1);
    assert.deepEqual(Array.from(l.spawns.slice(0, 9)), spawnBytes(one[0]));
    assert.ok(l.spawns.slice(9).every((b) => b === 0));
  });

  it("rejects more than SPAWNS_MAX spawns", async () => {
    const dm = await funded();
    const { map } = await create(dm);
    const tooMany = Array.from({ length: SPAWNS_MAX + 1 }, () => ({ id: WOLF, x: 5, y: 5, pack: 1 }));
    await expectErr(send(dm, [await writeSpawnsIx(dm, map, ENTRANCE, EXIT, tooMany)]), err("TooManySpawns"));
  });

  it("rejects a signer that is not the DM", async () => {
    const dm = await funded();
    const other = await funded();
    const { map } = await create(dm);
    await expectErr(send(other, [await writeSpawnsIx(other, map, ENTRANCE, EXIT, demoSpawns())]), err("NotDm"));
  });

  it("rejects writes after finalize", async () => {
    const dm = await funded();
    const { map } = await demoFinal(dm);
    await expectErr(send(dm, [await writeSpawnsIx(dm, map, ENTRANCE, EXIT, [])]), err("NotDraft"));
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
    const expected = createHash("sha256").update(raw.slice(8, 8 + LAYOUT_CORE_SIZE)).digest();
    assert.deepEqual(m.layoutHash, Array.from(expected));
  });

  it("finalizes a 32x32 map within the default compute budget", async () => {
    const dm = await funded();
    const { map } = await create(dm, 0, 32, 32);
    const grid = new Array(MAP_CELLS).fill(TILE.Floor);
    grid[0] = TILE.Entrance;
    grid[MAP_CELLS - 1] = TILE.Exit;
    await send(dm, [await writeLayoutIx(dm, map, 0, grid.slice(0, 512))]);
    await send(dm, [await writeLayoutIx(dm, map, 512, grid.slice(512))]);
    await send(dm, [await writeSpawnsIx(dm, map, { x: 0, y: 0 }, { x: 31, y: 31 }, demoSpawns())]);
    const cu = await send(dm, [await finalizeIx(dm, map, new Array(32).fill(0))]);
    console.log(`finalize 32x32: ${cu} CU`);
    assert.ok(cu < 200_000, `${cu} CU`);
    assert.equal((await readMap(map)).status, STATUS.Finalized);
  });

  it("rejects a map without a path from the entrance to the exit", async () => {
    const dm = await funded();
    const { map } = await demoDraft(dm);
    // A wall across row 13 cuts the exit off.
    await send(dm, [await writeLayoutIx(dm, map, 13 * MAP_MAX + 1, new Array(14).fill(TILE.Wall))]);
    await expectErr(send(dm, [await finalizeIx(dm, map, new Array(32).fill(0))]), err("ExitUnreachable"));
    assert.equal((await readMap(map)).status, STATUS.Draft);
  });

  it("rejects a map without a boss", async () => {
    const dm = await funded();
    const { map } = await demoDraft(dm);
    await send(dm, [await writeSpawnsIx(dm, map, ENTRANCE, EXIT, demoSpawns().slice(0, 4))]);
    await expectErr(send(dm, [await finalizeIx(dm, map, new Array(32).fill(0))]), err("BossCount"));
  });

  it("rejects an empty draft", async () => {
    const dm = await funded();
    const { map } = await create(dm);
    await expectErr(send(dm, [await finalizeIx(dm, map, new Array(32).fill(0))]), err("EntranceCount"));
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
    assert.deepEqual((await readMap(map)).secretsRoot, new Array(32).fill(7));
  });
});

// The happy path (session authority PDA via CPI, trusted in GameConfig) needs the registry
// and session programs: covered by the cross-program tests (L7 open_session, L8.2).
describe("map: set_status", () => {
  it("rejects a caller when the registry GameConfig is missing or foreign", async () => {
    const dm = await funded();
    const { map } = await demoFinal(dm);
    // The DM signs as "session authority"; the config PDA is not a registry account here.
    const ix = setStatusIx(map, dm, await configAddress(), STATUS.InSession);
    await expectErr(send(dm, [ix]), err("InvalidConfig"));
    assert.equal((await readMap(map)).status, STATUS.Finalized);
  });

  it("rejects a config at another address", async () => {
    const dm = await funded();
    const { map } = await demoFinal(dm);
    const ix = setStatusIx(map, dm, SYSTEM_PROGRAM, STATUS.InSession);
    await expectErr(send(dm, [ix]), CONSTRAINT_SEEDS);
  });
});
