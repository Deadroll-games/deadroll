//! Deadroll render: on-chain rasterizer code, palettes and sprite books.
//!
//! Each blob is created by `init_*`, grown by `extend`, filled by `write_chunk` and frozen by
//! `finalize_*` (SHA-256 checked). `finalize_rasterizer` also points `GameConfig` at the new
//! rasterizer (CPI into the registry); palettes and sprite books are pulled by the registry.
use anchor_lang::prelude::*;

pub mod config;
// Outgoing CPI wrappers. Not named `cpi`: `#[program]` generates a crate-level `cpi` module.
pub mod cpi_out;
pub mod errors;
pub mod events;
pub mod instructions;
pub mod state;

use instructions::*;

declare_id!("DgQCsXAe7Q7h8zdtToiTvk8XqNjjbXuD6xith2BmEiRB");

#[program]
pub mod deadroll_render_program {
    use super::*;

    pub fn init_rasterizer(
        ctx: Context<InitRasterizer>,
        version: u16,
        len: u32,
        sha256: [u8; 32],
    ) -> Result<()> {
        init_rasterizer::handler(ctx, version, len, sha256)
    }

    pub fn init_palette(
        ctx: Context<InitPalette>,
        version: u16,
        len: u32,
        sha256: [u8; 32],
    ) -> Result<()> {
        init_palette::handler(ctx, version, len, sha256)
    }

    pub fn init_sprite_book(
        ctx: Context<InitSpriteBook>,
        book_id: u16,
        len: u32,
        sha256: [u8; 32],
    ) -> Result<()> {
        init_sprite_book::handler(ctx, book_id, len, sha256)
    }

    pub fn extend(ctx: Context<Extend>) -> Result<()> {
        extend::handler(ctx)
    }

    pub fn write_chunk(ctx: Context<WriteChunk>, offset: u32, bytes: Vec<u8>) -> Result<()> {
        write_chunk::handler(ctx, offset, bytes)
    }

    pub fn finalize_rasterizer(ctx: Context<FinalizeRasterizer>) -> Result<()> {
        finalize_rasterizer::handler(ctx)
    }

    pub fn finalize_palette(ctx: Context<FinalizePalette>) -> Result<()> {
        finalize_palette::handler(ctx)
    }

    pub fn finalize_sprite_book(ctx: Context<FinalizeSpriteBook>) -> Result<()> {
        finalize_sprite_book::handler(ctx)
    }
}
