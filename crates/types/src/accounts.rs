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
    Aspect, Class, EquipSlot, EquipSource, ItemBase, LayerRef, MapStatus, Pubkey, Race,
    SessionStatus, StateKind, EMPTY_ITEM, EQUIP_SLOTS, LAYERS_MAX, MAP_CELLS, MONSTERS_MAX,
    PARTY_MAX, PROGRAMS_COUNT, SOULBOUND_MAX, SPAWNS_MAX, STATES_MAX, STATS_COUNT, TURN_ORDER_MAX,
    URI_BASE_LEN,
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
    pub const SIZE: usize =
        1 + PUBKEY + MAP_CELLS + SPAWNS_MAX * Spawn::SIZE + 1 + Cell::SIZE * 2 + 1;
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
