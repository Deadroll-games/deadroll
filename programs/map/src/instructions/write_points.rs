use anchor_lang::prelude::*;
use deadroll_types::MapPoint;

use crate::{events::PointsWritten, instructions::WriteLayout, state::LayoutBytes};

/// Patch 13: replaces the points (the entrance, the exit and the visible chests, each chest with
/// its reward table) and the boss reward table (docs/MAP_DESCRIPTION.md 7.1,
/// docs/PARTY_AND_TURNS.md 9.2). Placement, tables and the budget are checked at `finalize_map`,
/// so the builder can save a half-done map. Replaces `write_chests` (patch 11).
pub fn handler(ctx: Context<WriteLayout>, points: Vec<MapPoint>, boss_table: u8) -> Result<()> {
    let layout = LayoutBytes::load(&ctx.accounts.layout)?;
    layout.write_points(&points, boss_table)?;
    emit!(PointsWritten {
        map: ctx.accounts.map.key(),
        point_count: layout.point_count()?,
        boss_table,
    });
    Ok(())
}
