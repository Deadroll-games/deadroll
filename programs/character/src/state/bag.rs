use anchor_lang::prelude::*;
use deadroll_types::{BagLayout, BAG_SLOTS};

/// `["bag", character]`: created with the character, because game moves never create accounts.
/// Patch 11: resources and consumables in slots (docs/ITEMS_AND_CRAFT.md 6.1).
#[account]
#[derive(InitSpace)]
pub struct Bag {
    pub version: u8,
    pub character: Pubkey,
    /// Item assets (lane items, P1). Slot `i < item_count` is used.
    pub items: [Pubkey; BAG_SLOTS],
    pub item_count: u8,
    pub bump: u8,
    /// Resources (recipes, blanks and reagents too), one kind per slot, and consumables. Changed
    /// only by "Done" in the bag screen (`apply_bag`) and by death.
    pub layout: BagLayout,
}

impl Bag {
    /// Death: everything in the bag burns (docs/ITEMS_AND_CRAFT.md, section 6.4). Item NFTs
    /// are not in the game yet; when they are, the items program burns the assets themselves.
    pub fn burn(&mut self) {
        self.layout = BagLayout::EMPTY;
        self.items = [Pubkey::default(); BAG_SLOTS];
        self.item_count = 0;
    }
}
