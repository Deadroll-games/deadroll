use anchor_lang::prelude::*;
use deadroll_rules::timeout::turn_overdue;
use deadroll_types::{seeds, SessionStatus};
use session_keys::{session_auth_or, Session, SessionTokenV2};

use crate::{
    auth,
    errors::SessionError,
    events::TurnTimedOut,
    rules::{Unit, TIMEOUTS},
    state::{GameSession, Monsters, Participation},
    turns,
};

#[derive(Accounts, Session)]
pub struct SkipTurn<'info> {
    /// Any participant: a player's main wallet or session key (with its `participation`), or the
    /// DM's (without one).
    pub signer: Signer<'info>,
    #[account(
        mut,
        seeds = [seeds::SESSION, session.core.map.as_ref()],
        bump = session.core.bump,
    )]
    pub session: Box<Account<'info, GameSession>>,
    /// CHECK: `MonsterState` of this session, checked by `Monsters::load` (written in place: the
    /// next monster's AP).
    #[account(mut)]
    pub monsters: UncheckedAccount<'info>,
    /// The signer's participation in this party; none when the DM signs.
    #[account(
        seeds = [
            seeds::PARTICIPATION,
            session.key().as_ref(),
            participation.core.character.as_ref(),
        ],
        bump = participation.core.bump,
    )]
    pub participation: Option<Box<Account<'info, Participation>>>,
    /// MagicBlock `SessionTokenV2` of the session key; none when a main wallet signs.
    #[session(signer = signer, authority = authority())]
    pub session_token: Option<Account<'info, SessionTokenV2>>,
}

impl SkipTurn<'_> {
    /// The main wallet behind the signer: the participation's player, else the DM.
    fn authority(&self) -> Pubkey {
        match &self.participation {
            Some(p) => p.player,
            None => self.session.core.dm,
        }
    }
}

/// Stuck party (docs/PARTY_AND_TURNS.md 5.3, 6.2): the current unit, a player or a monster the DM
/// does not move, did nothing for `turn_timeout_secs`; any participant skips its turn. A turn that
/// never started adds the bare delay on the timeline, without the unit's effects; a turn whose
/// start ran already took its delay. Not an action of the unit: a Retreat or Escape vote stays.
#[session_auth_or(
    ctx.accounts.signer.key() == ctx.accounts.authority(),
    SessionError::NotParticipant
)]
pub fn handler(ctx: Context<SkipTurn>) -> Result<()> {
    let accounts = ctx.accounts;
    let session = &mut accounts.session;
    auth::participant(
        accounts.signer.key,
        session,
        accounts.participation.as_deref().map(|p| &**p),
    )?;
    let unit = check(session, Clock::get()?.unix_timestamp)?;
    let key = session.key();
    emit!(TurnTimedOut {
        session: key,
        unit: unit.id(),
        turn_seq: session.turn_seq,
        started: session.turn_started(),
    });
    let monsters = Monsters::load(&accounts.monsters, &key)?;
    turns::advance(session, key, &monsters)
}

/// The current unit may be skipped at `now`: the party plays (not frozen, nothing pending), and
/// the unit did nothing for `turn_timeout_secs`. Returns the unit.
pub fn check(session: &GameSession, now: i64) -> Result<Unit> {
    require!(
        session.core.status == SessionStatus::Active,
        SessionError::NotActive
    );
    require!(!session.core.pending_vrf, SessionError::RollPending);
    require!(session.pending_deaths == 0, SessionError::DeathsPending);
    // A missing DM freezes the party; a reveal owed holds it as well (MAP 8.5).
    session.require_no_reveals()?;
    session.require_in_play()?;
    let unit = session.current_unit().ok_or(SessionError::NotActive)?;
    require!(
        turn_overdue(session.turn_clock, now, TIMEOUTS),
        SessionError::TooEarly
    );
    Ok(unit)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::{test_support::session, ArenaMode};
    use deadroll_types::Cell;

    const CLOCK: i64 = 1_000;

    fn idle() -> (GameSession, i64) {
        let mut s = session(&[Cell::default(), Cell { x: 1, y: 1 }]);
        s.core.turn_order[..2].copy_from_slice(&[0, 1]);
        s.turn_order_len = 2;
        s.turn_clock = CLOCK;
        (s, CLOCK + TIMEOUTS.turn_timeout_secs as i64)
    }

    #[test]
    fn an_idle_unit_is_skipped_on_the_map_and_in_an_arena() {
        let (mut s, late) = idle();
        assert_eq!(check(&s, late).unwrap(), s.current_unit().unwrap());
        s.mode = ArenaMode::Arena;
        check(&s, late).unwrap();
    }

    #[test]
    fn rejections() {
        let (s, late) = idle();
        assert_eq!(
            check(&s, late - 1).unwrap_err(),
            error!(SessionError::TooEarly)
        );
        let (mut s, late) = idle();
        s.dm_missing_at = 900;
        assert_eq!(
            check(&s, late).unwrap_err(),
            error!(SessionError::PartyFrozen)
        );
        let (mut s, late) = idle();
        s.obligation_count = 1;
        assert_eq!(
            check(&s, late).unwrap_err(),
            error!(SessionError::RevealsPending)
        );
        let (mut s, late) = idle();
        s.core.pending_vrf = true;
        assert_eq!(
            check(&s, late).unwrap_err(),
            error!(SessionError::RollPending)
        );
        let (mut s, late) = idle();
        s.pending_deaths = 1;
        assert_eq!(
            check(&s, late).unwrap_err(),
            error!(SessionError::DeathsPending)
        );
        let (mut s, late) = idle();
        s.mode = ArenaMode::Engaged;
        assert_eq!(
            check(&s, late).unwrap_err(),
            error!(SessionError::ArenaTransition)
        );
        let (mut s, late) = idle();
        s.core.status = SessionStatus::Victory;
        assert_eq!(
            check(&s, late).unwrap_err(),
            error!(SessionError::NotActive)
        );
    }
}
