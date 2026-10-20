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
  getWriteLayoutInstruction,
  getWriteSpawnsInstruction,
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

async function createMapIx(dm: Signer, index: number, width: number, height: number, tier: number) {
  const map = await mapAddress(dm.address, index);
  return getCreateMapInstruction({ dm, map, layout: await layoutAddress(map), index, width, height, tier });
}

async function editAccounts(dm: Signer, map: string) {
  return { dm, map: address(map), layout: await layoutAddress(map) };
}

async function writeLayoutIx(dm: Signer, map: string, offset: number, tiles: Uint8Array | number[]) {
  return getWriteLayoutInstruction({ ...(await editAccounts(dm, map)), offset, tiles: Uint8Array.from(tiles) });
}

async function writeSpawnsIx(dm: Signer, map: string, entrance: Cell, exit: Cell, spawns: Spawn[]) {
  return getWriteSpawnsInstruction({ ...(await editAccounts(dm, map)), entrance, exit, spawns: spawns.map(spawnArgs) });
}

/** Patch 6: the registry bestiary and monster skills tables follow the layout; `tables` overrides them. */
async function finalizeIx(dm: Signer, map: string, root: number[], tables: string[] | null = null) {
  const [bestiary, skills] = tables ?? [await tableAddress("bestiary"), await tableAddress("monster_skills")];
  return getFinalizeMapInstruction({
    ...(await editAccounts(dm, map)),
    bestiary: address(bestiary),
    monsterSkills: address(skills),
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
  const { core, tier } = getMapDecoder().decode(await data(addr));
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
    tiles: Uint8Array.from(core.tiles),
    spawns: Uint8Array.from(core.spawns.flatMap((s) => Array.from(spawn.encode(s)))),
    spawnCount: core.spawnCount,
    entrance: core.entrance,
    exit: core.exit,
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

/** The skill choices of assets/registry/maps/demo_16x16.json: exactly the slots of each kind. */
const demoSpawns = (): Spawn[] => [
  { id: WOLF, x: 6, y: 2, pack: 1, skills: 0b0111 },
  { id: WOLF, x: 7, y: 3, pack: 1, skills: 0b1110 },
  { id: GOBLIN, x: 2, y: 10, pack: 2, skills: 0b11 },
  { id: GOBLIN, x: 3, y: 11, pack: 2, skills: 0b11 },
  { id: CHIEFTAIN, x: 12, y: 12, pack: 3, skills: 0b11111 },
  { id: WARRIOR, x: 11, y: 12, pack: 3, skills: 0b11 },
  { id: SHAMAN, x: 12, y: 11, pack: 3, skills: 0b111 },
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
    const one = [{ id: WOLF, x: 5, y: 5, pack: 1, skills: 0b0111 }];
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
    const tooMany = Array.from({ length: SPAWNS_MAX + 1 }, () => ({ id: WOLF, x: 5, y: 5, pack: 1, skills: 0b0111 }));
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

  it("patch 6: the spawns' skills, elite flags and the registry tables", async () => {
    const dm = await funded();
    const { map } = await demoDraft(dm);
    const finalizeWith = async (edit: (s: Spawn[]) => void) => {
      const spawns = demoSpawns();
      edit(spawns);
      await send(dm, [await writeSpawnsIx(dm, map, ENTRANCE, EXIT, spawns)]);
      return send(dm, [await finalizeIx(dm, map, new Array(32).fill(0))]);
    };
    // The wolf's pool has 4 skills: bit 4 is past it.
    await expectErr(finalizeWith((s) => (s[0].skills = 0b10111)), err("MonsterSkillUnknown"));
    // 3 active slots: one chosen is too few, all 4 too many.
    await expectErr(finalizeWith((s) => (s[0].skills = 0b1)), err("MonsterSkillCount"));
    await expectErr(finalizeWith((s) => (s[0].skills = 0b1111)), err("MonsterSkillCount"));
    // Elite: one more active slot; never a boss; one per tier-1 map.
    await expectErr(finalizeWith((s) => (s[0].flags = 1)), err("MonsterSkillCount"));
    await expectErr(finalizeWith((s) => (s[4].flags = 1)), err("EliteBoss"));
    await expectErr(
      finalizeWith((s) => {
        s[0].flags = 1;
        s[0].skills = 0b1111;
        s[1].flags = 1;
        s[1].skills = 0b1111;
      }),
      err("TooManyElites"),
    );
    // Other accounts in place of the registry tables.
    await send(dm, [await writeSpawnsIx(dm, map, ENTRANCE, EXIT, demoSpawns())]);
    const tileset = await tableAddress("tileset");
    const skills = await tableAddress("monster_skills");
    await expectErr(send(dm, [await finalizeIx(dm, map, new Array(32).fill(0), [tileset, skills])]), err("InvalidRegistryTable"));
    // An elite wolf with all 4 skills finalizes.
    const cu = await finalizeWith((s) => {
      s[0].flags = 1;
      s[0].skills = 0b1111;
    });
    console.log(`finalize 16x16 with an elite: ${cu} CU`);
    assert.equal((await readMap(map)).status, STATUS.Finalized);
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
