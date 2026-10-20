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
2. build: `anchor build --tools-version v1.54 -p <program>` for each program (local: session with `--features mock-vrf`),
   `crates/render/wasm/build.sh`, `cargo run -p art -- pack` (palette + sprite book 1 from `assets/`);
3. `solana program deploy` for the 7 programs whose on-chain bytes differ from the build. Each program is
   copied to `target/deploy-stage/` right after its own `anchor build -p`: a later `-p` build overwrites the
   programs it depends on in `target/deploy/` with their `cpi` builds (no entrypoint);
4. `init_config` (first caller becomes the authority), `resize_config` for a pre-patch-7 `GameConfig`, or
   `set_programs` if the list changed;
5. `seed-registry.ts`, `upload-sprites.ts` (palette, book), `upload-rasterizer.ts` (its finalize points
   `GameConfig` at the rasterizer), then `set_palette` / `set_sprite_books` (and `set_rasterizer` only if
   the config points elsewhere);
6. demo map from `assets/registry/maps/demo_16x16.json`, DM = the wallet, `["map", wallet, --map-index]`;
7. summary: program ids, GameConfig, blobs, map and layout addresses.

Flags: `--url`, `--wallet`, `--skip-build`, `--map-index <n>` (a map used by a session cannot be reused:
pass the next index for a fresh one), `--uri-base` (NFT uri prefix), `--rasterizer-version`, `--book-id` (both default to 2: sprite format 2;
version 1 and book 1 hold format 1 and are frozen).
Devnet only: `--compute-unit-price`, `--yes` (no prompt), `--check-build` (build and check the VRF mode, no network).

The devnet script refuses a session `.so` built with `mock-vrf` (it looks for the mock VRF identity in the
binary) and refuses when `mock-vrf` appears in its arguments or environment.

CLIs are found through `SOLANA_BIN`, `ANCHOR_BIN`, `BASH_BIN` (Git Bash on Windows), `CARGO_BIN`, then PATH,
then the default install folders. Neither script starts, stops or resets a validator.
