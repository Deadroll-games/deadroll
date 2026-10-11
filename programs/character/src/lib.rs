//! Deadroll character: player profiles, characters, skills, bag.
use anchor_lang::prelude::*;

// Outgoing CPI wrappers. Not named `cpi`: `#[program]` generates a crate-level `cpi` module.
pub mod cpi_out;
pub mod errors;
pub mod instructions;
pub mod state;

declare_id!("9uVNvkt5kTMad65GtdqQmBb3X1JQMWwQ985qLwTCdqfL");

#[program]
pub mod deadroll_character {}
