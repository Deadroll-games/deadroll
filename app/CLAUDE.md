# Zone: app

Owner lane: L9. Read the root CLAUDE.md first.

## What lives here
The Vite + React + TypeScript website: wallet, session key, rasterizer loader, Character / Map builder / Party screens and the battle UI (grid, dice cascade, hit breakdown). It reads the chain directly; there is no backend.

## Rules for this zone
- Docs: CLAUDE.md section 8 (demo script), crates/render/FORMAT.md section 7 (WASM ABI), crates/combat/wasm/README.md (HitResult), docs/STORAGE_AND_DELIVERY.md, docs/GAME_COMMISSIONS.md (session keys).
- `src/chain/` is a thin layer over `@deadroll/sdk` (Codama, generated from the IDLs): instructions, accounts, PDAs, errors and events come from it, program addresses too (`config.ts` re-exports them). No layouts or discriminators of our programs here; tests pin bytes against Anchor's sha256 rule and hand-built encodings.
- The rasterizer is always loaded from chain and checked against `GameConfig.rasterizer_sha256` and the finalized header. No fallback to a bundled WASM. Mock mode builds synthetic accounts and runs the same check.
- Components in `src/components/*` are pure UI over the props in their `types.ts` (no chain calls). Changing a `types.ts` is the lane owner's call.
- Pixel art: integer scale, `imageSmoothingEnabled = false`, class `pixel`.
- After `CharacterDied` the app sends `character.freeze_tombstone` (`src/chain/tombstone.ts`).
- Instruction builders call the generated `get*Instruction` (remaining accounts through `withRemaining`); mirror `tests/demo.test.ts` account lists. Enums are numbers (`variantKey` for names) and Rust options are kit `Option`s (`optional`). After an IDL change, run `pnpm --filter @deadroll/sdk generate`, then the tests.
- The mock VRF oracle (`MOCK_VRF`) and the dev wallet (`?dev=1`) are gated on a localhost RPC. Never relax that gate.
- Forbidden: editing other zones, crates/types, agents/CONTRACTS.md, sdk/ by hand.

## Commands
```
pnpm --filter @deadroll/app dev          # VITE_DATA=mock by default
pnpm --filter @deadroll/app typecheck
pnpm --filter @deadroll/app test
pnpm --filter @deadroll/app build
```

## Done means
- typecheck, tests and build green
- every decoder and instruction builder has a byte-level test
- mock mode renders every screen without a validator
