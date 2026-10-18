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

impl Bag {
    /// Death: everything in the bag burns (docs/ITEMS_AND_CRAFT.md, section 6.4). Item NFTs
    /// are not in the game yet; when they are, the items program burns the assets themselves.
    pub fn burn(&mut self) {
        self.resources = [0; RESOURCE_KINDS];
        self.items = [Pubkey::default(); BAG_SLOTS];
        self.item_count = 0;
    }
}
