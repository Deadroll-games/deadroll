//! Turn changes shared by `end_turn` and `flee`, and the escape rule.
use anchor_lang::prelude::*;

use crate::{
    errors::SessionError,
    events::TurnStarted,
    rules::{monster_ap, Unit},
    state::{GameSession, Monsters},
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
        monster.action_points = monster_ap(monster.bestiary_id);
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
