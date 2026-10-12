use anchor_lang::prelude::*;
use deadroll_mapcheck::limits::{tier_limits, SIDE_MAX, SIDE_MIN};
use deadroll_types::{seeds, MapStatus};

use crate::{
    errors::MapError,
    events::MapCreated,
    state::{Map, MapLayout, ACCOUNT_VERSION},
};

#[derive(Accounts)]
#[instruction(index: u32)]
pub struct CreateMap<'info> {
    #[account(mut)]
    pub dm: Signer<'info>,
    #[account(
        init,
        payer = dm,
        space = 8 + Map::INIT_SPACE,
        seeds = [seeds::MAP, dm.key().as_ref(), &index.to_le_bytes()],
        bump,
    )]
    pub map: Account<'info, Map>,
    #[account(
        init,
        payer = dm,
        space = 8 + MapLayout::INIT_SPACE,
        seeds = [seeds::LAYOUT, map.key().as_ref()],
        bump,
    )]
    pub layout: Box<Account<'info, MapLayout>>,
    pub system_program: Program<'info, System>,
}

pub fn handler(ctx: Context<CreateMap>, index: u32, width: u8, height: u8, tier: u8) -> Result<()> {
    let side_ok = |side: u8| (SIDE_MIN..=SIDE_MAX).contains(&side);
    require!(side_ok(width) && side_ok(height), MapError::SizeOutOfRange);
    require!(tier_limits(tier).is_some(), MapError::TierOutOfRange);

    let map_key = ctx.accounts.map.key();
    let dm = ctx.accounts.dm.key();
    let map = &mut ctx.accounts.map;
    map.core.version = ACCOUNT_VERSION;
    map.core.dm = dm;
    map.core.index = index;
    map.core.width = width;
    map.core.height = height;
    map.core.status = MapStatus::Draft;
    map.core.bump = ctx.bumps.map;
    map.tier = tier;

    // Field by field: a full `MapLayoutData` literal would put 1.3 KB on the stack.
    // Tiles start zeroed, which is `Tile::Floor`; the DM overwrites them with `write_layout`.
    let layout = &mut ctx.accounts.layout.core;
    layout.version = ACCOUNT_VERSION;
    layout.map = map_key;
    layout.bump = ctx.bumps.layout;

    emit!(MapCreated {
        map: map_key,
        dm,
        index,
        width,
        height,
        tier
    });
    Ok(())
}
