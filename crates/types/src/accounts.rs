//! Minimum P0 account layouts (agents/CONTRACTS.md, section 4).
//!
//! A program wraps the `<Name>Data` core in its own `#[account]` struct and appends its
//! lane-specific fields after it (CONTRACTS.md, section 4.1):
//!
//! ```ignore
//! #[account]
//! #[derive(InitSpace)]
//! pub struct Character {
//!     pub core: CharacterData,
//!     // lane extras, only appended
//! }
//! ```
//!
//! `version` is the first field of every core, so it is the first byte after the
//! discriminator. `SIZE` is the max borsh size (`Option` counted as `Some`).

use crate::{
    Aspect, BuildingKind, BuildingStatus, Class, EquipSlot, EquipSource, Facing, ItemBase,
    LayerRef, MapStatus, PointKind, Pubkey, Race, Requirement, RewardKind, SessionStatus,
    StateKind, ARENA_CELLS, ARENA_SIDE_MAX, BAG_CONSUMABLE_SLOTS, BAG_RESOURCE_SLOTS, CACHE_BYTES,
    CACHE_SLOTS, CHESTS_MAX, CITY_CELLS, CITY_OBJECTS_MAX, EMPTY_ITEM, EQUIP_SLOTS, LAYERS_MAX,
    MAP_CELLS, MONSTERS_MAX, PACKS_MAX, PARTY_MAX, POINTS_MAX, PROGRAMS_COUNT, SOULBOUND_MAX,
    SPAWNS_MAX, STATES_MAX, STATS_COUNT, TURN_ORDER_MAX, URI_BASE_LEN,
};

const PUBKEY: usize = 32;
const OPTION_PUBKEY: usize = 1 + PUBKEY;

