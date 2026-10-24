// Upload a rasterizer WASM blob to the render program.
// node --experimental-strip-types --no-warnings upload-rasterizer.ts --version <u16> [flags]
import { readFileSync } from "node:fs";
import { createKeyPairSignerFromBytes, address } from "@solana/kit";
import {
  GAME_CONFIG,
  WASM_MAGIC,
  connect,
  planUpload,
  samePrefix,
  uploadBlob,
} from "./lib/render-program.ts";
import { maskText } from "./lib/mask-url.ts";

const USAGE = `usage: upload-rasterizer.ts --version <u16> [--wasm <path>] [--url <rpc>] [--wallet <keypair.json>] [--config <pubkey>] [--dry-run]

--version is required.
--wasm defaults to target/render-wasm/deadroll_render.wasm.
--url defaults to ANCHOR_PROVIDER_URL or http://127.0.0.1:8899.
--wallet defaults to ANCHOR_WALLET, a 64-byte JSON array. Required unless --dry-run.
--config defaults to the registry GameConfig PDA.
--dry-run prints the upload plan as JSON and does not contact the network.`;

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
  const known = new Set(["--version", "--wasm", "--url", "--wallet", "--config", "--dry-run"]);
  let version: number | undefined;
  let wasm = "target/render-wasm/deadroll_render.wasm";
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
    if (flag === "--version") version = parseU16(value);
    else if (flag === "--wasm") wasm = value;
    else if (flag === "--url") url = value;
    else if (flag === "--wallet") wallet = value;
    else if (flag === "--config") config = address(value);
  }
  if (version === undefined) usage();
  return { version, wasm, url, wallet, config, dryRun };
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
  const body = new Uint8Array(readFileSync(flags.wasm));
  if (!samePrefix(body, WASM_MAGIC)) {
    throw new Error("wasm does not start with magic 00 61 73 6d 01 00 00 00");
  }
  if (flags.dryRun) {
    console.log(JSON.stringify(await planUpload("rasterizer", flags.version, body)));
    return;
  }
  if (!flags.wallet) usage();
  const authority = await loadWallet(flags.wallet);
  const { rpc, rpcSubscriptions } = connect(flags.url);
  await uploadBlob({
    rpc,
    rpcSubscriptions,
    authority,
    kind: "rasterizer",
    id: flags.version,
    body,
    config: flags.config,
  });
}

main().catch((err: unknown) => {
  console.error(maskText(err instanceof Error && err.message ? err.message : "upload failed"));
  process.exit(1);
});
