use anchor_lang::prelude::*;
use deadroll_types::{seeds, PROGRAMS_COUNT};

use super::init_config::check_programs;
use crate::{errors::RegistryError, events::ProgramsSet, state::GameConfig};

/// Accounts of every authority-only edit of `GameConfig`.
#[derive(Accounts)]
pub struct EditConfig<'info> {
    pub authority: Signer<'info>,
    #[account(
        mut,
        seeds = [seeds::CONFIG],
        bump = config.core.bump,
        constraint = config.core.authority == authority.key() @ RegistryError::Unauthorized,
    )]
    pub config: Account<'info, GameConfig>,
}

pub fn handler(ctx: Context<EditConfig>, programs: [Pubkey; PROGRAMS_COUNT]) -> Result<()> {
    check_programs(&programs)?;
    ctx.accounts.config.core.programs = programs;
    emit!(ProgramsSet { programs });
    Ok(())
}
