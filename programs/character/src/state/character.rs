use anchor_lang::prelude::*;
use deadroll_types::CharacterData;

use super::Gear;

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
    /// Soulbound starter items and what is worn, by slot (docs/CHARACTER.md, section 10).
    /// Changed only outside a session (`equip`, `unequip`); burned at death.
    pub gear: Gear,
    /// Class trait tier, `1..=14` (1 best), next to the race trait tier `core.predisposition_tier`
    /// (docs/CHARACTER.md, sections 6.3 and 10). Starts low, grows only deterministically.
    pub class_tier: u8,
}
