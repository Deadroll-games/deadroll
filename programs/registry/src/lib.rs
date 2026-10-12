//! Deadroll registry: GameConfig, aspect and resonance tables, bestiary, tileset, prefabs.
use anchor_lang::prelude::*;

// Outgoing CPI wrappers. Not named `cpi`: `#[program]` generates a crate-level `cpi` module.
pub mod cpi_out;
pub mod errors;
pub mod events;
pub mod instructions;
pub mod state;

use deadroll_types::{PROGRAMS_COUNT, URI_BASE_LEN};
use instructions::*;
use state::TableKind;

declare_id!("dZ1jraMfS3bAUTkRw2TA1ddg8UDGzgUdFQz5pmBxjoG");

#[program]
pub mod deadroll_registry {
    use super::*;

    /// First caller becomes the game authority. `programs` is in `ProgramIdx` order.
    pub fn init_config(
        ctx: Context<InitConfig>,
        programs: [Pubkey; PROGRAMS_COUNT],
        uri_base: [u8; URI_BASE_LEN],
    ) -> Result<()> {
        init_config::handler(ctx, programs, uri_base)
    }

    /// Authority replaces the trusted program list.
    pub fn set_programs(
        ctx: Context<EditConfig>,
        programs: [Pubkey; PROGRAMS_COUNT],
    ) -> Result<()> {
        set_programs::handler(ctx, programs)
    }

    /// Authority sets the NFT metadata uri prefix.
    pub fn set_uri_base(ctx: Context<EditConfig>, uri_base: [u8; URI_BASE_LEN]) -> Result<()> {
        set_uri_base::handler(ctx, uri_base)
    }

    /// Authority points the config at a finalized `RasterizerCode` (registry pulls, no CPI).
    pub fn set_rasterizer(ctx: Context<PullBlob>) -> Result<()> {
        set_rasterizer::handler(ctx)
    }

    /// Authority points the config at a finalized `Palette`.
    pub fn set_palette(ctx: Context<PullBlob>) -> Result<()> {
        set_palette::handler(ctx)
    }

    /// Authority creates an empty table.
    pub fn init_table(ctx: Context<InitTable>, kind: TableKind) -> Result<()> {
        init_table::handler(ctx, kind)
    }

    /// Authority writes whole records into a table.
    pub fn write_table(
        ctx: Context<WriteTable>,
        kind: TableKind,
        offset: u16,
        bytes: Vec<u8>,
    ) -> Result<()> {
        write_table::handler(ctx, kind, offset, bytes)
    }
}