shared! {
    /// A map cell coordinate.
    #[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
    pub struct Cell {
        pub x: u8,
        pub y: u8,
    }

    /// A state on a unit. `turns == 0` marks an empty slot.
    #[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
    pub struct ActiveState {
        pub kind: StateKind,
        pub turns: u8,
        /// Strength of the state (damage per turn for DoT, basis points for slows).
        pub power: u32,
        /// Turn-order slot of the unit that applied it.
        pub source: u8,
    }

    /// A monster spawn point in a map layout.
    #[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
    pub struct Spawn {
        pub bestiary_id: u8,
        pub x: u8,
        pub y: u8,
        pub level: u8,
        pub skills: u16,
        pub flags: u8,
        pub aspects: u8,
        pub pack: u8,
    }

    /// Patch 11: a map point `(type, x, y, data)` (docs/MAP_DESCRIPTION.md 7.1).
    #[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
    pub struct MapPoint {
        pub kind: PointKind,
        pub x: u8,
        pub y: u8,
        /// A visible chest: its reward table id.
        pub data: u8,
    }

    /// Patch 11: one reward kind in a temporary cache, `qty == 0` = empty.
    #[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
    pub struct CacheSlot {
        pub kind: RewardKind,
        pub id: u16,
        pub qty: u16,
    }

    /// Patch 11: one bag slot (docs/ITEMS_AND_CRAFT.md 6.1), `qty == 0` = empty (then `kind`
    /// and `id` are zero too).
    #[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
    pub struct BagSlot {
        pub kind: RewardKind,
        pub id: u16,
        pub qty: u8,
    }

    /// Patch 11: what a character carries besides items (docs/ITEMS_AND_CRAFT.md 6.1): resources
    /// (recipes, blanks and reagents too) one kind per slot, and consumables. In character `Bag`
    /// and the "Done" of the bag screen.
    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    pub struct BagLayout {
        pub resources: [BagSlot; BAG_RESOURCE_SLOTS],
        pub consumables: [BagSlot; BAG_CONSUMABLE_SLOTS],
    }

    /// A live monster in a session.
    #[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
    pub struct Monster {
        pub bestiary_id: u8,
        pub level: u8,
        pub x: u8,
        pub y: u8,
        pub hp: u32,
        pub hp_max: u32,
        pub action_points: u8,
        pub alive: bool,
        pub pack: u8,
        pub flags: u8,
        pub states: [ActiveState; STATES_MAX],
    }

    /// registry `["config"]`.
    #[derive(Clone, Debug, PartialEq, Eq)]
    pub struct GameConfigData {
        pub version: u8,
        pub authority: Pubkey,
        /// Trusted game program ids, indexed by `ProgramIdx`.
        pub programs: [Pubkey; PROGRAMS_COUNT],
        pub rasterizer_version: u16,
        pub rasterizer: Pubkey,
        pub rasterizer_sha256: [u8; 32],
        pub palette: Pubkey,
        /// NFT metadata uri prefix, zero-padded.
        pub uri_base: [u8; URI_BASE_LEN],
        pub bump: u8,
    }

    /// character `["character", owner, index]`.
    #[derive(Clone, Debug, PartialEq, Eq)]
    pub struct CharacterData {
        pub version: u8,
        pub owner: Pubkey,
        pub index: u8,
        /// Core NFT asset; `None` under the `nft` fallback.
        pub nft: Option<Pubkey>,
        pub race: Race,
        pub class: Class,
        pub level: u16,
        pub xp: u32,
        /// Indexed by `Stat`.
        pub stats: [u16; STATS_COUNT],
        pub predisposition: Aspect,
        /// `1..=14`, see `Tier`.
        pub predisposition_tier: u8,
        pub hp_max: u32,
        pub alive: bool,
        /// The session that locked this character.
        pub in_session: Option<Pubkey>,
        pub appearance: [LayerRef; LAYERS_MAX],
        /// Bumped on every visible change (NFT uri cache key).
        pub state_version: u32,
        pub bump: u8,
    }

    /// map `["map", dm, map_index]`.
    #[derive(Clone, Debug, PartialEq, Eq)]
    pub struct MapData {
        pub version: u8,
        pub dm: Pubkey,
        pub index: u32,
        pub width: u8,
        pub height: u8,
        pub status: MapStatus,
        pub layout_hash: [u8; 32],
        pub secrets_root: [u8; 32],
        pub bump: u8,
    }

    /// map `["layout", map]`.
    #[derive(Clone, Debug, PartialEq, Eq)]
    pub struct MapLayoutData {
        pub version: u8,
        pub map: Pubkey,
        /// Row-major `Tile` bytes, `MAP_MAX` cells per row.
        pub tiles: [u8; MAP_CELLS],
        pub spawns: [Spawn; SPAWNS_MAX],
        pub spawn_count: u8,
        pub entrance: Cell,
        pub exit: Cell,
        pub bump: u8,
        // Patch 11: points and rewards (docs/MAP_DESCRIPTION.md 7.1, PARTY_AND_TURNS.md 9.2).
        /// Visible chests, `0..chest_count` used: `data` is the reward table id.
        pub chests: [MapPoint; CHESTS_MAX],
        pub chest_count: u8,
        /// The reward table of the boss (registry `Rewards` id, `NO_TABLE` = none).
        pub boss_table: u8,
    }

    /// session `["session", map]`.
    #[derive(Clone, Debug, PartialEq, Eq)]
    pub struct GameSessionData {
        pub version: u8,
        pub map: Pubkey,
        pub dm: Pubkey,
        pub status: SessionStatus,
        pub price_lamports: u64,
        pub players: [Pubkey; PARTY_MAX],
        pub player_count: u8,
        pub turn_order: [u8; TURN_ORDER_MAX],
        pub turn_index: u8,
        pub round: u16,
        pub action_counter: u32,
        pub pending_vrf: bool,
        pub last_seed: [u8; 32],
        pub bump: u8,
    }

    /// session `["participation", session, character]`.
    #[derive(Clone, Debug, PartialEq, Eq)]
    pub struct ParticipationData {
        pub version: u8,
        /// The party (`GameSession` PDA). Patch 10: named as in docs/GAME_COMMISSIONS.md, 5
        /// (was `session`; same bytes).
        pub game_session: Pubkey,
        pub character: Pubkey,
        /// The temporary key that acts for this character in this party, `Pubkey::default()` =
        /// none. Patch 10: named as in docs/GAME_COMMISSIONS.md, 5 (was `actor`; same bytes).
        pub session_key: Pubkey,
        pub slot: u8,
        pub x: u8,
        pub y: u8,
        pub hp: u32,
        pub action_points: u8,
        pub states: [ActiveState; STATES_MAX],
        pub ready: bool,
        pub bump: u8,
    }

    /// A soulbound item record (a class starter item), in `deadroll_character` `Gear`.
    #[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
    pub struct ItemRecord {
        pub base: ItemBase,
        /// Item level (docs/ITEMS_AND_CRAFT.md, section 4).
        pub level: u8,
    }

    /// What one equipment slot holds. `index` is the soulbound record for `Soulbound`; `asset`
    /// is the item NFT for `Asset` (crafting, later), default otherwise. In `deadroll_character`
    /// `Gear`.
    #[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
    pub struct EquipRef {
        pub source: EquipSource,
        pub index: u8,
        pub asset: Pubkey,
    }

    /// items `["loadout", character]`: the soulbound items and what is worn, by slot. Not used
    /// since patch 1: the gear lives in the character account (`deadroll_character` `Gear`).
    /// Kept because this crate only grows.
    #[derive(Clone, Debug, PartialEq, Eq)]
    pub struct LoadoutData {
        pub version: u8,
        pub character: Pubkey,
        /// The character owner (for `memcmp` filters).
        pub owner: Pubkey,
        /// Records `0..item_count` are used.
        pub items: [ItemRecord; SOULBOUND_MAX],
        pub item_count: u8,
        /// Indexed by `EquipSlot`.
        pub equipped: [EquipRef; EQUIP_SLOTS],
        pub bump: u8,
    }

    /// session `["monsters", session]`.
    #[derive(Clone, Debug, PartialEq, Eq)]
    pub struct MonsterStateData {
        pub version: u8,
        pub session: Pubkey,
        pub monsters: [Monster; MONSTERS_MAX],
        pub count: u8,
        pub bump: u8,
    }
}

