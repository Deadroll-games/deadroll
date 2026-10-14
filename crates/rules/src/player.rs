//! Hero numbers from race, class, level, stats and trait tiers (docs/CHARACTER.md).
use deadroll_combat::{
    modifiers::{counter, more},
    DamageRange, Mod, ModFilter,
};
use deadroll_skillcheck::Effect;
use deadroll_types::{Aspect, Class, Form, Race, Source, Stat, BP, STATS_COUNT};

use crate::config::{PlayerTuning, TIERS};

/// Index of `tier` (`1..=14`, 1 best) in a by-tier table; out-of-range values clamp.
pub fn tier_index(tier: u8) -> usize {
    tier.clamp(1, TIERS as u8) as usize - 1
}

/// Speed before items and effects (CHARACTER 3.1: race base, class adjustment, Dexterity).
pub fn player_speed(race: Race, class: Class, dexterity: u16, t: &PlayerTuning) -> u32 {
    let base = t.ap_base[race as usize][class as usize] as u32;
    base.saturating_add(u32::from(dexterity / t.ap_dex_div.max(1)))
}

/// Action points per turn from speed: items add `item_speed`, then their counterweight divides
/// (CHARACTER 3.1, steps 1-4), bounded. Effects (Haste, Slow) apply at the turn start.
pub fn speed_to_ap(speed: u32, item_speed: u16, item_counter_bp: u32, t: &PlayerTuning) -> u8 {
    let speed = speed.saturating_add(u32::from(item_speed));
    let ap = counter(u64::from(speed) * BP as u64, item_counter_bp) / BP as u64;
    ap.clamp(t.ap_min as u64, t.ap_max.max(t.ap_min) as u64) as u8
}

/// Action points per turn without items.
pub fn player_ap(race: Race, class: Class, dexterity: u16, t: &PlayerTuning) -> u8 {
    speed_to_ap(player_speed(race, class, dexterity, t), 0, 0, t)
}

/// Initiative of a player without items (CHARACTER 3: Intelligence; items and effects on top).
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

/// Class trait: tag increase in bp at `tier` and the source it applies to (CHARACTER 6.3, 8.1);
/// `None` when the class has none (Support's increase is on buff power, not damage).
pub fn class_increase(class: Class, tier: u8, t: &PlayerTuning) -> Option<(Source, u32)> {
    match t.class_increase_by_tier_bp[class as usize][tier_index(tier)] {
        0 => None,
        bp => Some((class_source(class), bp)),
    }
}

/// Class trait: increase on the power of heals, buffs and debuffs at `tier`, bp (CHARACTER 8.1).
pub fn support_increase(class: Class, tier: u8, t: &PlayerTuning) -> u32 {
    t.support_increase_by_tier_bp[class as usize][tier_index(tier)]
}

/// HP range of a heal skill with `power_bp`: the `weapon` range (the damage base of every skill,
/// SKILL_BUILDER 5.1) x `heal_weapon_bp` x power, with the support increase. At least 1.
pub fn heal_range(
    class: Class,
    class_tier: u8,
    weapon: DamageRange,
    power_bp: u32,
    t: &PlayerTuning,
) -> (u32, u32) {
    let scale = |v: u32| {
        let v = v as u64 * t.heal_weapon_bp as u64 / BP as u64 * power_bp as u64 / BP as u64;
        more(v, support_increase(class, class_tier, t)).clamp(1, u32::MAX as u64) as u32
    };
    let min = scale(weapon.min);
    (min, scale(weapon.max).max(min))
}

/// Race trait: flat added damage of the predisposition aspect at `tier` (CHARACTER 6.2, 6.3,
/// 7.1; COMBAT_MATH 9: the strength comes from the carrier's tier).
pub fn predisposition_added(aspect: Aspect, tier: u8, t: &PlayerTuning) -> DamageRange {
    let [min, max] = t.predisposition_added_by_tier[tier_index(tier)];
    DamageRange::new(aspect, min, max.max(min))
}

/// Accuracy, the `d20` bonus, without items: base + Dexterity (CHARACTER 3).
pub fn player_accuracy(stats: &[u16; STATS_COUNT], t: &PlayerTuning) -> u32 {
    t.accuracy_base
        .saturating_add(stats[Stat::Dexterity as usize] as u32 / t.accuracy_dex_div.max(1))
}

/// Evasion (the hit threshold) and base armor of a player before items (warriors have high base
/// armor, CHARACTER 8.1).
pub fn player_defense(class: Class, stats: &[u16; STATS_COUNT], t: &PlayerTuning) -> (u32, u32) {
    let defense =
        t.defense_base + stats[Stat::Dexterity as usize] as u32 / t.defense_dex_div.max(1);
    (defense, t.armor[class as usize])
}

