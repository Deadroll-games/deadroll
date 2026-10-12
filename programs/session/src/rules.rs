//! Session-local game rules: unit ids, action points, step costs, combat numbers, rewards.
//!
//! Numbers here are placeholders until balancing (docs have none yet). Each one sits behind a
//! single function so the registry tables (L4) can replace it without touching instructions.
use deadroll_combat::{DamageRange, RollSource};
use deadroll_mapcheck::roster::{ids, p0_role, Role};
use deadroll_types::{Aspect, Cell, Class, Form, Race, Source, Tile, PARTY_MAX, STATS_COUNT};

use crate::state::STASH_RESOURCES;

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

/// Monster HP at spawn (docs/BESTIARY.md, section 7: wolf medium, goblin low, chieftain boss).
// TODO(L4): read HP from the registry `Bestiary`; it has no layout yet.
pub fn monster_hp(bestiary_id: u8, level: u8) -> u32 {
    let level = level as u32;
    match bestiary_id {
        ids::WOLF => 12 + 4 * level,
        ids::GOBLIN => 8 + 3 * level,
        ids::GOBLIN_CHIEFTAIN => 40 + 10 * level,
        ids::GOBLIN_WARRIOR => 16 + 5 * level,
        ids::GOBLIN_SHAMAN => 10 + 3 * level,
        _ => 10 + 5 * level,
    }
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

// ---------------------------------------------------------------------------------------------
// Combat (L7.4). TODO(balance): every number below; TODO(L4): monsters from the `Bestiary`.

/// `act` skill slot for the weapon's basic attack.
pub const BASE_ATTACK: u8 = 0xFF;
/// `monster_act` action: the monster's natural attack (the only P0 action).
pub const NATURAL_ATTACK: u8 = 0;
/// Action points one attack costs.
pub const ACT_AP: u8 = 2;
/// Strikes reach the 8 neighbouring cells.
pub const STRIKE_RANGE: u8 = 1;
/// Projectiles reach this far (Chebyshev distance). TODO(P1): line of sight.
pub const PROJECTILE_RANGE: u8 = 6;
/// Native state chance of a skill / basic attack (crates/combat `Skill::state_chance_bp`).
pub const SKILL_STATE_CHANCE_BP: u32 = 2_000;
pub const BASIC_STATE_CHANCE_BP: u32 = 1_000;

/// Whether `form` reaches from `from` to `to` (Chebyshev distance, never the own cell).
pub fn in_range(form: Form, from: Cell, to: Cell) -> bool {
    let dx = from.x.abs_diff(to.x);
    let dy = from.y.abs_diff(to.y);
    let distance = dx.max(dy);
    let reach = match form {
        Form::Strike => STRIKE_RANGE,
        Form::Projectile => PROJECTILE_RANGE,
    };
    distance >= 1 && distance <= reach
}

/// Power source of a class's own attacks (docs/CHARACTER.md, section 8.1).
pub fn class_source(class: Class) -> Source {
    match class {
        Class::Warrior => Source::Combat,
        Class::Mage | Class::Support => Source::Magical,
    }
}

/// Starting weapon until items exist (P0 has no equipment). Skills convert its base.
pub fn class_weapon(class: Class) -> DamageRange {
    match class {
        Class::Warrior => DamageRange::new(Aspect::Blunt, 4, 8),
        Class::Mage => DamageRange::new(Aspect::Blunt, 3, 6),
        Class::Support => DamageRange::new(Aspect::Blunt, 2, 5),
    }
}

/// Class tag increase in bp and the source it applies to (docs/CHARACTER.md, section 8.1;
/// Support's increase is on buff power, not damage).
pub fn class_increase(class: Class) -> Option<(Source, u32)> {
    match class {
        Class::Warrior => Some((Source::Combat, 2_000)),
        Class::Mage => Some((Source::Magical, 2_000)),
        Class::Support => None,
    }
}

/// Race trait: flat added damage of the predisposition aspect (docs/CHARACTER.md, 6.2, 7.1).
pub fn predisposition_added(aspect: Aspect) -> DamageRange {
    DamageRange::new(aspect, 1, 2)
}

/// `d20` bonus: level plus the attack stat (Strength for martial, Intelligence for magical).
pub fn player_attack_bonus(level: u16, stats: &[u16; STATS_COUNT], source: Source) -> u32 {
    let stat = match source {
        Source::Combat => stats[deadroll_types::Stat::Strength as usize],
        Source::Magical => stats[deadroll_types::Stat::Intelligence as usize],
    };
    level as u32 + stat as u32 / 5
}

/// Crit chance besides a natural 20, from Dexterity.
pub fn player_crit_bp(stats: &[u16; STATS_COUNT]) -> u32 {
    stats[deadroll_types::Stat::Dexterity as usize] as u32 * 20
}

/// Defense threshold and armor of a player (warriors have high base armor, CHARACTER 8.1).
pub fn player_defense(class: Class, stats: &[u16; STATS_COUNT]) -> (u32, u32) {
    let defense = 10 + stats[deadroll_types::Stat::Dexterity as usize] as u32 / 5;
    let armor = match class {
        Class::Warrior => 30,
        Class::Mage | Class::Support => 5,
    };
    (defense, armor)
}

/// One bestiary template at a level (docs/BESTIARY.md, sections 7-8).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MonsterProfile {
    pub weapon: DamageRange,
    pub source: Source,
    pub form: Form,
    pub attack_bonus: u32,
    pub defense: u32,
    pub armor: u32,
    /// Resistance (counterweight) for one aspect, `None` = no special defense.
    pub resist: Option<(Aspect, u32)>,
    /// Experience for killing it.
    pub xp: u32,
}

