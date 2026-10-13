//! Every balance number of the game rules in one table.
//!
//! [`BalanceConfig::DEFAULT`] is what the programs use today. All values are placeholders until
//! balancing (`tools/balance` tunes them). Tables indexed by an enum (`Race`, `Class`, ...) have
//! that enum's `COUNT` as their length, so a new variant does not compile until it has numbers.

use deadroll_combat::{Balance, SupportBalance};
use deadroll_skillcheck::SkillTuning;
use deadroll_types::{Class, Race};

#[cfg(feature = "serde")]
use serde::{Deserialize, Serialize};

/// The whole table.
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BalanceConfig {
    /// Hit math (`crates/combat`).
    #[cfg_attr(feature = "serde", serde(with = "remote::BalanceDef"))]
    pub combat: Balance,
    /// Buffs, debuffs, heals and skill effects (`crates/combat::support`).
    #[cfg_attr(feature = "serde", serde(with = "remote::SupportBalanceDef"))]
    pub support: SupportBalance,
    /// Skill power, shares and cooldowns (`crates/skillcheck::derive_with`).
    #[cfg_attr(feature = "serde", serde(with = "remote::SkillTuningDef"))]
    pub skills: SkillTuning,
    pub players: PlayerTuning,
    pub monsters: MonsterScaling,
}

impl BalanceConfig {
    pub const DEFAULT: BalanceConfig = BalanceConfig {
        combat: Balance::DEFAULT,
        support: SupportBalance::DEFAULT,
        skills: SkillTuning::DEFAULT,
        players: PlayerTuning::DEFAULT,
        monsters: MonsterScaling::DEFAULT,
    };
}

impl Default for BalanceConfig {
    fn default() -> Self {
        Self::DEFAULT
    }
}

/// Hero numbers (docs/CHARACTER.md, sections 2-4 and 8). P0 has no equipment, so the class
/// weapon stands in for it.
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PlayerTuning {
    /// Action points per turn before Dexterity, by race then class (CHARACTER 3.1).
    pub ap_base: [[u8; Class::COUNT]; Race::COUNT],
    /// One extra action point per this much Dexterity.
    pub ap_dex_div: u16,
    pub ap_min: u8,
    pub ap_max: u8,
    /// Initiative = Intelligence / this (CHARACTER 3.2).
    pub initiative_div: u16,
    /// Action points one attack or active skill costs.
    pub act_ap: u8,
    /// Reach in cells (Chebyshev distance) of strikes and projectiles.
    pub strike_range: u8,
    pub projectile_range: u8,
    /// Native state chance of a skill and of the basic attack.
    pub skill_state_chance_bp: u32,
    pub basic_state_chance_bp: u32,
    /// Class weapon damage `[min, max]` (Blunt), by class: the damage base of every skill.
    pub weapon: [[u32; 2]; Class::COUNT],
    /// Class tag increase on the class's own source, by class (0 = none).
    pub class_increase_bp: [u32; Class::COUNT],
    /// Class increase on heal, buff and debuff power, by class.
    pub support_increase_bp: [u32; Class::COUNT],
    /// HP a heal restores per point of the class weapon's range, bp.
    pub heal_weapon_bp: u32,
    /// Race trait: flat added damage `[min, max]` of the predisposition aspect.
    pub predisposition_added: [u32; 2],
    /// `d20` bonus = level + attack stat / this.
    pub attack_stat_div: u32,
    /// Crit chance per point of Dexterity.
    pub crit_per_dex_bp: u32,
    /// Defense = base + Dexterity / div.
    pub defense_base: u32,
    pub defense_dex_div: u32,
    /// Armor by class.
    pub armor: [u32; Class::COUNT],
    /// HP = base + Endurance x per point + (level - 1) x per level, by class.
    pub hp_base: [u32; Class::COUNT],
    pub hp_per_endurance: u32,
    pub hp_per_level: [u32; Class::COUNT],
    /// Creation: every stat starts at `stat_base`, `stat_points_start` points to spend, no stat
    /// above `stat_max_start`.
    pub stat_base: u16,
    pub stat_points_start: u16,
    pub stat_max_start: u16,
    /// Predisposition tier at creation (1 best .. 14 worst).
    pub start_tier: u8,
}

