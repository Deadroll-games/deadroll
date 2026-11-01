use anchor_lang::prelude::*;
use deadroll_rules::{
    arena::on_pack_won,
    spread::{BurningCell, BURNING_CELLS_MAX},
};
use deadroll_types::{seeds, SessionStatus, PARTY_MAX, TURN_ORDER_MAX};

use crate::{
    arena::{arena_pack_xp, living_participations, store_participations, ArenaCaches},
    errors::SessionError,
    events::{ArenaLeft, ArenaRetreated},
    loot::{pay_kills, Caches},
    rules::EMPTY_UNIT,
    state::{spawn_bit, ArenaMode, GameSession, Monsters, NO_PACK},
    turns,
};

#[derive(Accounts)]
pub struct LeaveArena<'info> {
    /// Any participant: leaving a won arena holds no choice.
    pub payer: Signer<'info>,
    #[account(
        mut,
        seeds = [seeds::SESSION, session.core.map.as_ref()],
        bump = session.core.bump,
    )]
    pub session: Box<Account<'info, GameSession>>,
    /// CHECK: `MonsterState` of this session, checked by `Monsters::load` (written in place).
    #[account(mut)]
    pub monsters: UncheckedAccount<'info>,
}

/// Arena update (docs/PARTY_AND_TURNS.md 4.1.4): the arena's pack is dead. The kills still waiting
/// for their body rewards are paid (from the latest VRF value, like the over-time kills of
/// `start_turn`), then the pack's experience goes in full to every living player
/// (`deadroll_rules::arena::on_pack_won`); the pack is gone from the map, the players return to
/// the cells they stood on before the fight, and the map circle goes on with the player after the
/// one who engaged. Patch B: after a retreat (7.5) the waiting kills are paid the same way but
/// nobody gets experience, and the pack stays on the map without the monsters killed here, on
/// guard and buffed for its next fight. `remaining_accounts`: each living player's
/// `Participation`, in slot order.
pub fn handler<'info>(ctx: Context<'info, LeaveArena<'info>>) -> Result<()> {
    let accounts = ctx.accounts;
    let session = &mut accounts.session;
    let session_key = session.key();
    require!(
        session.core.status == SessionStatus::Active,
        SessionError::NotActive
    );
    require!(!session.core.pending_vrf, SessionError::RollPending);
    require!(session.pending_deaths == 0, SessionError::DeathsPending);
    let retreated = session.mode == ArenaMode::Retreated;
    require!(
        retreated || session.mode == ArenaMode::Won,
        SessionError::ArenaNotWon
    );
    let mut players = living_participations(ctx.remaining_accounts, session)?;
    let monsters = Monsters::load(&accounts.monsters, &session_key)?;

    let pack = session.arena_pack;
    let xp = {
        let mut caches = ArenaCaches {
            players: &mut players,
        };
        if session.unpaid_kills != 0 {
            let kills = session.unpaid_kills;
            let seed = session.core.last_seed;
            pay_kills(session, session_key, &monsters, kills, seed, &mut caches)?;
            session.unpaid_kills = 0;
        }
        // Patch B: no experience for a fight the party retreated from, not even for its kills
        // (docs/PARTY_AND_TURNS.md 7.5); their body rewards stay in the caches.
        let alive = session.alive_mask & session.full_mask();
        let xp = if retreated {
            [0; PARTY_MAX]
        } else {
            on_pack_won(arena_pack_xp(&monsters)?, alive, &session.xp_bp)
        };
        for (slot, &x) in xp.iter().enumerate() {
            if x > 0 {
                caches.add(slot as u8, x, &[])?;
            }
        }
        xp
    };
    emit!(ArenaLeft {
        session: session_key,
        pack,
        xp,
    });

    if retreated {
        // Patch B: the pack stays, without the monsters killed in this fight; on guard and
        // buffed for its next fight, where the rest come back healed (`enter_arena`).
        for i in 0..monsters.count()? {
            if !monsters.get(i)?.alive {
                session.dead_spawns |= spawn_bit(monsters.spawn_of(i)?);
            }
        }
        session.packs_guarded |= 1u8 << pack;
        session.packs_buffed |= 1u8 << pack;
        session.retreats = session.retreats.saturating_add(1);
        emit!(ArenaRetreated {
            session: session_key,
            pack,
            retreats: session.retreats,
            dead_spawns: session.dead_spawns,
        });
    } else {
        // Back on the map, without the pack.
        session.packs_alive &= !(1u8 << pack);
    }
    session.width = session.map_width;
    session.height = session.map_height;
    session.player_pos = session.map_pos;
    for (slot, p) in players.iter_mut().enumerate() {
        if let Some(p) = p.as_mut() {
            p.core.x = session.map_pos[slot].x;
            p.core.y = session.map_pos[slot].y;
        }
    }
    monsters.set_count(0)?;
    monsters.set_burning_cells(&[BurningCell::default(); BURNING_CELLS_MAX])?;
    session.core.turn_order = [EMPTY_UNIT; TURN_ORDER_MAX];
    let len = (session.map_order_len as usize).min(PARTY_MAX);
    let order = session.map_order;
    session.core.turn_order[..len].copy_from_slice(&order[..len]);
    session.turn_order_len = len as u8;
    session.core.turn_index = session.map_turn_index;
    session.core.round = session.map_round;
    session.mode = ArenaMode::Map;
    session.arena_pack = NO_PACK;
    turns::advance(session, session_key, &monsters)?;
    store_participations(&players)
}
