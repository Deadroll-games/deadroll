//! Retreat and Escape from an arena (docs/PARTY_AND_TURNS.md 7.5): one player proposes, the other
//! living players confirm on their turns (a vote spends the turn), any other action on a player's
//! turn cancels the proposal; when every living player voted, the arena's monsters move once more
//! and the survivors leave. A Retreat brings the party back to the map: no experience for the
//! fight, the pack heals, is on guard (it moves first) and is buffed for its next fight. An Escape
//! ends the party as in section 7.3.

use deadroll_combat::{
    support::{apply_mod, mod_turns, ModKind},
    TimedMod, TimedMods,
};
use deadroll_types::Facing;

use crate::{
    arena::{first_side, Approach, FirstSide},
    config::ArenaTuning,
};

/// What the players vote for. Stored as its `u8` (`0` = no proposal, [`NO_EXIT`]).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum ExitKind {
    /// From the arena back to the map.
    Retreat = 1,
    /// Out of the dungeon: no experience, no rewards.
    Escape = 2,
}

/// No proposal on the table.
pub const NO_EXIT: u8 = 0;

impl ExitKind {
    pub const fn from_u8(v: u8) -> Option<Self> {
        match v {
            1 => Some(Self::Retreat),
            2 => Some(Self::Escape),
            _ => None,
        }
    }
}

/// The proposal on the table: its kind (`NO_EXIT` = none) and a bit per player slot that voted.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Vote {
    pub kind: u8,
    pub mask: u8,
}

impl Vote {
    pub const NONE: Vote = Vote {
        kind: NO_EXIT,
        mask: 0,
    };
}

/// Player `slot` votes for `kind`: the first vote proposes it, a vote for the other kind replaces
/// the proposal (the earlier votes were for something else), a vote for the same kind confirms it.
pub fn vote(current: Vote, kind: ExitKind, slot: u8) -> Vote {
    let bit = 1u8 << slot;
    if current.kind == kind as u8 {
        Vote {
            kind: current.kind,
            mask: current.mask | bit,
        }
    } else {
        Vote {
            kind: kind as u8,
            mask: bit,
        }
    }
}

/// Every living player voted (the dead do not take part, docs/PARTY_AND_TURNS.md 7.3).
pub fn passed(mask: u8, alive: u8) -> bool {
    alive != 0 && mask & alive == alive
}

/// The monsters' last turns once the vote passed: the bit of the arena slot whose turn just ended
/// (`ended`, `None` for a player's turn) goes, and so does every dead monster's (`living`, bit per
/// arena slot). The party leaves when nothing is left.
pub fn after_last_move(last_moves: u8, ended: Option<u8>, living: u8) -> u8 {
    let done = ended.map_or(0, |slot| 1u8 << slot);
    last_moves & !done & living
}

/// One more retreat fits the party's limit.
pub fn can_retreat(used: u8, t: &ArenaTuning) -> bool {
    used < t.retreats_max
}

/// The side that moves first in a pack's arena: a pack on guard (the party retreated from it)
/// moves first from any side, otherwise the approach decides (`crate::arena::first_side`).
pub fn first_side_guarded(facing: Facing, approach: Approach, on_guard: bool) -> FirstSide {
    if on_guard {
        FirstSide::Monsters
    } else {
        first_side(facing, approach)
    }
}

