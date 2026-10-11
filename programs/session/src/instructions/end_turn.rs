use anchor_lang::prelude::*;
use deadroll_types::{seeds, SessionStatus};

use crate::{
    auth,
    errors::SessionError,
    events::TurnStarted,
    rules::{monster_ap, Unit},
    state::{GameSession, Monsters, Participation},
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
        }
        Unit::Monster(_) => {
            auth::check_actor(&accounts.signer, &session.core.dm, &session.dm_actor, token)?;
        }
    }

    // Next living unit in the order; passing the end starts a new round. Unspent AP are lost:
    // players get fresh AP lazily through `turn_seq`, monsters right here.
    let monsters = Monsters::load(&accounts.monsters, &session.key())?;
    let len = session.turn_order_len;
    let mut index = session.core.turn_index;
    for _ in 0..len {
        index = (index + 1) % len;
        if index == 0 {
            session.core.round = session
                .core
                .round
                .checked_add(1)
                .ok_or(SessionError::Overflow)?;
        }
        let alive = match Unit::from_id(session.core.turn_order[index as usize]) {
            Some(Unit::Player(slot)) => session.player_alive(slot),
            Some(Unit::Monster(i)) => monsters.get(i)?.alive,
            None => false,
        };
        if alive {
            break;
        }
    }
    session.core.turn_index = index;
    session.turn_seq = session
        .turn_seq
        .checked_add(1)
        .ok_or(SessionError::Overflow)?;
    session.turn_started_at = Clock::get()?.unix_timestamp;

    let unit = session.core.turn_order[index as usize];
    if let Some(Unit::Monster(i)) = Unit::from_id(unit) {
        let mut monster = monsters.get(i)?;
        monster.action_points = monster_ap(monster.bestiary_id);
        monsters.set(i, &monster)?;
    }

    emit!(TurnStarted {
        session: session.key(),
        unit,
        round: session.core.round,
        turn_seq: session.turn_seq,
    });
    Ok(())
}
