use anchor_lang::prelude::*;
use deadroll_types::CharacterData;

/// Bytes of a character name, zero-padded printable ASCII.
pub const NAME_LEN: usize = 32;

/// `["character", owner, index]`. The frozen core comes first (CONTRACTS.md, section 4.1), so
/// other programs read it as `CharacterData` right after the discriminator.
#[account]
#[derive(InitSpace)]
pub struct Character {
    pub core: CharacterData,
    pub name: [u8; NAME_LEN],
    /// Stat points from level-ups, not spent yet.
    pub unspent_points: u8,
    /// The Core NFT got its permanent freeze after death (`freeze_tombstone`).
    pub nft_frozen: bool,
}
