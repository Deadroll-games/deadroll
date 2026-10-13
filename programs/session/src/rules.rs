//! Session-local game rules: unit ids, action points, step costs, combat numbers, rewards.
//!
//! Monster HP, speed, initiative and tile costs come from the registry tables (copied at
//! `open_session`). The other numbers are placeholders until balancing (docs have none yet),
//! each behind a single function.
use deadroll_combat::{d20, modifiers::more, DamageRange, RollSource};
use deadroll_mapcheck::roster::{ids, p0_role, Role};
use deadroll_types::{
    Aspect, Cell, Class, Form, Race, Source, Tile, BP, PARTY_MAX, STATS_COUNT, TURN_ORDER_MAX,
};

use crate::state::{external::BestiaryRecord, STASH_RESOURCES};

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

/// Action points per turn for a monster: its registry bestiary speed (copied at open).
pub fn monster_ap(speed: u8) -> u8 {
    speed.clamp(1, 12)
}

/// Monster HP at spawn from its registry bestiary record (at least 1).
pub fn monster_hp(record: &BestiaryRecord, level: u8) -> u32 {
    record
        .hp
        .saturating_add(record.hp_per_level.saturating_mul(level as u32))
        .max(1)
}

/// Action points to step onto a tile, `None` if it cannot be entered. `costs` is the registry
/// `Tileset` copied at open (`GameSession.tile_cost`, 0 = impassable).
pub fn step_cost(tile: Tile, costs: &[u8; Tile::COUNT]) -> Option<u8> {
    match costs[tile as usize] {
        0 => None,
        cost => Some(cost),
    }
}

/// Initiative of a player (docs/CHARACTER.md, 3.2: Intelligence; items and effects in P1).
// TODO(balance): placeholder scale, same order as the bestiary values.
pub fn player_initiative(stats: &[u16; STATS_COUNT]) -> u16 {
    stats[deadroll_types::Stat::Intelligence as usize] / 2
}

/// The turn order rolled from one VRF value (docs/PARTY_AND_TURNS.md, 6; the timeline is P1).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Initiative {
    pub len: u8,
    /// Turn-order ids, first to act first; `EMPTY_UNIT` after `len`.
    pub order: [u8; TURN_ORDER_MAX],
    pub d20: [u8; TURN_ORDER_MAX],
    pub initiative: [u16; TURN_ORDER_MAX],
}

/// Every unit in `units` (players by slot, then monsters by index) rolls a d20 in that order
/// (roll `i` of the seed's stream); the higher `d20 + initiative` acts first, ties go to the
/// higher initiative, then to the lower unit id.
pub fn roll_initiative(units: &[(u8, u16)], src: &mut impl RollSource) -> Initiative {
    let mut out = Initiative {
        len: units.len().min(TURN_ORDER_MAX) as u8,
        order: [EMPTY_UNIT; TURN_ORDER_MAX],
        d20: [0; TURN_ORDER_MAX],
        initiative: [0; TURN_ORDER_MAX],
    };
    let len = out.len as usize;
    for (i, &(unit, initiative)) in units[..len].iter().enumerate() {
        out.order[i] = unit;
        out.d20[i] = d20(src);
        out.initiative[i] = initiative;
    }
    // Insertion sort: at most 28 units, stable, no allocation.
    let key = |o: &Initiative, i: usize| {
        (
            o.d20[i] as u32 + o.initiative[i] as u32,
            o.initiative[i],
            u8::MAX - o.order[i],
        )
    };
    for i in 1..len {
        let mut j = i;
        while j > 0 && key(&out, j) > key(&out, j - 1) {
            out.order.swap(j, j - 1);
            out.d20.swap(j, j - 1);
            out.initiative.swap(j, j - 1);
            j -= 1;
        }
    }
    out
}

// ---------------------------------------------------------------------------------------------
// Combat (L7.4). TODO(balance): every number below. The bestiary combat fields (armor, evasion,
// accuracy, damage) are relative levels until balancing, so the combat profile stays here.

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

