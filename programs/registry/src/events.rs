use anchor_lang::prelude::*;

use crate::state::TableKind;

#[event]
pub struct ConfigInitialized {
    pub authority: Pubkey,
}

#[event]
pub struct ProgramsSet {
    pub programs: [Pubkey; 7],
}

#[event]
pub struct UriBaseSet {
    pub uri_base: [u8; 64],
}

#[event]
pub struct RasterizerSet {
    pub rasterizer: Pubkey,
    pub version: u16,
    pub sha256: [u8; 32],
}

#[event]
pub struct PaletteSet {
    pub palette: Pubkey,
}

/// `GameConfig.sprite_books` replaced (empty slots are `Pubkey::default()`).
#[event]
pub struct SpriteBooksSet {
    pub sprite_books: [Pubkey; 4],
}

/// A pre-patch-7 `GameConfig` grew to the current size.
#[event]
pub struct ConfigResized {
    pub old_len: u32,
    pub new_len: u32,
}

#[event]
pub struct TableCreated {
    pub kind: TableKind,
    pub table: Pubkey,
}

#[event]
pub struct TableWritten {
    pub kind: TableKind,
    pub offset: u16,
    pub len: u16,
}
