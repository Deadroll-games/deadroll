//! Arena update: who moves first in a pack's arena, the initiative timeline it runs on and the
//! experience of a won arena (docs/PARTY_AND_TURNS.md 4.1, 6; docs/CHARACTER.md 5).
//!
//! The geometry (which side a character came from, where the party stands) is
//! `deadroll_mapcheck::arena`, shared with the map builder.

use deadroll_combat::{
    modifiers::counter,
    support::{mod_power, ModKind},
    TimedMods,
};
use deadroll_types::{Facing, PARTY_MAX};

pub use deadroll_mapcheck::arena::{anchor, entry_cells, from_back, Approach, Sign};

use crate::{config::ArenaTuning, loot::on_monster_killed};

/// The side whose units start the arena timeline at 0.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FirstSide {
    Party,
    Monsters,
}

/// From the pack's back the party moves first; from the front and the sides the monsters do
/// (docs/PARTY_AND_TURNS.md 4.1.3; Ivan: the side counts as the front).
pub const fn first_side(facing: Facing, approach: Approach) -> FirstSide {
    if from_back(facing, approach) {
        FirstSide::Party
    } else {
        FirstSide::Monsters
    }
}

/// The time a turn adds to a unit's time until its next turn (docs/PARTY_AND_TURNS.md 6.1):
/// `time_scale / (initiative_base + initiative)`, at least 1.
pub fn delay(initiative: u16, t: &ArenaTuning) -> u32 {
    let div = (t.initiative_base as u32 + initiative as u32).max(1);
    (t.time_scale / div).max(1)
}

/// Initiative under the bearer's buffs and debuffs (docs/PARTY_AND_TURNS.md 6.2): a Slow divides
/// it as a counterweight of `slow_initiative_bp` of its power (COMBAT_MATH 3). Haste changes the
/// AP of a turn, not the order of turns (PARTY_AND_TURNS 5.1); Chill slows through speed only.
pub fn effective_initiative(initiative: u16, mods: &TimedMods, t: &ArenaTuning) -> u16 {
    let slow = mod_power(mods, ModKind::Slow) as u64 * t.slow_initiative_bp as u64 / 10_000;
    counter(initiative as u64, slow.min(u32::MAX as u64) as u32) as u16
}

/// The delay of a turn under `mods` (the bearer's own with the auras over it), taken when the turn
/// starts: an effect that lands later in the turn changes the delay from the next turn on
/// (docs/PARTY_AND_TURNS.md 6.2). A turn lost to Freeze or Stun takes it as well.
pub fn turn_delay(initiative: u16, mods: &TimedMods, t: &ArenaTuning) -> u32 {
    delay(effective_initiative(initiative, mods, t), t)
}

/// A unit's time when the arena starts: 0 on the side that moves first, its own delay on the
/// other, so every unit of the first side acts before any of the other.
pub fn start_time(first: bool, initiative: u16, t: &ArenaTuning) -> u32 {
    if first {
        0
    } else {
        delay(initiative, t)
    }
}

/// One unit on the timeline.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct TimelineUnit {
    /// Turn-order id (`crate::Unit::id`): players `0..PARTY_MAX`, then monsters.
    pub id: u8,
    pub time: u32,
    /// What wins a tie: a player's Dexterity, a monster's initiative (monsters have no
    /// Dexterity).
    pub tie: u16,
    pub alive: bool,
}

/// The unit that acts next: the living one with the lowest time; a tie goes to the higher `tie`,
/// then to the lower id (players before monsters, then by slot and index).
pub fn next_unit(units: &[TimelineUnit]) -> Option<usize> {
    let key = |u: &TimelineUnit| (u.time, u16::MAX - u.tie, u.id);
    units
        .iter()
        .enumerate()
        .filter(|(_, u)| u.alive)
        .min_by_key(|(_, u)| key(u))
        .map(|(i, _)| i)
}

/// The arena round a time falls in, from 1.
pub fn round_at(time: u32, t: &ArenaTuning) -> u16 {
    let round = time / t.round_time.max(1) + 1;
    round.min(u16::MAX as u32) as u16
}

/// The experience of a pack: the sum of its monsters' (`MonsterProfile::xp`).
pub fn pack_xp(monster_xp: impl IntoIterator<Item = u32>) -> u32 {
    monster_xp
        .into_iter()
        .fold(0u32, |sum, xp| sum.saturating_add(xp))
}

/// The experience of a won arena, by player slot: every living player (`alive`, bit per slot)
/// gets the pack's full experience times their share (`xp_bp`); it is not split
/// (docs/PARTY_AND_TURNS.md 4.1.4).
pub fn on_pack_won(pack_xp: u32, alive: u8, xp_bp: &[u16; PARTY_MAX]) -> [u32; PARTY_MAX] {
    on_monster_killed(pack_xp, alive, xp_bp)
}

#[cfg(test)]
mod tests {
    use super::*;
    use deadroll_types::Cell;

    const T: ArenaTuning = ArenaTuning::DEFAULT;

    fn unit(id: u8, time: u32, tie: u16) -> TimelineUnit {
        TimelineUnit {
            id,
            time,
            tie,
            alive: true,
        }
    }

    #[test]
    fn the_back_lets_the_party_move_first() {
        let monster = Cell { x: 5, y: 5 };
        let at = |x, y| Approach::between(monster, Cell { x, y }).unwrap();
        // Looking south: the north row is the back.
        for x in 4..=6 {
            assert_eq!(first_side(Facing::S, at(x, 4)), FirstSide::Party);
        }
        // The sides and the front: the monsters.
        for (x, y) in [(4, 5), (6, 5), (4, 6), (5, 6), (6, 6)] {
            assert_eq!(first_side(Facing::S, at(x, y)), FirstSide::Monsters);
        }
    }

