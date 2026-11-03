//! A party that would hang (docs/PARTY_AND_TURNS.md 3.2, 5.3; docs/MAP_DESCRIPTION.md 8.5):
//!
//! - **The DM is missing.** A reveal the DM owes stays open for `reveal_timeout_secs`: any living
//!   player may mark the DM as missing. Nothing is paid then; the party freezes and the map's
//!   issue deadline (`settle_timeout_secs`) counts down. The DM may come back before it ends
//!   ("I'm back"); then every living player has `dm_return_confirm_secs` to confirm, and when all
//!   did the party goes on. If not all confirmed in time, the DM settles the party without the
//!   escrow; after the DM's deadline each player claims their own. If the DM never came back, each
//!   player claims their own once the countdown ended.
//! - **A unit does not act.** After `turn_timeout_secs` without an action any participant may
//!   skip its turn.
//! - **The VRF oracle does not answer.** After `vrf_retry_slots` any participant may request a new
//!   value for the same action.
//!
//! Times are unix seconds, slots are Solana slots. Integers only.

use crate::config::TimeoutTuning;

/// What a party records about a missing DM (`GameSession.dm_missing_at`, `dm_back_at`,
/// `dm_confirm_mask`). `0` = not set.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct DmWatch {
    /// When a player marked the DM as missing.
    pub missing_at: i64,
    /// When the DM said "I'm back".
    pub back_at: i64,
    /// Bit per player slot that confirmed the return.
    pub confirmed: u8,
}

/// Where a party stands with its DM.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DmPhase {
    /// Nobody marked the DM as missing: the party plays.
    Present,
    /// Marked missing; the DM may come back until `until` (inclusive). The party is frozen.
    Missing { until: i64 },
    /// The DM did not come back in time: the party ended at `ended_at` (the mark), and the
    /// countdown is its deadline. Each player claims their own.
    Gone { ended_at: i64 },
    /// The DM came back; the living players confirm until `until` (inclusive). Still frozen.
    Back { until: i64 },
    /// Not every living player confirmed in time: the party ended at `ended_at` (the end of the
    /// window). The DM settles it before `ended_at + settle_timeout_secs`, the players claim after.
    Unconfirmed { ended_at: i64 },
}

impl DmPhase {
    /// No move and no turn goes through.
    pub fn frozen(&self) -> bool {
        !matches!(self, DmPhase::Present)
    }

    /// The party is over because of the DM, and when it ended.
    pub fn ended_at(&self) -> Option<i64> {
        match *self {
            DmPhase::Gone { ended_at } | DmPhase::Unconfirmed { ended_at } => Some(ended_at),
            DmPhase::Present | DmPhase::Missing { .. } | DmPhase::Back { .. } => None,
        }
    }
}

/// Why a step of the missing-DM flow is refused.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DmWatchError {
    /// The DM owes no reveal.
    NothingOwed,
    /// The reveal is not old enough yet.
    TooEarly,
    /// The step does not fit where the party stands.
    WrongPhase,
    /// This player already confirmed.
    AlreadyConfirmed,
}

impl DmWatch {
    pub const PRESENT: DmWatch = DmWatch {
        missing_at: 0,
        back_at: 0,
        confirmed: 0,
    };

    /// Where the party stands at `now`, with the map's issue deadline `settle_timeout_secs`.
    pub fn phase(&self, now: i64, settle_timeout_secs: u32, t: &TimeoutTuning) -> DmPhase {
        if self.missing_at == 0 {
            return DmPhase::Present;
        }
        if self.back_at == 0 {
            let until = after(self.missing_at, settle_timeout_secs);
            return if now <= until {
                DmPhase::Missing { until }
            } else {
                DmPhase::Gone {
                    ended_at: self.missing_at,
                }
            };
        }
        let until = after(self.back_at, t.dm_return_confirm_secs);
        if now <= until {
            DmPhase::Back { until }
        } else {
            DmPhase::Unconfirmed { ended_at: until }
        }
    }

