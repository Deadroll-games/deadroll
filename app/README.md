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
cargo test -p deadroll-render --test golden      # target/render-golden/{input,palette,book}.bin
bash crates/render/wasm/build.sh                 # target/render-wasm/deadroll_render.wasm
```

Demo pages on mock data: `/dev/grid`, `/dev/dice`, `/dev/forms`.

### Chain mode

`app/.env.local`:
```
VITE_DATA=chain
VITE_RPC_URL=http://127.0.0.1:8899      # or a devnet RPC
VITE_CHAIN=solana:localnet              # Wallet Standard chain id
VITE_SPRITE_BOOKS=1                     # sprite book ids to load
```
This needs the registry `GameConfig` and the finalized render blobs (`tools/scripts/upload-rasterizer.ts`, `upload-sprites.ts`).

## Layout

| Path | What |
|---|---|
| `src/chain/` | types, decoders, PDAs, instruction builders, log/event parsing, tombstone watcher (thin wrapper until `sdk/`) |
| `src/raster/` | rasterizer loader (GameConfig -> RasterizerCode -> SHA-256 -> IndexedDB -> WASM), C ABI wrapper, `SpriteCanvas` |
| `src/session-key/` | MagicBlock Session Keys `create_session_v2`, burner key for the tab |
| `src/wallet/` | kit client (wallet signer + RPC), connect button |
| `src/components/` | grid, dice cascade + hit breakdown, forms; pure UI over `types.ts` props |
| `src/routes/` | Home, Character, Map builder, Party |

## Rasterizer loading

1. Read `GameConfig` (registry `["config"]`).
2. Check `GameConfig.rasterizer` is the `["rasterizer", version]` PDA.
3. IndexedDB hit by hash: re-hash, use if equal.
4. Otherwise fetch `RasterizerCode`, check discriminator, kind, id, `finalized`, exact length; SHA-256 the body with WebCrypto; it must equal both `GameConfig.rasterizer_sha256` and the finalized header hash. Cache it.
5. `WebAssembly.instantiate`, then render with `Rasterizer.render(request, palette, books)`.

`src/raster/raster.test.ts` renders the reference warrior through this path and compares the PNG SHA-256 with the native renderer.
