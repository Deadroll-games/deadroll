//! Deadroll map: maps, layouts, validation and finalization.
use anchor_lang::prelude::*;

// Outgoing CPI wrappers. Not named `cpi`: `#[program]` generates a crate-level `cpi` module.
pub mod cpi_out;
pub mod errors;
pub mod events;
pub mod instructions;
pub mod state;
pub mod trust;

use deadroll_types::{MapPoint, MapStatus, Spawn};
use instructions::*;
use state::MapSettings;

declare_id!("2YeyBfQ5xkRKD2kagxRS2yZYCQdWt5qidikaKfQhLBXH");

#[program]
pub mod deadroll_map {
    use super::*;

    /// DM creates an empty draft map and its layout.
    /// Patch 11: `settings` holds the minimum players, the chest radius and the DM's time to
    /// settle.
    pub fn create_map(
        ctx: Context<CreateMap>,
        index: u32,
        width: u8,
        height: u8,
        tier: u8,
        settings: MapSettings,
    ) -> Result<()> {
        create_map::handler(ctx, index, width, height, tier, settings)
    }

    /// DM writes a chunk of cell bytes (tile type and variant) into a draft layout.
    pub fn write_layout(ctx: Context<WriteLayout>, offset: u16, cells: Vec<u8>) -> Result<()> {
        write_layout::handler(ctx, offset, cells)
    }

    /// DM replaces the spawn list of a draft layout.
    pub fn write_spawns(ctx: Context<WriteLayout>, spawns: Vec<Spawn>) -> Result<()> {
        write_spawns::handler(ctx, spawns)
    }

    /// Patch 13: DM replaces the points (entrance, exit, visible chests) and the boss reward
    /// table of a draft layout.
    pub fn write_points(
        ctx: Context<WriteLayout>,
        points: Vec<MapPoint>,
        boss_table: u8,
    ) -> Result<()> {
        write_points::handler(ctx, points, boss_table)
    }

    /// DM finalizes a draft: builder rules, layout hash, secrets root. The map is immutable after.
    pub fn finalize_map(ctx: Context<FinalizeMap>, secrets_root: [u8; 32]) -> Result<()> {
        finalize_map::handler(ctx, secrets_root)
    }

    /// Session program only: map enters or leaves a party.
    pub fn set_status(ctx: Context<SetStatus>, status: MapStatus) -> Result<()> {
        set_status::handler(ctx, status)
    }
}
