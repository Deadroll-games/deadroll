// Load registry JSON onto the on-chain tables.
// node --experimental-strip-types --no-warnings seed-registry.ts [--url <rpc>] [--wallet <keypair.json>] [--dry-run]
import { readFileSync } from "node:fs";
import {
  address,
  appendTransactionMessageInstructions,
  createKeyPairSignerFromBytes,
  createTransactionMessage,
  pipe,
  sendAndConfirmTransactionFactory,
  setTransactionMessageFeePayerSigner,
  setTransactionMessageLifetimeUsingBlockhash,
  signTransactionMessageWithSigners,
  type TransactionSigner,
} from "@solana/kit";
import { findTablePda, getGameConfigDataDecoder, getInitTableInstruction, getWriteTableInstruction } from "@deadroll/sdk/registry";
import { GAME_CONFIG, connect } from "./lib/render-program.ts";
import {
  TableKind,
  loadEncodedTables,
  seedPlan,
  type EncodedTable,
} from "./lib/registry-records.ts";

const USAGE = `usage: seed-registry.ts [--url <rpc>] [--wallet <keypair.json>] [--dry-run]

--url defaults to ANCHOR_PROVIDER_URL or http://127.0.0.1:8899.
--wallet defaults to ANCHOR_WALLET, a 64-byte JSON array. Required unless --dry-run.
--dry-run prints the seed plan as JSON and does not contact the network.`;

const TABLE_BODY = 16;
const MAX_IX_DATA = 1000;
const WRITE_OVERHEAD = 8 + 1 + 2 + 4;

function usage(): never {
  console.error(USAGE);
  process.exit(1);
}

function parseArgs(argv: string[]) {
  const known = new Set(["--url", "--wallet", "--dry-run"]);
  let url = process.env.ANCHOR_PROVIDER_URL ?? "http://127.0.0.1:8899";
  let wallet = process.env.ANCHOR_WALLET;
  let dryRun = false;
  for (let i = 0; i < argv.length; i++) {
    const flag = argv[i];
    if (!known.has(flag)) usage();
    if (flag === "--dry-run") {
      dryRun = true;
      continue;
    }
    const value = argv[++i];
    if (value === undefined || value.startsWith("--")) usage();
    if (flag === "--url") url = value;
    else if (flag === "--wallet") wallet = value;
  }
  return { url, wallet, dryRun };
}

async function loadWallet(path: string) {
  let parsed: unknown;
  try {
    parsed = JSON.parse(readFileSync(path, "utf8"));
  } catch {
    throw new Error(`cannot read wallet ${path}`);
  }
  if (!Array.isArray(parsed) || parsed.length !== 64 || parsed.some((n) => typeof n !== "number")) {
    throw new Error("wallet must be a 64-byte JSON array keypair");
  }
  return createKeyPairSignerFromBytes(Uint8Array.from(parsed as number[]));
}

function sameBytes(a: Uint8Array, b: Uint8Array): boolean {
  if (a.length !== b.length) return false;
  for (let i = 0; i < a.length; i++) if (a[i] !== b[i]) return false;
  return true;
}

type Signer = TransactionSigner;

async function tablePda(seed: string): Promise<string> {
  return (await findTablePda({ seed }))[0];
}

function chunkLen(recordSize: number, remaining: number): number {
  const maxBytes = MAX_IX_DATA - WRITE_OVERHEAD;
  const n = Math.max(1, Math.floor(maxBytes / recordSize));
  return Math.min(remaining, n * recordSize);
}

async function fetchAccount(rpc: any, target: string): Promise<Uint8Array | null> {
  const { value } = await rpc
    .getAccountInfo(address(target), { encoding: "base64", commitment: "confirmed" })
    .send();
  if (!value) return null;
  const raw = value.data;
  if (typeof raw === "string") return new Uint8Array(Buffer.from(raw, "base64"));
  if (Array.isArray(raw) && typeof raw[0] === "string") return new Uint8Array(Buffer.from(raw[0], "base64"));
  if (raw instanceof Uint8Array) return new Uint8Array(raw);
  throw new Error(`account ${target} returned no base64 data`);
}

async function send(rpc: any, rpcSubscriptions: any, payer: Signer, instructions: any[]): Promise<void> {
  const sendAndConfirm = sendAndConfirmTransactionFactory({ rpc, rpcSubscriptions });
  const { value: blockhash } = await rpc.getLatestBlockhash().send();
  const message = pipe(
    createTransactionMessage({ version: 0 }),
    (current) => setTransactionMessageFeePayerSigner(payer, current),
    (current) => setTransactionMessageLifetimeUsingBlockhash(blockhash, current),
    (current) => appendTransactionMessageInstructions(instructions, current),
  );
  const tx = await signTransactionMessageWithSigners(message);
  await sendAndConfirm(tx as any, { commitment: "confirmed" });
}

async function requireAuthority(rpc: any, walletAddress: string): Promise<void> {
  const cfg = await fetchAccount(rpc, GAME_CONFIG);
  if (!cfg) throw new Error("GameConfig is missing; run init_config first");
  if (getGameConfigDataDecoder().decode(cfg, 8).authority !== walletAddress) {
    throw new Error("wallet is not GameConfig.authority");
  }
}

async function seedTable(
  rpc: any,
  rpcSubscriptions: any,
  authority: Signer,
  table: EncodedTable,
): Promise<void> {
  const target = await tablePda(table.seed);
  let data = await fetchAccount(rpc, target);
  if (!data) {
    await send(rpc, rpcSubscriptions, authority, [getInitTableInstruction({ authority, config: GAME_CONFIG, table: address(target), kind: table.kind })]);
    data = await fetchAccount(rpc, target);
    if (!data) throw new Error(`init_table did not create ${table.table}`);
  }
  if (data[9] !== table.kind) {
    throw new Error(`table ${table.table} has kind ${data[9]}, expected ${table.kind}`);
  }
  if (table.kind === TableKind.Prefabs || table.bytes.length === 0) return;

  const size = table.recordSize;
  for (let offset = 0; offset < table.bytes.length; ) {
    const length = chunkLen(size, table.bytes.length - offset);
    const chunk = table.bytes.subarray(offset, offset + length);
    const onChain = data.subarray(TABLE_BODY + offset, TABLE_BODY + offset + length);
    if (onChain.length === chunk.length && sameBytes(onChain, chunk)) {
      offset += length;
      continue;
    }
    await send(rpc, rpcSubscriptions, authority, [
      getWriteTableInstruction({ authority, config: GAME_CONFIG, table: address(target), kind: table.kind, offset, bytes: chunk }),
    ]);
    data = await fetchAccount(rpc, target);
    if (!data) throw new Error(`table ${table.table} missing after write`);
    offset += length;
  }
}

async function main() {
  const flags = parseArgs(process.argv.slice(2));
  const tables = loadEncodedTables();
  if (flags.dryRun) {
    console.log(JSON.stringify(seedPlan(tables)));
    return;
  }
  if (!flags.wallet) usage();
  const authority = await loadWallet(flags.wallet);
  const { rpc, rpcSubscriptions } = connect(flags.url);
  await requireAuthority(rpc, authority.address);
  for (const table of tables) {
    await seedTable(rpc, rpcSubscriptions, authority, table);
  }
}

main().catch((err: unknown) => {
  console.error(err instanceof Error && err.message ? err.message : "seed failed");
  process.exit(1);
});
