// Deploy the whole stack to the shared local validator (session with mock-vrf), seed it and
// create the demo map. Idempotent: rerun it after any change.
// node --experimental-strip-types --no-warnings deploy-local.ts [flags]
import { defaultWallet, deploy, intFlag, parseFlags, RASTERIZER_VERSION } from "./lib/deploy.ts";

const USAGE = `usage: deploy-local.ts [--url <rpc>] [--wallet <keypair.json>] [--skip-build] [--map-index <u32>]
                       [--uri-base <text>] [--rasterizer-version <u16>] [--book-id <u16>]

--url defaults to http://127.0.0.1:8899, --wallet to ~/.config/solana/id.json.
--skip-build deploys the existing target/ outputs (the session .so must be the mock-vrf build).
--map-index picks the demo map PDA ["map", wallet, index] (default 0).
Never starts, stops or resets a validator.`;

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
    "--help": "flag",
  },
  USAGE,
);
if (f["--help"]) {
  console.log(USAGE);
  process.exit(0);
}

deploy({
  cluster: "local",
  url: (f["--url"] as string) ?? "http://127.0.0.1:8899",
  wallet: (f["--wallet"] as string) ?? defaultWallet(),
  mockVrf: true,
  skipBuild: f["--skip-build"] === true,
  uriBase: (f["--uri-base"] as string) ?? "http://127.0.0.1:8787/nft",
  uriBaseExplicit: f["--uri-base"] !== undefined,
  rasterizerVersion: intFlag(f["--rasterizer-version"], RASTERIZER_VERSION, 0xffff, "--rasterizer-version"),
  bookId: f["--book-id"] === undefined ? undefined : intFlag(f["--book-id"], 0, 0xffff, "--book-id"),
  mapIndex: intFlag(f["--map-index"], 0, 0xffffffff, "--map-index"),
}).catch((err: unknown) => {
  console.error(`\ndeploy-local failed: ${err instanceof Error ? err.message : String(err)}`);
  process.exit(1);
});
