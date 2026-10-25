# deadroll_map

Program id `2YeyBfQ5xkRKD2kagxRS2yZYCQdWt5qidikaKfQhLBXH`.

| Instruction | Who | What |
|---|---|---|
| `create_map(index, width, height, tier, settings)` | DM | inits `Map` and `MapLayout` (cells Void), status Draft; `settings`: minimum players, chest radius, time to settle (patch 11) |
| `write_layout(offset, cells)` | DM, Draft | writes cell bytes (tile type in bits 7-3, variant in 2-0; row-major, 32 per row); 16x16 fits one tx, 32x32 two |
| `write_spawns(spawns)` | DM, Draft | replaces the spawns (max 24) |
| `write_points(points, boss_table)` | DM, Draft | replaces the points: the entrance, the exit, the visible chests (max `POINTS_MAX`), and the boss reward table (patch 13) |
| `finalize_map(secrets_root)` | DM, Draft | runs `deadroll-mapcheck` with the registry `bestiary`, `monster_skills`, `rewards` and `tileset` tables (accounts, read in place), stores `layout_hash`, status Finalized |
| `set_status(status)` | session PDA | Finalized -> InSession -> Finished |

Events: `MapCreated`, `LayoutWritten`, `SpawnsWritten`, `PointsWritten`, `MapFinalized`, `MapStatusChanged`.
Compute (localnet): `finalize_map` 16x16 demo ~36k CU, 32x32 ~60k CU.
Tests: `tests/map.test.ts` (`pnpm --filter @deadroll/tests run test:map` on a local validator after `pnpm deploy:local`).
