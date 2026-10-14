//! Deadroll items: item NFTs, their state, affixes, artisan listings and crafting
//! (docs/ITEMS_AND_CRAFT.md), none of which is in the game yet. The class starter items and what
//! a character wears live in the character's own account (`deadroll_character`, `Character.gear`).
use anchor_lang::prelude::*;

// Outgoing CPI wrappers. Not named `cpi`: `#[program]` generates a crate-level `cpi` module.
pub mod cpi_out;
pub mod errors;

declare_id!("48FtbrDegNrAUHFza9HdJzZAvUo3uUNqs1kNnZxFsqJS");

#[program]
pub mod deadroll_items {}
