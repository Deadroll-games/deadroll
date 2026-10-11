use anchor_lang::prelude::*;

/// A blob account was created (`init_*`).
#[event]
pub struct BlobInitialized {
    pub address: Pubkey,
    pub kind: u8,
    pub id: u16,
    pub len: u32,
}

#[event]
pub struct RasterizerFinalized {
    pub address: Pubkey,
    pub version: u16,
    pub len: u32,
    pub sha256: [u8; 32],
}

#[event]
pub struct PaletteFinalized {
    pub address: Pubkey,
    pub version: u16,
    pub sha256: [u8; 32],
}

#[event]
pub struct SpriteBookFinalized {
    pub address: Pubkey,
    pub book_id: u16,
    pub sha256: [u8; 32],
}
