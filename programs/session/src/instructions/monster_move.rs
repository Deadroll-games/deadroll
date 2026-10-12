use anchor_lang::prelude::*;
use deadroll_types::{seeds, Cell, SessionStatus};

use crate::{
    auth,
    errors::SessionError,
    events::Moved,
    movement::{walk_path, Path},
    rules::{Unit, MAX_PATH},
    state::{external::MapLayoutView, GameSession, Monsters},
};

#[derive(Accounts)]
pub struct MonsterMove<'info> {
    /// DM main wallet or the DM's session key.
    pub signer: Signer<'info>,
    #[account(seeds = [seeds::SESSION, session.core.map.as_ref()], bump = session.core.bump)]
    pub session: Box<Account<'info, GameSession>>,
    /// CHECK: `MonsterState` of this session, checked by `Monsters::load` (written in place; a
    /// mutable `LazyAccount` would re-serialize the whole account on exit).
    #[account(mut)]
    pub monsters: UncheckedAccount<'info>,
    /// CHECK: the session's `MapLayout`, address pinned at `open_session`.
    #[account(address = session.layout @ SessionError::InvalidLayout)]
    pub layout: UncheckedAccount<'info>,
    /// CHECK: MagicBlock `SessionTokenV2`, checked in `auth::check_actor`.
    pub session_token: Option<UncheckedAccount<'info>>,
}

pub fn handler(
    ctx: Context<MonsterMove>,
    monster: u8,
    steps: [u8; MAX_PATH],
    len: u8,
) -> Result<()> {
    let accounts = ctx.accounts;
    let session = &accounts.session;
    auth::check_actor(
        &accounts.signer,
        &session.core.dm,
        &session.dm_actor,
        accounts.session_token.as_ref(),
    )?;
    require!(
        session.core.status == SessionStatus::Active,
        SessionError::NotActive
    );
    require!(!session.core.pending_vrf, SessionError::RollPending);
    let monsters = Monsters::load(&accounts.monsters, &session.key())?;
    require!(monster < monsters.count()?, SessionError::InvalidMonster);
    let unit = Unit::Monster(monster);
    require!(
        session.current_unit() == Some(unit),
        SessionError::NotYourTurn
    );
    let mut state = monsters.get(monster)?;
    require!(state.alive, SessionError::UnitDead);

    let layout = MapLayoutView::load(&accounts.layout)?;
    let (end, action_points) = walk_path(
        session,
        &layout,
        &monsters,
        unit,
        Cell {
            x: state.x,
            y: state.y,
        },
        Path { steps: &steps, len },
        state.action_points,
    )?;

    state.x = end.x;
    state.y = end.y;
    state.action_points = action_points;
    monsters.set(monster, &state)?;

    emit!(Moved {
        session: session.key(),
        unit: unit.id(),
        x: end.x,
        y: end.y,
        action_points,
    });
    Ok(())
}