/// Cooldown value stored when a skill with `cooldown` (own turns, `SkillRecord.cooldown`) is
/// used: own turn starts until it is usable again. Each start counts it down by one, so the
/// skill misses exactly its next `cooldown` own turns (docs/SKILL_BUILDER.md, section 8).
pub fn cooldown_after_use(cooldown: u8) -> u8 {
    match cooldown {
        0 => 0,
        c => c.saturating_add(1),
    }
}

/// Start of the unit's own turn: every cooldown goes down by one.
pub fn tick_cooldowns(cooldowns: &mut [u8]) {
    for c in cooldowns {
        *c = c.saturating_sub(1);
    }
}

/// Whether `form` reaches from `from` to `to` (Chebyshev distance, never the own cell).
pub fn in_range(form: Form, from: Cell, to: Cell) -> bool {
    let distance = chebyshev(from, to);
    distance >= 1 && distance <= reach(form)
}

/// Whether a heal or buff of `form` reaches an ally: the same, but allies may share a cell (the
/// whole party stands on the entrance when it joins).
pub fn in_ally_range(form: Form, from: Cell, to: Cell) -> bool {
    chebyshev(from, to) <= reach(form)
}

fn chebyshev(from: Cell, to: Cell) -> u8 {
    from.x.abs_diff(to.x).max(from.y.abs_diff(to.y))
}

