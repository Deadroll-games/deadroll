use anchor_lang::prelude::*;
use deadroll_types::{CacheSlot, CacheStatus, CACHE_SLOTS};

/// `["cache", character]` (patch 11): a character's temporary cache once its party is settled,
/// until "Done" in the bag screen (`close_cache`). During the party the cache lives in
/// `Participation` (docs/PARTY_AND_TURNS.md 8, 10.2); this account outlives the party and is found
/// by the character alone, so the next `join_session` burns a cache left open.
///
/// Created at `join_session` with the player's rent; a later `join_session` of the same
/// character rebinds it to the new party (what it held burns). Closed by `close_cache`, the rent
/// back to the owner.
#[account]
#[derive(InitSpace)]
pub struct RewardCache {
    pub version: u8,
    pub character: Pubkey,
    /// The party it belongs to.
    pub session: Pubkey,
    /// `Open` while the party runs (empty here), `Claimed` once settled: then it is the owner's
    /// to lay out.
    pub status: CacheStatus,
    /// Experience granted with the settlement (shown on the summary).
    pub xp: u32,
    pub slots: [CacheSlot; CACHE_SLOTS],
    pub bump: u8,
}
