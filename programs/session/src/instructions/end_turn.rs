use anchor_lang::prelude::*;
use deadroll_types::{seeds, SessionStatus};
use session_keys::{session_auth_or, Session, SessionTokenV2};

use crate::{
    auth,
    errors::SessionError,
    rules::Unit,
    state::{GameSession, Monsters, Participation},
    turns,
};

#[derive(Accounts, Session)]
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
    /// MagicBlock `SessionTokenV2` of the session key; none when the main wallet signs.
    #[session(signer = signer, authority = turn_keys().0)]
    pub session_token: Option<Account<'info, SessionTokenV2>>,
}

impl EndTurn<'_> {
    /// Whose turn it is: (main wallet, session key bound to this party). The player's on a
    /// player's turn (default keys when `participation` is missing or not theirs, so every check
    /// fails), the DM's on a monster's.
    fn turn_keys(&self) -> (Pubkey, Pubkey) {
        match self.session.current_unit() {
            Some(Unit::Player(slot)) => match &self.participation {
                Some(p) if p.core.slot == slot => (p.player, p.core.session_key),
                _ => (Pubkey::default(), Pubkey::default()),
            },
            _ => (self.session.core.dm, self.session.dm_actor),
        }
    }
}

#[session_auth_or(
    ctx.accounts.signer.key() == ctx.accounts.turn_keys().0,
    SessionError::Unauthorized
)]
pub fn handler(ctx: Context<EndTurn>) -> Result<()> {
    let accounts = ctx.accounts;
    let session = &mut accounts.session;
    require!(
        session.core.status == SessionStatus::Active,
        SessionError::NotActive
    );
    require!(!session.core.pending_vrf, SessionError::RollPending);
    // Patch 6: a player down in the last callback is finalized first (`start_turn`).
    require!(session.pending_deaths == 0, SessionError::DeathsPending);
    let current = session.current_unit().ok_or(SessionError::NotActive)?;
    match current {
        Unit::Player(slot) => {
            let participation = accounts
                .participation
                .as_ref()
                .ok_or(SessionError::ParticipationMissing)?;
            require!(participation.core.slot == slot, SessionError::NotYourTurn);
            auth::check_bound(
                accounts.signer.key,
                &participation.player,
                &participation.core.session_key,
            )?;
            // Any other action on a player's turn cancels a Retreat or Escape proposal
            // (docs/PARTY_AND_TURNS.md 7.5).
            let key = session.key();
            crate::exit_vote::cancel(session, key, slot);
        }
        Unit::Monster(_) => {
            auth::check_bound(accounts.signer.key, &session.core.dm, &session.dm_actor)?;
        }
    }

    // The start (and its over-time damage) cannot be skipped by ending the turn.
    require!(session.turn_started(), SessionError::TurnNotStarted);
    session.require_in_play()?;
    let monsters = Monsters::load(&accounts.monsters, &session.key())?;
    let key = session.key();
    turns::advance(session, key, &monsters)
}
