use anchor_lang::prelude::*;
use deadroll_types::{seeds, MapStatus, MAP_CELLS};

use crate::{
    errors::MapError,
    events::LayoutWritten,
    state::{Map, MapLayout},
};

/// Accounts shared by the DM's draft edits.
#[derive(Accounts)]
pub struct WriteLayout<'info> {
    pub dm: Signer<'info>,
    #[account(
        seeds = [seeds::MAP, map.core.dm.as_ref(), &map.core.index.to_le_bytes()],
        bump = map.core.bump,
        constraint = map.core.dm == dm.key() @ MapError::NotDm,
        constraint = map.core.status == MapStatus::Draft @ MapError::NotDraft,
    )]
    pub map: Account<'info, Map>,
    #[account(mut, seeds = [seeds::LAYOUT, map.key().as_ref()], bump = layout.core.bump)]
    pub layout: Box<Account<'info, MapLayout>>,
}

/// Writes cell bytes (`cell_byte`: tile type and variant) into the row-major grid (`MAP_MAX`
/// cells per row) starting at `offset`. Any byte is a cell: whether its tile type exists is up
/// to the `Tileset`, checked at `finalize_map` (patch 13).
pub fn handler(ctx: Context<WriteLayout>, offset: u16, cells: Vec<u8>) -> Result<()> {
    let start = offset as usize;
    let end = start
        .checked_add(cells.len())
        .ok_or(MapError::ChunkOutOfRange)?;
    require!(end <= MAP_CELLS, MapError::ChunkOutOfRange);
    ctx.accounts.layout.core.cells[start..end].copy_from_slice(&cells);

    emit!(LayoutWritten {
        map: ctx.accounts.map.key(),
        offset,
        len: cells.len() as u16,
    });
    Ok(())
}
