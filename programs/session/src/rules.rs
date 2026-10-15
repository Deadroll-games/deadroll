//! Session-local game rules: step costs, instruction constants, rewards, and the shared rules of
//! `deadroll-rules` with the program's numbers ([`BALANCE`]).
//!
//! Monster HP, speed, initiative and tile costs come from the registry tables (copied at
//! `open_session`). Every combat number lives in `deadroll_rules::BalanceConfig` (placeholders
//! until balancing, tuned in `tools/balance`); the wrappers below keep one call per rule.
use deadroll_combat::{DamageRange, RollSource};
use deadroll_mapcheck::roster::{p0_role, Role};
use deadroll_rules::{
    config::BalanceConfig, input::hero, items, monster, player, units, HeroStats,
};
use deadroll_types::{Aspect, Cell, Class, Form, Race, Source, Tile, STATS_COUNT};

pub use deadroll_rules::units::{
    cooldown_after_use, roll_initiative, tick_cooldowns, Initiative, Unit, EMPTY_UNIT,
    MONSTER_UNIT_BASE,
};
pub use deadroll_rules::MonsterProfile;

use crate::state::{external::BestiaryRecord, STASH_RESOURCES};

/// The numbers the program plays with.
pub const BALANCE: &BalanceConfig = &BalanceConfig::DEFAULT;

/// Minimum party price: 0.01 SOL (docs/PARTY_AND_TURNS.md, section 3.1).
pub const MIN_PRICE_LAMPORTS: u64 = 10_000_000;
/// Max steps in one `move_unit` / `monster_move`.
pub const MAX_PATH: usize = 16;

/// Grid step: 0 = north (y - 1), 1 = east, 2 = south, 3 = west.
pub const DIRECTIONS: u8 = 4;

/// Action points per turn from speed (docs/CHARACTER.md, section 3.1).
pub fn player_ap(race: Race, class: Class, dexterity: u16) -> u8 {
    player::player_ap(race, class, dexterity, &BALANCE.players)
}

/// Action points per turn for a monster: its registry bestiary speed (copied at open).
pub fn monster_ap(speed: u8) -> u8 {
    monster::monster_ap(speed, &BALANCE.monsters)
}

/// Monster HP at spawn from its registry bestiary record (at least 1).
pub fn monster_hp(record: &BestiaryRecord, level: u8) -> u32 {
    monster::monster_hp(record.hp, record.hp_per_level, level)
}

/// Action points to step onto a tile, `None` if it cannot be entered. `costs` is the registry
/// `Tileset` copied at open (`GameSession.tile_cost`, 0 = impassable).
pub fn step_cost(tile: Tile, costs: &[u8; Tile::COUNT]) -> Option<u8> {
    match costs[tile as usize] {
        0 => None,
        cost => Some(cost),
    }
}

/// Initiative of a player (docs/CHARACTER.md, 3.2: Intelligence; effects in P1).
pub fn player_initiative(stats: &[u16; STATS_COUNT]) -> u16 {
    player::player_initiative(stats, &BALANCE.players)
}

/// Action points per turn of a hero with what it wears.
pub fn hero_ap(h: &HeroStats) -> u8 {
    hero::ap(BALANCE, h)
}

/// Initiative of a hero with what it wears.
pub fn hero_initiative(h: &HeroStats) -> u16 {
    hero::initiative(BALANCE, h)
}

/// HP the worn items add on top of the stored `CharacterData.hp_max`.
pub fn item_hp(h: &HeroStats) -> u32 {
    hero::item_hp(BALANCE, h)
}

/// Maximum HP in a party: the stored `CharacterData.hp_max` plus what the items add.
pub fn party_hp_max(stored: u32, h: &HeroStats) -> u32 {
    stored.saturating_add(item_hp(h))
}

/// The damage base of a hero: the main-hand weapon, or bare hands.
pub fn hero_weapon(h: &HeroStats) -> DamageRange {
    hero::weapon(BALANCE, h)
}

/// What a new character of `class` wears (`create_character` gives the class starter kit).
pub fn starter_equipment(class: Class) -> items::Equipment {
    items::starter_loadout(class, &BALANCE.items).equipment()
}

/// Everything worn items add, for the hit breakdown.
pub fn item_bonus(h: &HeroStats) -> items::ItemBonus {
    items::item_bonus(&h.equipment, &BALANCE.items)
}

// ---------------------------------------------------------------------------------------------
// Combat (L7.4). Numbers: `BALANCE`.

