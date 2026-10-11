//! Deadroll map: maps, layouts, validation and finalization.
use anchor_lang::prelude::*;

// Outgoing CPI wrappers. Not named `cpi`: `#[program]` generates a crate-level `cpi` module.
pub mod cpi_out;
pub mod errors;
pub mod instructions;
pub mod state;

declare_id!("2YeyBfQ5xkRKD2kagxRS2yZYCQdWt5qidikaKfQhLBXH");

#[program]
pub mod deadroll_map {}
