use anchor_lang::prelude::*;
use deadroll_types::{seeds, SessionStatus};

use crate::{
    auth,
    errors::SessionError,
    rules::Unit,
    state::{GameSession, Monsters, Participation},
    turns,
};

#[derive(Accounts)]
pub struct EndTurn<'info> {
    /// On a player's turn: their main wallet or session key. On a monster's turn: the DM's.
    pub signer: Signer<'info>,
    #[account(
        mut,
        seeds = [seeds::SESSION, session.core.map.as_ref()],
        bump = session.core.bump,
    )]
    pub session: Box<Account<'info, GameSession>>,
    /// CHECK: `MonsterState` of this session, checked by `Monsters::load` (written in place; a
    /// mutable `LazyAccount` would re-serialize the whole account on exit).
    #[account(mut)]
    pub monsters: UncheckedAccount<'info>,
    /// Required on a player's turn: the participation whose turn it is.
    #[account(
        seeds = [
            seeds::PARTICIPATION,
            session.key().as_ref(),
            participation.core.character.as_ref(),
        ],
        bump = participation.core.bump,
    )]
    pub participation: Option<Box<Account<'info, Participation>>>,
    /// CHECK: MagicBlock `SessionTokenV2`, checked in `auth::check_actor`.
    pub session_token: Option<UncheckedAccount<'info>>,
}

pub fn handler(ctx: Context<EndTurn>) -> Result<()> {
    let accounts = ctx.accounts;
    let session = &mut accounts.session;
    require!(
        session.core.status == SessionStatus::Active,
        SessionError::NotActive
    );
    require!(!session.core.pending_vrf, SessionError::RollPending);
    let current = session.current_unit().ok_or(SessionError::NotActive)?;
    let token = accounts.session_token.as_ref();
    match current {
        Unit::Player(slot) => {
            let participation = accounts
                .participation
                .as_ref()
                .ok_or(SessionError::ParticipationMissing)?;
            require!(participation.core.slot == slot, SessionError::NotYourTurn);
            auth::check_actor(
                &accounts.signer,
                &participation.player,
                &participation.core.actor,
                token,
            )?;
            // Any other action on a player's turn resets every escape intent (PARTY_AND_TURNS 7.3).
            session.flee_mask = 0;
        }
        Unit::Monster(_) => {
            auth::check_actor(&accounts.signer, &session.core.dm, &session.dm_actor, token)?;
        }
    }

    // The start (and its over-time damage) cannot be skipped by ending the turn.
    require!(session.turn_started(), SessionError::TurnNotStarted);
    let monsters = Monsters::load(&accounts.monsters, &session.key())?;
    let key = session.key();
    turns::advance(session, key, &monsters)
}
