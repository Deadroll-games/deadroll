---
name: rasterizer
description: Use when working on crates/render (sprite DSL decoder, shading, outline, PNG encoder), its WASM build, programs/render (storing the rasterizer, palette and sprites on-chain) or the upload scripts and app loader for them.
---

# On-chain rasterizer

Rules: `crates/render/FORMAT.md` is the contract for code (DSL, binary formats, rasterization,
errors, validator rules, WASM ABI, crafted look). Design background: `docs/ART_DESCRIPTION.md`
(what is drawn) and `docs/STORAGE_AND_DELIVERY.md` (where it lives). The same WASM binary runs in
the browser, in tools and later in the wallet worker, so the output must be **bit-identical
everywhere**.

## Versions

- Sprite format **2**, rasterizer version **2**, sprite books **2..** (at most 4, 64 KiB each),
  palette format **2** (6-color ramps, material dither flag, curse and cold ramps): canvases
  32/64/96/128 (character layers 96x96, tiles 64x64, icons 32x32, monsters 96/128 square),
  `BITMAP` pixel sprites. Format 1 (16..64 px) and palette 1 are drawn only by rasterizer 1.
- A rule change after a version is frozen on a chain is a **new rasterizer version**, never a
  silent fix. Until devnet holds version 2, version 2 may still change; on the local validator a
  frozen v2 means asking the user to restart it with `--reset`, then `pnpm deploy:local`.

## crates/render

- `#![no_std]` + `alloc`, **integers only**, no floats, no randomness, no time, no hash maps with
  random seeds. Unknown opcode or out-of-range arg -> error code (FORMAT.md 5), never panic.
- Pipeline (`render`): decode the books -> per layer: commands (geometry, `BITMAP`) -> slot grid
  -> auto shading (levels to ramp colors) -> dithering (material flag) -> overlap shadow on the
  canvas below -> colored outline -> fill, into an indexed canvas (ramp + color per pixel) ->
  colors last: enchantment cast, `FLAG_CURSE` blend, `FLAG_GRAY` -> `RAW_RGBA`, or `PNG`
  enlarged by the integer scale that fits 512 px (96 -> x5 = 480).
- `SOCKET` marks a 4x4 square (decoder bound `SOCKET_SIZE`); `render` ignores it.
- Features: `text` (DSL and palette compilers, tools only) and `craft` (`render_crafted`:
  enchantment cast, runes in `U`, 4x4 sockets, 2 px rune glow bands; FORMAT.md 8; uses `sha2`).
  Neither is in the WASM build. `Craft` mirrors the item state of `ART_DESCRIPTION.md` 9.2 so
  crafting can add it to `RenderRequest` later; that would be a new rasterizer version.

## PNG encoder (own, no `png` crate)

- Indexed PNG (color type 3, bit depth 8): `PLTE` in first-appearance order, `tRNS` only when a
  color is not opaque; more than 256 colors is error 10.
- Each canvas row becomes `k` rows: the first with filter `None`, the rest `Up`.
- One `IDAT`: zlib `78 01`, one fixed-Huffman deflate block with repeats at distance 1 or the row
  length only, Adler-32; CRC-32 per chunk.
- Tests decode the output with the `png` crate (dev-dependency) and compare pixels.

## WASM build

- `bash crates/render/wasm/build.sh` -> `target/render-wasm/deadroll_render.wasm`, prints size
  and SHA-256. C ABI, no `wasm-bindgen`:
  ```
  alloc(len) -> ptr          dealloc(ptr, len)
  render(sprites_ptr, sprites_len, palette_ptr, palette_len, state_ptr, state_len, asset_ptr, mode)
         -> result_ptr       (0 = error)
  result_len()  result_width()  result_height()  last_error()
  ```
  `sprites` = one or more `u32 len + book`; `state` = Borsh `RenderRequest` (106 bytes);
  `asset` = 32 bytes; `mode` 0 = RAW_RGBA, 1 = PNG.
- Profile `wasm`: `opt-level = "z"`, LTO, one codegen unit, `panic = "abort"`, stripped; then
  `wasm-opt -Oz` (binaryen pinned in the script). **Budget 60 KB** (25,364 bytes with `BITMAP` and
  palette 2). Report the size in the PR.
- Golden: `cargo test -p deadroll-render --test golden` renders the reference warrior
  (`crates/render/tests/data/warrior.dsl`) and writes `target/render-golden/`; then
  `node crates/render/wasm/golden.mjs target/render-wasm/deadroll_render.wasm` must print
  "WASM output matches native". When the reference sprites change, update the data file and the
  expected hash in `golden.rs` in the same PR.

## programs/render

- `RasterizerCode` is larger than 10 KB, so it **cannot be created by `init` through CPI**. The
  client sends `SystemProgram::create_account(space = HEADER + wasm_len, owner = render program)` +
  `init_rasterizer(version, len, expected_sha256)`. Raw data, no Borsh for the body.
- `write_chunk(offset, bytes)`: authority only, before finalize, bounds checked.
- `finalize_rasterizer`: SHA-256 over the body, compare, freeze, then CPI
  `registry::set_rasterizer_from_render` signed by the render `["authority"]` PDA, which writes
  version, address and hash to `GameConfig`. Signer must be `GameConfig.authority`.
- `Palette` and `SpriteBook` follow the same create + write + finalize pattern; the books the app
  uses are listed in `GameConfig.sprite_books` (`set_sprite_books`, lookup order).
- Request extra compute units in the client for finalize (`ComputeBudgetProgram.setComputeUnitLimit`).

## Upload

`cargo run -p art -- pack` writes `target/render-assets/palette.bin` and `book_<id>.bin` (books 2..,
split at 64 KiB, identical sprites stored once). `tools/scripts` (`pnpm deploy:local`,
`deploy-devnet.ts`) upload the palette and every book, then the rasterizer, then
`set_sprite_books` with all books in id order. The app takes the palette version from the palette
`GameConfig.palette` points at. Uploads are resumable (chunks already equal on-chain are
skipped) and re-read the account before finalize.

## App loader

`GameConfig` -> `RasterizerCode` (one `getAccountInfo`) -> verify SHA-256 with WebCrypto -> cache
in IndexedDB by hash -> `WebAssembly.instantiate` -> books from `GameConfig.sprite_books` ->
render RGBA to a canvas with `imageSmoothingEnabled = false` and integer (or half-step) scale.
