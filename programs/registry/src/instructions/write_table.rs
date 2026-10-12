use anchor_lang::prelude::*;
use deadroll_types::seeds;

use crate::{
    errors::RegistryError,
    events::TableWritten,
    state::{
        records::{check_range, validate_write, RangeError},
        GameConfig, Table, TableKind,
    },
};

#[derive(Accounts)]
#[instruction(kind: TableKind)]
pub struct WriteTable<'info> {
    pub authority: Signer<'info>,
    #[account(
        seeds = [seeds::CONFIG],
        bump = config.core.bump,
        constraint = config.core.authority == authority.key() @ RegistryError::Unauthorized,
    )]
    pub config: Account<'info, GameConfig>,
    #[account(
        mut,
        seeds = [kind.seed()],
        bump = table.load()?.bump,
    )]
    pub table: AccountLoader<'info, Table>,
}

/// Writes whole records at `offset`. Typed tables take aligned, valid records only; the prefab
/// book is opaque. Tables stay editable: new monsters need no program update.
pub fn handler(
    ctx: Context<WriteTable>,
    kind: TableKind,
    offset: u16,
    bytes: Vec<u8>,
) -> Result<()> {
    let start = offset as usize;
    let end = check_range(kind, start, bytes.len()).map_err(|e| match e {
        RangeError::OutOfBounds => error!(RegistryError::TableRangeOutOfBounds),
        RangeError::Misaligned => error!(RegistryError::TableMisaligned),
    })?;
    require!(
        validate_write(kind, start, &bytes),
        RegistryError::InvalidRecord
    );

    let mut table = ctx.accounts.table.load_mut()?;
    table.body[start..end].copy_from_slice(&bytes);
    table.len = table.len.max(end as u32);
    emit!(TableWritten {
        kind,
        offset,
        len: bytes.len() as u16,
    });
    Ok(())
}
