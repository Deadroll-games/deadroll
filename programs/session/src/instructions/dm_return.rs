use anchor_lang::prelude::*;
use deadroll_types::{seeds, SessionStatus};
use session_keys::{session_auth_or, Session, SessionTokenV2};

use crate::{auth, errors::SessionError, events::DmReturned, rules::TIMEOUTS, state::GameSession};

#[derive(Accounts, Session)]
pub struct DmReturn<'info> {
    /// The DM's main wallet or session key.
    pub signer: Signer<'info>,
    #[account(
        mut,
        seeds = [seeds::SESSION, session.core.map.as_ref()],
        bump = session.core.bump,
    )]
    pub session: Box<Account<'info, GameSession>>,
    /// MagicBlock `SessionTokenV2` of the session key; none when the main wallet signs.
    #[session(signer = signer, authority = session.core.dm)]
    pub session_token: Option<Account<'info, SessionTokenV2>>,
}

/// Stuck party (docs/PARTY_AND_TURNS.md 3.2): the DM marked missing says "I'm back" before the
/// countdown ends. The party stays frozen; every living player has `dm_return_confirm_secs` to
/// confirm (`confirm_dm_return`).
#[session_auth_or(
    ctx.accounts.signer.key() == ctx.accounts.session.core.dm,
    SessionError::Unauthorized
)]
pub fn handler(ctx: Context<DmReturn>) -> Result<()> {
    let accounts = ctx.accounts;
    let session = &mut accounts.session;
    auth::check_bound(accounts.signer.key, &session.core.dm, &session.dm_actor)?;
    let now = Clock::get()?.unix_timestamp;
    come_back(session, now)?;
    emit!(DmReturned {
        session: session.key(),
        until: now.saturating_add(TIMEOUTS.dm_return_confirm_secs as i64),
    });
    Ok(())
}

/// The DM comes back at `now`.
pub fn come_back(session: &mut GameSession, now: i64) -> Result<()> {
    require!(
        session.core.status == SessionStatus::Active,
        SessionError::NotActive
    );
    let mut watch = session.dm_watch();
    watch
        .come_back(now, session.settle_timeout_secs, TIMEOUTS)
        .map_err(SessionError::from)?;
    session.set_dm_watch(watch);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::test_support::session;
    use deadroll_types::Cell;

    fn missing() -> GameSession {
        let mut s = session(&[Cell::default()]);
        s.settle_timeout_secs = 600;
        s.dm_missing_at = 1_000;
        s
    }

    #[test]
    fn the_dm_comes_back_before_the_countdown_ends() {
        let mut s = missing();
        come_back(&mut s, 1_600).unwrap();
        assert_eq!(s.dm_back_at, 1_600);
        // Still frozen until the players confirm.
        assert_eq!(
            s.require_no_reveals().unwrap_err(),
            error!(SessionError::PartyFrozen)
        );
    }

    #[test]
    fn rejections() {
        // Too late: the countdown ended.
        let mut s = missing();
        assert_eq!(
            come_back(&mut s, 1_601).unwrap_err(),
            error!(SessionError::WrongDmPhase)
        );
        // Not marked missing.
        let mut s = session(&[Cell::default()]);
        assert_eq!(
            come_back(&mut s, 1_000).unwrap_err(),
            error!(SessionError::WrongDmPhase)
        );
        // Once.
        let mut s = missing();
        come_back(&mut s, 1_100).unwrap();
        assert_eq!(
            come_back(&mut s, 1_200).unwrap_err(),
            error!(SessionError::WrongDmPhase)
        );
        let mut s = missing();
        s.core.status = SessionStatus::Abandoned;
        assert_eq!(
            come_back(&mut s, 1_100).unwrap_err(),
            error!(SessionError::NotActive)
        );
    }
}
