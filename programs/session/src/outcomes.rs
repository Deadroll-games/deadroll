//! Deaths and the end of the party, shared by hits (`consume_randomness`) and over-time damage
//! (`start_turn`), so both take the same path. Patch 6: a callback only puts a player down; the
//! death queue is finalized by `start_turn` and the claims (`finalize_deaths`).
use anchor_lang::prelude::*;
use deadroll_types::{seeds, Monster, SessionStatus, PARTY_MAX};

use crate::{
    cpi_out::{self, character::DeathAccounts, SessionSigner},
    errors::SessionError,
    events::{ArenaWon, DeathFinalized, MonsterKilled, PlayerDied, SessionEnded},
    state::{ArenaMode, GameSession, Monsters},
};

/// Monster `index` reached 0 HP, killed by unit `by`: `MonsterKilled`, and the kill waits in
/// `GameSession.unpaid_kills` for its experience and rewards (`crate::loot::pay_kills`, at the end
/// of the callback or at the settlement). The boss wins the party (docs/PARTY_AND_TURNS.md, 7.1).
/// XP and the boss role come from its copied bestiary record. The caller writes `monster` back.
pub fn monster_killed(
    session: &mut GameSession,
    session_key: Pubkey,
    monsters: &Monsters,
    monster: &mut Monster,
    index: u8,
    by: u8,
    seed: [u8; 32],
) -> Result<()> {
    monster.alive = false;
    let profile = monsters.profile(index, monster.level)?;
    let (xp, boss) = (profile.xp, profile.boss);
    session.unpaid_kills |= 1 << index;
    emit!(MonsterKilled {
        session: session_key,
        monster: index,
        bestiary_id: monster.bestiary_id,
        by,
        xp,
        boss,
    });
    if boss {
        session.core.status = SessionStatus::Victory;
        session.victory_seed = seed;
    } else if session.mode == ArenaMode::Arena && last_of_pack(monsters, index)? {
        // Arena update: the pack is dead, `leave_arena` pays it and brings the party back
        // (docs/PARTY_AND_TURNS.md 4.1.4).
        session.mode = ArenaMode::Won;
        emit!(ArenaWon {
            session: session_key,
            pack: session.arena_pack,
        });
    }
    Ok(())
}

/// Every monster of the arena but `index` (dying now, written back by the caller) is dead.
fn last_of_pack(monsters: &Monsters, index: u8) -> Result<bool> {
    for i in 0..monsters.count()? {
        if i != index && monsters.get(i)?.alive {
            return Ok(false);
        }
    }
    Ok(true)
}

/// Player `slot` reached 0 HP, put down by unit `by` in a VRF callback: out of the living at once
/// (no turn, no target, no action), `PlayerDied`, and into the death queue
/// (`GameSession.pending_deaths`) that `start_turn` and the claims finalize before anything else
/// ([`finalize_deaths`]). No CPI here: the callback's compute budget is the oracle's. The last
/// living player wipes the party.
pub fn player_down(session: &mut GameSession, session_key: Pubkey, slot: u8, by: u8) {
    session.alive_mask &= !(1 << slot);
    session.pending_deaths |= 1 << slot;
    emit!(PlayerDied {
        session: session_key,
        character: session.core.players[slot as usize],
        slot,
        by,
    });
    if session.alive_mask == 0 {
        session.core.status = SessionStatus::Wiped;
    }
}

/// Player `slot`'s death is final: `character.mark_dead` (the tombstone; its skills, bag and
/// soulbound items burn), out of the queue, `DeathFinalized`.
pub fn finalize_death<'info>(
    session: &mut GameSession,
    session_key: Pubkey,
    slot: u8,
    signer: &SessionSigner<'_, 'info>,
    death: &DeathAccounts<'_, 'info>,
) -> Result<()> {
    cpi_out::character::mark_dead(signer, death)?;
    session.pending_deaths &= !(1 << slot);
    emit!(DeathFinalized {
        session: session_key,
        character: death.character.key(),
        slot,
    });
    Ok(())
}

