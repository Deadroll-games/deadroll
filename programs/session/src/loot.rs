//! Patch 11: the reward events of a party (docs/PARTY_AND_TURNS.md 8-9, docs/BESTIARY.md 9) over
//! `deadroll_rules::loot`, written into each player's temporary cache (`Participation.cache`).
//!
//! - A kill marks the monster in `GameSession.unpaid_kills` (`outcomes::monster_killed`); the end
//!   of the same VRF callback pays it ([`pay_kills`]): the monster's full experience to every
//!   living player (their share applied) and a body reward to each one whose rewards are rolled,
//!   then the boss table with its guarantee. A kill outside a callback (over-time damage in
//!   `start_turn`, which has no party accounts) waits for the next callback or the settlement.
//! - A chest's roll is its own VRF request (`open_chest`): [`open_chest`].
//!
//! Every die comes from the VRF value of the action that pays (CLAUDE.md rule 10), each event on
//! its own stretch of the stream, after the combat rolls.
use anchor_lang::prelude::*;
use deadroll_rules::loot::{on_monster_killed, reward_event, LootTable, RewardRoll};
use deadroll_types::{CacheSlot, RewardSource, MONSTERS_MAX, NO_TABLE, PARTY_MAX};

use crate::{
    errors::SessionError,
    events::{ChestOpened, RewardsRolled},
    party::Party,
    rules::BALANCE,
    state::{GameSession, Monsters, Participation},
    vrf::SyscallRoll,
};

/// First roll index of the reward events in an action's stream (far past the combat rolls).
pub const LOOT_ROLL_BASE: u32 = 1_000_000;
/// Roll indexes one reward event may use (a level and two rewards per player at most).
pub const LOOT_ROLL_STRIDE: u32 = 64;
/// Event numbers in the stream: a monster's body `2 * index`, its boss table `2 * index + 1`,
/// a chest `CHEST_EVENT + chest`.
const CHEST_EVENT: u32 = 2 * MONSTERS_MAX as u32;

/// Where the rewards of an event go: the players' caches at hand.
pub trait Caches {
    /// Adds to player `slot`'s cache; `Ok(false)` if the cache was full and something was lost.
    /// A slot whose participation is not at hand is skipped (`Ok(true)`).
    fn add(&mut self, slot: u8, xp: u32, rewards: &[CacheSlot]) -> Result<bool>;
}

/// The caches of a VRF callback: its own participation and the other living players'.
pub struct CallbackCaches<'p, 'a, 'info> {
    pub main: &'p mut Participation,
    pub party: &'p mut Party<'a, 'info>,
}

impl Caches for CallbackCaches<'_, '_, '_> {
    fn add(&mut self, slot: u8, xp: u32, rewards: &[CacheSlot]) -> Result<bool> {
        if slot == self.main.core.slot {
            return Ok(self.main.add_to_cache(xp, rewards) == 0);
        }
        if !self.party.has(slot) {
            return Ok(true);
        }
        Ok(self.party.add_to_cache(slot, xp, rewards)? == 0)
    }
}

fn roll_stream(seed: [u8; 32], event: u32) -> SyscallRoll {
    SyscallRoll::at(seed, LOOT_ROLL_BASE + event * LOOT_ROLL_STRIDE)
}

/// Writes one event into the caches and emits `RewardsRolled`.
#[allow(clippy::too_many_arguments)]
fn deliver(
    session: &GameSession,
    session_key: Pubkey,
    source: RewardSource,
    index: u8,
    table: u8,
    roll: &RewardRoll,
    xp: &[u32; PARTY_MAX],
    caches: &mut impl Caches,
) -> Result<()> {
    let mut lost = 0u8;
    for slot in 0..session.core.player_count.min(PARTY_MAX as u8) {
        let s = slot as usize;
        let rewards = &roll.rewards[s];
        if xp[s] == 0 && rewards.iter().all(|r| r.is_empty()) {
            continue;
        }
        if !caches.add(slot, xp[s], rewards)? {
            lost |= 1 << slot;
        }
    }
    emit!(RewardsRolled {
        session: session_key,
        source,
        index,
        table,
        level: roll.level,
        xp: *xp,
        rewards: roll.rewards,
        lost,
    });
    Ok(())
}

