# Zone: crates/render

Owner lane: L2. Read the root CLAUDE.md first. Skill: `rasterizer` (and `pixel-sprite` for sprite sources).

## What lives here
The rasterizer and PNG encoder (`README.md` has the modules, `FORMAT.md` the formats). Its WASM is stored on-chain by `programs/render`.

## Rules for this zone
- Docs: docs/ART_DESCRIPTION.md, docs/STORAGE_AND_DELIVERY.md; format contract: `FORMAT.md`.
- `no_std` + `alloc`, integers only: no floats, no time, no randomness, no hash maps.
- Binary formats are on-chain: a new sprite format needs new book ids, the palette is append-only.
- `text` and `craft` stay out of the WASM build.
- A change to the output changes the on-chain rasterizer: a new rasterizer version, uploaded by `tools/scripts`.
- Forbidden: editing other zones, crates/types, agents/CONTRACTS.md.

## Commands
```
cargo test -p deadroll-render
cargo clippy -p deadroll-render --all-targets -- -D warnings
bash crates/render/wasm/build.sh
```

## Done means
- tests green (`tests/golden.rs`, `tests/types_layout.rs`), clippy clean, WASM builds