impl PlayerTuning {
    //                       Warrior  Mage  Support
    pub const DEFAULT: PlayerTuning = PlayerTuning {
        ap_base: [
            [4, 3, 4], // Human
            [5, 4, 5], // ForestElf
        ],
        ap_dex_div: 10,
        ap_min: 1,
        ap_max: 12,
        initiative_div: 2,
        act_ap: 2,
        strike_range: 1,
        projectile_range: 6,
        skill_state_chance_bp: 2_000,
        basic_state_chance_bp: 1_000,
        weapon: [[4, 8], [3, 6], [2, 5]],
        class_increase_bp: [2_000, 2_000, 0],
        support_increase_bp: [0, 0, 2_000],
        heal_weapon_bp: 20_000,
        predisposition_added: [1, 2],
        attack_stat_div: 5,
        crit_per_dex_bp: 20,
        defense_base: 10,
        defense_dex_div: 5,
        armor: [30, 5, 5],
        hp_base: [30, 18, 22],
        hp_per_endurance: 2,
        hp_per_level: [6, 3, 4],
        stat_base: 5,
        stat_points_start: 10,
        stat_max_start: 15,
        start_tier: 14,
    };
}

impl Default for PlayerTuning {
    fn default() -> Self {
        Self::DEFAULT
    }
}

/// How a monster template grows with its level (docs/BESTIARY.md, sections 7-8). The template
/// itself is content: [`crate::monster::MonsterBase`].
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MonsterScaling {
    /// Weapon minimum + level x this / BP.
    pub weapon_min_per_level_bp: u32,
    /// Weapon maximum + level x this / BP.
    pub weapon_max_per_level_bp: u32,
    /// Attack bonus + level x this / BP.
    pub attack_per_level_bp: u32,
    /// Action points per turn from the bestiary speed, clamped here.
    pub ap_min: u8,
    pub ap_max: u8,
}

impl MonsterScaling {
    pub const DEFAULT: MonsterScaling = MonsterScaling {
        weapon_min_per_level_bp: 5_000,
        weapon_max_per_level_bp: 10_000,
        attack_per_level_bp: 10_000,
        ap_min: 1,
        ap_max: 12,
    };
}

impl Default for MonsterScaling {
    fn default() -> Self {
        Self::DEFAULT
    }
}

/// Serde mirrors of the tables owned by other crates (they stay serde-free for the programs).
#[cfg(feature = "serde")]
mod remote {
    use super::*;
    use deadroll_skillcheck::SKILL_ASPECTS_MAX;
    use deadroll_types::StateKind;

    #[derive(Serialize, Deserialize)]
    #[serde(remote = "Balance")]
    pub struct BalanceDef {
        pub crit_more_bp: u32,
        pub armor_k: u32,
        pub resist_cap_bp: u32,
        pub chance_min_bp: u32,
        pub chance_max_bp: u32,
        pub min_damage: u32,
        pub tint_chance_bp: [u32; 14],
        pub resonance_more_bp: [u32; 14],
        pub pure_pair_bp: [u32; 14],
        pub state_turns: [u8; StateKind::COUNT],
        pub state_turns_max: u8,
        pub dot_share_bp: u32,
        pub chill_buildup_bp: u32,
        pub freeze_threshold_bp: u32,
        pub chill_slow_share_bp: u32,
    }

    #[derive(Serialize, Deserialize)]
    #[serde(remote = "SupportBalance")]
    pub struct SupportBalanceDef {
        pub mod_min_bp: [u32; deadroll_combat::MODS_MAX],
        pub mod_max_bp: [u32; deadroll_combat::MODS_MAX],
        pub mod_duration: [u8; deadroll_combat::MODS_MAX],
        pub rider_chance_bp: u32,
    }

    #[derive(Serialize, Deserialize)]
    #[serde(remote = "SkillTuning")]
    pub struct SkillTuningDef {
        pub shares_bp: [[u32; SKILL_ASPECTS_MAX]; SKILL_ASPECTS_MAX],
        pub line_counter_bp: u32,
        pub effect_counter_bp: u32,
        pub third_aspect_counter_bp: u32,
        pub active_cooldown: u8,
        pub reactive_cooldown: u8,
    }
}
