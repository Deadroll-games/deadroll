use anchor_lang::prelude::*;
use deadroll_types::{seeds, Cell, SessionStatus};
use session_keys::{session_auth_or, Session, SessionTokenV2};

use crate::{
    auth,
    errors::SessionError,
    events::CellExamined,
    rules::{Unit, BALANCE},
    secrets::oblige_cells,
    state::{GameSession, MonsterState, Monsters, Participation},
};

#[derive(Accounts, Session)]
pub struct Examine<'info> {
    /// Main wallet or the participation's session key.
    pub signer: Signer<'info>,
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
    /// Read-only, never Borsh-decoded: owner and discriminator checked here, session in `Monsters`.
    pub monsters: LazyAccount<'info, MonsterState>,
    /// MagicBlock `SessionTokenV2` of the session key; none when the main wallet signs.
    #[session(signer = signer, authority = participation.player)]
    pub session_token: Option<Account<'info, SessionTokenV2>>,
}

/// Patch C (docs/MAP_DESCRIPTION.md 8.4: interaction with an object, a barrel searched, a wall
/// examined, reveals that cell): on their turn a player examines the cell `(x, y)` next to them
/// (Chebyshev 1) on the ground the party stands on, for `SecretTuning.examine_ap` action points.
/// An interaction, not the turn's one action. A cell not revealed yet becomes the DM's obligation:
/// a secret door or a hidden stash shows itself once the DM reveals it.
#[session_auth_or(
    ctx.accounts.signer.key() == ctx.accounts.participation.player,
    SessionError::Unauthorized
)]
pub fn handler(ctx: Context<Examine>, x: u8, y: u8) -> Result<()> {
    let accounts = ctx.accounts;
    let session = &mut accounts.session;
    let participation = &mut accounts.participation;
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
    require!(session.pending_deaths == 0, SessionError::DeathsPending);
    session.require_no_reveals()?;
    let slot = participation.core.slot;
    require!(
        session.current_unit() == Some(Unit::Player(slot)),
        SessionError::NotYourTurn
    );
    require!(session.turn_started(), SessionError::TurnNotStarted);
    session.require_in_play()?;
    require!(!session.leaving(), SessionError::NotYourTurn);
    require!(
        session.player_alive(slot) && participation.core.hp > 0,
        SessionError::UnitDead
    );
    let cell = Cell { x, y };
    require!(
        x < session.width && y < session.height,
        SessionError::OutOfBounds
    );
    let at = Cell {
        x: participation.core.x,
        y: participation.core.y,
    };
    require!(
        deadroll_mapcheck::walk::adjacent(at, cell),
        SessionError::NotAdjacent
    );

    // A monster's debuff aura over the player counts for the action points, as in `act`.
    let monsters = Monsters::load(accounts.monsters.as_ref(), &session.key())?;
    let field = crate::monster_cb::Field::load(session, &monsters)?;
    participation.refresh_ap(session.turn_seq, &field.view());
    participation.core.action_points = participation
        .core
        .action_points
        .checked_sub(BALANCE.secrets.examine_ap)
        .ok_or(SessionError::NotEnoughActionPoints)?;
    // Any other action cancels a Retreat or Escape proposal (docs/PARTY_AND_TURNS.md 7.5).
    let key = session.key();
    crate::exit_vote::cancel(session, key, slot);
    oblige_cells(session, key, slot, &[cell])?;
    emit!(CellExamined {
        session: key,
        slot,
        x,
        y,
        action_points: participation.core.action_points,
    });
    Ok(())
}
