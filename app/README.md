# Deadroll app

The website for Deadroll (lane L9). Vite 8, React 19, TypeScript, `@solana/kit` 8 with the Wallet Standard plugin.

## Run

```
pnpm install
pnpm --filter @deadroll/app dev
```

### Mock mode (default)

`VITE_DATA=mock` needs no validator. The dev server serves local build outputs under `/dev-assets/`, and the app wraps them in synthetic `GameConfig`, `RasterizerCode`, `Palette` and `SpriteBook` accounts, so the loader runs its real decode and SHA-256 checks. Build them first:

```
cargo test -p deadroll-render --test golden      # target/render-golden/{state,sprites,palette,book}.bin
bash crates/render/wasm/build.sh                 # target/render-wasm/deadroll_render.wasm
```

Demo pages on mock data: `/dev/grid`, `/dev/dice`, `/dev/forms`.

### Combat module

The character screen's derived stats and the battle screen's attack preview run the combat crate in
the browser: `src/chain/wasm/combat.wasm`, committed (Vercel builds without Rust) and wrapped by
`src/chain/combatWasm.ts`. After a change in `crates/combat`, `crates/rules`, `crates/skillcheck` or
`crates/types`, rebuild and commit it (Docker), and refresh the fixtures:

```
UPDATE_TS_FIXTURES=1 cargo test -p deadroll-combat-wasm --test ts_fixtures
bash crates/combat/wasm/build.sh                 # CI runs it with --check and fails on a stale file
```

### Chain mode

`app/.env.local`:
```
VITE_DATA=chain
VITE_RPC_URL=http://127.0.0.1:8899      # or a devnet RPC
VITE_CHAIN=solana:localnet              # Wallet Standard chain id
```
This needs the registry `GameConfig` and the finalized render blobs (`tools/scripts/upload-rasterizer.ts`, `upload-sprites.ts`).

### Localnet demo (DM + 2 players in one browser)

After `tools/scripts/deploy-local.ts` (all 7 programs, session built with `mock-vrf`):

```
pnpm --filter @deadroll/app dev:localnet      # vite --mode localnet: chain data, 127.0.0.1:8899, book 1, mock VRF oracle
```

Open `http://localhost:5173/?dev=1` in three tabs or windows: DM, warrior, mage.

- `?dev=1` (localnet RPC only) gives each tab its own keypair in `sessionStorage` and airdrops 5 SOL. No wallet extension, no popups. Without it, the normal Wallet Standard connect is used.
- **Character**: create a character. It mints a Core NFT; the predisposition depends on race, and you spend 10 stat points.
- **Map builder** (DM): Validate, then Finalize. This sends `create_map`, `write_layout`, `write_spawns` and `finalize_map`, then links to "Open a party on it". A party needs a map the DM owns (`open_session` checks `map.dm`), so the deployer's demo map is not used from the browser.
- **Party**: the DM sets a price and opens the party, which starts the DM session key. Players open the party link (or pick it from the list), join (pay half into escrow, binding their session key) and press Ready. The DM starts.
- **Battle**: click a cell to move and a monster to attack with the selected skill; then End turn. The DM moves monsters by clicking, uses "Auto monster turn", or ticks "auto-play monsters". Every `HitResolved` plays the dice cascade in all windows, and every event links its transaction.
- **Localnet VRF oracle**: the `mock-vrf` build skips the VRF request. The acting window signs `consume_randomness` with the fixed mock identity, and the DM window answers any roll left pending for more than 3.5 s. "Next seed" picks a random seed, a natural 20 or 1, or a crit with a successful tint roll (a resonance), the same seeds as `tests/demo.test.ts`. It is never enabled for a non-local RPC.
- **Death**: `CharacterDied` makes the owner's window send `freeze_tombstone`. Other windows send it after 8 to 14 s if the NFT is still not frozen.
- **End**: Claim rewards (anyone may send it), then Claim escrow (DM).

