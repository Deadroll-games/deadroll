use anchor_lang::prelude::*;
use deadroll_types::MapData;

/// `["map", dm, index: u32 LE]`.
#[account]
#[derive(InitSpace)]
pub struct Map {
    pub core: MapData,
    /// Map tier: monster levels and limits (`deadroll_mapcheck::limits`).
    pub tier: u8,
}
