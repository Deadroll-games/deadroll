//! Session-local game rules: unit ids, action points, step costs.
//!
//! Numbers here are placeholders until balancing (docs have none yet). Each one sits behind a
//! single function so the registry tables (L4) can replace it without touching instructions.
use deadroll_types::{Class, Race, Tile, PARTY_MAX};

/// Minimum party price: 0.01 SOL (docs/PARTY_AND_TURNS.md, section 3.1).
pub const MIN_PRICE_LAMPORTS: u64 = 10_000_000;
/// Max steps in one `move_unit` / `monster_move`.
pub const MAX_PATH: usize = 16;
/// Unused `turn_order` entry.
pub const EMPTY_UNIT: u8 = 0xFF;
/// `turn_order` ids `0..PARTY_MAX` are player slots, `PARTY_MAX + i` is monster `i`.
pub const MONSTER_UNIT_BASE: u8 = PARTY_MAX as u8;

/// A participant in the turn order. The id is also `ActiveState.source`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Unit {
    Player(u8),
    Monster(u8),
}

impl Unit {
    pub fn from_id(id: u8) -> Option<Unit> {
        match id {
            EMPTY_UNIT => None,
            id if id < MONSTER_UNIT_BASE => Some(Unit::Player(id)),
            id => Some(Unit::Monster(id - MONSTER_UNIT_BASE)),
        }
    }

    pub fn id(self) -> u8 {
        match self {
            Unit::Player(slot) => slot,
            Unit::Monster(index) => MONSTER_UNIT_BASE + index,
        }
    }
}

/// Grid step: 0 = north (y - 1), 1 = east, 2 = south, 3 = west.
pub const DIRECTIONS: u8 = 4;

/// Action points per turn from speed (docs/CHARACTER.md, section 3.1: race base, class
/// adjustment, Dexterity bonus). Items and effects join in P1.
// TODO(balance): placeholder numbers.
pub fn player_ap(race: Race, class: Class, dexterity: u16) -> u8 {
    let base: u16 = match (race, class) {
        (Race::Human, Class::Mage) => 3,
        (Race::Human, _) => 4,
        (Race::ForestElf, Class::Mage) => 4,
        (Race::ForestElf, _) => 5,
    };
    (base + dexterity / 10).clamp(1, 12) as u8
}

/// Action points per turn for a monster, from its bestiary speed.
// TODO(L4): read speed from the registry `Bestiary`; it has no layout yet.
pub fn monster_ap(_bestiary_id: u8) -> u8 {
    4
}

/// Monster HP at spawn.
// TODO(L4): read HP from the registry `Bestiary`; it has no layout yet.
pub fn monster_hp(_bestiary_id: u8, level: u8) -> u32 {
    10 + 5 * level as u32
}

/// Action points to step onto a tile, `None` if it cannot be entered.
// TODO(L4): read passability and cost from the registry `Tileset`.
pub fn step_cost(tile: Tile) -> Option<u8> {
    match tile {
        Tile::Floor | Tile::Door | Tile::Entrance | Tile::Exit | Tile::Trap => Some(1),
        // Shallow water is difficult terrain (docs/MAP_DESCRIPTION.md, section 4.2).
        Tile::Water => Some(2),
        Tile::Wall => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unit_ids_round_trip() {
        for id in 0..EMPTY_UNIT {
            assert_eq!(Unit::from_id(id).unwrap().id(), id);
        }
        assert_eq!(Unit::from_id(EMPTY_UNIT), None);
        assert_eq!(Unit::from_id(3), Some(Unit::Player(3)));
        assert_eq!(Unit::from_id(4), Some(Unit::Monster(0)));
    }

    #[test]
    fn ap_is_bounded() {
        assert_eq!(player_ap(Race::Human, Class::Mage, 0), 3);
        assert_eq!(player_ap(Race::ForestElf, Class::Warrior, 25), 7);
        assert_eq!(player_ap(Race::ForestElf, Class::Warrior, u16::MAX), 12);
    }
}
