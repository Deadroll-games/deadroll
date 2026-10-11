use anchor_lang::prelude::*;
use deadroll_mapcheck::{check_map, roster::p0_role};
use deadroll_types::{seeds, MapLayoutData, MapStatus};
use sha2::{Digest, Sha256};

use crate::{
    errors::MapError,
    events::MapFinalized,
    state::{Map, MapLayout},
};

#[derive(Accounts)]
pub struct FinalizeMap<'info> {
    pub dm: Signer<'info>,
    #[account(
        mut,
        seeds = [seeds::MAP, map.core.dm.as_ref(), &map.core.index.to_le_bytes()],
        bump = map.core.bump,
        constraint = map.core.dm == dm.key() @ MapError::NotDm,
        constraint = map.core.status == MapStatus::Draft @ MapError::NotDraft,
    )]
    pub map: Account<'info, Map>,
    #[account(seeds = [seeds::LAYOUT, map.key().as_ref()], bump = layout.core.bump)]
    pub layout: Box<Account<'info, MapLayout>>,
}

/// Runs every builder rule, then freezes the map: `layout_hash` = SHA-256 of the layout core
/// bytes (account data `[8 .. 8 + MapLayoutData::SIZE]`).
pub fn handler(ctx: Context<FinalizeMap>, secrets_root: [u8; 32]) -> Result<()> {
    let map = &mut ctx.accounts.map;
    // TODO(post-demo): roles from the registry `Bestiary` instead of the P0 roster.
    check_map(
        map.core.width,
        map.core.height,
        map.tier,
        &ctx.accounts.layout.core,
        p0_role,
    )
    .map_err(MapError::from)?;

    let layout_hash: [u8; 32] = {
        let info = ctx.accounts.layout.to_account_info();
        let data = info.try_borrow_data()?;
        Sha256::digest(&data[8..8 + MapLayoutData::SIZE]).into()
    };
    map.core.layout_hash = layout_hash;
    // TODO(post-demo): declared secret counts with the commit-reveal trap (L6.5).
    map.core.secrets_root = secrets_root;
    map.core.status = MapStatus::Finalized;

    emit!(MapFinalized {
        map: map.key(),
        dm: map.core.dm,
        layout_hash,
        secrets_root,
    });
    Ok(())
}
