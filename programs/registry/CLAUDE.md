# Zone: programs/registry

Owner lane: L4. Read the root CLAUDE.md first.

## What lives here
`GameConfig` and the data tables (aspects, resonances, bestiary, tileset, prefabs) that other programs read without CPI (docs/ARCHITECTURE.md, 4.2, 4.3). The registry never calls other programs: it pulls blobs from `render` by reading their bytes.

## Rules for this zone
- Docs: docs/ARCHITECTURE.md (4.2, 4.3), docs/BESTIARY.md (2, 7, 12), docs/STORAGE_AND_DELIVERY.md.
- `GameConfig { core: GameConfigData }` and the struct name are part of the format: `map` and `render` read it by discriminator and offset. Never reorder or insert before the core; append only.
- Table record layouts are in README.md and `src/state/records.rs`; they are consumed by `session` (monster hp, speed, tile cost), the seed script and the app. Append fields only inside the reserved bytes.
- Render blob offsets come from `programs/render/README.md`; if that header changes, update `src/state/pull.rs` and its tests.
- Only `GameConfig.authority` writes. Program ids in the list: all 7, none empty.
- Forbidden: editing other zones, crates/types, agents/CONTRACTS.md.

## Commands
```
cargo test -p deadroll-registry
cargo clippy -p deadroll-registry --all-targets
anchor build --tools-version v1.54 -p deadroll_registry
```

## Done means
- builds, all tests green, clippy clean
- every instruction has happy path + rejection tests (unit tests here, `tests/registry.test.ts` on the validator)
- events emitted for every state change
