use anchor_lang::prelude::*;
use deadroll_types::{seeds, MapStatus, SessionStatus};

use crate::{
    cpi_out::{self, SessionSigner},
    errors::SessionError,
    events::EscrowClaimed,
    state::{Escrow, GameSession},
};

#[derive(Accounts)]
pub struct ClaimEscrow<'info> {
    /// DM main wallet only: this moves lamports (docs/GAME_COMMISSIONS.md, section 4).
    #[account(mut)]
    pub dm: Signer<'info>,
    #[account(seeds = [seeds::SESSION, session.core.map.as_ref()], bump = session.core.bump)]
    pub session: Box<Account<'info, GameSession>>,
    #[account(
        mut,
        seeds = [seeds::ESCROW, session.key().as_ref()],
        bump = escrow.bump,
    )]
    pub escrow: Box<Account<'info, Escrow>>,
    /// CHECK: the session's map, written by the map CPI.
    #[account(mut, address = session.core.map @ SessionError::InvalidMap)]
    pub map: UncheckedAccount<'info>,
    /// CHECK: signer PDA for CPI into other game programs.
    #[account(seeds = [seeds::AUTHORITY], bump)]
    pub session_authority: UncheckedAccount<'info>,
    /// CHECK: registry `GameConfig`, forwarded to the callee, which checks the trust list.
    #[account(seeds = [seeds::CONFIG], bump, seeds::program = deadroll_registry::ID)]
    pub config: UncheckedAccount<'info>,
    /// CHECK: the map program.
    #[account(address = deadroll_map::ID)]
    pub map_program: UncheckedAccount<'info>,
}

/// The DM takes the party price once the party ended, whatever the outcome (the DM ran the
/// game; docs/PARTY_AND_TURNS.md, section 3.2), and the map goes `Finished`.
///
/// TODO(P1, L6.5): only after the final audit (every declared secret revealed).
/// TODO(post-demo): `Abandoned` and unfilled parties refund the players instead.
pub fn handler(ctx: Context<ClaimEscrow>) -> Result<()> {
    let accounts = ctx.accounts;
    let session = &accounts.session;
    require_keys_eq!(session.core.dm, accounts.dm.key(), SessionError::NotDm);
    require!(
        matches!(
            session.core.status,
            SessionStatus::Victory | SessionStatus::Fled | SessionStatus::Wiped
        ),
        SessionError::NotFinished
    );
    let escrow = &mut accounts.escrow;
    require!(!escrow.claimed, SessionError::AlreadyClaimed);
    escrow.claimed = true;
    cpi_out::map::set_status(
        &SessionSigner {
            session_authority: &accounts.session_authority,
            config: &accounts.config,
            bump: ctx.bumps.session_authority,
        },
        &accounts.map,
        MapStatus::Finished,
    )?;
    // Lamports move last, after the CPI: escrow is owned by this program, the DM is a signer.
    let amount = escrow.collected;
    escrow.sub_lamports(amount)?;
    accounts.dm.add_lamports(amount)?;

    emit!(EscrowClaimed {
        session: session.key(),
        dm: accounts.dm.key(),
        amount,
    });
    Ok(())
}
