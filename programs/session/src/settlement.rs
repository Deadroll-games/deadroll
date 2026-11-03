//! Patch 11: the end of a party for each player (docs/PARTY_AND_TURNS.md 7-9; decisions of
//! patch 11). The DM settles everyone at once with the main wallet (`settle_party`); after the
//! DM's deadline each player claims their own (`claim_after_timeout`). Both take the same path:
//!
//! - victory, alive: the cache in `Participation` becomes the player's (its `RewardCache` entry
//!   turns `Claimed`) and its experience goes to the character (`character.grant_xp`);
//! - abandoned by the DM (stuck party), alive: the same, as on a peaceful exit; the DM gets no
//!   escrow (`settle_unconfirmed`, `claim_after_timeout`);
//! - fled, alive: the cache is abandoned (the entry empties);
//! - dead: the cache burned (the entry empties); the tombstone stays locked.
//!
//! The character is free once the party ends (`leave_party`), so it may be in a new party by
//! now: it is unlocked here only if it is still locked to this one, and nothing is granted if it
//! died since or if its entry burned at a later `join_session` (the deadline had passed).
//! Nothing goes into the bag here: the owner lays the cache out with "Done" (`close_cache`).
use anchor_lang::prelude::*;
use deadroll_types::{CacheSlot, CacheStatus, SessionStatus, PARTY_MAX};

use crate::{
    cpi_out::{self, SessionSigner},
    errors::SessionError,
    events::CacheSettled,
    loot::Caches,
    state::{external::read_character, CacheEntry, GameSession, Participation, RewardCache},
};

/// One player's accounts at the end of the party: the participation, the reward cache and the
/// character.
pub struct PlayerAccounts<'info> {
    pub participation: Box<Account<'info, Participation>>,
    pub cache: Box<Account<'info, RewardCache>>,
    pub character: &'info AccountInfo<'info>,
}

/// Player `slot`'s `(Participation, RewardCache, Character)`, all writable: the session's
/// participation, that character's reward caches, the character itself.
pub fn load_player<'info>(
    session: &GameSession,
    slot: u8,
    infos: &'info [AccountInfo<'info>],
) -> Result<PlayerAccounts<'info>> {
    require!(
        infos.len() == 3 && infos.iter().all(|i| i.is_writable),
        SessionError::InvalidSettleAccounts
    );
    let s = slot as usize;
    require!(
        *infos[0].key == session.participations[s] && *infos[2].key == session.core.players[s],
        SessionError::InvalidSettleAccounts
    );
    let participation = Box::new(Account::<Participation>::try_from(&infos[0])?);
    let cache = Box::new(Account::<RewardCache>::try_from(&infos[1])?);
    require!(
        cache.character == session.core.players[s],
        SessionError::InvalidRewardCache
    );
    Ok(PlayerAccounts {
        participation,
        cache,
        character: &infos[2],
    })
}

/// The caches of the players at hand, by slot (the settlement pays the kills still waiting).
pub struct SettleCaches<'p, 'info> {
    pub players: &'p mut [Option<PlayerAccounts<'info>>; PARTY_MAX],
}

impl Caches for SettleCaches<'_, '_> {
    fn add(&mut self, slot: u8, xp: u32, rewards: &[CacheSlot]) -> Result<bool> {
        Ok(
            match self.players.get_mut(slot as usize).and_then(|p| p.as_mut()) {
                Some(p) => p.participation.add_to_cache(xp, rewards) == 0,
                None => true,
            },
        )
    }
}

