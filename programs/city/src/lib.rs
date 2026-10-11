//! Deadroll city: city, buildings, storage.
use anchor_lang::prelude::*;

// Outgoing CPI wrappers. Not named `cpi`: `#[program]` generates a crate-level `cpi` module.
pub mod cpi_out;
pub mod errors;
pub mod instructions;
pub mod state;

declare_id!("2Cr5vB5awqDVx6XRcxtFFYQEG6Zy3aSAcKcqzh1xbVjS");

#[program]
pub mod deadroll_city {}
