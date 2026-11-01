use anchor_lang::prelude::*;
use deadroll_types::{seeds, MapStatus};

use crate::{
    errors::MapError,
    events::LayoutGrown,
    state::{LayoutBytes, Map},
};

#[derive(Accounts)]
pub struct GrowLayout<'info> {
    #[account(mut)]
    pub dm: Signer<'info>,
    #[account(
        seeds = [seeds::MAP, map.core.dm.as_ref(), &map.core.index.to_le_bytes()],
        bump = map.core.bump,
        constraint = map.core.dm == dm.key() @ MapError::NotDm,
        constraint = map.core.status == MapStatus::Draft @ MapError::NotDraft,
    )]
    pub map: Account<'info, Map>,
    /// CHECK: the map's `MapLayout` (seeds), owner, discriminator and version checked by
    /// `LayoutBytes::load_header`; resized in place.
    #[account(mut, seeds = [seeds::LAYOUT, map.key().as_ref()], bump)]
    pub layout: UncheckedAccount<'info>,
    pub system_program: Program<'info, System>,
}

/// Patch D (docs/MAP_DESCRIPTION.md 17.6): adds up to 10 KB to a layout bigger than one
/// instruction can create, the DM paying the rent. The website sends as many as the size needs
/// right after `create_map`, in the same transaction; the writes need the full size.
pub fn handler(ctx: Context<GrowLayout>) -> Result<()> {
    let layout = LayoutBytes::load_header(&ctx.accounts.layout)?;
    let len = layout.grow(&ctx.accounts.dm, &ctx.accounts.system_program)?;
    emit!(LayoutGrown {
        map: ctx.accounts.map.key(),
        len: len as u32,
        full: layout.geom.end as u32,
    });
    Ok(())
}
