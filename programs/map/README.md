# deadroll_map

Program id `2YeyBfQ5xkRKD2kagxRS2yZYCQdWt5qidikaKfQhLBXH`.

| Instruction | Who | What |
|---|---|---|
| `create_map(index, width, height, tier)` | DM | inits `Map` and `MapLayout`, status Draft |
| `write_layout(offset, tiles)` | DM, Draft | writes tile bytes (row-major, 32 per row); 16x16 fits one tx, 32x32 two |
| `write_spawns(entrance, exit, spawns)` | DM, Draft | replaces points and spawns (max 24) |
| `finalize_map(secrets_root)` | DM, Draft | runs `deadroll-mapcheck`, stores `layout_hash`, status Finalized |
| `set_status(status)` | session PDA | Finalized -> InSession -> Finished |

Events: `MapCreated`, `LayoutWritten`, `SpawnsWritten`, `MapFinalized`, `MapStatusChanged`.
Tests: `tests/map.test.ts` (`anchor test --validator legacy --skip-build`).
