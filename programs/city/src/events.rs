use anchor_lang::prelude::*;
use deadroll_types::{BuildingKind, BuildingStatus};

#[event]
pub struct CityInitialized {
    pub city: Pubkey,
    pub authority: Pubkey,
    pub width: u8,
    pub height: u8,
}

#[event]
pub struct CityWritten {
    pub offset: u16,
    pub len: u16,
}

#[event]
pub struct CityObjectsSet {
    pub object_count: u8,
    pub spawn_x: u8,
    pub spawn_y: u8,
}

/// `add_building` and `update_building`.
#[event]
pub struct BuildingChanged {
    pub building: Pubkey,
    pub id: u16,
    pub kind: BuildingKind,
    pub program: Pubkey,
    pub target: Pubkey,
    pub status: BuildingStatus,
    /// `add_building`: true.
    pub added: bool,
}

#[event]
pub struct BuildingClosed {
    pub building: Pubkey,
    pub id: u16,
}

#[event]
pub struct TavernBoardInitialized {
    pub board: Pubkey,
    pub building: Pubkey,
}

#[event]
pub struct QuestPosted {
    pub quest: Pubkey,
    pub board: Pubkey,
    pub session: Pubkey,
    pub map: Pubkey,
    pub dm: Pubkey,
    pub tier: u8,
    pub price_lamports: u64,
}

#[event]
pub struct QuestClosed {
    pub quest: Pubkey,
    pub session: Pubkey,
    pub dm: Pubkey,
}
