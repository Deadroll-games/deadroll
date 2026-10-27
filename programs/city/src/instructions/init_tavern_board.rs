use anchor_lang::prelude::*;
use deadroll_types::{seeds, BuildingKind};

use crate::{
    errors::CityError,
    events::TavernBoardInitialized,
    state::{Building, City, TavernBoard, ACCOUNT_VERSION},
};

#[derive(Accounts)]
pub struct InitTavernBoard<'info> {
    #[account(mut)]
    pub authority: Signer<'info>,
    #[account(
        seeds = [seeds::CITY],
        bump = city.core.bump,
        constraint = city.core.authority == authority.key() @ CityError::Unauthorized,
    )]
    pub city: Box<Account<'info, City>>,
    #[account(
        seeds = [seeds::BUILDING, &building.core.id.to_le_bytes()],
        bump = building.core.bump,
        constraint = building.core.kind == BuildingKind::Tavern @ CityError::NotATavern,
    )]
    pub building: Account<'info, Building>,
    #[account(
        init,
        payer = authority,
        space = 8 + TavernBoard::INIT_SPACE,
        seeds = [seeds::TAVERN_BOARD, building.key().as_ref()],
        bump,
    )]
    pub board: Account<'info, TavernBoard>,
    pub system_program: Program<'info, System>,
}

/// The order board of a Tavern (its `Building.target`).
pub fn handler(ctx: Context<InitTavernBoard>) -> Result<()> {
    let building = ctx.accounts.building.key();
    let board = &mut ctx.accounts.board;
    board.core.version = ACCOUNT_VERSION;
    board.core.building = building;
    board.core.posted = 0;
    board.core.open = 0;
    board.core.bump = ctx.bumps.board;
    emit!(TavernBoardInitialized {
        board: board.key(),
        building,
    });
    Ok(())
}
