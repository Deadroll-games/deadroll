use anchor_lang::prelude::*;
use deadroll_types::{seeds, BuildingStatus};

use crate::{
    errors::CityError,
    events::BuildingClosed,
    state::{Building, City},
};

#[derive(Accounts)]
pub struct CloseBuilding<'info> {
    pub authority: Signer<'info>,
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

/// Closes a building: it stays on the map with a boarded-up door and cannot be entered
/// (docs/CITY_DESCRIPTION.md 8). `update_building` opens it again.
pub fn handler(ctx: Context<CloseBuilding>) -> Result<()> {
    let building = &mut ctx.accounts.building;
    require!(
        building.core.status != BuildingStatus::Closed,
        CityError::BuildingClosed
    );
    building.core.status = BuildingStatus::Closed;
    emit!(BuildingClosed {
        building: building.key(),
        id: building.core.id,
    });
    Ok(())
}