/// Pays the kills in `kills` (bit per monster index), in index order, from `seed`. The caller
/// clears them from `GameSession.unpaid_kills` once every cache got its part (the claims after
/// the DM's deadline pay one player at a time from the same seed).
pub fn pay_kills(
    session: &mut GameSession,
    session_key: Pubkey,
    monsters: &Monsters,
    kills: u32,
    seed: [u8; 32],
    caches: &mut impl Caches,
) -> Result<()> {
    let step = BALANCE.loot.level_step_bp;
    for index in 0..MONSTERS_MAX as u8 {
        if kills & (1 << index) == 0 {
            continue;
        }
        let monster = monsters.get(index)?;
        let profile = monsters.profile(index, monster.level)?;
        // Experience: every living player, the full amount times their share (Ivan's decision,
        // patch 11); `on_monster_killed` is the one place that becomes "per pack" later.
        let alive = session.alive_mask & session.full_mask();
        let xp = on_monster_killed(profile.xp, alive, &session.xp_bp);
        let eligible = session.loot_eligible();
        // The body (`profile.loot_table` is `NO_TABLE` for a summoned helper).
        let body = match monsters.loot_table(profile.loot_table)? {
            Some(t) => reward_event(
                RewardSource::Body,
                &t,
                monster.level,
                eligible,
                step,
                &mut roll_stream(seed, 2 * index as u32),
            ),
            None => RewardRoll::default(),
        };
        deliver(
            session,
            session_key,
            RewardSource::Body,
            index,
            profile.loot_table,
            &body,
            &xp,
            caches,
        )?;
        if profile.boss {
            // The boss table, with its guarantee (docs/PARTY_AND_TURNS.md 9.4).
            let table = session.boss_table;
            let t: LootTable = monsters
                .loot_table(table)?
                .ok_or(SessionError::InvalidRewardTables)?;
            let roll = reward_event(
                RewardSource::Boss,
                &t,
                0,
                eligible,
                step,
                &mut roll_stream(seed, 2 * index as u32 + 1),
            );
            deliver(
                session,
                session_key,
                RewardSource::Boss,
                index,
                table,
                &roll,
                &[0; PARTY_MAX],
                caches,
            )?;
        }
    }
    Ok(())
}

/// Compute units a VRF callback keeps before it pays out one more kill: one kill to 4 players
/// with its boss table, then the party write-back and the end of the callback. Measured: a kill
/// paid out to 4 players costs about 42k (programs/session/tests/session.test.ts).
pub const PAYOUT_RESERVE_CU: u64 = 75_000;

/// `pay_kills` for the waiting kills (`GameSession.unpaid_kills`), lowest index first, one at a time
/// while more than [`PAYOUT_RESERVE_CU`] remain of the callback's budget (the oracle's 256k);
/// the rest wait. Which VRF value pays a kill changes no rule: a cache counts at the settlement.
pub fn pay_kills_within_budget(
    session: &mut GameSession,
    session_key: Pubkey,
    monsters: &Monsters,
    seed: [u8; 32],
    caches: &mut impl Caches,
) -> Result<()> {
    for index in 0..MONSTERS_MAX as u32 {
        let bit = 1u32 << index;
        if session.unpaid_kills & bit == 0 {
            continue;
        }
        if solana_program::compute_units::sol_remaining_compute_units() < PAYOUT_RESERVE_CU {
            break;
        }
        pay_kills(session, session_key, monsters, bit, seed, caches)?;
        session.unpaid_kills &= !bit;
    }
    Ok(())
}

