use anchor_lang::prelude::*;
use deadroll_types::{seeds, SessionStatus};

use crate::{
    errors::SessionError,
    events::TurnStarted,
    rules::{monster_ap, Unit},
    state::{Escrow, GameSession, Monsters},
};

#[derive(Accounts)]
pub struct StartSession<'info> {
    /// DM main wallet.
    pub dm: Signer<'info>,
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
    #[account(seeds = [seeds::ESCROW, session.key().as_ref()], bump = escrow.bump)]
    pub escrow: Box<Account<'info, Escrow>>,
}

pub fn handler(ctx: Context<StartSession>) -> Result<()> {
    let session = &mut ctx.accounts.session;
    let monsters = Monsters::load(&ctx.accounts.monsters, &session.key())?;
    require_keys_eq!(session.core.dm, ctx.accounts.dm.key(), SessionError::NotDm);
    require!(
        session.core.status == SessionStatus::Lobby,
        SessionError::NotInLobby
    );
    require!(session.core.player_count > 0, SessionError::NoPlayers);
    require!(
        session.ready_mask == session.full_mask(),
        SessionError::NotAllReady
    );
    require!(
        ctx.accounts.escrow.collected >= session.core.price_lamports,
        SessionError::PriceNotCollected
    );

    // Round-robin: players by slot, then monsters by index.
    // TODO(L7.4): initiative roll from VRF; P1: initiative timeline (docs/PARTY_AND_TURNS.md, 6).
    let mut len = 0usize;
    for slot in 0..session.core.player_count {
        session.core.turn_order[len] = Unit::Player(slot).id();
        len += 1;
    }
    for index in 0..monsters.count()? {
        if monsters.get(index)?.alive {
            session.core.turn_order[len] = Unit::Monster(index).id();
            len += 1;
        }
    }
    session.turn_order_len = len as u8;
    session.core.turn_index = 0;
    session.core.round = 1;
    session.core.status = SessionStatus::Active;
    session.turn_seq = 1;
    session.turn_started_at = Clock::get()?.unix_timestamp;

    // Players are first, so the first unit is a player slot: their AP refill on first action.
    let unit = session.core.turn_order[0];
    if let Some(Unit::Monster(index)) = Unit::from_id(unit) {
        let mut monster = monsters.get(index)?;
        monster.action_points = monster_ap(monster.bestiary_id);
        monsters.set(index, &monster)?;
    }

    emit!(TurnStarted {
        session: session.key(),
        unit,
        round: session.core.round,
        turn_seq: session.turn_seq,
    });
    Ok(())
}
