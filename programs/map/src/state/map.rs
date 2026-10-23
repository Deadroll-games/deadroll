use anchor_lang::prelude::*;
use deadroll_types::MapData;

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
}
