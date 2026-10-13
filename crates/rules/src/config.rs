//! Every balance number of the game rules in one table.
//!
//! [`BalanceConfig::DEFAULT`] is what the programs use today. All values are placeholders until
//! balancing (`tools/balance` tunes them). Tables indexed by an enum (`Race`, `Class`, ...) have
//! that enum's `COUNT` as their length, so a new variant does not compile until it has numbers.

use deadroll_combat::{Balance, SupportBalance};
use deadroll_skillcheck::SkillTuning;
use deadroll_types::{Aspect, Class, EquipSlot, ItemBase, Race, STATS_COUNT};

use crate::items::KIT_MAX;

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
    /// Item bases and class starter kits (`crate::items`).
    pub items: ItemTuning,
}

impl BalanceConfig {
    pub const DEFAULT: BalanceConfig = BalanceConfig {
        combat: Balance::DEFAULT,
        support: SupportBalance::DEFAULT,
        skills: SkillTuning::DEFAULT,
        players: PlayerTuning::DEFAULT,
        monsters: MonsterScaling::DEFAULT,
        items: ItemTuning::DEFAULT,
    };
}

impl Default for BalanceConfig {
    fn default() -> Self {
        Self::DEFAULT
    }
}

/// Hero numbers (docs/CHARACTER.md, sections 2-4 and 8). The weapon and item armor come from
/// [`ItemTuning`].
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
    /// Class tag increase on the class's own source, by class (0 = none).
    pub class_increase_bp: [u32; Class::COUNT],
    /// Class increase on heal, buff and debuff power, by class.
    pub support_increase_bp: [u32; Class::COUNT],
    /// HP a heal restores per point of the weapon's range, bp.
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
    /// Base armor by class; worn items add theirs.
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
        class_increase_bp: [2_000, 2_000, 0],
        support_increase_bp: [0, 0, 2_000],
        heal_weapon_bp: 20_000,
        predisposition_added: [1, 2],
        attack_stat_div: 5,
        crit_per_dex_bp: 20,
        defense_base: 10,
        defense_dex_div: 5,
        // With the starter armor (10, 2, 5) the totals stay 30, 5, 5.
        armor: [20, 3, 0],
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

/// Base numbers of one item base (docs/ITEMS_AND_CRAFT.md, section 3.1). Flat values only.
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ItemBaseStats {
    /// Where it is worn (rings fit either ring slot).
    #[cfg_attr(feature = "serde", serde(with = "names::variant"))]
    pub slot: EquipSlot,
    /// Item level: the character needs `level - ItemTuning.level_gap`.
    pub level: u8,
    /// Weapon damage aspect.
    #[cfg_attr(feature = "serde", serde(with = "names::variant"))]
    pub aspect: Aspect,
    /// Weapon damage `[min, max]`; `[0, 0]` = not a weapon. The main-hand weapon is the damage
    /// base of every attack and skill.
    pub damage: [u32; 2],
    pub armor: u32,
    /// Added to the defense threshold.
    pub defense: u32,
    /// Added to the `d20` attack bonus.
    pub attack: u32,
    /// Added to the core stats, by `Stat`.
    #[cfg_attr(feature = "serde", serde(with = "names::by_stat"))]
    pub stats: [u16; STATS_COUNT],
    pub hp: u32,
}

/// Items (docs/ITEMS_AND_CRAFT.md): every base's numbers and the class starter kits.
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ItemTuning {
    /// By `ItemBase`.
    #[cfg_attr(feature = "serde", serde(with = "names::bases"))]
    pub bases: [ItemBaseStats; ItemBase::COUNT],
    /// Soulbound items a new character gets, by class (ITEMS_AND_CRAFT 10).
    #[cfg_attr(feature = "serde", serde(with = "names::kits"))]
    pub starter_kits: [[Option<ItemBase>; KIT_MAX]; Class::COUNT],
    /// Damage `[min, max]` (Blunt) with no weapon in the main hand.
    pub unarmed: [u32; 2],
    /// A character may wear items up to this many levels above their own (ITEMS_AND_CRAFT 4.2).
    pub level_gap: u8,
}

const fn base(slot: EquipSlot, damage: [u32; 2], armor: u32) -> ItemBaseStats {
    ItemBaseStats {
        slot,
        level: 1,
        // Blunt until balancing: the starter weapons keep the numbers of the old class weapon.
        aspect: Aspect::Blunt,
        damage,
        armor,
        defense: 0,
        attack: 0,
        stats: [0; STATS_COUNT],
        hp: 0,
    }
}

impl ItemTuning {
    //                                                   Sword  Staff  Wand  Leather  Robe  Light
    pub const DEFAULT: ItemTuning = ItemTuning {
        bases: [
            base(EquipSlot::MainHand, [4, 8], 0),
            base(EquipSlot::MainHand, [3, 6], 0),
            base(EquipSlot::MainHand, [2, 5], 0),
            base(EquipSlot::Body, [0, 0], 10),
            base(EquipSlot::Body, [0, 0], 2),
            base(EquipSlot::Body, [0, 0], 5),
        ],
        starter_kits: [
            [
                Some(ItemBase::Sword),
                Some(ItemBase::LeatherArmor),
                None,
                None,
            ],
            [Some(ItemBase::Staff), Some(ItemBase::Robe), None, None],
            [
                Some(ItemBase::Wand),
                Some(ItemBase::LightLeatherArmor),
                None,
                None,
            ],
        ],
        unarmed: [1, 2],
        level_gap: 4,
    };

    pub fn base(&self, base: ItemBase) -> &ItemBaseStats {
        &self.bases[base as usize]
    }
}

