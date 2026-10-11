# deadroll-mapcheck

Map builder rules and pathfinding (docs/MAP_DESCRIPTION.md, section 10). Pure Rust, `no_std`.

```rust
use deadroll_mapcheck::{check_map, roster::p0_role};
check_map(width, height, tier, &layout, p0_role)?; // Err(MapCheckError::ExitUnreachable), ...
```

Checks, in order: sides 8..=32, tier limits, tile bytes, exactly one entrance and exit (matching the layout points), spawn count, spawn bounds / passability / overlap, known monsters, levels in the tier, one boss, helpers only in the boss pack (max 2), regular monster and pack limits, one loner per pack, pack radius 4, exit and every spawn reachable from the entrance (orthogonal BFS).

Tier limits (starter values): tier 1 = levels 1-2, 4 monsters, 3 packs; tier 2 = levels 3-4, 8 monsters, 4 packs. The boss pack does not count.

Presets: `assets/registry/maps/*.json`, format in `tests/demo_preset.rs`.