/// P0 bestiary ids: 0 wolf, 1 goblin, 2 goblin chieftain, 3 goblin warrior, 4 goblin shaman.
pub fn monster_profile(bestiary_id: u8, level: u8) -> Option<MonsterProfile> {
    let l = level as u32;
    let strike = |aspect, min: u32, max: u32| DamageRange::new(aspect, min + l / 2, max + l);
    let profile = match bestiary_id {
        // Bite: martial, strike, Piercing; thick fur resists Cold.
        ids::WOLF => MonsterProfile {
            weapon: strike(Aspect::Piercing, 2, 4),
            source: Source::Combat,
            form: Form::Strike,
            attack_bonus: 2 + l,
            defense: 10,
            armor: 5,
            resist: Some((Aspect::Cold, 5_000)),
            xp: 10 * l.max(1),
        },
        // Rusty blade: martial, strike, Piercing; no special defenses.
        ids::GOBLIN => MonsterProfile {
            weapon: strike(Aspect::Piercing, 1, 3),
            source: Source::Combat,
            form: Form::Strike,
            attack_bonus: 1 + l,
            defense: 9,
            armor: 5,
            resist: None,
            xp: 6 * l.max(1),
        },
        // Heavy blade: martial, strike, Piercing; trophy armor resists Piercing.
        ids::GOBLIN_CHIEFTAIN => MonsterProfile {
            weapon: strike(Aspect::Piercing, 4, 8),
            source: Source::Combat,
            form: Form::Strike,
            attack_bonus: 4 + l,
            defense: 13,
            armor: 40,
            resist: Some((Aspect::Piercing, 5_000)),
            xp: 50 * l.max(1),
        },
        ids::GOBLIN_WARRIOR => MonsterProfile {
            weapon: strike(Aspect::Blunt, 2, 5),
            source: Source::Combat,
            form: Form::Strike,
            attack_bonus: 3 + l,
            defense: 11,
            armor: 25,
            resist: None,
            xp: 15 * l.max(1),
        },
        // Support caster: a magical bolt from range.
        ids::GOBLIN_SHAMAN => MonsterProfile {
            weapon: strike(Aspect::Fire, 2, 4),
            source: Source::Magical,
            form: Form::Projectile,
            attack_bonus: 2 + l,
            defense: 10,
            armor: 5,
            resist: Some((Aspect::Fire, 5_000)),
            xp: 15 * l.max(1),
        },
        _ => return None,
    };
    Some(profile)
}

