use anchor_lang::prelude::*;
use deadroll_types::{MapPoint, CHESTS_MAX};

use crate::{errors::MapError, events::ChestsWritten, instructions::WriteLayout};

/// Patch 11: replaces the visible chests (each with its reward table) and the boss reward table
/// (docs/MAP_DESCRIPTION.md 7.1, docs/PARTY_AND_TURNS.md 9.2). Placement, tables and the budget
/// are checked at `finalize_map`, so the builder can save a half-done map.
pub fn handler(ctx: Context<WriteLayout>, chests: Vec<MapPoint>, boss_table: u8) -> Result<()> {
    require!(chests.len() <= CHESTS_MAX, MapError::TooManyChests);
    let layout = &mut ctx.accounts.layout.core;
    layout.chests = [MapPoint::default(); CHESTS_MAX];
    layout.chests[..chests.len()].copy_from_slice(&chests);
    layout.chest_count = chests.len() as u8;
    layout.boss_table = boss_table;
    emit!(ChestsWritten {
        map: ctx.accounts.map.key(),
        chest_count: layout.chest_count,
        boss_table,
    });
    Ok(())
}
