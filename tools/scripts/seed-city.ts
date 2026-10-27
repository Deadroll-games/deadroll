// Load assets/city/city.json onto the city program: the city, its buildings and the Tavern board.
// Idempotent: creates what is missing, rewrites what differs. Needs the GameConfig authority.
// node --experimental-strip-types --no-warnings seed-city.ts [--url <rpc>] [--wallet <keypair.json>] [--dry-run]
import { readFileSync } from "node:fs";
import {
  address,
  appendTransactionMessageInstructions,
  createKeyPairSignerFromBytes,
  createTransactionMessage,
  getProgramDerivedAddress,
  pipe,
  sendAndConfirmTransactionFactory,
  setTransactionMessageFeePayerSigner,
  setTransactionMessageLifetimeUsingBlockhash,
  signTransactionMessageWithSigners,
  type Address,
  type TransactionSigner,
} from "@solana/kit";
import { PROGRAMS } from "@deadroll/sdk";
import {
  BuildingKind,
  BuildingStatus,
  Requirement,
  findBoardPda,
  findCityPda,
  getAddBuildingInstruction,
  getBuildingDecoder,
  getCityDecoder,
  getInitCityInstruction,
  getInitTavernBoardInstruction,
  getSetCityObjectsInstruction,
  getUpdateBuildingInstruction,
  getWriteCityInstruction,
  type BuildingSpec,
} from "@deadroll/sdk/city";
import { getGameConfigDataDecoder } from "@deadroll/sdk/registry";
import { GAME_CONFIG, connect } from "./lib/render-program.ts";
import { CITY_W_MAX, loadCity, type CityPlan } from "./lib/city-data.ts";
import { maskText } from "./lib/mask-url.ts";

const USAGE = `usage: seed-city.ts [--url <rpc>] [--wallet <keypair.json>] [--dry-run]

--url defaults to ANCHOR_PROVIDER_URL or http://127.0.0.1:8899.
--wallet defaults to ANCHOR_WALLET, a 64-byte JSON array: the GameConfig authority. Required unless --dry-run.
--dry-run prints the parsed city and does not contact the network.`;

/** Cells per write_city: keeps the transaction well under 1232 bytes. */
const WRITE_CHUNK = 600;
const DEFAULT_ADDRESS = "11111111111111111111111111111111" as Address;
const CITY_PROGRAM = address(PROGRAMS.city);

function usage(): never {
  console.error(USAGE);
  process.exit(1);
}

function parseArgs(argv: string[]) {
  let url = process.env.ANCHOR_PROVIDER_URL ?? "http://127.0.0.1:8899";
  let wallet = process.env.ANCHOR_WALLET;
  let dryRun = false;
  for (let i = 0; i < argv.length; i++) {
    const flag = argv[i];
    if (flag === "--dry-run") {
      dryRun = true;
      continue;
    }
    const value = argv[++i];
    if (value === undefined || value.startsWith("--")) usage();
    if (flag === "--url") url = value;
    else if (flag === "--wallet") wallet = value;
    else usage();
  }
  return { url, wallet, dryRun };
}

async function loadWallet(path: string) {
  const parsed = JSON.parse(readFileSync(path, "utf8"));
  if (!Array.isArray(parsed) || parsed.length !== 64) throw new Error("wallet must be a 64-byte JSON array keypair");
  return createKeyPairSignerFromBytes(Uint8Array.from(parsed));
}

type Conn = ReturnType<typeof connect>;

async function fetchData(conn: Conn, target: Address): Promise<Uint8Array | null> {
  const { value } = await conn.rpc.getAccountInfo(target, { encoding: "base64", commitment: "confirmed" }).send();
  return value ? new Uint8Array(Buffer.from(value.data[0], "base64")) : null;
}

async function send(conn: Conn, payer: TransactionSigner, instructions: any[]): Promise<void> {
  const sendAndConfirm = sendAndConfirmTransactionFactory(conn as any);
  const { value: blockhash } = await conn.rpc.getLatestBlockhash().send();
  const message = pipe(
    createTransactionMessage({ version: 0 }),
    (m) => setTransactionMessageFeePayerSigner(payer, m),
    (m) => setTransactionMessageLifetimeUsingBlockhash(blockhash, m),
    (m) => appendTransactionMessageInstructions(instructions, m),
  );
  await sendAndConfirm((await signTransactionMessageWithSigners(message)) as any, { commitment: "confirmed" });
}

/** `["building", id: u16 LE]`. */
async function buildingAddress(id: number): Promise<Address> {
  const [pda] = await getProgramDerivedAddress({ programAddress: CITY_PROGRAM, seeds: ["building", new Uint8Array([id & 0xff, id >> 8])] });
  return pda;
}

const sameBytes = (a: ArrayLike<number>, b: ArrayLike<number>) => a.length === b.length && Array.from(a).every((v, i) => v === b[i]);

