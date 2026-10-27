use anchor_lang::prelude::*;
use deadroll_types::MapData;

/// Patch 11: what a DM sets for a map at `create_map` besides its size and tier.
#[derive(AnchorSerialize, AnchorDeserialize, Clone, Copy, Debug, PartialEq, Eq)]
pub struct MapSettings {
    /// The party size the reward budget is checked for; the party does not start with fewer.
    pub min_players: u8,
    /// No living monster within this many cells (Chebyshev) of a chest or of any living player
    /// when the chest opens (`LootTuning.chest_radius_min..=max`).
    pub chest_radius: u8,
    /// Seconds the DM has after a party ends to issue the rewards (`settle_party`); then each
    /// player claims their own (`LootTuning.settle_timeout_*`).
    pub settle_timeout_secs: u32,
    /// The art set of the map (`deadroll_types::Biome`); a value past `Biome::COUNT` is rejected.
    pub biome: u8,
}

/// `["map", dm, index: u32 LE]`.
#[account]
#[derive(InitSpace)]
pub struct Map {
    pub core: MapData,
    /// Map tier: monster levels and limits (`deadroll_mapcheck::limits`).
    pub tier: u8,
    /// Patch 11: the party size the reward budget is checked for (docs/PARTY_AND_TURNS.md 3.1,
    /// 9.2): the party does not start with fewer players.
    pub min_players: u8,
    /// Patch 11, set by the DM: no living monster within this many cells (Chebyshev) of a chest
    /// or of any living player when the chest opens.
    pub chest_radius: u8,
    /// Patch 11, set by the DM: seconds the DM has after a party ends to issue the rewards
    /// (`settle_party`); then each player claims their own.
    pub settle_timeout_secs: u32,
    /// The art set of the map (`deadroll_types::Biome`), set by the DM at `create_map`. Picture
    /// only. A map from before the field (`ACCOUNT_VERSION` 1) is one byte shorter and reads as
    /// `Caves` through `Map::decode_padded`.
    pub biome: u8,
}

impl Map {
    /// Data size of a map from before `biome` (`ACCOUNT_VERSION` 1), discriminator excluded.
    pub const V1_SPACE: usize = Self::INIT_SPACE - 1;
    /// Offset of `core.status` in the account data (discriminator included): version, dm, index,
    /// width, height. `MapData` is a frozen core, so this never moves.
    pub const STATUS_OFFSET: usize = 8 + 1 + 32 + 4 + 1 + 1;

    /// A `Map` from its account data (discriminator included, checked by the caller). A map from
    /// before `biome` is one byte shorter: the missing byte reads as zero, `Biome::Caves`.
    /// `Account<Map>` cannot read such a map; `set_status` and the session read maps this way.
    pub fn decode_padded(data: &[u8]) -> Option<Map> {
        let body = data.get(8..)?;
        if body.len() < Self::V1_SPACE {
            return None;
        }
        let mut buf = [0u8; Self::INIT_SPACE];
        let len = body.len().min(Self::INIT_SPACE);
        buf[..len].copy_from_slice(&body[..len]);
        Map::deserialize(&mut &buf[..]).ok()
    }
}
