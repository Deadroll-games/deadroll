# Zone: crates/mapcheck

Owner lane: L6. Read the root CLAUDE.md first.

## What lives here
The builder rules of docs/MAP_DESCRIPTION.md, section 10, and BFS pathfinding. `programs/map` runs `check_map` in `finalize_map`; the website builder runs the same function through WASM.

## Rules for this zone
- Docs: docs/MAP_DESCRIPTION.md (2.3, 4.2, 7, 10), docs/BESTIARY.md (2.1, 6).
- `no_std`, integers only, fixed-size buffers. The reachability frame must stay under the 4 KB SBF stack.
- Tile properties are data: `TileRules` from the registry `Tileset` (`deadroll_rules::tiles::tile_rules`), never constants here (patch 13). Steps go to the 8 neighbors, as in movement.
- Templates come through the `template_of` argument (`roster::SpawnRules`, built by `deadroll_rules` from the registry); this crate holds no bestiary data.
- `MapCheckError` codes are stable: append variants only, and mirror them in `programs/map` `MapError`.
- Every preset in `assets/registry/maps/*.json` must pass `crates/rules/tests/demo_preset.rs`.
- Forbidden: editing other zones, crates/types, agents/CONTRACTS.md.

## Commands
```
cargo test -p deadroll-mapcheck
cargo clippy -p deadroll-mapcheck --all-targets -- -D warnings
bash crates/mapcheck/wasm/build.sh                     # the builder WASM (wasm/README.md), commit it
UPDATE_TS_FIXTURES=1 cargo test -p deadroll-mapcheck-wasm --test ts_fixtures
```

## Done means
- all tests green, clippy clean
- a rule change: mapcheck.wasm rebuilt and the fixture refreshed (CI compares both)
- every rule has a passing case and a rejection test
