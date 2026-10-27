use anchor_lang::prelude::*;
use deadroll_types::{
    seeds, Cell, CityObject, CITY_CELLS, CITY_H_MAX, CITY_OBJECTS_MAX, CITY_W_MAX,
};

use crate::{
    errors::CityError,
    events::CityInitialized,
    reads,
    state::{City, ACCOUNT_VERSION},
};

#[derive(Accounts)]
pub struct InitCity<'info> {
    /// The registry `GameConfig.authority`; it becomes the city's authority.
    #[account(mut)]
    pub authority: Signer<'info>,
    /// CHECK: registry `GameConfig`, checked by `reads::config`.
    #[account(seeds = [seeds::CONFIG], bump, seeds::program = deadroll_registry::ID)]
    pub config: UncheckedAccount<'info>,
    #[account(
        init,
        payer = authority,
        space = 8 + City::INIT_SPACE,
        seeds = [seeds::CITY],
        bump,
    )]
    pub city: Box<Account<'info, City>>,
    pub system_program: Program<'info, System>,
}

pub fn handler(ctx: Context<InitCity>, width: u8, height: u8, spawn: Cell) -> Result<()> {
    let config = reads::config(&ctx.accounts.config)?;
    let authority = ctx.accounts.authority.key();
    require_keys_eq!(authority, config.authority, CityError::Unauthorized);
    require!(
        (1..=CITY_W_MAX as u8).contains(&width) && (1..=CITY_H_MAX as u8).contains(&height),
        CityError::SizeOutOfRange
    );
    let city = &mut ctx.accounts.city;
    city.core.version = ACCOUNT_VERSION;
    city.core.authority = authority;
    city.core.width = width;
    city.core.height = height;
    city.core.cells = [0; CITY_CELLS];
    city.core.objects = [CityObject::default(); CITY_OBJECTS_MAX];
    city.core.object_count = 0;
    require!(City::contains(&city.core, spawn), CityError::OutOfCity);
    city.core.spawn = spawn;
    city.core.building_count = 0;
    city.core.bump = ctx.bumps.city;
    emit!(CityInitialized {
        city: city.key(),
        authority,
        width,
        height,
    });
    Ok(())
}
