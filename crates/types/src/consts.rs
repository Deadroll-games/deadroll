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
/// Soulbound item records in a `Loadout` (class starter items, docs/ITEMS_AND_CRAFT.md, 10).
pub const SOULBOUND_MAX: usize = 4;
/// Equipment slots, indexed by `EquipSlot` (`EquipSlot::COUNT`).
pub const EQUIP_SLOTS: usize = 11;
/// An empty slot in a raw equipment snapshot (`[u8; EQUIP_SLOTS]` of `ItemBase` bytes).
pub const EMPTY_ITEM: u8 = 0xFF;
