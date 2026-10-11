use anchor_lang::prelude::*;
use deadroll_types::{seeds, MapStatus, ProgramIdx};

use crate::{errors::MapError, events::MapStatusChanged, state::Map, trust};

/// CPI target for `session` (CONTRACTS.md, section 5). Account order is part of the contract:
/// `[map (mut), session_authority (signer), config]`.
#[derive(Accounts)]
pub struct SetStatus<'info> {
    #[account(
        mut,
        seeds = [seeds::MAP, map.core.dm.as_ref(), &map.core.index.to_le_bytes()],
        bump = map.core.bump,
    )]
    pub map: Account<'info, Map>,
    /// The session program's `["authority"]` PDA, checked against `GameConfig` in the handler.
    pub session_authority: Signer<'info>,
    /// CHECK: registry `GameConfig`; owner and discriminator checked in `trust::check_caller`.
    #[account(seeds = [seeds::CONFIG], bump, seeds::program = deadroll_registry::ID)]
    pub config: UncheckedAccount<'info>,
}

/// `Finalized -> InSession` when a party opens, `InSession -> Finished` when it ends.
pub fn handler(ctx: Context<SetStatus>, status: MapStatus) -> Result<()> {
    trust::check_caller(
        &ctx.accounts.config,
        ctx.accounts.session_authority.key,
        ProgramIdx::Session,
    )?;
    let map = &mut ctx.accounts.map;
    let from = map.core.status;
    let allowed = matches!(
        (from, status),
        (MapStatus::Finalized, MapStatus::InSession) | (MapStatus::InSession, MapStatus::Finished)
    );
    require!(allowed, MapError::InvalidStatusTransition);
    map.core.status = status;

    emit!(MapStatusChanged {
        map: map.key(),
        from,
        to: status
    });
    Ok(())
}