impl Default for ItemTuning {
    fn default() -> Self {
        Self::DEFAULT
    }
}

/// Enums by variant name in JSON (the name is the `Debug` text, the list is the enum's `ALL`).
#[cfg(feature = "serde")]
pub mod names {
    use std::{collections::BTreeMap, fmt::Debug, format, string::String, vec::Vec};

    use deadroll_types::{Aspect, Class, EquipSlot, ItemBase, Stat, STATS_COUNT};
    use serde::{de::Error as _, ser::SerializeMap, Deserialize, Deserializer, Serializer};

    use super::{ItemBaseStats, KIT_MAX};

    pub trait Variant: Copy + Debug + PartialEq + 'static {
        fn all() -> &'static [Self];

        fn name(self) -> String {
            format!("{self:?}")
        }

        fn parse(text: &str) -> Option<Self> {
            Self::all().iter().copied().find(|v| v.name() == text)
        }

        fn index(self) -> usize {
            Self::all().iter().position(|v| *v == self).unwrap_or(0)
        }
    }

    macro_rules! variant {
        ($($ty:ty),*) => {$(
            impl Variant for $ty {
                fn all() -> &'static [Self] {
                    <$ty>::ALL
                }
            }
        )*};
    }
    variant!(Aspect, Class, EquipSlot, ItemBase, Stat);

    fn parse<'de, E: Variant, D: Deserializer<'de>>(text: &str) -> Result<E, D::Error> {
        E::parse(text).ok_or_else(|| {
            let names: Vec<String> = E::all().iter().map(|v| v.name()).collect();
            D::Error::custom(format!(
                "unknown `{text}`, expected one of: {}",
                names.join(", ")
            ))
        })
    }

    fn ser_keyed<E: Variant, T: serde::Serialize, S: Serializer>(
        v: &[T],
        s: S,
    ) -> Result<S::Ok, S::Error> {
        let mut map = s.serialize_map(Some(v.len()))?;
        for (e, t) in E::all().iter().zip(v) {
            map.serialize_entry(&e.name(), t)?;
        }
        map.end()
    }

    fn de_keyed<'de, E: Variant, T, const N: usize, D>(d: D) -> Result<[T; N], D::Error>
    where
        T: Deserialize<'de> + Default + Copy,
        D: Deserializer<'de>,
    {
        let mut out = [T::default(); N];
        for (key, value) in BTreeMap::<String, T>::deserialize(d)? {
            let e: E = parse::<E, D>(&key)?;
            if let Some(slot) = out.get_mut(e.index()) {
                *slot = value;
            }
        }
        Ok(out)
    }

    /// One variant as its name.
    pub mod variant {
        use super::*;

        pub fn serialize<E: Variant, S: Serializer>(v: &E, s: S) -> Result<S::Ok, S::Error> {
            s.serialize_str(&v.name())
        }

        pub fn deserialize<'de, E: Variant, D: Deserializer<'de>>(d: D) -> Result<E, D::Error> {
            parse::<E, D>(&String::deserialize(d)?)
        }
    }

    /// `[u16; STATS_COUNT]` keyed by `Stat`.
    pub mod by_stat {
        use super::*;

        pub fn serialize<S: Serializer>(v: &[u16; STATS_COUNT], s: S) -> Result<S::Ok, S::Error> {
            ser_keyed::<Stat, _, _>(v, s)
        }

        pub fn deserialize<'de, D: Deserializer<'de>>(
            d: D,
        ) -> Result<[u16; STATS_COUNT], D::Error> {
            de_keyed::<Stat, _, STATS_COUNT, _>(d)
        }
    }

    /// Item bases keyed by `ItemBase`.
    pub mod bases {
        use super::*;

        pub fn serialize<S: Serializer>(
            v: &[ItemBaseStats; ItemBase::COUNT],
            s: S,
        ) -> Result<S::Ok, S::Error> {
            ser_keyed::<ItemBase, _, _>(v, s)
        }

        pub fn deserialize<'de, D: Deserializer<'de>>(
            d: D,
        ) -> Result<[ItemBaseStats; ItemBase::COUNT], D::Error> {
            de_keyed::<ItemBase, _, { ItemBase::COUNT }, _>(d)
        }
    }

    /// Starter kits keyed by `Class`, each `KIT_MAX` base names or `null` (fixed length, so a
    /// config file laid over the defaults replaces a kit entry by entry).
    pub mod kits {
        use super::*;

        type Kits = [[Option<ItemBase>; KIT_MAX]; Class::COUNT];

        pub fn serialize<S: Serializer>(v: &Kits, s: S) -> Result<S::Ok, S::Error> {
            let mut map = s.serialize_map(Some(v.len()))?;
            for (class, kit) in Class::ALL.iter().zip(v) {
                let names: Vec<Option<String>> = kit.iter().map(|b| b.map(|b| b.name())).collect();
                map.serialize_entry(&class.name(), &names)?;
            }
            map.end()
        }

        pub fn deserialize<'de, D: Deserializer<'de>>(d: D) -> Result<Kits, D::Error> {
            let mut out = [[None; KIT_MAX]; Class::COUNT];
            for (key, list) in BTreeMap::<String, Vec<Option<String>>>::deserialize(d)? {
                let class: Class = parse::<Class, D>(&key)?;
                if list.len() > KIT_MAX {
                    return Err(D::Error::custom(format!(
                        "{key}: at most {KIT_MAX} starter items"
                    )));
                }
                let kit = &mut out[class as usize];
                for (slot, name) in kit.iter_mut().zip(&list) {
                    *slot = match name {
                        Some(name) => Some(parse::<ItemBase, D>(name)?),
                        None => None,
                    };
                }
            }
            Ok(out)
        }
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