    /// A living player marks the DM as missing: a reveal is owed since `owed_at` (`None`: nothing
    /// owed) and is at least `reveal_timeout_secs` old.
    pub fn mark(
        &mut self,
        owed_at: Option<i64>,
        now: i64,
        settle_timeout_secs: u32,
        t: &TimeoutTuning,
    ) -> Result<(), DmWatchError> {
        if self.phase(now, settle_timeout_secs, t) != DmPhase::Present {
            return Err(DmWatchError::WrongPhase);
        }
        let owed_at = owed_at.ok_or(DmWatchError::NothingOwed)?;
        if !reveal_overdue(owed_at, now, t) {
            return Err(DmWatchError::TooEarly);
        }
        *self = DmWatch {
            missing_at: now,
            back_at: 0,
            confirmed: 0,
        };
        Ok(())
    }

    /// The DM says "I'm back" before the countdown ends.
    pub fn come_back(
        &mut self,
        now: i64,
        settle_timeout_secs: u32,
        t: &TimeoutTuning,
    ) -> Result<(), DmWatchError> {
        match self.phase(now, settle_timeout_secs, t) {
            DmPhase::Missing { .. } => {
                self.back_at = now;
                self.confirmed = 0;
                Ok(())
            }
            _ => Err(DmWatchError::WrongPhase),
        }
    }

    /// Living player `slot` confirms the DM's return; `living` is a bit per living player.
    /// Returns `true` when every living player confirmed: the party goes on (the caller resets
    /// the watch to [`DmWatch::PRESENT`]).
    pub fn confirm(
        &mut self,
        slot: u8,
        living: u8,
        now: i64,
        settle_timeout_secs: u32,
        t: &TimeoutTuning,
    ) -> Result<bool, DmWatchError> {
        if !matches!(
            self.phase(now, settle_timeout_secs, t),
            DmPhase::Back { .. }
        ) {
            return Err(DmWatchError::WrongPhase);
        }
        let bit = 1u8.checked_shl(slot as u32).unwrap_or(0);
        if bit == 0 || living & bit == 0 {
            return Err(DmWatchError::WrongPhase);
        }
        if self.confirmed & bit != 0 {
            return Err(DmWatchError::AlreadyConfirmed);
        }
        self.confirmed |= bit;
        Ok(self.confirmed & living == living)
    }
}

/// `at + secs`, saturating.
const fn after(at: i64, secs: u32) -> i64 {
    at.saturating_add(secs as i64)
}

/// The reveal owed since `owed_at` is at least `reveal_timeout_secs` old at `now`.
pub fn reveal_overdue(owed_at: i64, now: i64, t: &TimeoutTuning) -> bool {
    now >= after(owed_at, t.reveal_timeout_secs)
}

/// The current unit did nothing since `clock` (its turn's start or its last action) for at least
/// `turn_timeout_secs`: any participant may skip its turn.
pub fn turn_overdue(clock: i64, now: i64, t: &TimeoutTuning) -> bool {
    now >= after(clock, t.turn_timeout_secs)
}

/// The VRF request sent in `requested_slot` waited at least `vrf_retry_slots`: any participant
/// may send a new one.
pub fn vrf_retry_due(requested_slot: u64, slot: u64, t: &TimeoutTuning) -> bool {
    slot >= requested_slot.saturating_add(t.vrf_retry_slots as u64)
}

#[cfg(test)]
mod tests {
    use super::*;

    const T: TimeoutTuning = TimeoutTuning {
        reveal_timeout_secs: 300,
        dm_return_confirm_secs: 1_000,
        turn_timeout_secs: 60,
        vrf_retry_slots: 75,
    };
    const SETTLE: u32 = 600;

    #[test]
    fn a_reveal_owed_long_enough_lets_a_player_mark_the_dm() {
        let mut w = DmWatch::PRESENT;
        assert_eq!(w.phase(5_000, SETTLE, &T), DmPhase::Present);
        assert_eq!(
            w.mark(None, 5_000, SETTLE, &T),
            Err(DmWatchError::NothingOwed)
        );
        assert_eq!(
            w.mark(Some(1_000), 1_299, SETTLE, &T),
            Err(DmWatchError::TooEarly)
        );
        assert_eq!(w, DmWatch::PRESENT);
        assert_eq!(w.mark(Some(1_000), 1_300, SETTLE, &T), Ok(()));
        assert_eq!(w.missing_at, 1_300);
        assert_eq!(
            w.mark(Some(1_000), 1_400, SETTLE, &T),
            Err(DmWatchError::WrongPhase),
            "once"
        );
    }

