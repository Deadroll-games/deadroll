# Zone: programs/render

Owner lane: L2. Read the root CLAUDE.md first.

## What lives here
On-chain storage of the rasterizer WASM, the master palette and sprite books (`README.md` has the accounts, byte offsets and instructions). The rasterizer itself is `crates/render`.

## Rules for this zone
- Docs: docs/STORAGE_AND_DELIVERY.md, docs/ART_DESCRIPTION.md; formats in `crates/render/FORMAT.md`.
- The blob layout (discriminator, `BlobHeader` at offset 8, body at 116) is read by the registry and RPC clients: never move a field.
- A finalized blob is frozen. New content means a new version or book id, never an overwrite.
- One outgoing CPI: `finalize_rasterizer` -> `registry::set_rasterizer_from_render`, signed by the `["authority"]` PDA.
- Uploads are done by `tools/scripts` (skill `rasterizer`), never by hand.
- Forbidden: editing other zones, crates/types, agents/CONTRACTS.md.

## Commands
```
anchor build --tools-version v1.54 -p deadroll_render_program
cargo clippy -p deadroll-render-program --all-targets -- -D warnings
pnpm --filter @deadroll/tests run test:render
```

## Done means
- builds, all tests green, clippy clean
- every instruction has happy path + rejection tests in `tests/render.test.ts`
