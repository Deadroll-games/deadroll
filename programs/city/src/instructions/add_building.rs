use anchor_lang::prelude::*;
use deadroll_types::seeds;

use crate::{
    errors::CityError,
    events::BuildingChanged,
    reads,
    state::{Building, BuildingSpec, City, ACCOUNT_VERSION},
};

#[derive(Accounts)]
pub struct AddBuilding<'info> {
    #[account(mut)]
    pub authority: Signer<'info>,
    /// CHECK: registry `GameConfig`, checked by `reads::config` (the program allowlist).
    #[account(seeds = [seeds::CONFIG], bump, seeds::program = deadroll_registry::ID)]
    pub config: UncheckedAccount<'info>,
    #[account(
        mut,
        seeds = [seeds::CITY],
        bump = city.core.bump,
        constraint = city.core.authority == authority.key() @ CityError::Unauthorized,
    )]
    pub city: Box<Account<'info, City>>,
    #[account(
        init,
        payer = authority,
        space = 8 + Building::INIT_SPACE,
        seeds = [seeds::BUILDING, &city.core.building_count.to_le_bytes()],
        bump,
    )]
    pub building: Account<'info, Building>,
    pub system_program: Program<'info, System>,
}

/// A new building, id `City.building_count`.
pub fn handler(ctx: Context<AddBuilding>, spec: BuildingSpec) -> Result<()> {
    let config = reads::config(&ctx.accounts.config)?;
    let key = ctx.accounts.building.key();
    let city = &mut ctx.accounts.city;
    spec.check(&city.core, &reads::allowlist(&config), &key)?;
    let id = city.core.building_count;
    city.core.building_count = id.checked_add(1).ok_or(CityError::SizeOutOfRange)?;
    ctx.accounts.building.core = spec.data(id, ACCOUNT_VERSION, ctx.bumps.building);
    emit!(BuildingChanged {
        building: key,
        id,
        kind: spec.kind,
        program: spec.program,
        target: spec.target,
        status: spec.status,
        added: true,
    });
    Ok(())
}