impl Cell {
    pub const SIZE: usize = 2;
}

impl ActiveState {
    pub const SIZE: usize = StateKind::SIZE + 1 + 4 + 1;

    pub const fn is_empty(&self) -> bool {
        self.turns == 0
    }
}

impl Spawn {
    pub const SIZE: usize = 4 + 2 + 3;
}

impl Monster {
    pub const SIZE: usize = 4 + 4 + 4 + 4 + STATES_MAX * ActiveState::SIZE;
}

impl GameConfigData {
    pub const SIZE: usize =
        1 + PUBKEY + PROGRAMS_COUNT * PUBKEY + 2 + PUBKEY + 32 + PUBKEY + URI_BASE_LEN + 1;
}

impl CharacterData {
    pub const SIZE: usize = 1
        + PUBKEY
        + 1
        + OPTION_PUBKEY
        + Race::SIZE
        + Class::SIZE
        + 2
        + 4
        + STATS_COUNT * 2
        + Aspect::SIZE
        + 1
        + 4
        + 1
        + OPTION_PUBKEY
        + LAYERS_MAX * LayerRef::SIZE
        + 4
        + 1;
}

impl MapData {
    pub const SIZE: usize = 1 + PUBKEY + 4 + 1 + 1 + MapStatus::SIZE + 32 + 32 + 1;
}

impl MapLayoutData {
    pub const SIZE: usize = 1
        + PUBKEY
        + MAP_CELLS
        + SPAWNS_MAX * Spawn::SIZE
        + 1
        + Cell::SIZE * 2
        + 1
        + CHESTS_MAX * MapPoint::SIZE
        + 1
        + 1;

    /// The visible chests in use.
    pub fn chest_list(&self) -> &[MapPoint] {
        &self.chests[..(self.chest_count as usize).min(CHESTS_MAX)]
    }
}

impl MapPoint {
    pub const SIZE: usize = PointKind::SIZE + 3;

    pub const fn cell(&self) -> Cell {
        Cell {
            x: self.x,
            y: self.y,
        }
    }
}

impl CacheSlot {
    pub const SIZE: usize = RewardKind::SIZE + 2 + 2;
    pub const EMPTY: CacheSlot = CacheSlot {
        kind: RewardKind::Resource,
        id: 0,
        qty: 0,
    };

    pub const fn is_empty(&self) -> bool {
        self.qty == 0
    }

    /// Slot `at` of a raw cache (`Participation` keeps it as bytes, so a VRF callback does not
    /// decode it field by field). A byte that is not a `RewardKind` reads as empty.
    pub fn read(raw: &[u8; CACHE_BYTES], at: usize) -> CacheSlot {
        let b = &raw[at * Self::SIZE..(at + 1) * Self::SIZE];
        match RewardKind::try_from(b[0]) {
            Ok(kind) => CacheSlot {
                kind,
                id: u16::from_le_bytes([b[1], b[2]]),
                qty: u16::from_le_bytes([b[3], b[4]]),
            },
            Err(_) => Self::EMPTY,
        }
    }

    /// Writes slot `at` of a raw cache (the same bytes as borsh).
    pub fn write(&self, raw: &mut [u8; CACHE_BYTES], at: usize) {
        let b = &mut raw[at * Self::SIZE..(at + 1) * Self::SIZE];
        b[0] = self.kind as u8;
        b[1..3].copy_from_slice(&self.id.to_le_bytes());
        b[3..5].copy_from_slice(&self.qty.to_le_bytes());
    }