`src/chain/localnet.e2e.test.ts` plays the same script through the app's builders without a browser:
`DEADROLL_LOCALNET=1 pnpm --filter @deadroll/app exec vitest run src/chain/localnet.e2e.test.ts`.

### Devnet build (hosted site)

```
pnpm --filter @deadroll/app build:devnet      # vite build --mode devnet -> app/dist
pnpm --filter @deadroll/app preview:devnet
```

Reads only `app/.env.devnet` (public values). Chain data only: mock data, the `/dev/*` demo pages, the `?dev=1` wallet and the mock VRF oracle are compiled out or off, even with a localhost Custom RPC; players connect a real Wallet Standard wallet. The rasterizer, palette and sprite books come from the chain, so the build needs neither cargo nor `target/`. `app/vercel.json` holds the hosting config (SPA fallback to `index.html`).

## Layout

| Path | What |
|---|---|
| `src/chain/` | the screens' calls over `@deadroll/sdk` (generated from the IDLs): instruction builders, account reads, PDAs, events, tombstone watcher |
| `src/chain/game.ts` | the demo calls of `tests/demo.test.ts` as instruction builders and reads; `useParty.ts` live party state + events |
| `src/raster/` | rasterizer loader (GameConfig -> RasterizerCode -> SHA-256 -> IndexedDB -> WASM), C ABI wrapper, `SpriteCanvas` |
| `src/session-key/` | MagicBlock Session Keys `create_session_v2`, burner key for the tab |
| `src/wallet/` | kit client (wallet signer + RPC), `AppWallet` (Wallet Standard or `?dev=1` keypair), connect button |
| `src/components/` | grid, dice cascade + hit breakdown, forms; pure UI over `types.ts` props |
| `src/routes/` | Home, Character, Map builder, Party |

## Rasterizer loading

1. Read `GameConfig` (registry `["config"]`).
2. Check `GameConfig.rasterizer` is the `["rasterizer", version]` PDA.
3. IndexedDB hit by hash: re-hash, use if equal.
4. Otherwise fetch `RasterizerCode`, check discriminator, kind, id, `finalized`, exact length; SHA-256 the body with WebCrypto; it must equal both `GameConfig.rasterizer_sha256` and the finalized header hash. Cache it.
5. Load `GameConfig.palette` and every `GameConfig.sprite_books` entry (each must be the `["sprites", id]` PDA of its header id), in that lookup order.
6. `WebAssembly.instantiate`, then render with `Rasterizer.render(request, palette, books)`: `render(sprites, palette, state, asset, mode)` of `ART_DESCRIPTION.md` 2.2. RGBA is the canvas 1:1 (64 px characters, `SpriteCanvas` and `Grid` scale it); PNG is 512 px.

`src/raster/raster.test.ts` renders the reference warrior through this path and compares the PNG SHA-256 with the native renderer.

## RPC policy

- Hosted builds use the public devnet RPC and WebSocket (`https://api.devnet.solana.com`). `vite build` refuses a `VITE_RPC_URL` or `VITE_RPC_WS_URL` that points anywhere else, and fails if any emitted chunk mentions a provider host or an `api-key` parameter (`findBundledRpcKey` in `src/rpcSettings.ts`), so a provider key can never be bundled. In `--mode devnet` it also refuses `VITE_DATA=mock` and `VITE_MOCK_VRF=1`.
- Settings > RPC (header) sets a Custom RPC in this browser's localStorage; it overrides everything else. For local runs a private URL may live in an untracked `app/.env.devnet.local` (`pnpm --filter @deadroll/app dev:devnet` with `--mode devnet`).
- The party view refreshes on the `logsNotifications` stream. The poll is a fallback: every 10 s while the stream is live, every 3 s while it is down, paused in a hidden tab. A refresh is one `getMultipleAccounts` call once the roster is known.
- HTTP 429 is retried with backoff (1 s, 2 s, 4 s ..., or `Retry-After`) and the header shows "network busy, retrying".
