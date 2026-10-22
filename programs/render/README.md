# programs/render

On-chain storage of the rasterizer WASM, the master palette and sprite books
(`docs/STORAGE_AND_DELIVERY.md`, formats in `crates/render/FORMAT.md`). Owner lane: L2.

## Accounts

| Account | Seeds | Body |
|---|---|---|
| `RasterizerCode` | `["rasterizer", version: u16 LE]` | WASM from `crates/render/wasm/build.sh`, max 128 KiB |
| `Palette` | `["palette", version: u16 LE]` | FORMAT.md 3.2, max 8 KiB |
| `SpriteBook` | `["sprites", book_id: u16 LE]` | FORMAT.md 3.3, max 64 KiB |

All three share one layout: Anchor discriminator, `BlobHeader`, then the body.
Byte offsets in the account data (for the registry and RPC readers):

| Offset | Field | Type |
|---|---|---|
| 0..8 | discriminator | `RasterizerCode` / `Palette` / `SpriteBook` |
| 8 | `version` (layout, = 1) | u8 |
| 9 | `kind` (1 rasterizer, 2 palette, 3 sprite book) | u8 |
| 10 | `finalized` (1 = frozen) | u8 |
| 11 | `bump` | u8 |
| 12..16 | `len` (body bytes) | u32 LE |
| 16..18 | `id` (version or book id, = PDA seed) | u16 LE |
| 18..20 | padding | |
| 20..52 | `authority` (`GameConfig.authority` at init) | Pubkey |
| 52..84 | `expected_sha256` | [u8; 32] |
| 84..116 | `sha256` (set at finalize) | [u8; 32] |
| 116.. | body | `len` bytes |

## Instructions

| Instruction | Who | What |
|---|---|---|
| `init_rasterizer(version, len, sha256)`, `init_palette(version, len, sha256)`, `init_sprite_book(book_id, len, sha256)` | `GameConfig.authority` (registry account read, no CPI) | creates the PDA with at most 10 KB |
| `extend()` | blob authority | grows by up to 10 KB (runtime limit per instruction), tops up rent; several per transaction |
| `write_chunk(offset, bytes)` | blob authority, before finalize | writes inside the body and the allocated part (~900 bytes per transaction) |
| `finalize_rasterizer()` | blob authority, who must also be `GameConfig.authority` | SHA-256 == expected, WASM magic, freeze, then CPI `registry::set_rasterizer_from_render` signed by the render `["authority"]` PDA: `GameConfig` gets the version, address and SHA-256 (`docs/STORAGE_AND_DELIVERY.md` 3.2); emits `RasterizerFinalized` |
| `finalize_palette()`, `finalize_sprite_book()` | blob authority | SHA-256 == expected, content check (palette parses with this version / book parses with this id), freeze; emits `PaletteFinalized` / `SpriteBookFinalized` |

One outgoing CPI: `finalize_rasterizer` -> `registry::set_rasterizer_from_render` (accounts:
`render_authority` signer, `config` mut, `blob`). The registry checks the signer against
`GameConfig.programs[Render]`, then pulls the finalized account like `set_rasterizer`: it checks
`owner == GameConfig.programs[Render]` and `finalized == 1` (offset 10), then copies `id`, the
address and `sha256` (offsets 16, 84). `set_rasterizer` (authority-only) stays for pointing back
at an older version. Palettes and books are pulled: `set_palette`, `set_sprite_books`.

`finalize_rasterizer` accounts: `authority` (signer), `rasterizer` (mut), `config` (mut, registry
`["config"]`), `render_authority` (this program `["authority"]` PDA), `registry_program`.

Upload order: `init_*` -> `extend` until `data_len == 116 + len` -> `write_chunk` for every chunk
-> re-read and compare -> `finalize_*` (request more compute units for large bodies). Sprite
books hold format 2 sprites (`crates/render/FORMAT.md`); a new format needs new book ids.

## Build and test

```
anchor build --tools-version v1.54 -p deadroll_render_program
bash crates/render/wasm/build.sh
cargo test -p deadroll-render --test golden
pnpm --filter @deadroll/tests run test:render   # tests/render.test.ts, local validator after pnpm deploy:local
```
