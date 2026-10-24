use anchor_lang::prelude::*;
use deadroll_types::{seeds, SessionStatus};
use session_keys::{session_auth_or, Session, SessionTokenV2};

use crate::{
    auth,
    errors::SessionError,
    events::FleeVoted,
    outcomes,
    rules::Unit,
    state::{GameSession, Monsters, Participation},
    turns,
};

#[derive(Accounts, Session)]
pub struct Flee<'info> {
    /// Main wallet or the participation's session key.
    pub signer: Signer<'info>,
    #[account(
        mut,
        seeds = [seeds::SESSION, session.core.map.as_ref()],
        bump = session.core.bump,
    )]
    pub session: Box<Account<'info, GameSession>>,
    /// CHECK: `MonsterState` of this session, checked by `Monsters::load` (written in place).
    #[account(mut)]
    pub monsters: UncheckedAccount<'info>,
    #[account(
        seeds = [
            seeds::PARTICIPATION,
            session.key().as_ref(),
            participation.core.character.as_ref(),
        ],
        bump = participation.core.bump,
    )]
    pub participation: Box<Account<'info, Participation>>,
    /// MagicBlock `SessionTokenV2` of the session key; none when the main wallet signs.
    #[session(signer = signer, authority = participation.player)]
    pub session_token: Option<Account<'info, SessionTokenV2>>,
}

/// "Escape" (docs/PARTY_AND_TURNS.md, section 7.3): sets this player's intent and spends the
/// whole turn. When every living player has the intent, the party flees: no XP, no rewards.
#[session_auth_or(
    ctx.accounts.signer.key() == ctx.accounts.participation.player,
    SessionError::Unauthorized
)]
pub fn handler(ctx: Context<Flee>) -> Result<()> {
    let accounts = ctx.accounts;
    let session = &mut accounts.session;
    let participation = &accounts.participation;
    auth::check_bound(
        accounts.signer.key,
        &participation.player,
        &participation.core.session_key,
    )?;
    require!(
        session.core.status == SessionStatus::Active,
        SessionError::NotActive
    );
    require!(!session.core.pending_vrf, SessionError::RollPending);
    // Patch 6: a player down in the last callback is finalized first (`start_turn`).
    require!(session.pending_deaths == 0, SessionError::DeathsPending);
    let slot = participation.core.slot;
    require!(
        session.current_unit() == Some(Unit::Player(slot)),
        SessionError::NotYourTurn
    );
    require!(session.turn_started(), SessionError::TurnNotStarted);
    require!(session.player_alive(slot), SessionError::UnitDead);

    let session_key = session.key();
    session.flee_mask |= 1 << slot;
    emit!(FleeVoted {
        session: session_key,
        slot,
        flee_mask: session.flee_mask,
        alive_mask: session.alive_mask,
    });

    if turns::everyone_flees(session.flee_mask, session.alive_mask) {
        session.core.status = SessionStatus::Fled;
        outcomes::emit_if_ended(session, session_key)?;
        return Ok(());
    }
    let monsters = Monsters::load(&accounts.monsters, &session_key)?;
    turns::advance(session, session_key, &monsters)
}