    #[test]
    fn a_higher_initiative_waits_less() {
        assert_eq!(delay(0, &T), 200);
        assert_eq!(delay(6, &T), 100);
        assert_eq!(delay(10, &T), 75);
        assert!(delay(u16::MAX, &T) >= 1);
        let zero = ArenaTuning {
            time_scale: 0,
            initiative_base: 0,
            ..T
        };
        assert_eq!(delay(0, &zero), 1);
        assert_eq!(start_time(true, 6, &T), 0);
        assert_eq!(start_time(false, 6, &T), 100);
    }

    #[test]
    fn the_lowest_time_acts_ties_go_to_the_higher_tie_then_the_lower_id() {
        let mut units = [
            unit(0, 100, 5),
            unit(1, 75, 1),
            unit(4, 75, 9),
            unit(5, 75, 9),
        ];
        assert_eq!(next_unit(&units), Some(2));
        units[2].alive = false;
        assert_eq!(next_unit(&units), Some(3));
        units[3].alive = false;
        assert_eq!(next_unit(&units), Some(1));
        for u in units.iter_mut() {
            u.alive = false;
        }
        assert_eq!(next_unit(&units), None);
        assert_eq!(next_unit(&[]), None);
    }

    #[test]
    fn a_fast_unit_sometimes_acts_twice() {
        // Initiative 18 (delay 50) against 2 (delay 150), the fast one first.
        let mut units = [unit(0, 0, 0), unit(4, 0, 0)];
        let inits = [18, 2];
        let mut order = [0u8; 8];
        for o in order.iter_mut() {
            let i = next_unit(&units).unwrap();
            *o = units[i].id;
            units[i].time += delay(inits[i], &T);
        }
        assert_eq!(order, [0, 4, 0, 0, 0, 4, 0, 0]);
    }

    fn mods(kinds: &[(ModKind, u32)]) -> TimedMods {
        let mut out = TimedMods::default();
        for (m, &(kind, power_bp)) in out.iter_mut().zip(kinds) {
            m.kind = kind;
            m.turns = 2;
            m.power_bp = power_bp;
        }
        out
    }

    #[test]
    fn a_slow_divides_initiative_haste_and_others_do_not() {
        let none = mods(&[]);
        assert_eq!(effective_initiative(10, &none, &T), 10);
        assert_eq!(turn_delay(10, &none, &T), delay(10, &T));
        // A 100% Slow halves it, 300% quarters it, never below 0.
        assert_eq!(
            effective_initiative(10, &mods(&[(ModKind::Slow, 10_000)]), &T),
            5
        );
        assert_eq!(
            effective_initiative(12, &mods(&[(ModKind::Slow, 30_000)]), &T),
            3
        );
        assert_eq!(
            effective_initiative(0, &mods(&[(ModKind::Slow, 10_000)]), &T),
            0
        );
        assert_eq!(
            effective_initiative(9, &mods(&[(ModKind::Slow, u32::MAX)]), &T),
            0
        );
        assert_eq!(
            turn_delay(10, &mods(&[(ModKind::Slow, 10_000)]), &T),
            delay(5, &T)
        );
        // Haste and the other kinds leave the order alone.
        let other = mods(&[(ModKind::Haste, 10_000), (ModKind::Weakening, 10_000)]);
        assert_eq!(effective_initiative(10, &other, &T), 10);
        // The share of the Slow that reaches initiative is a balance number.
        let half = ArenaTuning {
            slow_initiative_bp: 5_000,
            ..T
        };
        assert_eq!(
            effective_initiative(12, &mods(&[(ModKind::Slow, 10_000)]), &half),
            8
        );
        let off = ArenaTuning {
            slow_initiative_bp: 0,
            ..T
        };
        assert_eq!(
            effective_initiative(12, &mods(&[(ModKind::Slow, 10_000)]), &off),
            12
        );
    }

    #[test]
    fn a_slowed_fast_unit_stops_acting_twice() {
        // Initiative 18 against 2: the fast one acts twice in a row, slowed 200% it does not.
        let slow = mods(&[(ModKind::Slow, 20_000)]);
        let none = mods(&[]);
        let run = |m: &TimedMods| {
            let mut units = [unit(0, 0, 0), unit(4, 0, 0)];
            let inits = [18, 2];
            let mut order = [0u8; 4];
            for o in order.iter_mut() {
                let i = next_unit(&units).unwrap();
                *o = units[i].id;
                let own = if i == 0 { m } else { &none };
                units[i].time += turn_delay(inits[i], own, &T);
            }
            order
        };
        // Delays 50 against 150; slowed, 100 against 150.
        assert_eq!(run(&none), [0, 4, 0, 0]);
        assert_eq!(run(&slow), [0, 4, 0, 4]);
    }

    #[test]
    fn rounds_count_round_time() {
        assert_eq!(round_at(0, &T), 1);
        assert_eq!(round_at(99, &T), 1);
        assert_eq!(round_at(100, &T), 2);
        assert_eq!(round_at(u32::MAX, &T), u16::MAX);
    }

    #[test]
    fn a_won_arena_pays_the_pack_to_every_living_player() {
        assert_eq!(pack_xp([30, 30, 55]), 115);
        assert_eq!(pack_xp([u32::MAX, 1]), u32::MAX);
        let shares = [10_000, 5_000, 10_000, 10_000];
        assert_eq!(on_pack_won(115, 0b0011, &shares), [115, 57, 0, 0]);
    }
}
