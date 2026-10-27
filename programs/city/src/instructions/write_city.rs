use anchor_lang::prelude::*;
use deadroll_types::{seeds, CITY_CELLS};

use crate::{errors::CityError, events::CityWritten, state::City};

#[derive(Accounts)]
pub struct WriteCity<'info> {
    pub authority: Signer<'info>,
    #[account(
        mut,
        seeds = [seeds::CITY],
        bump = city.core.bump,
        constraint = city.core.authority == authority.key() @ CityError::Unauthorized,
    )]
    pub city: Box<Account<'info, City>>,
}

/// Writes `cells` (site tile ids, `CITY_W_MAX` per row) from `offset`.
pub fn handler(ctx: Context<WriteCity>, offset: u16, cells: Vec<u8>) -> Result<()> {
    let start = offset as usize;
    let end = start
        .checked_add(cells.len())
        .filter(|end| *end <= CITY_CELLS)
        .ok_or(CityError::OutOfCity)?;
    ctx.accounts.city.core.cells[start..end].copy_from_slice(&cells);
    emit!(CityWritten {
        offset,
        len: cells.len() as u16,
    });
    Ok(())
}
