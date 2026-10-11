use anchor_lang::prelude::*;
use deadroll_types::BAG_SLOTS;

/// Stackable resource kinds (wood, iron, leather, stone, ...), ids from the registry. BALANCE.
pub const RESOURCE_KINDS: usize = 8;

/// `["bag", character]`: created with the character, because game moves never create accounts.
#[account]
#[derive(InitSpace)]
pub struct Bag {
    pub version: u8,
    pub character: Pubkey,
    /// Amount per resource kind.
    pub resources: [u32; RESOURCE_KINDS],
    /// Item assets (lane items, P1). Slot `i < item_count` is used.
    pub items: [Pubkey; BAG_SLOTS],
    pub item_count: u8,
    pub bump: u8,
}
