//! Patch B: Retreat and Escape from an arena (docs/PARTY_AND_TURNS.md 7.5), shared by `retreat`,
//! `flee` and every other player action. The rules are `deadroll_rules::exit`: one player proposes,
//! the other living players confirm on their turns (a vote spends the turn), any other action on a
//! player's turn cancels the proposal; when every living player voted, the arena's monsters take
//! one more turn each (`GameSession.last_moves`, `turns::advance`), then the party leaves.
use anchor_lang::prelude::*;
use deadroll_rules::exit::{passed, vote, ExitKind, Vote, NO_EXIT};

use crate::{
    errors::SessionError,
    events::{ExitCancelled, ExitPassed},
    rules::Unit,
    state::{ArenaMode, GameSession, Monsters},
    turns,
};

/// What a vote needs: the voter's turn in an arena, started, nothing pending.
pub fn check_can_vote(session: &GameSession, slot: u8) -> Result<()> {
    require!(
        session.core.status == deadroll_types::SessionStatus::Active,
        SessionError::NotActive
    );
    require!(!session.core.pending_vrf, SessionError::RollPending);
    // Patch 6: a player down in the last callback is finalized first (`start_turn`).
    require!(session.pending_deaths == 0, SessionError::DeathsPending);
    session.require_no_reveals()?;
    require!(
        session.current_unit() == Some(Unit::Player(slot)),
        SessionError::NotYourTurn
    );
    require!(session.turn_started(), SessionError::TurnNotStarted);
    session.require_in_play()?;
    require!(session.mode == ArenaMode::Arena, SessionError::NotInArena);
    require!(session.player_alive(slot), SessionError::UnitDead);
    Ok(())
}

/// Player `slot` votes for `kind`; returns the new vote. The caller emits its vote event, then
/// [`end_vote_turn`].
pub fn cast(session: &mut GameSession, kind: ExitKind, slot: u8) -> Vote {
    let v = vote(session.exit_vote(), kind, slot);
    session.set_exit_vote(v);
    v
}

/// The voter's turn ends. When every living player voted, the arena's living monsters are owed
/// one more turn each (`ExitPassed`).
pub fn end_vote_turn(
    session: &mut GameSession,
    session_key: Pubkey,
    monsters: &Monsters,
) -> Result<()> {
    let v = session.exit_vote();
    if passed(v.mask, session.alive_mask) {
        let living = turns::living(monsters)?;
        // A pack that is dead is won, not left (`outcomes::monster_killed`).
        require!(living != 0, SessionError::InvalidExit);
        session.last_moves = living;
        emit!(ExitPassed {
            session: session_key,
            kind: v.kind,
            last_moves: living,
        });
    }
    turns::advance(session, session_key, monsters)
}

/// Any other action on player `slot`'s turn: the proposal is off (`ExitCancelled` if there was
/// one). The votes of earlier turns are not returned.
pub fn cancel(session: &mut GameSession, session_key: Pubkey, slot: u8) {
    if session.exit_kind == NO_EXIT && session.flee_mask == 0 {
        return;
    }
    emit!(ExitCancelled {
        session: session_key,
        kind: session.exit_kind,
        slot,
    });
    session.set_exit_vote(Vote::NONE);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::test_support::session;
    use deadroll_types::{Cell, SessionStatus};

    /// Three players in an arena, player 1's turn started.
    fn arena() -> GameSession {
        let mut s = session(&[Cell::default(); 3]);
        s.core.turn_order[0] = Unit::Player(1).id();
        s.turn_order_len = 1;
        s.started_seq = s.turn_seq;
        s
    }

    fn code(r: Result<()>) -> anchor_lang::error::Error {
        r.unwrap_err()
    }

    #[test]
    fn a_vote_needs_the_voters_started_turn_in_an_arena() {
        let s = arena();
        assert!(check_can_vote(&s, 1).is_ok());
        // Not your turn.
        assert_eq!(
            code(check_can_vote(&s, 0)),
            SessionError::NotYourTurn.into()
        );
        // On the map, or between the map and an arena.
        let mut m = arena();
        m.mode = ArenaMode::Map;
        assert_eq!(code(check_can_vote(&m, 1)), SessionError::NotInArena.into());
        for mode in [ArenaMode::Engaged, ArenaMode::Won, ArenaMode::Retreated] {
            m.mode = mode;
            assert_eq!(
                code(check_can_vote(&m, 1)),
                SessionError::ArenaTransition.into()
            );
        }
        // The turn has not started.
        let mut t = arena();
        t.started_seq = 0;
        assert_eq!(
            code(check_can_vote(&t, 1)),
            SessionError::TurnNotStarted.into()
        );
        // A roll or a death is pending, the party ended.
        let mut p = arena();
        p.core.pending_vrf = true;
        assert_eq!(
            code(check_can_vote(&p, 1)),
            SessionError::RollPending.into()
        );
        let mut d = arena();
        d.pending_deaths = 0b0100;
        assert_eq!(
            code(check_can_vote(&d, 1)),
            SessionError::DeathsPending.into()
        );
        let mut e = arena();
        e.core.status = SessionStatus::Fled;
        assert_eq!(code(check_can_vote(&e, 1)), SessionError::NotActive.into());
        // A dead player does not vote.
        let mut dead = arena();
        dead.alive_mask = 0b0101;
        assert_eq!(
            code(check_can_vote(&dead, 1)),
            SessionError::UnitDead.into()
        );
    }

    #[test]
    fn votes_add_up_and_another_action_cancels_them() {
        let mut s = arena();
        let v = cast(&mut s, ExitKind::Retreat, 1);
        assert_eq!(
            (s.exit_kind, s.flee_mask),
            (ExitKind::Retreat as u8, 0b0010)
        );
        assert_eq!(v, s.exit_vote());
        cast(&mut s, ExitKind::Retreat, 2);
        assert_eq!(s.flee_mask, 0b0110);
        assert!(!passed(s.flee_mask, s.alive_mask));
        // Player 0 moves instead: the proposal is off.
        cancel(&mut s, Pubkey::new_unique(), 0);
        assert_eq!(s.exit_vote(), Vote::NONE);
        // Cancelling nothing changes nothing.
        cancel(&mut s, Pubkey::new_unique(), 0);
        assert_eq!(s.exit_vote(), Vote::NONE);
        // An Escape vote replaces a Retreat proposal.
        cast(&mut s, ExitKind::Retreat, 1);
        cast(&mut s, ExitKind::Escape, 2);
        assert_eq!((s.exit_kind, s.flee_mask), (ExitKind::Escape as u8, 0b0100));
    }
}
