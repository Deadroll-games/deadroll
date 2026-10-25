use anchor_lang::prelude::*;
use deadroll_mapcheck::limits::{tier_limits, SIDE_MAX, SIDE_MIN};
use deadroll_rules::{
    loot::{chest_radius_ok, settle_timeout_ok},
    BalanceConfig,
};
use deadroll_types::{seeds, MapStatus, NO_TABLE, PARTY_MAX};

use crate::{
    errors::MapError,
    events::MapCreated,
    state::{Map, MapLayout, MapSettings, ACCOUNT_VERSION, LAYOUT_VERSION},
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

pub fn handler(
    ctx: Context<CreateMap>,
    index: u32,
    width: u8,
    height: u8,
    tier: u8,
    settings: MapSettings,
) -> Result<()> {
    let MapSettings {
        min_players,
        chest_radius,
        settle_timeout_secs,
    } = settings;
    let side_ok = |side: u8| (SIDE_MIN..=SIDE_MAX).contains(&side);
    require!(side_ok(width) && side_ok(height), MapError::SizeOutOfRange);
    require!(tier_limits(tier).is_some(), MapError::TierOutOfRange);
    require!(
        (1..=PARTY_MAX as u8).contains(&min_players),
        MapError::MinPlayersOutOfRange
    );
    let loot = &BalanceConfig::DEFAULT.loot;
    require!(
        chest_radius_ok(chest_radius, loot),
        MapError::ChestRadiusOutOfRange
    );
    // The minimum (10 minutes) is the session's to check at `open_session`: a localnet test
    // build of the session accepts seconds, the map program has no such build.
    require!(
        settle_timeout_ok(settle_timeout_secs, loot, true),
        MapError::SettleTimeoutOutOfRange
    );

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
    map.min_players = min_players;
    map.chest_radius = chest_radius;
    map.settle_timeout_secs = settle_timeout_secs;

    // Field by field: a full `MapLayoutData` literal would put 1.3 KB on the stack.
    // Cells start zeroed, which is `TileKind::Void`; the DM overwrites them with `write_layout`.
    let layout = &mut ctx.accounts.layout.core;
    layout.version = LAYOUT_VERSION;
    layout.map = map_key;
    layout.bump = ctx.bumps.layout;
    layout.boss_table = NO_TABLE;

    emit!(MapCreated {
        map: map_key,
        dm,
        index,
        width,
        height,
        tier,
        min_players,
        chest_radius,
        settle_timeout_secs,
    });
    Ok(())
}
