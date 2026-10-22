# deadroll-render

The sprite rasterizer and PNG encoder of docs/ART_DESCRIPTION.md. Pure Rust, `no_std` + `alloc`, integers only. The same code runs natively (tools, tests) and as WASM (`wasm/`, stored on-chain by `programs/render`), so the output is bit-identical everywhere.

The contract (DSL, binary formats, rules, ABI) is `FORMAT.md`.

| Module | What |
|---|---|
| `sprite`, `book` | sprite and sprite book decoding |
| `palette` | the master palette |
| `raster`, `compose` | rasterizing, shading, outline, layers (body plus equipped gear) |
| `request` | `RenderRequest` from `crates/types` |
| `png` | PNG encoder |
| `text` (feature) | text DSL and palette compilers, tools only |
| `craft` (feature) | crafted look for previews |

## Build and test

```
cargo test -p deadroll-render
cargo test -p deadroll-render --test golden     # writes target/render-golden/ (used by app dev mode)
bash crates/render/wasm/build.sh
```