/// Chest `chest` opens for player `slot`: one level for the party, a reward for each living
/// player whose rewards are rolled (docs/PARTY_AND_TURNS.md 9.3); it no longer occupies its cell.
pub fn open_chest(
    session: &mut GameSession,
    session_key: Pubkey,
    monsters: &Monsters,
    chest: u8,
    slot: u8,
    seed: [u8; 32],
    caches: &mut impl Caches,
) -> Result<()> {
    let point = *session
        .chest_list()
        .get(chest as usize)
        .ok_or(SessionError::InvalidChest)?;
    let t = monsters
        .loot_table(point.data)?
        .ok_or(SessionError::InvalidRewardTables)?;
    let roll = reward_event(
        RewardSource::Chest,
        &t,
        0,
        session.loot_eligible(),
        BALANCE.loot.level_step_bp,
        &mut roll_stream(seed, CHEST_EVENT + chest as u32),
    );
    session.chests_opened |= 1 << chest;
    deliver(
        session,
        session_key,
        RewardSource::Chest,
        chest,
        point.data,
        &roll,
        &[0; PARTY_MAX],
        caches,
    )?;
    emit!(ChestOpened {
        session: session_key,
        chest,
        slot,
    });
    Ok(())
}

/// The reward tables a party needs: each spawn's body table, each chest's and the boss's
/// (registry ids, `NO_TABLE` skipped), each once.
pub fn needed_tables(
    body: impl Iterator<Item = u8>,
    chests: impl Iterator<Item = u8>,
    boss: u8,
) -> impl Iterator<Item = u8> {
    body.chain(chests)
        .chain(core::iter::once(boss))
        .filter(|id| *id != NO_TABLE)
}

#[cfg(test)]
mod tests {
    use super::*;
    use deadroll_types::RewardKind;

    /// A cache per slot, in memory.
    #[derive(Default)]
    struct Mem {
        xp: [u32; PARTY_MAX],
        got: [Vec<CacheSlot>; PARTY_MAX],
    }

    impl Caches for Mem {
        fn add(&mut self, slot: u8, xp: u32, rewards: &[CacheSlot]) -> Result<bool> {
            self.xp[slot as usize] += xp;
            self.got[slot as usize].extend(rewards.iter().filter(|r| !r.is_empty()));
            Ok(true)
        }
    }

    #[test]
    fn deliver_skips_empty_slots_and_counts_xp() {
        let mut s = crate::state::test_support::session(&[Default::default(); 3]);
        s.core.player_count = 3;
        let ore = CacheSlot {
            kind: RewardKind::Resource,
            id: 2,
            qty: 1,
        };
        let mut roll = RewardRoll::default();
        roll.rewards[0][0] = ore;
        roll.rewards[2][1] = ore;
        let mut mem = Mem::default();
        deliver(
            &s,
            Pubkey::new_unique(),
            RewardSource::Body,
            0,
            1,
            &roll,
            &[5, 5, 0, 0],
            &mut mem,
        )
        .unwrap();
        assert_eq!(mem.xp, [5, 5, 0, 0]);
        assert_eq!(mem.got[0], vec![ore]);
        assert!(mem.got[1].is_empty());
        assert_eq!(mem.got[2], vec![ore]);
    }

    #[test]
    fn events_use_their_own_stretch_of_the_stream() {
        // The last event's stretch ends before the next action's would collide: index u32 room.
        let last = CHEST_EVENT + crate::state::NO_CHEST as u32;
        assert!(LOOT_ROLL_BASE
            .checked_add((last + 1) * LOOT_ROLL_STRIDE)
            .is_some());
    }

    #[test]
    fn needed_tables_skip_none() {
        let ids: Vec<u8> =
            needed_tables([4, NO_TABLE, 4].into_iter(), [0].into_iter(), 1).collect();
        assert_eq!(ids, vec![4, 4, 0, 1]);
    }
}