async function seedCity(conn: Conn, authority: TransactionSigner, plan: CityPlan) {
  const [city] = await findCityPda();
  let data = await fetchData(conn, city);
  if (!data) {
    await send(conn, authority, [getInitCityInstruction({ authority, config: GAME_CONFIG, city, width: plan.width, height: plan.height, spawn: plan.spawn })]);
    console.log(`init_city: ${city} (${plan.width}x${plan.height})`);
    data = (await fetchData(conn, city))!;
  }
  let core = getCityDecoder().decode(data).core;
  if (core.width !== plan.width || core.height !== plan.height) {
    throw new Error(`the city on chain is ${core.width}x${core.height}, city.json is ${plan.width}x${plan.height}: the size is fixed at init_city`);
  }
  let written = 0;
  for (let offset = 0; offset < plan.cells.length; offset += WRITE_CHUNK) {
    const chunk = plan.cells.subarray(offset, Math.min(offset + WRITE_CHUNK, plan.cells.length));
    if (sameBytes(core.cells.slice(offset, offset + chunk.length), chunk)) continue;
    await send(conn, authority, [getWriteCityInstruction({ authority, city, offset, cells: chunk })]);
    written += chunk.length;
  }
  if (written) console.log(`write_city: ${written} cells`);
  core = getCityDecoder().decode((await fetchData(conn, city))!).core;
  const objects = core.objects.slice(0, core.objectCount);
  const same = objects.length === plan.objects.length && objects.every((o, i) => o.sprite === plan.objects[i].sprite && o.x === plan.objects[i].x && o.y === plan.objects[i].y);
  if (!same || core.spawn.x !== plan.spawn.x || core.spawn.y !== plan.spawn.y) {
    await send(conn, authority, [getSetCityObjectsInstruction({ authority, city, objects: plan.objects, spawn: plan.spawn })]);
    console.log(`set_city_objects: ${plan.objects.length} objects, spawn ${plan.spawn.x},${plan.spawn.y}`);
  }
  return city;
}

async function seedBuildings(conn: Conn, authority: TransactionSigner, city: Address, plan: CityPlan) {
  for (const [id, b] of plan.buildings.entries()) {
    const building = await buildingAddress(id);
    const [board] = await findBoardPda({ building });
    const tavern = b.kind === BuildingKind.Tavern;
    const spec: BuildingSpec = {
      kind: b.kind,
      prefabId: b.prefab,
      x: b.x,
      y: b.y,
      door: b.door,
      // A closed building calls nothing; the Tavern is a board of the city program.
      program: tavern ? CITY_PROGRAM : DEFAULT_ADDRESS,
      target: tavern ? board : DEFAULT_ADDRESS,
      interiorId: b.interior,
      status: b.status,
      requirement: Requirement.None,
    };
    const data = await fetchData(conn, building);
    if (!data) {
      const count = getCityDecoder().decode((await fetchData(conn, city))!).core.buildingCount;
      if (count !== id) throw new Error(`building ${id} (${b.name}) is next, but the city has ${count} buildings: city.json lists buildings in id order`);
      await send(conn, authority, [getAddBuildingInstruction({ authority, config: GAME_CONFIG, city, building, spec })]);
      console.log(`add_building ${id}: ${b.name} (${BuildingStatus[b.status]}) ${building}`);
    } else {
      const core = getBuildingDecoder().decode(data).core;
      const current: BuildingSpec = { kind: core.kind, prefabId: core.prefabId, x: core.x, y: core.y, door: core.door, program: core.program, target: core.target, interiorId: core.interiorId, status: core.status, requirement: core.requirement };
      if (JSON.stringify(current) !== JSON.stringify(spec)) {
        await send(conn, authority, [getUpdateBuildingInstruction({ authority, config: GAME_CONFIG, city, building, spec })]);
        console.log(`update_building ${id}: ${b.name}`);
      }
    }
    if (tavern && !(await fetchData(conn, board))) {
      await send(conn, authority, [getInitTavernBoardInstruction({ authority, city, building, board })]);
      console.log(`init_tavern_board: ${board}`);
    }
  }
}

async function main() {
  const flags = parseArgs(process.argv.slice(2));
  const plan = loadCity();
  if (flags.dryRun) {
    console.log(JSON.stringify({ ...plan, cells: plan.cells.length / CITY_W_MAX + " rows" }));
    return;
  }
  if (!flags.wallet) usage();
  const authority = await loadWallet(flags.wallet);
  const conn = connect(flags.url);
  const config = await fetchData(conn, GAME_CONFIG);
  if (!config) throw new Error("no GameConfig: run seed-registry first");
  const owner = getGameConfigDataDecoder().decode(config.subarray(8)).authority;
  if (owner !== authority.address) throw new Error(`the wallet is not the GameConfig authority (${owner})`);
  const city = await seedCity(conn, authority, plan);
  await seedBuildings(conn, authority, city, plan);
  console.log(`city: ${city}, ${plan.buildings.length} buildings`);
}

main().catch((err: unknown) => {
  console.error(maskText(err instanceof Error && err.message ? err.message : "seed failed"));
  process.exit(1);
});