/// The buff of a pack the party retreated from, put on each of its monsters (turn-order id
/// `source`) when its next fight starts: an Empowerment and a Protection from `ArenaTuning`, each
/// for `retreat_buff_turns` of the monster's own turns; a power of 0 adds nothing.
pub fn retreat_buff(mods: &mut TimedMods, source: u8, t: &ArenaTuning) {
    let turns = mod_turns(t.retreat_buff_turns);
    for (kind, power_bp) in [
        (ModKind::Empowerment, t.retreat_empower_bp),
        (ModKind::Protection, t.retreat_protect_bp),
    ] {
        if power_bp == 0 {
            continue;
        }
        apply_mod(
            mods,
            TimedMod {
                kind,
                turns,
                power_bp,
                source,
                ..TimedMod::default()
            },
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use deadroll_combat::support::mod_power;
    use deadroll_types::Cell;

    const T: ArenaTuning = ArenaTuning::DEFAULT;

    #[test]
    fn the_first_vote_proposes_the_same_kind_confirms_the_other_replaces() {
        let v = vote(Vote::NONE, ExitKind::Retreat, 1);
        assert_eq!(
            v,
            Vote {
                kind: ExitKind::Retreat as u8,
                mask: 0b0010
            }
        );
        let v = vote(v, ExitKind::Retreat, 3);
        assert_eq!(v.mask, 0b1010);
        // Voting again changes nothing.
        assert_eq!(vote(v, ExitKind::Retreat, 3), v);
        // An Escape vote replaces the Retreat proposal: only its own vote counts.
        let v = vote(v, ExitKind::Escape, 0);
        assert_eq!(
            v,
            Vote {
                kind: ExitKind::Escape as u8,
                mask: 0b0001
            }
        );
        assert_eq!(ExitKind::from_u8(NO_EXIT), None);
        assert_eq!(ExitKind::from_u8(1), Some(ExitKind::Retreat));
        assert_eq!(ExitKind::from_u8(2), Some(ExitKind::Escape));
        assert_eq!(ExitKind::from_u8(3), None);
    }

    #[test]
    fn it_passes_when_every_living_player_voted() {
        assert!(!passed(0b0001, 0b0011));
        assert!(passed(0b0011, 0b0011));
        // The dead do not vote; a dead voter's bit does not matter.
        assert!(passed(0b1110, 0b1110));
        assert!(passed(0b1111, 0b1110));
        assert!(!passed(0, 0));
    }

    #[test]
    fn each_living_monster_moves_once_more() {
        // Monsters 0, 2 and 3 alive when the vote passed.
        let m = 0b1101;
        // The voter's own turn ends: nothing changes.
        assert_eq!(after_last_move(m, None, 0b1101), m);
        assert_eq!(after_last_move(m, Some(2), 0b1101), 0b1001);
        // Monster 3 died meanwhile: its turn is gone too.
        assert_eq!(after_last_move(0b1001, Some(0), 0b0001), 0);
        // A monster that already moved does not come back.
        assert_eq!(after_last_move(0b1000, Some(0), 0b1001), 0b1000);
    }

    #[test]
    fn retreats_are_limited() {
        assert!(can_retreat(0, &T));
        assert!(can_retreat(T.retreats_max - 1, &T));
        assert!(!can_retreat(T.retreats_max, &T));
        let none = ArenaTuning {
            retreats_max: 0,
            ..T
        };
        assert!(!can_retreat(0, &none));
    }

    #[test]
    fn a_pack_on_guard_moves_first_even_from_its_back() {
        let monster = Cell { x: 5, y: 5 };
        let back = Approach::between(monster, Cell { x: 5, y: 4 }).unwrap();
        assert_eq!(first_side_guarded(Facing::S, back, false), FirstSide::Party);
        assert_eq!(
            first_side_guarded(Facing::S, back, true),
            FirstSide::Monsters
        );
        let front = Approach::between(monster, Cell { x: 5, y: 6 }).unwrap();
        assert_eq!(
            first_side_guarded(Facing::S, front, false),
            FirstSide::Monsters
        );
    }

    #[test]
    fn the_retreat_buff_empowers_and_protects_for_its_turns() {
        let mut mods = TimedMods::default();
        retreat_buff(&mut mods, 7, &T);
        assert_eq!(mod_power(&mods, ModKind::Empowerment), T.retreat_empower_bp);
        assert_eq!(mod_power(&mods, ModKind::Protection), T.retreat_protect_bp);
        let e = mods
            .iter()
            .find(|m| m.kind == ModKind::Empowerment)
            .unwrap();
        assert_eq!((e.turns, e.source), (mod_turns(T.retreat_buff_turns), 7));
        // A power of 0 adds nothing.
        let mut mods = TimedMods::default();
        let off = ArenaTuning {
            retreat_protect_bp: 0,
            ..T
        };
        retreat_buff(&mut mods, 7, &off);
        assert_eq!(mod_power(&mods, ModKind::Protection), 0);
        assert_eq!(mods.iter().filter(|m| !m.is_empty()).count(), 1);
    }
}
