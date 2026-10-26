use anchor_lang::prelude::*;
use deadroll_types::seeds;

use crate::{
    errors::CityError,
    events::BuildingChanged,
    reads,
    state::{Building, BuildingSpec, City},
};

#[derive(Accounts)]
pub struct UpdateBuilding<'info> {
    pub authority: Signer<'info>,
    /// CHECK: registry `GameConfig`, checked by `reads::config` (the program allowlist).
    #[account(seeds = [seeds::CONFIG], bump, seeds::program = deadroll_registry::ID)]
    pub config: UncheckedAccount<'info>,
    #[account(
        seeds = [seeds::CITY],
        bump = city.core.bump,
        constraint = city.core.authority == authority.key() @ CityError::Unauthorized,
    )]
    pub city: Box<Account<'info, City>>,
    #[account(
        mut,
        seeds = [seeds::BUILDING, &building.core.id.to_le_bytes()],
        bump = building.core.bump,
    )]
    pub building: Account<'info, Building>,
}

/// Changes every field of a building but its id.
pub fn handler(ctx: Context<UpdateBuilding>, spec: BuildingSpec) -> Result<()> {
    let config = reads::config(&ctx.accounts.config)?;
    let key = ctx.accounts.building.key();
    spec.check(&ctx.accounts.city.core, &reads::allowlist(&config), &key)?;
    let building = &mut ctx.accounts.building;
    let id = building.core.id;
    building.core = spec.data(id, building.core.version, building.core.bump);
    emit!(BuildingChanged {
        building: key,
        id,
        kind: spec.kind,
        program: spec.program,
        target: spec.target,
        status: spec.status,
        added: false,
    });
    Ok(())
}