    /// Every slot of a raw cache.
    pub fn read_all(raw: &[u8; CACHE_BYTES]) -> [CacheSlot; CACHE_SLOTS] {
        let mut out = [Self::EMPTY; CACHE_SLOTS];
        for (i, s) in out.iter_mut().enumerate() {
            *s = Self::read(raw, i);
        }
        out
    }

    /// A raw cache holding these slots.
    pub fn write_all(slots: &[CacheSlot; CACHE_SLOTS]) -> [u8; CACHE_BYTES] {
        let mut raw = [0; CACHE_BYTES];
        for (i, s) in slots.iter().enumerate() {
            s.write(&mut raw, i);
        }
        raw
    }
}

const _: () = assert!(CACHE_BYTES == CACHE_SLOTS * CacheSlot::SIZE);

impl BagLayout {
    pub const SIZE: usize = (BAG_RESOURCE_SLOTS + BAG_CONSUMABLE_SLOTS) * BagSlot::SIZE;
    pub const EMPTY: BagLayout = BagLayout {
        resources: [BagSlot::EMPTY; BAG_RESOURCE_SLOTS],
        consumables: [BagSlot::EMPTY; BAG_CONSUMABLE_SLOTS],
    };

    /// Every slot, resources first.
    pub fn slots(&self) -> impl Iterator<Item = &BagSlot> {
        self.resources.iter().chain(&self.consumables)
    }
}

impl Default for BagLayout {
    fn default() -> Self {
        Self::EMPTY
    }
}

impl BagSlot {
    pub const SIZE: usize = RewardKind::SIZE + 2 + 1;
    pub const EMPTY: BagSlot = BagSlot {
        kind: RewardKind::Resource,
        id: 0,
        qty: 0,
    };

    pub const fn is_empty(&self) -> bool {
        self.qty == 0
    }
}

impl GameSessionData {
    pub const SIZE: usize = 1
        + PUBKEY * 2
        + SessionStatus::SIZE
        + 8
        + PARTY_MAX * PUBKEY
        + 1
        + TURN_ORDER_MAX
        + 1
        + 2
        + 4
        + 1
        + 32
        + 1;
}

impl ParticipationData {
    pub const SIZE: usize = 1 + PUBKEY * 3 + 3 + 4 + 1 + STATES_MAX * ActiveState::SIZE + 1 + 1;
}

impl MonsterStateData {
    pub const SIZE: usize = 1 + PUBKEY + MONSTERS_MAX * Monster::SIZE + 1 + 1;
}

impl ItemRecord {
    pub const SIZE: usize = ItemBase::SIZE + 1;
}

impl EquipRef {
    pub const EMPTY: EquipRef = EquipRef {
        source: EquipSource::Empty,
        index: 0,
        asset: Pubkey::new_from_array([0; 32]),
    };
    pub const SIZE: usize = EquipSource::SIZE + 1 + PUBKEY;

    /// Soulbound record `index` of the loadout.
    pub const fn soulbound(index: u8) -> EquipRef {
        EquipRef {
            source: EquipSource::Soulbound,
            index,
            asset: Pubkey::new_from_array([0; 32]),
        }
    }
}

impl LoadoutData {
    pub const SIZE: usize =
        1 + PUBKEY * 2 + SOULBOUND_MAX * ItemRecord::SIZE + 1 + EQUIP_SLOTS * EquipRef::SIZE + 1;

    /// The `ItemBase` byte worn in each slot (`EMPTY_ITEM` for none): what combat reads. Item
    /// NFTs (`EquipSource::Asset`) are not resolved yet and read as empty.
    pub fn equipment(&self) -> [u8; EQUIP_SLOTS] {
        let mut out = [EMPTY_ITEM; EQUIP_SLOTS];
        for (slot, r) in out.iter_mut().zip(&self.equipped) {
            if r.source == EquipSource::Soulbound && r.index < self.item_count {
                if let Some(item) = self.items.get(r.index as usize) {
                    *slot = item.base as u8;
                }
            }
        }
        out
    }

    /// The slot record `index` is worn in, if any.
    pub fn worn_slot(&self, index: u8) -> Option<EquipSlot> {
        self.equipped
            .iter()
            .position(|r| r.source == EquipSource::Soulbound && r.index == index)
            .and_then(|i| EquipSlot::ALL.get(i).copied())
    }
}

