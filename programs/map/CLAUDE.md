# Zone: programs/map

Owner lane: L6. Read the root CLAUDE.md first.

## What lives here
Maps, their validation and finalization, the secrets root and the `layout` PDA (docs/ARCHITECTURE.md, 3.1). The DM builds a draft (`create_map`, `write_layout`, `write_spawns`), `finalize_map` runs `deadroll-mapcheck` and freezes it, and the session program moves it in and out of a party with `set_status`.

## Rules for this zone
- Docs: docs/MAP_DESCRIPTION.md (sections 2-4, 7, 10, 13), docs/BESTIARY.md (2.1, 6).
- Accounts owned here: `Map { core: MapData, tier }` `["map", dm, index u32 LE]`, `MapLayout { core: MapLayoutData }` `["layout", map]`. The struct names are part of the format: `programs/session` checks `account:Map` / `account:MapLayout` discriminators and reads the core at fixed offsets. Append fields only.
- `layout_hash` = SHA-256 of the layout account bytes `[8 .. 8 + MapLayoutData::SIZE]`.
- CPI out: none. CPI in: `set_status(status)` from session only, accounts `[map (mut), session_authority (signer), config]`, trust checked in `src/trust.rs` against registry `GameConfig.programs[Session]`.
- All builder rules live in `crates/mapcheck`, never re-implemented here.
- Forbidden: editing other zones, crates/types, agents/CONTRACTS.md.

## Commands
```
anchor build --tools-version v1.54
anchor test --validator legacy --skip-build     # tests/map.test.ts
cargo clippy -p deadroll-map
```

## Done means
- builds, all tests green, clippy clean
- every instruction has happy path + rejection tests
- events emitted for every state change
