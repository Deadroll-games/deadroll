//! Deadroll items: item state, affixes, crafting.
use anchor_lang::prelude::*;

// Outgoing CPI wrappers. Not named `cpi`: `#[program]` generates a crate-level `cpi` module.
pub mod cpi_out;
pub mod errors;
pub mod instructions;
pub mod state;

declare_id!("48FtbrDegNrAUHFza9HdJzZAvUo3uUNqs1kNnZxFsqJS");

#[program]
pub mod deadroll_items {}
