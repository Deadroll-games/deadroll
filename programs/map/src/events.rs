use anchor_lang::prelude::*;
use deadroll_types::MapStatus;

#[event]
pub struct MapCreated {
    pub map: Pubkey,
    pub dm: Pubkey,
    pub index: u32,
    pub width: u8,
    pub height: u8,
    pub tier: u8,
}

#[event]
pub struct LayoutWritten {
    pub map: Pubkey,
    pub offset: u16,
    pub len: u16,
}

#[event]
pub struct SpawnsWritten {
    pub map: Pubkey,
    pub spawn_count: u8,
}

#[event]
pub struct MapFinalized {
    pub map: Pubkey,
    pub dm: Pubkey,
    pub layout_hash: [u8; 32],
    pub secrets_root: [u8; 32],
}

#[event]
pub struct MapStatusChanged {
    pub map: Pubkey,
    pub from: MapStatus,
    pub to: MapStatus,
}
