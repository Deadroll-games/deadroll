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
}
