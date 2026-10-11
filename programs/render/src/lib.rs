//! Deadroll render: on-chain rasterizer code, palettes and sprite books.
use anchor_lang::prelude::*;

// Outgoing CPI wrappers. Not named `cpi`: `#[program]` generates a crate-level `cpi` module.
pub mod cpi_out;
pub mod errors;
pub mod instructions;
pub mod state;

declare_id!("DgQCsXAe7Q7h8zdtToiTvk8XqNjjbXuD6xith2BmEiRB");

#[program]
pub mod deadroll_render_program {}
