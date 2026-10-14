use anchor_lang::prelude::*;
use deadroll_types::{Class, EquipSlot, ItemBase};

#[event]
pub struct LoadoutCreated {
    pub character: Pubkey,
    pub owner: Pubkey,
    pub class: Class,
    pub item_count: u8,
}

#[event]
pub struct ItemEquipped {
    pub character: Pubkey,
    pub slot: EquipSlot,
    pub item_index: u8,
    pub base: ItemBase,
    /// What the slot held before, if anything.
    pub replaced: Option<ItemBase>,
}

#[event]
pub struct ItemUnequipped {
    pub character: Pubkey,
    pub slot: EquipSlot,
    pub base: Option<ItemBase>,
}