const _: () = assert!(EQUIP_SLOTS == EquipSlot::COUNT);

// Patch 13: the map layout with points (docs/MAP_DESCRIPTION.md 3, 4.1, 7.1). It replaces
// `MapLayoutData` as the core of the map program's `MapLayout`; the old struct stays unused.
shared! {
    /// map `["layout", map]` (patch 13).
    #[derive(Clone, Debug, PartialEq, Eq)]
    pub struct MapLayoutDataV2 {
        pub version: u8,
        pub map: Pubkey,
        /// Row-major cell bytes (`cell_byte`: tile type and variant), `MAP_MAX` cells per row.
        pub cells: [u8; MAP_CELLS],
        pub spawns: [Spawn; SPAWNS_MAX],
        pub spawn_count: u8,
        /// The entrance, the exit and the visible chests, `0..point_count` used.
        pub points: [MapPoint; POINTS_MAX],
        pub point_count: u8,
        /// The reward table of the boss (registry `Rewards` id, `NO_TABLE` = none).
        pub boss_table: u8,
        pub bump: u8,
    }
}

impl MapLayoutDataV2 {
    pub const SIZE: usize = 1
        + PUBKEY
        + MAP_CELLS
        + SPAWNS_MAX * Spawn::SIZE
        + 1
        + POINTS_MAX * MapPoint::SIZE
        + 1
        + 1
        + 1;

    /// The points in use.
    pub fn point_list(&self) -> &[MapPoint] {
        &self.points[..(self.point_count as usize).min(POINTS_MAX)]
    }

    /// The first point of `kind`, if any (`deadroll_mapcheck` makes the entrance and the exit
    /// unique).
    pub fn point(&self, kind: PointKind) -> Option<Cell> {
        self.point_list()
            .iter()
            .find(|p| p.kind == kind)
            .map(MapPoint::cell)
    }

    pub fn entrance(&self) -> Option<Cell> {
        self.point(PointKind::Entrance)
    }

    pub fn exit(&self) -> Option<Cell> {
        self.point(PointKind::Exit)
    }

    /// The visible chests in point order (a chest's index in a session is its place here).
    pub fn chests(&self) -> impl Iterator<Item = &MapPoint> {
        self.point_list()
            .iter()
            .filter(|p| p.kind == PointKind::Chest)
    }
}

// Arena update (docs/MAP_DESCRIPTION.md 17, PARTY_AND_TURNS.md 4.1): every pack has one monster
// on the map and its own arena. A spawn's `x`, `y` are its cell in its pack's arena; the map cell
// is the pack's. In the map program's `MapLayout` after the core.
shared! {
    /// One pack of a map: the monster shown on the map, where it stands and looks, and the size of
    /// its arena. `width == 0` marks a pack number the map does not use.
    #[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
    pub struct PackLayout {
        /// Index into `MapLayoutDataV2.spawns` of the monster shown on the map (one of the pack).
        pub visible: u8,
        /// The visible monster's cell on the map.
        pub x: u8,
        pub y: u8,
        pub facing: Facing,
        /// Arena size in cells, within `deadroll_rules::BalanceConfig.arena`.
        pub width: u8,
        pub height: u8,
    }

    /// The packs of a map and their arenas, by pack number.
    #[derive(Clone, Debug, PartialEq, Eq)]
    pub struct MapArenasData {
        pub packs: [PackLayout; PACKS_MAX],
        /// Row-major cell bytes (`cell_byte`) of each pack's arena, `ARENA_SIDE_MAX` per row.
        pub cells: [[u8; ARENA_CELLS]; PACKS_MAX],
    }
}

impl PackLayout {
    pub const SIZE: usize = 1 + 2 + Facing::SIZE + 2;
    pub const EMPTY: PackLayout = PackLayout {
        visible: 0,
        x: 0,
        y: 0,
        facing: Facing::N,
        width: 0,
        height: 0,
    };

    /// The map uses this pack number.
    pub const fn is_used(&self) -> bool {
        self.width != 0
    }

    /// The visible monster's cell on the map.
    pub const fn cell(&self) -> Cell {
        Cell {
            x: self.x,
            y: self.y,
        }
    }
}

impl MapArenasData {
    pub const SIZE: usize = PACKS_MAX * PackLayout::SIZE + PACKS_MAX * ARENA_CELLS;
    pub const EMPTY: MapArenasData = MapArenasData {
        packs: [PackLayout::EMPTY; PACKS_MAX],
        cells: [[0; ARENA_CELLS]; PACKS_MAX],
    };