/// `act` skill slot for the weapon's basic attack.
pub const BASE_ATTACK: u8 = 0xFF;
/// `monster_act` action: the monster's natural attack (the only P0 action).
pub const NATURAL_ATTACK: u8 = 0;
/// Action points one attack costs.
pub const ACT_AP: u8 = BALANCE.players.act_ap;
/// Strikes reach the 8 neighbouring cells.
pub const STRIKE_RANGE: u8 = BALANCE.players.strike_range;
/// Projectiles reach this far (Chebyshev distance). TODO(P1): line of sight.
pub const PROJECTILE_RANGE: u8 = BALANCE.players.projectile_range;
/// Native state chance of a skill / basic attack (crates/combat `Skill::state_chance_bp`).
pub const SKILL_STATE_CHANCE_PCT: u32 = BALANCE.players.skill_state_chance_pct;
pub const BASIC_STATE_CHANCE_PCT: u32 = BALANCE.players.basic_state_chance_pct;
/// HP a heal restores per point of the weapon's range, bp.
pub const HEAL_WEAPON_BP: u32 = BALANCE.players.heal_weapon_bp;

/// Whether `form` reaches from `from` to `to` (Chebyshev distance, never the own cell).
pub fn in_range(form: Form, from: Cell, to: Cell) -> bool {
    units::in_range(form, from, to, &BALANCE.players)
}

/// Whether a heal or buff of `form` reaches an ally (allies may share a cell).
pub fn in_ally_range(form: Form, from: Cell, to: Cell) -> bool {
    units::in_ally_range(form, from, to, &BALANCE.players)
}

/// Power source of a class's own attacks (docs/CHARACTER.md, section 8.1).
pub fn class_source(class: Class) -> Source {
    player::class_source(class)
}

/// Class trait: tag increase in bp at `tier` and the source it applies to (docs/CHARACTER.md,
/// sections 6.3 and 8.1).
pub fn class_increase(class: Class, tier: u8) -> Option<(Source, u32)> {
    player::class_increase(class, tier, &BALANCE.players)
}

/// Class trait: increase on the power of heals, buffs and debuffs at `tier`, bp
/// (docs/CHARACTER.md, 8.1).
pub fn support_increase(class: Class, tier: u8) -> u32 {
    player::support_increase(class, tier, &BALANCE.players)
}

/// HP range of a heal skill with `power_bp` from `weapon` (see
/// `deadroll_rules::player::heal_range`).
pub fn heal_range(class: Class, class_tier: u8, weapon: DamageRange, power_bp: u32) -> (u32, u32) {
    player::heal_range(class, class_tier, weapon, power_bp, &BALANCE.players)
}

/// Race trait: flat added damage of the predisposition aspect at `tier` (docs/CHARACTER.md, 6.2,
/// 6.3, 7.1).
pub fn predisposition_added(aspect: Aspect, tier: u8) -> DamageRange {
    player::predisposition_added(aspect, tier, &BALANCE.players)
}

/// Accuracy, the `d20` bonus, before items: Dexterity (docs/CHARACTER.md, section 3).
pub fn player_accuracy(stats: &[u16; STATS_COUNT]) -> u32 {
    player::player_accuracy(stats, &BALANCE.players)
}

/// Defense threshold and base armor of a player before items (CHARACTER 8.1).
pub fn player_defense(class: Class, stats: &[u16; STATS_COUNT]) -> (u32, u32) {
    player::player_defense(class, stats, &BALANCE.players)
}

/// One P0 bestiary template at a level (`deadroll_rules::monster::p0`).
pub fn monster_profile(bestiary_id: u8, level: u8) -> Option<MonsterProfile> {
    monster::p0::monster_base(bestiary_id).map(|b| b.profile(level, &BALANCE.monsters))
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
    use deadroll_mapcheck::roster::ids;
    use deadroll_types::BP;

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

    fn kit_weapon(class: Class) -> DamageRange {
        items::weapon_range(&starter_equipment(class), &BALANCE.items)
    }

    #[test]
    fn heal_scales_with_power_and_support() {
        let heal = |class, power| heal_range(class, 14, kit_weapon(class), power);
        // Support wand 2..5, x2, +20%.
        assert_eq!(heal(Class::Support, BP), (4, 12));
        // Mage staff 3..6, x2.
        assert_eq!(heal(Class::Mage, BP), (6, 12));
        assert_eq!(heal(Class::Warrior, 5_000), (4, 8));
        assert_eq!(heal(Class::Warrior, 0), (1, 1));
        assert_eq!(heal(Class::Support, u32::MAX).1, 5_153_960);
        assert_eq!(support_increase(Class::Support, 14), 2_000);
        assert_eq!(support_increase(Class::Mage, 14), 0);
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
