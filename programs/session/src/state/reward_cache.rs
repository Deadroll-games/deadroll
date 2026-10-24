use anchor_lang::prelude::*;
use deadroll_types::{CacheSlot, CacheStatus, CACHE_SLOTS};

/// Parties of one character that may wait for their rewards at the same time: the character is
/// free when a party ends (`leave_party`), so it can be in a new party while an old one is not
/// settled yet.
pub const CACHE_PARTIES: usize = 3;

/// One party's rewards in a [`RewardCache`].
#[derive(
    AnchorSerialize, AnchorDeserialize, InitSpace, Clone, Copy, Debug, Default, PartialEq, Eq,
)]
pub struct CacheEntry {
    /// The party.
    pub session: Pubkey,
    pub status: CacheStatus,
    /// Experience granted with the settlement (shown on the summary).
    pub xp: u32,
    /// `Claimed`: the rewards, the owner's to lay out with "Done".
    pub slots: [CacheSlot; CACHE_SLOTS],
}

impl CacheEntry {
    pub const EMPTY: CacheEntry = CacheEntry {
        session: Pubkey::new_from_array([0; 32]),
        status: CacheStatus::Empty,
        xp: 0,
        slots: [CacheSlot::EMPTY; CACHE_SLOTS],
    };
}

/// `["cache", character]` (patch 11): a character's temporary caches once its parties end, until
/// "Done" in the bag screen (`close_cache`). During a party the cache lives in `Participation`
/// (docs/PARTY_AND_TURNS.md 8, 10.2); its entry here is `Open`, and the settlement turns it into
/// `Claimed` (or empties it). Found by the character alone, so `join_session` burns what is no
/// longer safe to keep: a `Claimed` entry (the owner did not press "Done"), and an `Open` one whose
/// party ended and whose DM deadline passed (the player could have claimed it).
///
/// Created at the character's first `join_session` with the player's rent; closed by
/// `close_cache` once every entry is empty, the rent back to the owner.
#[account]
#[derive(InitSpace)]
pub struct RewardCache {
    pub version: u8,
    pub character: Pubkey,
    pub entries: [CacheEntry; CACHE_PARTIES],
    pub bump: u8,
}

impl RewardCache {
    /// The entry of party `session` with status `status`.
    pub fn entry_mut(&mut self, session: &Pubkey, status: CacheStatus) -> Option<&mut CacheEntry> {
        self.entries
            .iter_mut()
            .find(|e| e.status == status && e.session == *session)
    }

    pub fn all_empty(&self) -> bool {
        self.entries.iter().all(|e| e.status == CacheStatus::Empty)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finds_an_entry_by_party_and_status() {
        let (a, b) = (Pubkey::new_unique(), Pubkey::new_unique());
        let mut c = RewardCache {
            version: 1,
            character: Pubkey::new_unique(),
            entries: [CacheEntry::EMPTY; CACHE_PARTIES],
            bump: 0,
        };
        assert!(c.all_empty());
        c.entries[1] = CacheEntry {
            session: a,
            status: CacheStatus::Open,
            ..CacheEntry::EMPTY
        };
        assert!(c.entry_mut(&a, CacheStatus::Open).is_some());
        assert!(c.entry_mut(&a, CacheStatus::Claimed).is_none());
        assert!(c.entry_mut(&b, CacheStatus::Open).is_none());
        assert!(!c.all_empty());
    }
}
