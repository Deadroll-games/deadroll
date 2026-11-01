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
// Patch C: secrets under the Merkle root (docs/MAP_DESCRIPTION.md 8, 17.5).
/// Secret types counted by the declared counts (`SecretKind` without `None`), indexed by
/// `SecretKind as usize - 1`.
pub const SECRET_TYPES: usize = 4;
/// Secrets one map holds (the encrypted list, the session's found list).
pub const SECRETS_MAX: usize = 16;
/// Cells a trigger may oblige the DM to reveal before the next action (`MAX_PATH` steps).
pub const OBLIGATIONS_MAX: usize = 16;
/// Bytes of the encrypted secrets the map stores for its DM (`MapSecretsData.blob`).
pub const SECRETS_BLOB_MAX: usize = 800;
/// Bytes of a leaf's salt.
pub const SECRET_SALT_LEN: usize = 16;
/// Leaves before the map cells: one per pack number (its composition).
pub const PACK_LEAVES: usize = PACKS_MAX;
/// The most leaves a map has: the packs, a full map and every arena at full size.
pub const SECRET_LEAVES_MAX: usize = PACK_LEAVES + MAP_CELLS + PACKS_MAX * ARENA_CELLS;
/// Hashes in the longest proof (`SECRET_LEAVES_MAX` padded to a power of two).
pub const SECRET_DEPTH_MAX: usize = 12;
/// `PackLayout.visible` of an ambush pack: no monster on the map.
pub const NO_SPAWN: u8 = 0xFF;
// Patch D: map sizes (docs/MAP_DESCRIPTION.md 17.6). The size class of a map follows from its
// area (`deadroll_rules::BalanceConfig.map_size`); each class has its own storage caps, and a
// `MapLayout` is as big as its class and sides need (`map_size::LayoutGeom`).
/// Size classes: Small, Medium, Big, Dungeon.
pub const MAP_CLASSES: usize = 4;
/// Largest map side of any class.
pub const MAP_SIDE_CAP: usize = 143;
/// Most cells of any map (the Dungeon class).
pub const MAP_CELLS_CAP: usize = MAP_SIDE_CAP * MAP_SIDE_CAP;
/// Pack numbers a map of each class can store (its arenas, threats, visible monsters, rosters).
pub const CLASS_PACKS: [usize; MAP_CLASSES] = [8, 16, 24, 40];
/// Visible chests a map of each class can store.
pub const CLASS_CHESTS: [usize; MAP_CLASSES] = [4, 8, 12, 16];
/// Secrets a map of each class can hold (the encrypted list, the session's found list).
pub const CLASS_SECRETS: [usize; MAP_CLASSES] = [16, 48, 80, 96];
/// Bytes of the DM's encrypted secrets a map of each class can store.
pub const CLASS_BLOB: [usize; MAP_CLASSES] = [800, 1600, 2400, 3600];
/// Pack numbers of the largest class: a session stores this many.
pub const PACKS_CAP: usize = CLASS_PACKS[MAP_CLASSES - 1];
/// Visible chests of the largest class.
pub const CHESTS_CAP: usize = CLASS_CHESTS[MAP_CLASSES - 1];
/// Points of the largest class: the entrance, the exit and the chests.
pub const POINTS_CAP: usize = 2 + CHESTS_CAP;
/// Secrets of the largest class.
pub const SECRETS_CAP: usize = CLASS_SECRETS[MAP_CLASSES - 1];
/// Monsters one session can hold revealed: every pack's roster on the largest map.
pub const MONSTERS_CAP: usize = 80;
/// Leaves of the largest map: its packs, its cells and every arena at full size.
pub const SECRET_LEAVES_CAP: usize = PACKS_CAP + MAP_CELLS_CAP + PACKS_CAP * ARENA_CELLS;
/// Hashes in the longest proof of any map (`SECRET_LEAVES_CAP` padded to a power of two).
pub const SECRET_DEPTH_CAP: usize = 15;
/// `u64` words of one map row in the reachability check (`MAP_SIDE_CAP` bits).
pub const MAP_ROW_WORDS: usize = MAP_SIDE_CAP.div_ceil(64);
/// Hidden stashes one map can hold (patch D: the session keeps them after the visible chests).
pub const STASHES_CAP: usize = 16;
