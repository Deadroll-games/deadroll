# deadroll-mapcheck

Map builder rules and pathfinding (docs/MAP_DESCRIPTION.md, section 10). Pure Rust, `no_std`.

```rust
use deadroll_mapcheck::check_map;
// `template_of`: a bestiary id's `SpawnRules` from the registry (`deadroll_rules` builds them).
check_map(width, height, tier, &layout, |id| registry.spawn_rules(id, &cfg.monsters))?;
```

Checks, in order: sides 8..=32, tier limits, tile bytes, exactly one entrance and exit (matching the layout points), spawn count, spawn bounds / passability / overlap, known monsters, levels in the tier, the template's minimum tier, elite not on a boss, the chosen skills (exactly min(slots, pool) per category, BESTIARY 3.2), one boss, at most the tier's elites, at most `SESSION_SKILLS_MAX` distinct chosen skills, helpers only in the boss pack (max 2), regular monster and pack limits, one loner per pack, pack radius 4, exit and every spawn reachable from the entrance (orthogonal BFS).

Tier limits (starter values): tier 1 = levels 1-2, 4 monsters, 3 packs, 1 elite; tier 2 = levels 3-4, 8 monsters, 4 packs, 2 elites. The boss pack does not count.

Presets: `assets/registry/maps/*.json`, checked with the shipped registry by `crates/rules/tests/demo_preset.rs` (format there).
