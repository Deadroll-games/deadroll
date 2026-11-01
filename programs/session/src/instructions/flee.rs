use anchor_lang::prelude::*;
use deadroll_rules::exit::ExitKind;
use deadroll_types::seeds;
use session_keys::{session_auth_or, Session, SessionTokenV2};

use crate::{
    auth,
    errors::SessionError,
    events::FleeVoted,
    exit_vote,
    state::{GameSession, Monsters, Participation},
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

/// "Escape" (docs/PARTY_AND_TURNS.md 7.3, 7.5): in an arena, this player proposes or confirms
/// leaving the dungeon, and the turn ends. When every living player voted, the arena's monsters
/// move once more, then the party flees: no experience, no rewards (`turns::advance`).
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
    let slot = participation.core.slot;
    exit_vote::check_can_vote(session, slot)?;

    let session_key = session.key();
    let monsters = Monsters::load(&accounts.monsters, &session_key)?;
    let alive_mask = session.alive_mask;
    let v = exit_vote::cast(session, ExitKind::Escape, slot);
    emit!(FleeVoted {
        session: session_key,
        slot,
        flee_mask: v.mask,
        alive_mask,
    });
    exit_vote::end_vote_turn(session, session_key, &monsters)
}