/// Player `slot` (its `death` accounts at hand) reached 0 HP outside a callback, killed by unit
/// `by` (over-time damage in `start_turn`): down and final in the same instruction.
pub fn player_killed<'info>(
    session: &mut GameSession,
    session_key: Pubkey,
    slot: u8,
    by: u8,
    signer: &SessionSigner<'_, 'info>,
    death: &DeathAccounts<'_, 'info>,
) -> Result<()> {
    player_down(session, session_key, slot, by);
    finalize_death(session, session_key, slot, signer, death)
}

/// One pending death: the slot and the `(Character, SkillSet, Bag)` keys its finalization takes.
pub type DeathKeys = (u8, Pubkey, Pubkey, Pubkey);

/// The keys of every pending death, in slot order.
pub fn death_keys(session: &GameSession) -> ([DeathKeys; PARTY_MAX], usize) {
    let mut out = [(0, Pubkey::default(), Pubkey::default(), Pubkey::default()); PARTY_MAX];
    let mut n = 0;
    for slot in 0..(session.core.player_count as usize).min(PARTY_MAX) {
        if session.pending_deaths & (1 << slot) == 0 {
            continue;
        }
        let character = session.core.players[slot];
        let pda = |seed: &[u8]| {
            Pubkey::find_program_address(&[seed, character.as_ref()], &deadroll_character::ID).0
        };
        out[n] = (slot as u8, character, pda(seeds::SKILLS), pda(seeds::BAG));
        n += 1;
    }
    (out, n)
}

/// `got` (key, writable) holds exactly a writable `(Character, SkillSet, Bag)` triple per pending
/// death, in slot order.
pub fn check_death_accounts(expected: &[DeathKeys], got: &[(Pubkey, bool)]) -> Result<()> {
    require!(
        got.len() == expected.len() * 3,
        SessionError::InvalidDeathAccounts
    );
    for (i, (_, character, skill_set, bag)) in expected.iter().enumerate() {
        let want = [character, skill_set, bag];
        for (k, key) in want.iter().enumerate() {
            let (have, writable) = got[i * 3 + k];
            require!(
                have == **key && writable,
                SessionError::InvalidDeathAccounts
            );
        }
    }
    Ok(())
}

/// Finalizes every pending death before anything else (`start_turn`, `claim_rewards`,
/// `claim_escrow`): `remaining` is a `(Character, SkillSet, Bag)` triple per queued slot, in slot
/// order, writable (`InvalidDeathAccounts` otherwise). Nothing to do on an empty queue (then
/// `remaining` must be empty too).
pub fn finalize_deaths<'info>(
    session: &mut GameSession,
    session_key: Pubkey,
    signer: &SessionSigner<'_, 'info>,
    remaining: &[AccountInfo<'info>],
) -> Result<()> {
    let (keys, n) = death_keys(session);
    let mut got = [(Pubkey::default(), false); PARTY_MAX * 3];
    require!(
        remaining.len() <= got.len(),
        SessionError::InvalidDeathAccounts
    );
    for (g, info) in got.iter_mut().zip(remaining) {
        *g = (info.key(), info.is_writable);
    }
    check_death_accounts(&keys[..n], &got[..remaining.len()])?;
    for (i, (slot, ..)) in keys[..n].iter().enumerate() {
        let death = DeathAccounts {
            character: &remaining[i * 3],
            skill_set: &remaining[i * 3 + 1],
            bag: &remaining[i * 3 + 2],
        };
        finalize_death(session, session_key, *slot, signer, &death)?;
    }
    Ok(())
}

