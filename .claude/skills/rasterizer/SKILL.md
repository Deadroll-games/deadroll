---
name: rasterizer
description: Use when working on crates/render (sprite DSL decoder, shading, outline, PNG encoder), its WASM build, programs/render (storing the rasterizer, palette and sprites on-chain) or the upload scripts and app loader for them.
---

# On-chain rasterizer

Rules: `docs/ART_DESCRIPTION.md` (what is drawn) and `docs/STORAGE_AND_DELIVERY.md` (where it lives). The same WASM binary runs in the browser, in tools and later in the wallet worker, so the output must be **bit-identical everywhere**.

## crates/render

- `#![no_std]` + `alloc`, **integers only**, no floats, no randomness, no time, no hash maps with random seeds.
- Pipeline: decode DSL bytes -> per layer: fill slots -> auto shading (top-left light, rule in `pixel-sprite`) -> outline `#120c16` -> composite layers in fixed order -> map slots to ramps from `Palette` -> RGBA 32x32 -> optional integer scale -> optional PNG.
- First task: write `crates/render/FORMAT.md` (text DSL and its binary encoding: opcodes, args as `u8`, slot ids) from `ART_DESCRIPTION.md`. It is the contract for `tools/art`, Grok sprite tasks and the upload script. Do not change it after art tasks start.
- Unknown opcode or out-of-range arg -> error, never panic.

## PNG encoder (own, no `png` crate)

- Signature, `IHDR` (RGBA8), one `IDAT`, `IEND`.
- zlib stream with **stored (uncompressed) deflate blocks**, max 65535 bytes per block, filter byte 0 per row.
- CRC32 per chunk (table built at compile time with `const fn`), Adler32 over raw data.
- Test: decode output with the `png` crate in `dev-dependencies` and compare pixels.

## WASM build

- C ABI, no `wasm-bindgen` (works the same in browser, worker and Node):
  ```
  alloc(len) -> ptr          dealloc(ptr, len)
  render(input_ptr, input_len, out_mode) -> result_ptr   // out_mode: 0 = RGBA, 1 = PNG
  result_len() -> u32
  ```
- Input = `RenderRequest` (Borsh from `crates/types`) + palette bytes + sprite bytes, concatenated with u32 length prefixes.
- Release profile: `opt-level = "z"`, `lto = true`, `codegen-units = 1`, `panic = "abort"`, `strip = true`; then `wasm-opt -Oz`. **Budget 60 KB.** Report the size in the PR.
- Golden test: render the reference warrior to PNG in native Rust and through the WASM (Node) and compare SHA-256.

## programs/render

- `RasterizerCode` is larger than 10 KB, so it **cannot be created by `init` through CPI**. The client sends one transaction: `SystemProgram::create_account(space = HEADER + wasm_len, owner = render program)` + `init_rasterizer(version, len, expected_sha256)`. Use `AccountLoader`/raw data, no Borsh for the body.
- `write_chunk(offset: u32, bytes: Vec<u8>)`: authority only, before finalize, bounds checked. ~900 bytes per transaction.
- `finalize_rasterizer`: SHA-256 over the body (`solana_program::hash` / `hashv`, compute in pieces if the CU budget needs it), compare with expected, set `finalized = true`, CPI `registry::set_rasterizer(version, address, sha256)` signed by the render `["authority"]` PDA. After finalize nothing can write.
- `Palette` and `SpriteBook` follow the same create + write + finalize pattern with their own versions.
- Request extra compute units in the client for finalize (`ComputeBudgetProgram.setComputeUnitLimit`).

## Upload script (`tools/scripts/upload-rasterizer.ts`)

Build -> wasm-opt -> sha256 -> create account -> write chunks in parallel batches with retries -> re-read account and compare bytes -> finalize -> print address and hash. Must be resumable (skip chunks already equal on-chain).

## App loader

`GameConfig` -> `RasterizerCode` (one `getAccountInfo`) -> verify SHA-256 with WebCrypto -> cache in IndexedDB by hash -> `WebAssembly.instantiate` -> render RGBA to canvas with `imageSmoothingEnabled = false` and integer scale.
