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
1. preflight: RPC, wallet, `target/deploy/*-keypair.json` match the ids in `@deadroll/sdk` (= `Anchor.toml`; keypairs are never generated);
2. build: `cargo build-sbf --tools-version v1.54 --arch v3` in `programs/<program>` for each program (local:
   session with `--features mock-vrf`), `crates/render/wasm/build.sh`, `cargo run -p art -- pack` (palette +
   sprite books 2.. from `assets/`). This is what `anchor build -p` runs, without the IDL, in a cargo cache of its own,
   `target/sbf-deploy/` (about 200 MB, only these scripts write it). anchor is not used because on Windows it starts
   cargo in a `\\?\D:\...` folder, which rebuilds the shared crates on every run and gives other `.so` bytes for the
   same code; a cache shared with `anchor build` would pass those bytes on (a deploy would upgrade programs that did
   not change);
3. sprite books: `art pack --first-book <n>` from n = 2 up to the first id whose `SpriteBook` accounts are
   missing, finalized with the same bytes, or an unfinished upload of the same bytes (`blobFits` in
   `lib/render-program.ts`). A book finalized with other sprites never changes, so after a sprite change the
   books move to the next free ids and `set_sprite_books` points `GameConfig` at them; a rerun finds the same
   ids. Runs with `--skip-build` too;
4. `solana program deploy` for the 7 programs whose on-chain bytes differ from the build. Each program is
   copied to `target/deploy-stage/` right after its own build; the mock-vrf session is moved
   there, so `target/deploy/` never keeps it. The build cache is never cleaned by hand: switching the session
   between mock-vrf and plain bumps the mtime of `programs/session/src/lib.rs`, and cargo rebuilds that crate;
5. `init_config` (first caller becomes the authority), `resize_config` for a pre-patch-7 `GameConfig`, or
   `set_programs` if the list changed;
6. `seed-registry.ts`, `upload-sprites.ts` (palette, book), `upload-rasterizer.ts` (its finalize points
   `GameConfig` at the rasterizer), then `set_palette` / `set_sprite_books` (and `set_rasterizer` only if
   the config points elsewhere);
7. demo map from `assets/registry/maps/demo_16x16.json`, DM = the wallet, `["map", wallet, --map-index]`;
8. summary: program ids, GameConfig, blobs, map and layout addresses.

Flags: `--url`, `--wallet`, `--skip-build`, `--map-index <n>` (a map used by a session cannot be reused:
pass the next index for a fresh one), `--uri-base` (NFT uri prefix), `--rasterizer-version` (default 2: sprite format 2;
version 1 and book 1 hold format 1 and are frozen), `--book-id <n>` (first sprite book id, 2 or more; fails
if book n.. holds other sprites; without it step 3 picks the id).
CI only: `--build-only` (build the deploy outputs, no network), `--preloaded` (the validator was started with the
staged programs at genesis, so no program keypair is needed and nothing is deployed; see `.github/workflows/integration.yml`).
Devnet only: `--compute-unit-price`, `--yes` (no prompt), `--check-build` (build and check the VRF mode, no network).

The devnet script refuses a session `.so` built with `mock-vrf` (it looks for the mock VRF identity in the
binary) and refuses when `mock-vrf` appears in its arguments or environment.

CLIs are found through `SOLANA_BIN`, `BASH_BIN` (Git Bash on Windows), `CARGO_BIN`, then PATH,
then the default install folders. Neither script starts, stops or resets a validator.