/// Killing this monster wins the party (docs/PARTY_AND_TURNS.md, section 7.1).
pub fn is_boss(bestiary_id: u8) -> bool {
    p0_role(bestiary_id) == Some(Role::Boss)
}

// ---------------------------------------------------------------------------------------------
// Rewards (L7.5). TODO(post-demo): reward tables and levels from the map (PARTY_AND_TURNS 9).

/// Boss reward rolls per player.
pub const BOSS_REWARD_ROLLS: u32 = 2;
/// Most units of one resource from one roll.
pub const REWARD_AMOUNT_MAX: u64 = 3;
/// First roll index of slot `s` in the victory seed's stream: `REWARD_ROLL_BASE + s * stride`.
pub const REWARD_ROLL_BASE: u32 = 1_000;
pub const REWARD_ROLL_STRIDE: u32 = 16;

/// Resources one living player gets from the boss: `BOSS_REWARD_ROLLS` rolls, each picks a
/// resource kind and `1..=REWARD_AMOUNT_MAX` units (docs/PARTY_AND_TURNS.md, 9.3: each player's
/// reward is rolled separately).
pub fn boss_rewards(src: &mut impl RollSource) -> [u32; STASH_RESOURCES] {
    let mut out = [0u32; STASH_RESOURCES];
    for _ in 0..BOSS_REWARD_ROLLS {
        let kind = (src.next_u64() % STASH_RESOURCES as u64) as usize;
        let amount = 1 + src.next_u64() % REWARD_AMOUNT_MAX;
        out[kind] = out[kind].saturating_add(amount as u32);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use deadroll_combat::SeededRoll;

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

    #[test]
    fn range_by_form() {
        let at = Cell { x: 5, y: 5 };
        let c = |x, y| Cell { x, y };
        assert!(!in_range(Form::Strike, at, at));
        assert!(in_range(Form::Strike, at, c(6, 6)));
        assert!(in_range(Form::Strike, at, c(4, 5)));
        assert!(!in_range(Form::Strike, at, c(7, 5)));
        assert!(in_range(Form::Projectile, at, c(11, 0)));
        assert!(!in_range(Form::Projectile, at, c(12, 5)));
        assert!(!in_range(Form::Projectile, at, at));
    }

    #[test]
    fn every_p0_monster_has_a_profile_and_one_boss() {
        for id in 0..=4u8 {
            let p = monster_profile(id, 1).unwrap();
            assert!(p.weapon.min >= 1 && p.weapon.max >= p.weapon.min);
            assert!(p.xp > 0 && monster_hp(id, 1) > 0);
        }
        assert_eq!(monster_profile(5, 1), None);
        assert!(is_boss(ids::GOBLIN_CHIEFTAIN));
        for id in [
            ids::WOLF,
            ids::GOBLIN,
            ids::GOBLIN_WARRIOR,
            ids::GOBLIN_SHAMAN,
            9,
        ] {
            assert!(!is_boss(id));
        }
        assert_eq!(
            monster_profile(ids::GOBLIN_SHAMAN, 1).unwrap().form,
            Form::Projectile
        );
    }

    #[test]
    fn boss_rewards_are_deterministic_and_bounded() {
        for seed in 0..200 {
            let a = boss_rewards(&mut SeededRoll::new(seed));
            assert_eq!(a, boss_rewards(&mut SeededRoll::new(seed)));
            let total: u32 = a.iter().sum();
            assert!(total >= BOSS_REWARD_ROLLS);
            assert!(total as u64 <= BOSS_REWARD_ROLLS as u64 * REWARD_AMOUNT_MAX);
        }
    }
}
