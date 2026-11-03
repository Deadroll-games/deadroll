use anchor_lang::prelude::*;
use deadroll_types::{seeds, SessionStatus};
use session_keys::{session_auth_or, Session, SessionTokenV2};

use crate::{
    auth,
    errors::SessionError,
    events::DmMarkedMissing,
    rules::TIMEOUTS,
    state::{GameSession, Participation},
};

#[derive(Accounts, Session)]
pub struct MarkDmMissing<'info> {
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

/// Stuck party (docs/PARTY_AND_TURNS.md 3.2, MAP_DESCRIPTION.md 8.5): the DM owes a reveal for
/// `reveal_timeout_secs`; a living player marks the DM as missing. Nothing is paid: the party
/// freezes and the map's issue deadline counts down (`deadroll_rules::timeout`).
#[session_auth_or(
    ctx.accounts.signer.key() == ctx.accounts.participation.player,
    SessionError::NotParticipant
)]
pub fn handler(ctx: Context<MarkDmMissing>) -> Result<()> {
    let accounts = ctx.accounts;
    let session = &mut accounts.session;
    let slot = auth::participant(accounts.signer.key, session, Some(&accounts.participation))?
        .ok_or(SessionError::NotParticipant)?;
    let now = Clock::get()?.unix_timestamp;
    mark(session, slot, now)?;
    emit!(DmMarkedMissing {
        session: session.key(),
        slot,
        until: now.saturating_add(session.settle_timeout_secs as i64),
    });
    Ok(())
}

/// Living player `slot` marks the DM as missing at `now`.
pub fn mark(session: &mut GameSession, slot: u8, now: i64) -> Result<()> {
    require!(
        session.core.status == SessionStatus::Active,
        SessionError::NotActive
    );
    // The DM cannot reveal while a roll is pending (`retry_vrf` first).
    require!(!session.core.pending_vrf, SessionError::RollPending);
    require!(session.player_alive(slot), SessionError::UnitDead);
    let owed = session.owes_reveal().then_some(session.reveals_owed_at);
    let mut watch = session.dm_watch();
    watch
        .mark(owed, now, session.settle_timeout_secs, TIMEOUTS)
        .map_err(SessionError::from)?;
    session.set_dm_watch(watch);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::{test_support::session, ArenaMode};
    use deadroll_types::Cell;

    const T0: i64 = 1_000;

    /// Two players, the DM owes one cell since `T0`.
    fn owing() -> GameSession {
        let mut s = session(&[Cell::default(), Cell { x: 1, y: 1 }]);
        s.settle_timeout_secs = 600;
        s.obligations[0] = 40;
        s.obligation_count = 1;
        s.reveals_owed_at = T0;
        s
    }

    fn late() -> i64 {
        T0 + TIMEOUTS.reveal_timeout_secs as i64
    }

    #[test]
    fn a_living_player_marks_a_dm_whose_reveal_is_overdue() {
        let mut s = owing();
        mark(&mut s, 1, late()).unwrap();
        assert_eq!(s.dm_missing_at, late());
        // The party is frozen: no turn instruction goes through.
        assert_eq!(
            s.require_no_reveals().unwrap_err(),
            error!(SessionError::PartyFrozen)
        );
    }

    #[test]
    fn a_pack_to_reveal_is_owed_too() {
        let mut s = owing();
        s.obligation_count = 0;
        s.mode = ArenaMode::Engaged;
        s.arena_pack = 2;
        mark(&mut s, 0, late()).unwrap();
        // Once revealed, nothing is owed.
        let mut s = owing();
        s.obligation_count = 0;
        s.mode = ArenaMode::Engaged;
        s.arena_pack = 2;
        s.packs_revealed = 1 << 2;
        assert_eq!(
            mark(&mut s, 0, late()).unwrap_err(),
            error!(SessionError::NoRevealOwed)
        );
    }

    #[test]
    fn rejections() {
        let mut s = owing();
        assert_eq!(
            mark(&mut s, 0, late() - 1).unwrap_err(),
            error!(SessionError::TooEarly)
        );
        let mut s = owing();
        s.obligation_count = 0;
        assert_eq!(
            mark(&mut s, 0, late()).unwrap_err(),
            error!(SessionError::NoRevealOwed)
        );
        let mut s = owing();
        s.core.status = SessionStatus::Victory;
        assert_eq!(
            mark(&mut s, 0, late()).unwrap_err(),
            error!(SessionError::NotActive)
        );
        let mut s = owing();
        s.core.pending_vrf = true;
        assert_eq!(
            mark(&mut s, 0, late()).unwrap_err(),
            error!(SessionError::RollPending)
        );
        let mut s = owing();
        s.alive_mask = 0b01;
        assert_eq!(
            mark(&mut s, 1, late()).unwrap_err(),
            error!(SessionError::UnitDead)
        );
        // Frozen already: once.
        let mut s = owing();
        mark(&mut s, 0, late()).unwrap();
        assert_eq!(
            mark(&mut s, 1, late() + 1).unwrap_err(),
            error!(SessionError::WrongDmPhase)
        );
    }
}
