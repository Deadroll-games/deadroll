use anchor_lang::prelude::*;
use deadroll_types::{seeds, SessionStatus};

use crate::{
    errors::SessionError,
    events::PlayerReady,
    state::{GameSession, Participation},
};

#[derive(Accounts)]
pub struct SetReady<'info> {
    /// Main wallet only: readiness is not a game move (docs/PARTY_AND_TURNS.md, section 2.1).
    pub player: Signer<'info>,
    #[account(
        mut,
        seeds = [seeds::SESSION, session.core.map.as_ref()],
        bump = session.core.bump,
    )]
    pub session: Box<Account<'info, GameSession>>,
    #[account(
        mut,
        seeds = [
            seeds::PARTICIPATION,
            session.key().as_ref(),
            participation.core.character.as_ref(),
        ],
        bump = participation.core.bump,
    )]
    pub participation: Box<Account<'info, Participation>>,
}

pub fn handler(ctx: Context<SetReady>, ready: bool) -> Result<()> {
    let session = &mut ctx.accounts.session;
    let participation = &mut ctx.accounts.participation;
    require_keys_eq!(
        participation.player,
        ctx.accounts.player.key(),
        SessionError::NotPlayer
    );
    require!(
        session.core.status == SessionStatus::Lobby,
        SessionError::NotInLobby
    );

    let slot = participation.core.slot;
    participation.core.ready = ready;
    if ready {
        session.ready_mask |= 1 << slot;
    } else {
        session.ready_mask &= !(1 << slot);
    }

    emit!(PlayerReady {
        session: session.key(),
        slot,
        ready,
    });
    Ok(())
}