    /// The cell byte at `(x, y)` of pack `pack`'s arena (`None` outside the storage).
    pub fn cell(&self, pack: u8, x: u8, y: u8) -> Option<u8> {
        if x as usize >= ARENA_SIDE_MAX || y as usize >= ARENA_SIDE_MAX {
            return None;
        }
        self.cells
            .get(pack as usize)
            .map(|arena| arena[y as usize * ARENA_SIDE_MAX + x as usize])
    }

    /// The used pack numbers' records, by number.
    pub fn pack(&self, pack: u8) -> Option<&PackLayout> {
        self.packs.get(pack as usize).filter(|p| p.is_used())
    }
}

// City (docs/CITY_DESCRIPTION.md 3): the city, its buildings, the Tavern board and its orders.
shared! {
    /// A decor object of the city (`CityData.objects`): a site sprite id on a cell.
    #[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
    pub struct CityObject {
        pub sprite: u16,
        pub x: u8,
        pub y: u8,
    }

    /// city `["city"]`: the one city every player appears in.
    #[derive(Clone, Debug, PartialEq, Eq)]
    pub struct CityData {
        pub version: u8,
        /// Adds and changes buildings (the registry `GameConfig.authority` at `init_city`).
        pub authority: Pubkey,
        pub width: u8,
        pub height: u8,
        /// Row-major site tile ids, `CITY_W_MAX` cells per row.
        pub cells: [u8; CITY_CELLS],
        /// Decor, `0..object_count` used.
        pub objects: [CityObject; CITY_OBJECTS_MAX],
        pub object_count: u8,
        /// The cell a player appears on.
        pub spawn: Cell,
        /// Buildings created so far; the next one gets this id.
        pub building_count: u16,
        pub bump: u8,
    }

    /// city `["building", id: u16]`.
    #[derive(Clone, Debug, PartialEq, Eq)]
    pub struct BuildingData {
        pub version: u8,
        pub id: u16,
        pub kind: BuildingKind,
        /// The site picture of the building, its top-left cell on the city map.
        pub prefab_id: u16,
        pub x: u8,
        pub y: u8,
        /// The cell that opens the building when clicked.
        pub door: Cell,
        /// The program the building calls: one of `GameConfig.programs`, or none when closed.
        pub program: Pubkey,
        /// The building's main account (a Tavern: its `TavernBoard`).
        pub target: Pubkey,
        /// The site interior drawn inside.
        pub interior_id: u16,
        pub status: BuildingStatus,
        pub requirement: Requirement,
        pub bump: u8,
    }

    /// city `["tavern_board", building]`: the order board of a Tavern.
    #[derive(Clone, Debug, PartialEq, Eq)]
    pub struct TavernBoardData {
        pub version: u8,
        pub building: Pubkey,
        /// Quests posted on this board, ever.
        pub posted: u32,
        /// Quests on the board now (posted and not closed).
        pub open: u16,
        pub bump: u8,
    }

    /// city `["quest", session]`: a DM's order on a Tavern board for one party. What the party
    /// is when posted; the live state (players, contributions, status) is the session's.
    #[derive(Clone, Debug, PartialEq, Eq)]
    pub struct QuestData {
        pub version: u8,
        pub board: Pubkey,
        pub session: Pubkey,
        pub map: Pubkey,
        pub dm: Pubkey,
        pub tier: u8,
        pub biome: u8,
        pub min_players: u8,
        pub price_lamports: u64,
        /// Unix time of `post_quest`.
        pub posted_at: i64,
        pub bump: u8,
    }
}

impl CityObject {
    pub const SIZE: usize = 2 + 1 + 1;
}

impl CityData {
    pub const SIZE: usize = 1
        + PUBKEY
        + 1
        + 1
        + CITY_CELLS
        + CITY_OBJECTS_MAX * CityObject::SIZE
        + 1
        + Cell::SIZE
        + 2
        + 1;
}

impl BuildingData {
    pub const SIZE: usize = 1
        + 2
        + BuildingKind::SIZE
        + 2
        + 1
        + 1
        + Cell::SIZE
        + PUBKEY * 2
        + 2
        + BuildingStatus::SIZE
        + Requirement::SIZE
        + 1;
}

impl TavernBoardData {
    pub const SIZE: usize = 1 + PUBKEY + 4 + 2 + 1;
}

impl QuestData {
    pub const SIZE: usize = 1 + PUBKEY * 4 + 1 + 1 + 1 + 8 + 8 + 1;
}
