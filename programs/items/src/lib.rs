//! Deadroll items: loadouts (soulbound starter items and what is worn, by slot). Item NFTs,
//! affixes and crafting come later (docs/ITEMS_AND_CRAFT.md).
use anchor_lang::prelude::*;

pub mod auth;
// Outgoing CPI wrappers. Not named `cpi`: `#[program]` generates a crate-level `cpi` module.
pub mod cpi_out;
pub mod errors;
pub mod events;
pub mod external;
pub mod instructions;
pub mod rules;
pub mod state;

use deadroll_types::EquipSlot;
use instructions::*;

declare_id!("48FtbrDegNrAUHFza9HdJzZAvUo3uUNqs1kNnZxFsqJS");

#[program]
pub mod deadroll_items {
    use super::*;

    /// Owner: creates the character's `Loadout` with the class starter kit, each item worn in its
    /// slot. Sent in the same transaction as `character::create_character`; works later for a
    /// character created before items existed.
    pub fn init_loadout(ctx: Context<InitLoadout>) -> Result<()> {
        init_loadout::handler(ctx)
    }

    /// Owner or their session key: wears soulbound item `item_index` in `slot`. Not while the
    /// character is in a session.
    pub fn equip(ctx: Context<Equip>, item_index: u8, slot: EquipSlot) -> Result<()> {
        equip::handler(ctx, item_index, slot)
    }

    /// Owner or their session key: empties `slot`. Not while the character is in a session.
    pub fn unequip(ctx: Context<Unequip>, slot: EquipSlot) -> Result<()> {
        unequip::handler(ctx, slot)
    }
}
