//! Shared constants (agents/CONTRACTS.md, section 3).

/// Basis points: `10_000` = 100% = x1.
pub const BP: u32 = 10_000;
/// Max map side in cells.
pub const MAP_MAX: usize = 32;
/// Cells in a full map layout (`MAP_MAX * MAP_MAX`).
pub const MAP_CELLS: usize = MAP_MAX * MAP_MAX;
pub const PARTY_MAX: usize = 4;
pub const MONSTERS_MAX: usize = 24;
/// Spawn points in a map layout, one per possible monster.
pub const SPAWNS_MAX: usize = MONSTERS_MAX;
/// Slots in the turn order: every player and every monster.
pub const TURN_ORDER_MAX: usize = PARTY_MAX + MONSTERS_MAX;
pub const SKILL_SLOTS_ACTIVE: usize = 4;
pub const BAG_SLOTS: usize = 20;
/// Active states on one unit: one instance per state kind.
pub const STATES_MAX: usize = 6;
/// Sprite layers in a render request and in a character appearance.
pub const LAYERS_MAX: usize = 20;
/// Game programs listed in `GameConfig.programs`, indexed by `ProgramIdx`.
pub const PROGRAMS_COUNT: usize = 7;
/// Core stats, indexed by `Stat`.
pub const STATS_COUNT: usize = 4;
/// Bytes reserved for the NFT metadata uri prefix in `GameConfig`.
pub const URI_BASE_LEN: usize = 64;
/// Soulbound item records of a character (class starter items, docs/ITEMS_AND_CRAFT.md, 10).
pub const SOULBOUND_MAX: usize = 4;
/// Equipment slots, indexed by `EquipSlot` (`EquipSlot::COUNT`).
pub const EQUIP_SLOTS: usize = 11;
/// An empty slot in a raw equipment snapshot (`[u8; EQUIP_SLOTS]` of `ItemBase` bytes).
pub const EMPTY_ITEM: u8 = 0xFF;
/// `Spawn.flags` / `Monster.flags` bit: the elite version (docs/BESTIARY.md, section 5).
pub const SPAWN_ELITE: u8 = 1;
/// Skill slots of one monster in a session (docs/BESTIARY.md 3.2, the boss's): `0..5` active,
/// `5` reactive, `6` passive.
pub const MONSTER_SLOTS: usize = 7;
/// Distinct monster skills one session copies from the registry (`MonsterState.skill_table`).
pub const SESSION_SKILLS_MAX: usize = 32;
/// Patch 11: visible chests one map layout can hold (`MapLayoutData.chests`); the limit per
/// map tier is `deadroll_rules::LootTuning::chests`.
pub const CHESTS_MAX: usize = 4;
/// "No reward table" (`MapLayoutData.boss_table`, a bestiary template's loot table).
pub const NO_TABLE: u8 = 0xFF;
/// Distinct rewards one temporary cache holds, merged by `(kind, id)`.
pub const CACHE_SLOTS: usize = 32;
/// Bytes of a raw cache (`CacheSlot::SIZE` per slot), as `Participation` stores it.
pub const CACHE_BYTES: usize = CACHE_SLOTS * 5;
/// Resource slots of the bag (docs/ITEMS_AND_CRAFT.md 6.1): one kind per slot.
pub const BAG_RESOURCE_SLOTS: usize = 40;
/// Consumable slots of the bag (docs/ITEMS_AND_CRAFT.md 6.1).
pub const BAG_CONSUMABLE_SLOTS: usize = 5;
/// Patch 13: tile types a cell byte can name (bits 7-3, docs/MAP_DESCRIPTION.md 4.1).
pub const TILE_KINDS_MAX: usize = 32;
/// Patch 13: variants of one tile type (bits 2-0).
pub const TILE_VARIANTS: usize = 8;
/// Patch 13: points of one map layout (`MapLayoutDataV2.points`): the entrance, the exit and
/// the visible chests.
pub const POINTS_MAX: usize = 2 + CHESTS_MAX;
/// City map size limits (docs/CITY_DESCRIPTION.md 3.1, 9): row-major cells, `CITY_W_MAX` per row.
pub const CITY_W_MAX: usize = 48;
pub const CITY_H_MAX: usize = 32;
/// Cells of the full city layout (`CITY_W_MAX * CITY_H_MAX`).
pub const CITY_CELLS: usize = CITY_W_MAX * CITY_H_MAX;
/// Decor objects of the city (`CityData.objects`).
pub const CITY_OBJECTS_MAX: usize = 32;
/// Arena update: packs of one map (`MapArenasData.packs`); a spawn's `pack` is its index here.
pub const PACKS_MAX: usize = 8;
/// Arena update: storage cap of an arena side; `deadroll_rules::BalanceConfig.arena` sets the
/// allowed range within it.
pub const ARENA_SIDE_MAX: usize = 12;
/// Cells of one stored arena, row-major, `ARENA_SIDE_MAX` per row.
pub const ARENA_CELLS: usize = ARENA_SIDE_MAX * ARENA_SIDE_MAX;
/// Arena update: monsters of one pack in its arena, the live slots of the session `MonsterState`.
pub const ARENA_MONSTERS_MAX: usize = 8;
/// Arena update: distinct bestiary templates one party copies at `open_session` (the session
/// `MonsterState` keeps one record per template, not per monster).
pub const SESSION_TEMPLATES_MAX: usize = 12;
