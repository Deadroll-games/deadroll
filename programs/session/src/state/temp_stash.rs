use anchor_lang::prelude::*;

/// Resource kinds kept in a temporary cache (P0 placeholder size).
pub const STASH_RESOURCES: usize = 8;

/// `["stash", session, character]`: experience and rewards that become the character's only on
/// victory or peaceful exit (docs/PARTY_AND_TURNS.md, section 8).
#[account]
#[derive(InitSpace)]
pub struct TempStash {
    pub version: u8,
    pub session: Pubkey,
    pub character: Pubkey,
    pub xp: u32,
    pub resources: [u32; STASH_RESOURCES],
    pub bump: u8,
}
