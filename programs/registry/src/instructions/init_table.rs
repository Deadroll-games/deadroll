use anchor_lang::prelude::*;
use deadroll_types::seeds;

use crate::{
    errors::RegistryError,
    events::TableCreated,
    state::{GameConfig, Table, TableKind, ACCOUNT_VERSION},
};

#[derive(Accounts)]
#[instruction(kind: TableKind)]
pub struct InitTable<'info> {
    #[account(mut)]
    pub authority: Signer<'info>,
    #[account(
        seeds = [seeds::CONFIG],
        bump = config.core.bump,
        constraint = config.core.authority == authority.key() @ RegistryError::Unauthorized,
    )]
    pub config: Account<'info, GameConfig>,
    #[account(
        init,
        payer = authority,
        space = 8 + core::mem::size_of::<Table>(),
        seeds = [kind.seed()],
        bump,
    )]
    pub table: AccountLoader<'info, Table>,
    pub system_program: Program<'info, System>,
}

/// Creates an empty table of `kind` (all zero, `len == 0`).
pub fn handler(ctx: Context<InitTable>, kind: TableKind) -> Result<()> {
    let mut table = ctx.accounts.table.load_init()?;
    table.version = ACCOUNT_VERSION;
    table.kind = kind as u8;
    table.bump = ctx.bumps.table;
    emit!(TableCreated {
        kind,
        table: ctx.accounts.table.key()
    });
    Ok(())
}
