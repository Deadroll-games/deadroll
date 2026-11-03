use anchor_lang::prelude::*;
use deadroll_rules::timeout::DmWatch;
use deadroll_types::{seeds, SessionStatus};
use session_keys::{session_auth_or, Session, SessionTokenV2};

use crate::{
    auth,
    errors::SessionError,
    events::{DmReturnConfirmed, PartyResumed},
    rules::TIMEOUTS,
    state::{GameSession, Participation},
};

#[derive(Accounts, Session)]
pub struct ConfirmDmReturn<'info> {
    /// A living player's main wallet or session key.
    pub signer: Signer<'info>,
    #[account(
        mut,
        seeds = [seeds::SESSION, session.core.map.as_ref()],
        bump = session.core.bump,
    )]
    pub session: Box<Account<'info, GameSession>>,
    #[account(
        seeds = [
            seeds::PARTICIPATION,
            session.key().as_ref(),
            participation.core.character.as_ref(),
        ],
        bump = participation.core.bump,
    )]
    pub participation: Box<Account<'info, Participation>>,
    /// MagicBlock `SessionTokenV2` of the session key; none when the main wallet signs.
    #[session(signer = signer, authority = participation.player)]
    pub session_token: Option<Account<'info, SessionTokenV2>>,
}

/// Stuck party (docs/PARTY_AND_TURNS.md 3.2): "Return to the party". A living player confirms the
/// DM's return within `dm_return_confirm_secs`; when every living player did, the party goes on
/// where it stopped (the DM reveals what it owes next).
#[session_auth_or(
    ctx.accounts.signer.key() == ctx.accounts.participation.player,
    SessionError::NotParticipant
)]
pub fn handler(ctx: Context<ConfirmDmReturn>) -> Result<()> {
    let accounts = ctx.accounts;
    let session = &mut accounts.session;
    let slot = auth::participant(accounts.signer.key, session, Some(&accounts.participation))?
        .ok_or(SessionError::NotParticipant)?;
    let key = session.key();
    let confirmed = session.dm_confirm_mask | (1 << slot);
    let resumed = confirm(session, slot, Clock::get()?.unix_timestamp)?;
    emit!(DmReturnConfirmed {
        session: key,
        slot,
        confirmed,
    });
    if resumed {
        emit!(PartyResumed { session: key });
    }
    Ok(())
}

/// Living player `slot` confirms at `now`; `true` when the party goes on: the watch is cleared,
/// and the reveal still owed and the turn count from now.
pub fn confirm(session: &mut GameSession, slot: u8, now: i64) -> Result<bool> {
    require!(
        session.core.status == SessionStatus::Active,
        SessionError::NotActive
    );
    require!(session.player_alive(slot), SessionError::UnitDead);
    let mut watch = session.dm_watch();
    let all = watch
        .confirm(
            slot,
            session.living_mask(),
            now,
            session.settle_timeout_secs,
            TIMEOUTS,
        )
        .map_err(SessionError::from)?;
    if all {
        session.set_dm_watch(DmWatch::PRESENT);
        session.reveals_owed_at = now;
        session.touch_turn(now);
    } else {
        session.set_dm_watch(watch);
    }
    Ok(all)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::test_support::session;
    use deadroll_types::Cell;

    const BACK: i64 = 1_200;

    /// Three players (slot 1 dead), the DM back at `BACK`, one cell still owed.
    fn back() -> GameSession {
        let mut s = session(&[Cell::default(), Cell { x: 1, y: 1 }, Cell { x: 2, y: 2 }]);
        s.alive_mask = 0b101;
        s.settle_timeout_secs = 600;
        s.dm_missing_at = 1_000;
        s.dm_back_at = BACK;
        s.obligations[0] = 40;
        s.obligation_count = 1;
        s.reveals_owed_at = 500;
        s.turn_clock = 400;
        s
    }

    #[test]
    fn every_living_player_confirms_and_the_party_goes_on() {
        let mut s = back();
        assert!(!confirm(&mut s, 0, BACK + 1).unwrap());
        assert_eq!(s.dm_confirm_mask, 0b001);
        assert_eq!(
            s.require_no_reveals().unwrap_err(),
            error!(SessionError::PartyFrozen)
        );
        let last = BACK + TIMEOUTS.dm_return_confirm_secs as i64;
        assert!(confirm(&mut s, 2, last).unwrap());
        assert_eq!(s.dm_watch(), DmWatch::PRESENT);
        // Not frozen: the DM's reveal is what the party waits for now, from now.
        assert_eq!(
            s.require_no_reveals().unwrap_err(),
            error!(SessionError::RevealsPending)
        );
        assert_eq!((s.reveals_owed_at, s.turn_clock), (last, last));
    }

    #[test]
    fn rejections() {
        let mut s = back();
        assert_eq!(
            confirm(&mut s, 1, BACK + 1).unwrap_err(),
            error!(SessionError::UnitDead)
        );
        let mut s = back();
        confirm(&mut s, 0, BACK + 1).unwrap();
        assert_eq!(
            confirm(&mut s, 0, BACK + 2).unwrap_err(),
            error!(SessionError::AlreadyConfirmed)
        );
        // Too late: the window closed.
        let mut s = back();
        let late = BACK + TIMEOUTS.dm_return_confirm_secs as i64 + 1;
        assert_eq!(
            confirm(&mut s, 0, late).unwrap_err(),
            error!(SessionError::WrongDmPhase)
        );
        // The DM is not back yet, or was never marked.
        let mut s = back();
        s.dm_back_at = 0;
        assert_eq!(
            confirm(&mut s, 0, 1_100).unwrap_err(),
            error!(SessionError::WrongDmPhase)
        );
        let mut s = session(&[Cell::default()]);
        assert_eq!(
            confirm(&mut s, 0, 1_100).unwrap_err(),
            error!(SessionError::WrongDmPhase)
        );
        let mut s = back();
        s.core.status = SessionStatus::Abandoned;
        assert_eq!(
            confirm(&mut s, 0, BACK + 1).unwrap_err(),
            error!(SessionError::NotActive)
        );
    }
}