fn reach(form: Form) -> u8 {
    match form {
        Form::Strike => STRIKE_RANGE,
        Form::Projectile => PROJECTILE_RANGE,
    }
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

/// Class increase on the power of heals, buffs and debuffs, bp (docs/CHARACTER.md, 8.1: Support's
/// class tag). TODO(balance): placeholder.
pub fn support_increase(class: Class) -> u32 {
    match class {
        Class::Support => 2_000,
        Class::Warrior | Class::Mage => 0,
    }
}

/// HP a heal restores per point of the class weapon's range, bp. TODO(balance): placeholder.
pub const HEAL_WEAPON_BP: u32 = 20_000;

/// HP range of a heal skill with `power_bp`: the class weapon's range (the damage base of every
/// skill, SKILL_BUILDER 5.1) x `HEAL_WEAPON_BP` x power, with the support increase. At least 1.
pub fn heal_range(class: Class, power_bp: u32) -> (u32, u32) {
    let weapon = class_weapon(class);
    let scale = |v: u32| {
        let v = v as u64 * HEAL_WEAPON_BP as u64 / BP as u64 * power_bp as u64 / BP as u64;
        more(v, support_increase(class)).clamp(1, u32::MAX as u64) as u32
    };
    let min = scale(weapon.min);
    (min, scale(weapon.max).max(min))
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
        assert_eq!(monster_ap(0), 1);
        assert_eq!(monster_ap(6), 6);
        assert_eq!(monster_ap(200), 12);
    }

    #[test]
    fn monster_hp_from_the_bestiary() {
        let wolf = BestiaryRecord {
            speed: 6,
            initiative: 6,
            hp: 12,
            hp_per_level: 3,
        };
        assert_eq!(monster_hp(&wolf, 0), 12);
        assert_eq!(monster_hp(&wolf, 2), 18);
        let zero = BestiaryRecord {
            hp: 0,
            hp_per_level: 0,
            ..wolf
        };
        assert_eq!(monster_hp(&zero, 5), 1);
        let huge = BestiaryRecord {
            hp: u32::MAX,
            ..wolf
        };
        assert_eq!(monster_hp(&huge, 255), u32::MAX);
    }

    #[test]
    fn step_cost_from_the_tileset() {
        let costs = crate::state::test_support::TILE_COST;
        assert_eq!(step_cost(Tile::Floor, &costs), Some(1));
        assert_eq!(step_cost(Tile::Water, &costs), Some(2));
        assert_eq!(step_cost(Tile::Wall, &costs), None);
        assert_eq!(step_cost(Tile::Trap, &[3; Tile::COUNT]), Some(3));
    }

    #[test]
    fn initiative_order() {
        use deadroll_combat::ScriptedRoll;
        // d20 = roll % 20 + 1. Units: player 0 (init 5), player 1 (init 0), monsters 4, 5, 6.
        let units = [(0, 5), (1, 0), (4, 2), (5, 2), (6, 10)];
        // d20s:    10       15      13      13      1
        // totals:  15       15      15      15      11
        let mut src = ScriptedRoll::new([9, 14, 12, 12, 0]);
        let r = roll_initiative(&units, &mut src);
        assert_eq!(r.len, 5);
        // Tie at 15: higher initiative first (0: 5), then 4 and 5 (init 2, lower id first), then 1.
        assert_eq!(&r.order[..5], &[0, 4, 5, 1, 6]);
        assert_eq!(&r.d20[..5], &[10, 13, 13, 15, 1]);
        assert_eq!(&r.initiative[..5], &[5, 2, 2, 0, 10]);
        assert!(r.order[5..].iter().all(|&u| u == EMPTY_UNIT));
    }

    #[test]
    fn initiative_is_deterministic_and_complete() {
        let units: Vec<(u8, u16)> = (0..2u8)
            .map(|s| (s, 3))
            .chain((0..20u8).map(|i| (MONSTER_UNIT_BASE + i, i as u16 % 7)))
            .collect();
        for seed in 0..50 {
            let a = roll_initiative(&units, &mut SeededRoll::new(seed));
            assert_eq!(a, roll_initiative(&units, &mut SeededRoll::new(seed)));
            let mut ids: Vec<u8> = a.order[..a.len as usize].to_vec();
            ids.sort();
            let mut expected: Vec<u8> = units.iter().map(|u| u.0).collect();
            expected.sort();
            assert_eq!(ids, expected);
            for w in 0..a.len as usize - 1 {
                let t = |i: usize| a.d20[i] as u32 + a.initiative[i] as u32;
                assert!(t(w) >= t(w + 1));
                assert!((1..=20).contains(&a.d20[w]));
            }
        }
        let none = roll_initiative(&[], &mut SeededRoll::new(1));
        assert_eq!(none.len, 0);
    }

    #[test]
    fn cooldown_blocks_exactly_its_own_turns() {
        for cooldown in 0..=3u8 {
            // Used on turn 0; turns 1.. start with a tick.
            let mut left = [cooldown_after_use(cooldown)];
            let mut blocked = 0;
            for _turn in 1..=10 {
                tick_cooldowns(&mut left);
                if left[0] > 0 {
                    blocked += 1;
                } else {
                    break;
                }
            }
            assert_eq!(blocked, cooldown, "cooldown {cooldown}");
        }
        assert_eq!(cooldown_after_use(u8::MAX), u8::MAX);
        let mut many = [0, 1, 2, 255];
        tick_cooldowns(&mut many);
        assert_eq!(many, [0, 0, 1, 254]);
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
        // Allies: the own cell counts, the reach is the same.
        assert!(in_ally_range(Form::Strike, at, at));
        assert!(in_ally_range(Form::Strike, at, c(6, 4)));
        assert!(!in_ally_range(Form::Strike, at, c(7, 5)));
        assert!(in_ally_range(Form::Projectile, at, c(11, 0)));
        assert!(!in_ally_range(Form::Projectile, at, c(5, 12)));
    }

    #[test]
    fn every_p0_monster_has_a_profile_and_one_boss() {
        for id in 0..=4u8 {
            let p = monster_profile(id, 1).unwrap();
            assert!(p.weapon.min >= 1 && p.weapon.max >= p.weapon.min);
            assert!(p.xp > 0);
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
    fn heal_scales_with_power_and_support() {
        // Support weapon 2..5, x2, +20%.
        assert_eq!(heal_range(Class::Support, BP), (4, 12));
        // Mage weapon 3..6, x2.
        assert_eq!(heal_range(Class::Mage, BP), (6, 12));
        assert_eq!(heal_range(Class::Warrior, 5_000), (4, 8));
        assert_eq!(heal_range(Class::Warrior, 0), (1, 1));
        assert_eq!(heal_range(Class::Support, u32::MAX).1, 5_153_960);
        assert_eq!(support_increase(Class::Support), 2_000);
        assert_eq!(support_increase(Class::Mage), 0);
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
