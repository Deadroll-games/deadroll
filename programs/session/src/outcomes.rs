//! Deaths and the end of the party, shared by hits (`consume_randomness`) and over-time damage
//! (`start_turn`), so both take the same path.
use anchor_lang::prelude::*;
use deadroll_types::{Monster, SessionStatus};

use crate::{
    cpi_out::{self, character::DeathAccounts, SessionSigner},
    events::{MonsterKilled, PlayerDied, SessionEnded},
    rules::{is_boss, monster_profile},
    state::GameSession,
};

/// Monster `index` reached 0 HP, killed by unit `by`: its XP goes into the pool, `MonsterKilled`.
/// The boss wins the party (docs/PARTY_AND_TURNS.md, 7.1) and `seed` becomes the reward seed.
/// The caller writes `monster` back.
pub fn monster_killed(
    session: &mut GameSession,
    session_key: Pubkey,
    monster: &mut Monster,
    index: u8,
    by: u8,
    seed: [u8; 32],
) {
    monster.alive = false;
    let xp = monster_profile(monster.bestiary_id, monster.level).map_or(0, |p| p.xp);
    session.xp_pool = session.xp_pool.saturating_add(xp);
    let boss = is_boss(monster.bestiary_id);
    emit!(MonsterKilled {
        session: session_key,
        monster: index,
        bestiary_id: monster.bestiary_id,
        by,
        xp,
        boss,
    });
    if boss {
        session.core.status = SessionStatus::Victory;
        session.victory_seed = seed;
    }
}

/// Player `slot` (its `character`) reached 0 HP, killed by unit `by`: out of the living,
/// `character.mark_dead` (the tombstone; its skills, bag and soulbound items burn), `PlayerDied`.
/// The last living player wipes the party.
pub fn player_killed<'info>(
    session: &mut GameSession,
    session_key: Pubkey,
    slot: u8,
    by: u8,
    signer: &SessionSigner<'_, 'info>,
    death: &DeathAccounts<'_, 'info>,
) -> Result<()> {
    session.alive_mask &= !(1 << slot);
    cpi_out::character::mark_dead(signer, death)?;
    emit!(PlayerDied {
        session: session_key,
        character: death.character.key(),
        slot,
        by,
    });
    if session.alive_mask == 0 {
        session.core.status = SessionStatus::Wiped;
    }
    Ok(())
}

/// Emits `SessionEnded` when the party is over; returns whether it is.
pub fn emit_if_ended(session: &GameSession, session_key: Pubkey) -> bool {
    if session.core.status == SessionStatus::Active {
        return false;
    }
    emit!(SessionEnded {
        session: session_key,
        status: session.core.status,
        round: session.core.round,
    });
    true
}
