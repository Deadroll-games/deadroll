use anchor_lang::prelude::*;

/// `["profile", owner]`: one per wallet, hands out character indexes.
#[account]
#[derive(InitSpace)]
pub struct PlayerProfile {
    pub version: u8,
    pub owner: Pubkey,
    /// Characters created so far; the next character gets this index.
    pub character_count: u8,
    pub bump: u8,
}
