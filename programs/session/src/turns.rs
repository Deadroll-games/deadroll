//! Turn changes: the first turn after the initiative roll, the advance shared by `end_turn` and
//! `flee`, and the escape rule.
use anchor_lang::prelude::*;
use deadroll_types::TURN_ORDER_MAX;

use crate::{
    errors::SessionError,
    events::{InitiativeRolled, TurnStarted},
    rules::{monster_ap, roll_initiative, Unit},
    state::{GameSession, Monsters},
    vrf::SyscallRoll,
};

/// Starts the next living unit's turn; passing the end of the order starts a new round. Unspent
/// AP are lost: players get fresh AP lazily through `turn_seq`, monsters right here.
pub fn advance(session: &mut GameSession, session_key: Pubkey, monsters: &Monsters) -> Result<()> {
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
        monster.action_points = monster_ap(session.monster_speed[i as usize]);
        monsters.set(i, &monster)?;
    }

    emit!(TurnStarted {
        session: session_key,
        unit,
        round: session.core.round,
        turn_seq: session.turn_seq,
    });
    Ok(())
}

/// Writes the turn order rolled from the initiative VRF value (`start_session`'s request) and
/// starts the first turn. Players by slot, then living monsters by index, roll in that order.
pub fn begin(
    session: &mut GameSession,
    session_key: Pubkey,
    monsters: &Monsters,
    seed: [u8; 32],
) -> Result<()> {
    let mut units = [(0u8, 0u16); TURN_ORDER_MAX];
    let mut len = 0usize;
    for slot in 0..session.core.player_count {
        units[len] = (
            Unit::Player(slot).id(),
            session.player_initiative[slot as usize],
        );
        len += 1;
    }
    for index in 0..monsters.count()? {
        if monsters.get(index)?.alive {
            units[len] = (
                Unit::Monster(index).id(),
                session.monster_initiative[index as usize],
            );
            len += 1;
        }
    }
    let rolled = roll_initiative(&units[..len], &mut SyscallRoll::new(seed));
    session.core.turn_order = rolled.order;
    session.turn_order_len = rolled.len;
    session.core.turn_index = 0;
    session.core.round = 1;
    session.turn_seq = 1;
    session.turn_started_at = Clock::get()?.unix_timestamp;

    emit!(InitiativeRolled {
        session: session_key,
        seed,
        action_counter: session.core.action_counter,
        len: rolled.len,
        order: rolled.order,
        d20: rolled.d20,
        initiative: rolled.initiative,
    });

    // Players get AP lazily on their first action; a monster that goes first gets them here.
    let unit = session.core.turn_order[0];
    if let Some(Unit::Monster(i)) = Unit::from_id(unit) {
        let mut monster = monsters.get(i)?;
        monster.action_points = monster_ap(session.monster_speed[i as usize]);
        monsters.set(i, &monster)?;
    }
    emit!(TurnStarted {
        session: session_key,
        unit,
        round: session.core.round,
        turn_seq: session.turn_seq,
    });
    Ok(())
}

/// The party flees when every living player has the escape intent (docs/PARTY_AND_TURNS.md,
/// section 7.3: the dead do not vote, unanimity is the check).
pub fn everyone_flees(flee_mask: u8, alive_mask: u8) -> bool {
    alive_mask != 0 && flee_mask & alive_mask == alive_mask
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn flee_needs_every_living_player() {
        assert!(!everyone_flees(0b0001, 0b0011));
        assert!(everyone_flees(0b0011, 0b0011));
        // The tank died: the other three decide.
        assert!(everyone_flees(0b1110, 0b1110));
        assert!(everyone_flees(0b1111, 0b1110));
        assert!(!everyone_flees(0b0110, 0b1110));
        assert!(!everyone_flees(0, 0));
    }
}
