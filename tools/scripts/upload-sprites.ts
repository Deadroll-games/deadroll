// Upload a palette and/or a sprite book to the render program.
// node --experimental-strip-types --no-warnings upload-sprites.ts --palette <bin> --palette-version <u16> [--book <bin> --book-id <u16>]
import { readFileSync } from "node:fs";
import { createKeyPairSignerFromBytes, address } from "@solana/kit";
import { GAME_CONFIG, connect, planUpload, uploadBlob, type Kind } from "./lib/render-program.ts";

const USAGE = `usage: upload-sprites.ts (--palette <bin> --palette-version <u16>) and/or (--book <bin> --book-id <u16>) [--url <rpc>] [--wallet <keypair.json>] [--config <pubkey>] [--dry-run]

--url defaults to ANCHOR_PROVIDER_URL or http://127.0.0.1:8899.
--wallet defaults to ANCHOR_WALLET, a 64-byte JSON array. Required unless --dry-run.
--config defaults to the registry GameConfig PDA.
--dry-run prints the upload plans as JSON and does not contact the network.`;

function usage(): never {
  console.error(USAGE);
  process.exit(1);
}

function parseU16(raw: string | undefined): number {
  if (raw === undefined || !/^\d+$/.test(raw)) usage();
  const n = Number(raw);
  if (n > 0xffff) usage();
  return n;
}

function parseArgs(argv: string[]) {
  const known = new Set([
    "--palette",
    "--palette-version",
    "--book",
    "--book-id",
    "--url",
    "--wallet",
    "--config",
    "--dry-run",
  ]);
  let palette: string | undefined;
  let paletteVersion: number | undefined;
  let book: string | undefined;
  let bookId: number | undefined;
  let url = process.env.ANCHOR_PROVIDER_URL ?? "http://127.0.0.1:8899";
  let wallet = process.env.ANCHOR_WALLET;
  let config = GAME_CONFIG;
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
    if (flag === "--palette") palette = value;
    else if (flag === "--palette-version") paletteVersion = parseU16(value);
    else if (flag === "--book") book = value;
    else if (flag === "--book-id") bookId = parseU16(value);
    else if (flag === "--url") url = value;
    else if (flag === "--wallet") wallet = value;
    else if (flag === "--config") config = address(value);
  }
  return { palette, paletteVersion, book, bookId, url, wallet, config, dryRun };
}

/** format byte 0 == 1, and the u16 at bytes 1..3 equals the flag. */
function assertHeader(body: Uint8Array, label: string, flag: string, expected: number): void {
  if (body.length < 3) throw new Error(`${label} is shorter than the 3-byte header`);
  if (body[0] !== 1) {
    throw new Error(`${label} format mismatch: byte 0 is ${body[0]}, expected 1`);
  }
  const found = body[1] | (body[2] << 8);
  if (found !== expected) {
    throw new Error(`${label} version mismatch: file has ${found}, ${flag} is ${expected}`);
  }
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

async function main() {
  const flags = parseArgs(process.argv.slice(2));
  const jobs: { kind: Kind; id: number; body: Uint8Array }[] = [];

  if (flags.palette !== undefined || flags.paletteVersion !== undefined) {
    if (flags.palette === undefined || flags.paletteVersion === undefined) usage();
    const body = new Uint8Array(readFileSync(flags.palette));
    assertHeader(body, "palette", "--palette-version", flags.paletteVersion);
    jobs.push({ kind: "palette", id: flags.paletteVersion, body });
  }
  if (flags.book !== undefined || flags.bookId !== undefined) {
    if (flags.book === undefined || flags.bookId === undefined) usage();
    const body = new Uint8Array(readFileSync(flags.book));
    assertHeader(body, "sprite book", "--book-id", flags.bookId);
    jobs.push({ kind: "sprite_book", id: flags.bookId, body });
  }
  if (jobs.length === 0) usage();

  if (flags.dryRun) {
    const plans = [];
    for (const job of jobs) plans.push(await planUpload(job.kind, job.id, job.body));
    console.log(JSON.stringify(plans));
    return;
  }
  if (!flags.wallet) usage();
  const authority = await loadWallet(flags.wallet);
  const { rpc, rpcSubscriptions } = connect(flags.url);
  for (const job of jobs) {
    await uploadBlob({
      rpc,
      rpcSubscriptions,
      authority,
      kind: job.kind,
      id: job.id,
      body: job.body,
      config: flags.config,
    });
  }
}

main().catch((err: unknown) => {
  console.error(err instanceof Error && err.message ? err.message : "upload failed");
  process.exit(1);
});
