//! Hero numbers from race, class, level and stats (docs/CHARACTER.md).
use deadroll_combat::{modifiers::more, DamageRange};
use deadroll_types::{Aspect, Class, Race, Source, Stat, BP, STATS_COUNT};

use crate::config::PlayerTuning;

/// Action points per turn from speed (CHARACTER 3.1: race base, class adjustment, Dexterity
/// bonus). Items and effects join in P1.
pub fn player_ap(race: Race, class: Class, dexterity: u16, t: &PlayerTuning) -> u8 {
    let base = t.ap_base[race as usize][class as usize] as u16;
    let ap = base.saturating_add(dexterity / t.ap_dex_div.max(1));
    ap.clamp(t.ap_min as u16, t.ap_max.max(t.ap_min) as u16) as u8
}

/// Initiative of a player (CHARACTER 3.2: Intelligence; items and effects in P1).
pub fn player_initiative(stats: &[u16; STATS_COUNT], t: &PlayerTuning) -> u16 {
    stats[Stat::Intelligence as usize] / t.initiative_div.max(1)
}

/// Power source of a class's own attacks (CHARACTER 8.1).
pub fn class_source(class: Class) -> Source {
    match class {
        Class::Warrior => Source::Combat,
        Class::Mage | Class::Support => Source::Magical,
    }
}

/// Starting weapon until items exist (P0 has no equipment). Skills convert its base.
pub fn class_weapon(class: Class, t: &PlayerTuning) -> DamageRange {
    let [min, max] = t.weapon[class as usize];
    DamageRange::new(Aspect::Blunt, min, max)
}

/// Class tag increase in bp and the source it applies to (CHARACTER 8.1); `None` when the class
/// has none (Support's increase is on buff power, not damage).
pub fn class_increase(class: Class, t: &PlayerTuning) -> Option<(Source, u32)> {
    match t.class_increase_bp[class as usize] {
        0 => None,
        bp => Some((class_source(class), bp)),
    }
}

/// Class increase on the power of heals, buffs and debuffs, bp (CHARACTER 8.1).
pub fn support_increase(class: Class, t: &PlayerTuning) -> u32 {
    t.support_increase_bp[class as usize]
}

/// HP range of a heal skill with `power_bp`: the class weapon's range (the damage base of every
/// skill, SKILL_BUILDER 5.1) x `heal_weapon_bp` x power, with the support increase. At least 1.
pub fn heal_range(class: Class, power_bp: u32, t: &PlayerTuning) -> (u32, u32) {
    let weapon = class_weapon(class, t);
    let scale = |v: u32| {
        let v = v as u64 * t.heal_weapon_bp as u64 / BP as u64 * power_bp as u64 / BP as u64;
        more(v, support_increase(class, t)).clamp(1, u32::MAX as u64) as u32
    };
    let min = scale(weapon.min);
    (min, scale(weapon.max).max(min))
}

/// Race trait: flat added damage of the predisposition aspect (CHARACTER 6.2, 7.1).
pub fn predisposition_added(aspect: Aspect, t: &PlayerTuning) -> DamageRange {
    let [min, max] = t.predisposition_added;
    DamageRange::new(aspect, min, max)
}

/// `d20` bonus: level plus the attack stat (Strength for martial, Intelligence for magical).
pub fn player_attack_bonus(
    level: u16,
    stats: &[u16; STATS_COUNT],
    source: Source,
    t: &PlayerTuning,
) -> u32 {
    let stat = match source {
        Source::Combat => stats[Stat::Strength as usize],
        Source::Magical => stats[Stat::Intelligence as usize],
    };
    level as u32 + stat as u32 / t.attack_stat_div.max(1)
}

/// Crit chance besides a natural 20, from Dexterity.
pub fn player_crit_bp(stats: &[u16; STATS_COUNT], t: &PlayerTuning) -> u32 {
    (stats[Stat::Dexterity as usize] as u32).saturating_mul(t.crit_per_dex_bp)
}

/// Defense threshold and armor of a player (warriors have high base armor, CHARACTER 8.1).
pub fn player_defense(class: Class, stats: &[u16; STATS_COUNT], t: &PlayerTuning) -> (u32, u32) {
    let defense =
        t.defense_base + stats[Stat::Dexterity as usize] as u32 / t.defense_dex_div.max(1);
    (defense, t.armor[class as usize])
}

/// Maximum HP (CHARACTER 4): class base + Endurance x per point + per level after the first.
pub fn hp_max(class: Class, stats: &[u16; STATS_COUNT], level: u16, t: &PlayerTuning) -> u32 {
    let c = class as usize;
    t.hp_base[c]
        + u32::from(stats[Stat::Endurance as usize]) * t.hp_per_endurance
        + u32::from(level.saturating_sub(1)) * t.hp_per_level[c]
}

/// Creation (CHARACTER 2): each stat in `stat_base..=stat_max_start` and exactly
/// `stat_points_start` points spent.
pub fn stats_valid(stats: &[u16; STATS_COUNT], t: &PlayerTuning) -> bool {
    let in_range = stats
        .iter()
        .all(|s| (t.stat_base..=t.stat_max_start).contains(s));
    let spent: u32 = stats
        .iter()
        .map(|s| u32::from(s.saturating_sub(t.stat_base)))
        .sum();
    in_range && spent == u32::from(t.stat_points_start)
}

/// CHARACTER 7.1: humans pick a physical aspect, forest elves an elemental one.
pub const fn predisposition_allowed(race: Race, aspect: Aspect) -> bool {
    match race {
        Race::Human => matches!(aspect, Aspect::Piercing | Aspect::Blunt | Aspect::Blood),
        Race::ForestElf => matches!(aspect, Aspect::Fire | Aspect::Cold | Aspect::Lightning),
    }
}
