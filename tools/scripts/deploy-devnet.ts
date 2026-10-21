// Deploy the whole stack to devnet: the same steps as deploy-local, with the real MagicBlock VRF.
// Refuses a mock-vrf session build. Prints the SOL it needs and asks before sending anything.
// node --experimental-strip-types --no-warnings deploy-devnet.ts [flags]
import { createInterface } from "node:readline/promises";
import {
  build,
  defaultWallet,
  deploy,
  intFlag,
  parseFlags,
  resolveBins,
  sessionVrfMode, RASTERIZER_VERSION } from "./lib/deploy.ts";

const USAGE = `usage: deploy-devnet.ts [--url <rpc>] [--wallet <keypair.json>] [--skip-build] [--map-index <u32>]
                        [--uri-base <text>] [--rasterizer-version <u16>] [--book-id <u16>]
                        [--compute-unit-price <micro-lamports>] [--yes] [--check-build]

--url defaults to https://api.devnet.solana.com, --wallet to ~/.config/solana/id.json.
--yes skips the cost confirmation prompt.
--check-build only builds (unless --skip-build) and checks that the session is the real-VRF build;
  no network calls.`;

const f = parseFlags(
  process.argv.slice(2),
  {
    "--url": "value",
    "--wallet": "value",
    "--skip-build": "flag",
    "--map-index": "value",
    "--uri-base": "value",
    "--rasterizer-version": "value",
    "--book-id": "value",
    "--compute-unit-price": "value",
    "--yes": "flag",
    "--check-build": "flag",
    "--help": "flag",
  },
  USAGE,
);
if (f["--help"]) {
  console.log(USAGE);
  process.exit(0);
}

function refuseMockVrf(): void {
  const hits = [...process.argv, ...Object.values(process.env)].filter((v) => v?.includes("mock-vrf"));
  if (hits.length > 0) throw new Error(`refusing: "mock-vrf" appears in the arguments or environment (${hits.join(", ")})`);
}

async function confirmCost(lines: string[], total: bigint, balance: bigint): Promise<boolean> {
  for (const line of lines) console.log(line);
  console.log(`${"total needed".padEnd(52)} ${(Number(total) / 1e9).toFixed(4).padStart(10)} SOL`);
  console.log(`${"wallet balance".padEnd(52)} ${(Number(balance) / 1e9).toFixed(4).padStart(10)} SOL`);
  if (balance < total) {
    console.log(`\nnot enough SOL: top up ${((Number(total - balance)) / 1e9).toFixed(4)} SOL (solana airdrop / faucet.solana.com)`);
    return false;
  }
  if (f["--yes"]) return true;
  const rl = createInterface({ input: process.stdin, output: process.stdout });
  const answer = await rl.question("\ndeploy to devnet? [y/N] ");
  rl.close();
  return answer.trim().toLowerCase() === "y";
}

async function main() {
  refuseMockVrf();
  if (f["--check-build"]) {
    if (!f["--skip-build"]) build(resolveBins(), false);
    const mode = sessionVrfMode();
    console.log(`target/deploy-stage/deadroll_session.so: ${mode} VRF build`);
    if (mode !== "real") throw new Error("refusing: the session build is not the real-VRF build");
    return;
  }
  if (f["--uri-base"] === undefined) {
    console.log("note: --uri-base not given; NFT uris use a placeholder until set_uri_base");
  }
  await deploy({
    cluster: "devnet",
    url: (f["--url"] as string) ?? "https://api.devnet.solana.com",
    wallet: (f["--wallet"] as string) ?? defaultWallet(),
    mockVrf: false,
    skipBuild: f["--skip-build"] === true,
    uriBase: (f["--uri-base"] as string) ?? "https://deadroll.invalid/nft",
    uriBaseExplicit: f["--uri-base"] !== undefined,
    rasterizerVersion: intFlag(f["--rasterizer-version"], RASTERIZER_VERSION, 0xffff, "--rasterizer-version"),
    bookId: f["--book-id"] === undefined ? undefined : intFlag(f["--book-id"], 0, 0xffff, "--book-id"),
    mapIndex: intFlag(f["--map-index"], 0, 0xffffffff, "--map-index"),
    computeUnitPrice: f["--compute-unit-price"] === undefined ? undefined : intFlag(f["--compute-unit-price"], 0, 10_000_000, "--compute-unit-price"),
    confirmCost,
  });
}

main().catch((err: unknown) => {
  console.error(`\ndeploy-devnet failed: ${err instanceof Error ? err.message : String(err)}`);
  process.exit(1);
});
