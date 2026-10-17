use anchor_lang::prelude::*;
use deadroll_types::{seeds, Cell, SessionStatus};

use crate::{
    auth,
    errors::SessionError,
    events::Moved,
    movement::{burn_on_path, walk_path, Path},
    rules::{Unit, MAX_PATH},
    state::{external::MapLayoutView, GameSession, MonsterState, Monsters, Participation},
};

#[derive(Accounts)]
pub struct MoveUnit<'info> {
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
    /// CHECK: the session's `MapLayout`, address pinned at `open_session`.
    #[account(address = session.layout @ SessionError::InvalidLayout)]
    pub layout: UncheckedAccount<'info>,
    /// CHECK: MagicBlock `SessionTokenV2`, checked in `auth::check_actor`.
    pub session_token: Option<UncheckedAccount<'info>>,
}

pub fn handler(ctx: Context<MoveUnit>, steps: [u8; MAX_PATH], len: u8) -> Result<()> {
    let accounts = ctx.accounts;
    let session = &mut accounts.session;
    let participation = &mut accounts.participation;
    auth::check_actor(
        &accounts.signer,
        &participation.player,
        &participation.core.actor,
        accounts.session_token.as_ref(),
    )?;
    require!(
        session.core.status == SessionStatus::Active,
        SessionError::NotActive
    );
    require!(!session.core.pending_vrf, SessionError::RollPending);
    let unit = Unit::Player(participation.core.slot);
    require!(
        session.current_unit() == Some(unit),
        SessionError::NotYourTurn
    );
    require!(session.turn_started(), SessionError::TurnNotStarted);
    require!(
        session.player_alive(participation.core.slot) && participation.core.hp > 0,
        SessionError::UnitDead
    );

    participation.refresh_ap(session.turn_seq);
    let layout = MapLayoutView::load(&accounts.layout)?;
    let monsters = Monsters::load(accounts.monsters.as_ref(), &session.key())?;
    let start = Cell {
        x: participation.core.x,
        y: participation.core.y,
    };
    let path = Path { steps: &steps, len };
    let (end, action_points) = walk_path(
        session,
        &layout,
        &monsters,
        unit,
        start,
        Path { steps: &steps, len },
        participation.core.action_points,
    )?;
    burn_on_path(
        session,
        session.key(),
        &monsters.burning_cells()?,
        unit,
        start,
        &path,
        &mut participation.core.states,
        0,
    )?;

    participation.core.x = end.x;
    participation.core.y = end.y;
    participation.core.action_points = action_points;
    session.player_pos[participation.core.slot as usize] = end;
    // Moving is another action: every escape intent resets (docs/PARTY_AND_TURNS.md, 7.3).
    session.flee_mask = 0;

    emit!(Moved {
        session: session.key(),
        unit: unit.id(),
        x: end.x,
        y: end.y,
        action_points,
    });
    Ok(())
}