/// Damage increases from the core stats (CHARACTER 2): Strength on martial strikes, Dexterity
/// on martial projectiles, Intelligence on every magical packet. Empty slots have `bp == 0`.
pub fn stat_damage_increases(stats: &[u16; STATS_COUNT], t: &PlayerTuning) -> [Mod; 3] {
    let per = |stat: Stat, bp: u32| (stats[stat as usize] as u32).saturating_mul(bp);
    [
        Mod::new(
            ModFilter::SourceForm(Source::Combat, Form::Strike),
            per(Stat::Strength, t.str_damage_bp),
        ),
        Mod::new(
            ModFilter::SourceForm(Source::Combat, Form::Projectile),
            per(Stat::Dexterity, t.dex_projectile_bp),
        ),
        Mod::new(
            ModFilter::Source(Source::Magical),
            per(Stat::Intelligence, t.int_magic_bp),
        ),
    ]
}

/// The stat that scales the states and debuffs of a skill with this power source: the aspect
/// picks the state, the source picks the stat.
pub const fn source_stat(source: Source) -> Stat {
    match source {
        Source::Combat => Stat::Strength,
        Source::Magical => Stat::Intelligence,
    }
}

/// The stat that scales the chance of a skill effect: Dexterity for tempo (Haste, Slow), else
/// the source's stat. `None` for effects that never roll a chance (heals and the other buffs).
pub const fn effect_stat(effect: Effect, source: Source) -> Option<Stat> {
    match effect {
        Effect::Slow | Effect::Haste => Some(Stat::Dexterity),
        Effect::Weakening | Effect::Vulnerability | Effect::Stun | Effect::Freeze => {
            Some(source_stat(source))
        }
        Effect::None | Effect::Empowerment | Effect::Protection | Effect::Heal => None,
    }
}

/// Increase on a state or effect chance from `stat`, bp.
pub fn state_chance_bonus(stats: &[u16; STATS_COUNT], stat: Stat, t: &PlayerTuning) -> u32 {
    let per = match stat {
        Stat::Strength => t.str_state_chance_bp,
        Stat::Intelligence => t.int_state_chance_bp,
        Stat::Dexterity => t.dex_state_chance_bp,
        Stat::Endurance => 0,
    };
    (stats[stat as usize] as u32).saturating_mul(per)
}

/// Extra turns of the player's own buffs on themselves, from Endurance (CHARACTER 2).
pub fn self_buff_turns(stats: &[u16; STATS_COUNT], t: &PlayerTuning) -> u8 {
    let extra = stats[Stat::Endurance as usize] / t.self_buff_end_div.max(1);
    extra.min(u16::from(t.self_buff_turns_max)) as u8
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

/// The level reached with `xp` total experience (CHARACTER 5).
pub fn level_for(xp: u32, t: &PlayerTuning) -> u16 {
    t.level_xp.iter().take_while(|need| xp >= **need).count() as u16
}

/// Stat points for going from `from` to `to` (0 if `to <= from`); `None` on overflow.
pub fn level_up_points(from: u16, to: u16, t: &PlayerTuning) -> Option<u8> {
    let gained = u8::try_from(to.saturating_sub(from)).ok()?;
    gained.checked_mul(t.stat_points_per_level)
}

/// Why stat points cannot be spent.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SpendError {
    /// Nothing to spend.
    Nothing,
    /// More points than the character has unspent.
    NotEnough,
    /// A stat would pass `u16::MAX`.
    Overflow,
}

/// Spending level-up points (CHARACTER 5): `add` per stat, all at once. Returns the new stats and
/// the points left.
pub fn spend_points(
    stats: &[u16; STATS_COUNT],
    unspent: u8,
    add: &[u16; STATS_COUNT],
) -> Result<([u16; STATS_COUNT], u8), SpendError> {
    let total: u32 = add.iter().map(|a| u32::from(*a)).sum();
    if total == 0 {
        return Err(SpendError::Nothing);
    }
    if total > u32::from(unspent) {
        return Err(SpendError::NotEnough);
    }
    let mut out = *stats;
    for (s, a) in out.iter_mut().zip(add) {
        *s = s.checked_add(*a).ok_or(SpendError::Overflow)?;
    }
    Ok((out, unspent - total as u8))
}

/// CHARACTER 7.1: humans pick a physical aspect, forest elves an elemental one.
pub const fn predisposition_allowed(race: Race, aspect: Aspect) -> bool {
    match race {
        Race::Human => matches!(aspect, Aspect::Piercing | Aspect::Blunt | Aspect::Blood),
        Race::ForestElf => matches!(aspect, Aspect::Fire | Aspect::Cold | Aspect::Lightning),
    }
}
