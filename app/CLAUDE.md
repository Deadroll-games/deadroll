# Zone: app

Owner lane: L9. Read the root CLAUDE.md first.

## What lives here
The Vite + React + TypeScript website: wallet, session key, rasterizer loader, Character / Map builder / Party screens and the battle UI (grid, dice cascade, hit breakdown). It reads the chain directly; there is no backend.

## Rules for this zone
- Docs: CLAUDE.md section 8 (demo script), crates/render/FORMAT.md section 7 (WASM ABI), crates/combat/wasm/README.md (HitResult), docs/STORAGE_AND_DELIVERY.md, docs/GAME_COMMISSIONS.md (session keys).
- `src/chain/` is the thin typed wrapper over the programs until `sdk/` (L8, Codama) lands. Layouts mirror `agents/CONTRACTS.md` 4 and lane fields; discriminators are pinned in `disc.ts` and recomputed in tests. When sdk/ exists, replace the wrapper, keep the names.
- The rasterizer is always loaded from chain and checked against `GameConfig.rasterizer_sha256` and the finalized header. No fallback to a bundled WASM. Mock mode builds synthetic accounts and runs the same check.
- Components in `src/components/*` are pure UI over the props in their `types.ts` (no chain calls). Changing a `types.ts` is the lane owner's call.
- Pixel art: integer scale, `imageSmoothingEnabled = false`, class `pixel`.
- After `CharacterDied` the app sends `character.freeze_tombstone` (`src/chain/tombstone.ts`).
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