    #[test]
    fn the_countdown_is_the_maps_issue_deadline() {
        let w = DmWatch {
            missing_at: 1_000,
            ..DmWatch::PRESENT
        };
        assert_eq!(
            w.phase(1_600, SETTLE, &T),
            DmPhase::Missing { until: 1_600 }
        );
        assert!(w.phase(1_000, SETTLE, &T).frozen());
        assert_eq!(w.phase(1_600, SETTLE, &T).ended_at(), None);
        let gone = w.phase(1_601, SETTLE, &T);
        assert_eq!(gone, DmPhase::Gone { ended_at: 1_000 });
        // The claims open when `ended_at + settle` has passed: the end of the countdown.
        assert_eq!(gone.ended_at(), Some(1_000));
    }

    #[test]
    fn the_dm_comes_back_only_before_the_countdown_ends() {
        let marked = DmWatch {
            missing_at: 1_000,
            ..DmWatch::PRESENT
        };
        let mut late = marked;
        assert_eq!(
            late.come_back(1_601, SETTLE, &T),
            Err(DmWatchError::WrongPhase)
        );
        let mut w = marked;
        assert_eq!(w.come_back(1_600, SETTLE, &T), Ok(()));
        assert_eq!(w.phase(1_600, SETTLE, &T), DmPhase::Back { until: 2_600 });
        assert!(w.phase(2_600, SETTLE, &T).frozen());
        assert_eq!(
            w.come_back(1_700, SETTLE, &T),
            Err(DmWatchError::WrongPhase),
            "once"
        );
        let mut present = DmWatch::PRESENT;
        assert_eq!(
            present.come_back(1_000, SETTLE, &T),
            Err(DmWatchError::WrongPhase)
        );
    }

    #[test]
    fn every_living_player_confirms_and_the_party_goes_on() {
        let mut w = DmWatch {
            missing_at: 1_000,
            back_at: 1_200,
            confirmed: 0,
        };
        // Slots 0 and 2 live, slot 1 is dead.
        let living = 0b101;
        assert_eq!(
            w.confirm(1, living, 1_300, SETTLE, &T),
            Err(DmWatchError::WrongPhase),
            "the dead do not vote"
        );
        assert_eq!(w.confirm(0, living, 1_300, SETTLE, &T), Ok(false));
        assert_eq!(
            w.confirm(0, living, 1_300, SETTLE, &T),
            Err(DmWatchError::AlreadyConfirmed)
        );
        assert_eq!(w.confirm(2, living, 2_200, SETTLE, &T), Ok(true));
        assert_eq!(
            w.confirm(9, living, 1_300, SETTLE, &T),
            Err(DmWatchError::WrongPhase)
        );
    }

    #[test]
    fn a_window_missed_ends_the_party_and_the_dm_settles_first() {
        let mut w = DmWatch {
            missing_at: 1_000,
            back_at: 1_200,
            confirmed: 0b001,
        };
        let phase = w.phase(2_201, SETTLE, &T);
        assert_eq!(phase, DmPhase::Unconfirmed { ended_at: 2_200 });
        assert_eq!(phase.ended_at(), Some(2_200));
        assert_eq!(
            w.confirm(2, 0b101, 2_201, SETTLE, &T),
            Err(DmWatchError::WrongPhase),
            "too late"
        );
        // Unconfirmed stays so after the DM's deadline too: the claims take it from there.
        assert_eq!(
            w.phase(10_000, SETTLE, &T),
            DmPhase::Unconfirmed { ended_at: 2_200 }
        );
    }

    #[test]
    fn turn_and_vrf_waits() {
        assert!(!turn_overdue(100, 159, &T));
        assert!(turn_overdue(100, 160, &T));
        assert!(!vrf_retry_due(1_000, 1_074, &T));
        assert!(vrf_retry_due(1_000, 1_075, &T));
        assert!(!reveal_overdue(0, 299, &T) && reveal_overdue(0, 300, &T));
        // Saturating: no overflow at the edges.
        assert!(!turn_overdue(i64::MAX, i64::MAX - 1, &T));
        assert!(!vrf_retry_due(u64::MAX, u64::MAX - 1, &T));
        assert!(vrf_retry_due(u64::MAX, u64::MAX, &T));
    }
}
