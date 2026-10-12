use anchor_lang::prelude::*;

/// `["escrow", session]`: holds the party price contributions as lamports above rent.
#[account]
#[derive(InitSpace)]
pub struct Escrow {
    pub version: u8,
    pub session: Pubkey,
    /// Contributions received so far.
    pub collected: u64,
    pub bump: u8,
}
