# Zone: programs/map

Owner lane: L6. Read the root CLAUDE.md first.

## What lives here
Maps, their validation and finalization, the secrets root and the `layout` PDA (docs/ARCHITECTURE.md, 3.1). The DM builds a draft (`create_map` with `grow_layout` for a big map, `write_layout`, `write_spawns`, `write_points`, `write_arena`, `write_secrets`), `check_step` runs a big map's reachability check in steps, `finalize_map` finishes it, runs `deadroll-mapcheck` and freezes the map; the session program moves it into a party with `set_status` and closes it after the rewards with `close_map` (patch D: a map is played once).

## Rules for this zone
- Docs: docs/MAP_DESCRIPTION.md (sections 2-4, 7, 10, 13), docs/BESTIARY.md (2.1, 6).
- `lib/Cargo.toml` is this program as a dependency of other crates (same `src/`, rlib): a dependency or feature added to `Cargo.toml` goes there too.
- Accounts owned here: `Map { core: MapData, tier, min_players, chest_radius, settle_timeout_secs, biome }` `["map", dm, index u32 LE]` (a version 1 map has no `biome` byte: read it with `Map::decode_padded`), `MapLayout { core: MapLayoutData }` `["layout", map]`. The struct names are part of the format: `programs/session` checks `account:Map` / `account:MapLayout` discriminators and reads the core at fixed offsets. Append fields only.
- Patch D: `MapLayout` version 5, sized by the map's class and sides; every offset comes from `deadroll_types::LayoutGeom` (shared with the session). `layout_hash` = SHA-256 of `[8 .. blob_len)`: everything public, not the encrypted secrets or the check's scratch. A layout past 10 KB grows 10 KB per instruction (`grow_layout`).
- Patch C: `MapLayout.core.spawns` hold only each pack's visible monster; the rosters and the secrets are under `secrets_root`, checked by the session when revealed. The program never reads the encrypted copy.
- CPI out: none. CPI in, from session only, trust checked in `src/trust.rs` against registry `GameConfig.programs[Session]`: `set_status(status)` `[map (mut), session_authority (signer), config]`; patch D `close_map` `[map (mut), layout (mut), dm (mut), session_authority (signer), config]`.
- All builder rules live in `crates/mapcheck`, never re-implemented here.
- Forbidden: editing other zones, crates/types, agents/CONTRACTS.md.

## Commands
```
anchor build --tools-version v1.54
pnpm --filter @deadroll/tests run test:map      # tests/map.test.ts, local validator after pnpm deploy:local
cargo clippy -p deadroll-map
```

## Done means
- builds, all tests green, clippy clean
- every instruction has happy path + rejection tests
- events emitted for every state change
