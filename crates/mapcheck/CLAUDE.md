# Zone: crates/mapcheck

Owner lane: L6. Read the root CLAUDE.md first.

## What lives here
The builder rules of docs/MAP_DESCRIPTION.md, section 10, and BFS pathfinding. `programs/map` runs `check_map` in `finalize_map`; the website builder runs the same function through WASM.

## Rules for this zone
- Docs: docs/MAP_DESCRIPTION.md (2.3, 4.2, 7, 10), docs/BESTIARY.md (2.1, 6).
- `no_std`, integers only, fixed-size buffers. The BFS frame must stay under the 4 KB SBF stack.
- Tile passability and step cost (`tiles.rs`) must match `programs/session` movement until the registry `Tileset` exists.
- Roles come through the `role_of` argument; `roster.rs` holds the P0 ids (0 Wolf, 1 Goblin, 2 Goblin Chieftain, 3 Goblin Warrior, 4 Goblin Shaman). Append ids, never renumber.
- `MapCheckError` codes are stable: append variants only, and mirror them in `programs/map` `MapError`.
- Every preset in `assets/registry/maps/*.json` must pass `tests/demo_preset.rs`.
- Forbidden: editing other zones, crates/types, agents/CONTRACTS.md.

## Commands
```
cargo test -p deadroll-mapcheck
cargo clippy -p deadroll-mapcheck --all-targets -- -D warnings
```

## Done means
- all tests green, clippy clean
- every rule has a passing case and a rejection test