/// Settles player `slot` (see the module doc) and writes its participation and cache back.
pub fn settle_player<'info>(
    session: &GameSession,
    session_key: Pubkey,
    slot: u8,
    p: &mut PlayerAccounts<'info>,
    signer: &SessionSigner<'_, 'info>,
) -> Result<()> {
    let character = read_character(p.character)?;
    // Alive in this party and still alive (it may have died in a later one).
    let alive = session.player_alive(slot) && character.alive;
    let entry = p.cache.entry_mut(&session_key, CacheStatus::Open);
    // Stuck party: a party the DM left gives the living their cache too, as a peaceful exit
    // (docs/PARTY_AND_TURNS.md 3.2).
    let granted = alive
        && matches!(
            session.core.status,
            SessionStatus::Victory | SessionStatus::Abandoned
        )
        && entry.is_some();
    let xp = if granted { p.participation.cache_xp } else { 0 };
    if let Some(e) = entry {
        *e = if granted {
            CacheEntry {
                session: session_key,
                status: CacheStatus::Claimed,
                xp,
                slots: p.participation.cache_slots(),
            }
        } else {
            CacheEntry::EMPTY
        };
    }
    if granted && xp > 0 {
        cpi_out::character::grant_xp(signer, p.character, xp)?;
    }
    if alive && character.in_session == Some(session_key) {
        cpi_out::character::unlock(signer, p.character)?;
    }
    p.participation.exit(&crate::ID)?;
    p.cache.exit(&crate::ID)?;
    emit!(CacheSettled {
        session: session_key,
        character: *p.character.key,
        slot,
        xp,
        granted,
    });
    Ok(())
}

/// The DM's deadline: `ended_at` plus the map's time to settle (set by the DM).
pub fn deadline(session: &GameSession) -> i64 {
    session
        .ended_at
        .saturating_add(session.settle_timeout_secs as i64)
}

/// The party is over: Victory, Wiped, Fled, or Abandoned by its DM.
pub fn require_ended(session: &GameSession) -> Result<()> {
    require!(
        matches!(
            session.core.status,
            SessionStatus::Victory
                | SessionStatus::Fled
                | SessionStatus::Wiped
                | SessionStatus::Abandoned
        ),
        SessionError::NotFinished
    );
    Ok(())
}

/// Stuck party (docs/PARTY_AND_TURNS.md 3.2): a party whose DM was marked missing and never came
/// back, or came back without every living player's confirmation in time, ends `Abandoned` at the
/// time the missing-DM watch names (`deadroll_rules::timeout::DmPhase::ended_at`). The DM's
/// deadline counts from there as after any end. Nothing for any other party.
pub fn end_if_dm_left(session: &mut GameSession, session_key: Pubkey, now: i64) -> Result<()> {
    if session.core.status != SessionStatus::Active {
        return Ok(());
    }
    if let Some(ended_at) = session.dm_phase(now).ended_at() {
        session.core.status = SessionStatus::Abandoned;
        session.ended_at = ended_at;
        crate::outcomes::emit_if_ended(session, session_key)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{rules::TIMEOUTS, state::test_support::session};
    use deadroll_types::Cell;

    /// Stuck party: a party whose DM left ends `Abandoned` at the time the watch names; the
    /// deadline counts from there as after any end.
    #[test]
    fn a_party_the_dm_left_ends_abandoned() {
        let key = Pubkey::new_unique();
        let mut s = session(&[Cell::default()]);
        s.settle_timeout_secs = 600;
        s.dm_missing_at = 1_000;
        // Still counting down: nothing.
        end_if_dm_left(&mut s, key, 1_600).unwrap();
        assert_eq!(s.core.status, SessionStatus::Active);
        assert!(require_ended(&s).is_err());
        // Never came back: ended at the mark, the claims open after the countdown.
        end_if_dm_left(&mut s, key, 1_601).unwrap();
        assert_eq!(
            (s.core.status, s.ended_at),
            (SessionStatus::Abandoned, 1_000)
        );
        assert_eq!(deadline(&s), 1_600);
        require_ended(&s).unwrap();

        // Came back, the window closed without everyone: ended at the window's end.
        let mut s = session(&[Cell::default()]);
        s.settle_timeout_secs = 600;
        s.dm_missing_at = 1_000;
        s.dm_back_at = 1_100;
        let end = 1_100 + TIMEOUTS.dm_return_confirm_secs as i64;
        end_if_dm_left(&mut s, key, end).unwrap();
        assert_eq!(s.core.status, SessionStatus::Active, "the window is open");
        end_if_dm_left(&mut s, key, end + 1).unwrap();
        assert_eq!((s.core.status, s.ended_at), (SessionStatus::Abandoned, end));

        // A party that ended on its own stays as it ended.
        let mut s = session(&[Cell::default()]);
        s.core.status = SessionStatus::Victory;
        s.dm_missing_at = 1_000;
        end_if_dm_left(&mut s, key, 99_999).unwrap();
        assert_eq!(s.core.status, SessionStatus::Victory);
    }
}
