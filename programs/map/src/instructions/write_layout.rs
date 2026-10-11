use anchor_lang::prelude::*;
use deadroll_types::{seeds, MapStatus, Tile, MAP_CELLS};

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

/// Writes `tiles` into the row-major grid (`MAP_MAX` cells per row) starting at `offset`.
pub fn handler(ctx: Context<WriteLayout>, offset: u16, tiles: Vec<u8>) -> Result<()> {
    let start = offset as usize;
    let end = start
        .checked_add(tiles.len())
        .ok_or(MapError::ChunkOutOfRange)?;
    require!(end <= MAP_CELLS, MapError::ChunkOutOfRange);
    require!(
        tiles.iter().all(|&b| Tile::try_from(b).is_ok()),
        MapError::InvalidTile
    );
    ctx.accounts.layout.core.tiles[start..end].copy_from_slice(&tiles);

    emit!(LayoutWritten {
        map: ctx.accounts.map.key(),
        offset,
        len: tiles.len() as u16,
    });
    Ok(())
}