/// Emits `SessionEnded` when the party is over and stamps `GameSession.ended_at` (patch 11:
/// the DM's deadline to settle counts from here); returns whether it is over.
pub fn emit_if_ended(session: &mut GameSession, session_key: Pubkey) -> Result<bool> {
    if session.core.status == SessionStatus::Active {
        return Ok(false);
    }
    if session.ended_at == 0 {
        session.ended_at = Clock::get()?.unix_timestamp;
    }
    emit!(SessionEnded {
        session: session_key,
        status: session.core.status,
        round: session.core.round,
    });
    Ok(true)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::test_support::session;
    use deadroll_types::Cell;

    fn party(n: usize) -> GameSession {
        let mut s = session(&vec![Cell::default(); n]);
        s.alive_mask = (1 << n) - 1;
        for slot in 0..n {
            s.core.players[slot] = Pubkey::new_unique();
        }
        s
    }

    #[test]
    fn down_is_out_of_battle_and_queued() {
        let mut s = party(3);
        player_down(&mut s, Pubkey::new_unique(), 2, 4);
        player_down(&mut s, Pubkey::new_unique(), 0, 4);
        assert_eq!((s.alive_mask, s.pending_deaths), (0b010, 0b101));
        assert!(!s.player_alive(0) && s.player_alive(1));
        assert_eq!(s.core.status, SessionStatus::Active);
        player_down(&mut s, Pubkey::new_unique(), 1, 5);
        assert_eq!(
            s.core.status,
            SessionStatus::Wiped,
            "the last one down wipes the party"
        );
        assert_eq!(s.pending_deaths, 0b111);
    }

    #[test]
    fn death_accounts_are_the_queue_in_slot_order() {
        let mut s = party(4);
        s.pending_deaths = 0b1010;
        let (keys, n) = death_keys(&s);
        assert_eq!(n, 2);
        assert_eq!((keys[0].0, keys[1].0), (1, 3));
        assert_eq!(keys[0].1, s.core.players[1]);
        let pda = |seed: &[u8], c: &Pubkey| {
            Pubkey::find_program_address(&[seed, c.as_ref()], &deadroll_character::ID).0
        };
        assert_eq!(keys[1].2, pda(seeds::SKILLS, &s.core.players[3]));
        assert_eq!(keys[1].3, pda(seeds::BAG, &s.core.players[3]));
        let good: Vec<(Pubkey, bool)> = keys[..n]
            .iter()
            .flat_map(|k| [(k.1, true), (k.2, true), (k.3, true)])
            .collect();
        assert!(check_death_accounts(&keys[..n], &good).is_ok());
        // Missing, out of order, read-only, a foreign bag.
        let err = |got: &[(Pubkey, bool)]| check_death_accounts(&keys[..n], got).unwrap_err();
        let invalid: anchor_lang::error::Error = SessionError::InvalidDeathAccounts.into();
        assert_eq!(err(&good[..3]), invalid);
        let mut swapped = good.clone();
        swapped.swap(0, 3);
        assert_eq!(err(&swapped), invalid);
        let mut read_only = good.clone();
        read_only[4].1 = false;
        assert_eq!(err(&read_only), invalid);
        let mut foreign = good.clone();
        foreign[5].0 = pda(seeds::BAG, &s.core.players[0]);
        assert_eq!(err(&foreign), invalid);
        // An empty queue takes no accounts.
        assert!(check_death_accounts(&[], &[]).is_ok());
    }

    #[test]
    fn the_last_kill_of_the_pack_wins_the_arena() {
        use crate::state::{
            test_support::{ids, monster, monsters_account},
            ArenaMode,
        };
        let key = Pubkey::new_unique();
        let mut account = monsters_account(key, &[monster(1, 1), monster(2, 1)]);
        let info = account.info();
        let monsters = Monsters::load(&info, &key).unwrap();
        let mut s = party(1);
        let mut first = monsters.get(0).unwrap();
        monster_killed(&mut s, key, &monsters, &mut first, 0, 0, [1; 32]).unwrap();
        monsters.set(0, &first).unwrap();
        assert_eq!(s.mode, ArenaMode::Arena);
        let mut second = monsters.get(1).unwrap();
        monster_killed(&mut s, key, &monsters, &mut second, 1, 0, [1; 32]).unwrap();
        assert_eq!(s.mode, ArenaMode::Won);
        assert_eq!(s.unpaid_kills, 0b11);
        assert_eq!(s.core.status, SessionStatus::Active);
        // The boss wins the party, the arena does not matter.
        let mut boss = monster(1, 1);
        boss.bestiary_id = ids::GOBLIN_CHIEFTAIN;
        let mut account = monsters_account(key, &[boss, monster(2, 1)]);
        let info = account.info();
        let monsters = Monsters::load(&info, &key).unwrap();
        let mut s = party(1);
        let mut boss = monsters.get(0).unwrap();
        monster_killed(&mut s, key, &monsters, &mut boss, 0, 0, [2; 32]).unwrap();
        assert_eq!(s.core.status, SessionStatus::Victory);
        assert_eq!(s.mode, ArenaMode::Arena);
    }
}
