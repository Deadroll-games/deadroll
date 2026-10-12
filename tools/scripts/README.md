# tools/scripts

Node scripts that talk to the chain. Run them from this folder or with `pnpm --filter @deadroll/scripts run <name>`.
All of them are safe to rerun: they skip what is already on chain.

| Script | What |
|---|---|
| `deploy-local.ts` | the whole stack on the local validator (session with `mock-vrf`) |
| `deploy-devnet.ts` | the same on devnet with the real MagicBlock VRF; prints the SOL it needs and asks first |
| `seed-registry.ts` | registry tables from `assets/registry/*.json` |
| `upload-rasterizer.ts` | rasterizer WASM to the render program |
| `upload-sprites.ts` | palette and sprite book to the render program |

## Deploy

```
pnpm --filter @deadroll/scripts run deploy:local            # http://127.0.0.1:8899, ~/.config/solana/id.json
pnpm --filter @deadroll/scripts run deploy:devnet           # https://api.devnet.solana.com, asks before sending
```

Steps (each one skips itself when done):
1. preflight: RPC, wallet, `target/deploy/*-keypair.json` match `Anchor.toml` (keypairs are never generated);
2. build: `anchor build --tools-version v1.54` (local: session again with `--features mock-vrf`),
   `crates/render/wasm/build.sh`, `cargo run -p art -- pack` (palette + sprite book 1 from `assets/`);
3. `solana program deploy` for the 7 programs whose on-chain bytes differ from `target/deploy/*.so`;
4. `init_config` (first caller becomes the authority) or `set_programs` if the list changed;
5. `seed-registry.ts`, `upload-rasterizer.ts`, `upload-sprites.ts`, then `set_rasterizer` / `set_palette`;
6. demo map from `assets/registry/maps/demo_16x16.json`, DM = the wallet, `["map", wallet, --map-index]`;
7. summary: program ids, GameConfig, blobs, map and layout addresses.

Flags: `--url`, `--wallet`, `--skip-build`, `--map-index <n>` (a map used by a session cannot be reused:
pass the next index for a fresh one), `--uri-base` (NFT uri prefix), `--rasterizer-version`, `--book-id`.
Devnet only: `--compute-unit-price`, `--yes` (no prompt), `--check-build` (build and check the VRF mode, no network).

The devnet script refuses a session `.so` built with `mock-vrf` (it looks for the mock VRF identity in the
binary) and refuses when `mock-vrf` appears in its arguments or environment.

CLIs are found through `SOLANA_BIN`, `ANCHOR_BIN`, `BASH_BIN` (Git Bash on Windows), `CARGO_BIN`, then PATH,
then the default install folders. Neither script starts, stops or resets a validator.
