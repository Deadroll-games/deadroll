//! Every balance number of the game rules in one table.
//!
//! [`BalanceConfig::DEFAULT`] is what the programs use today. All values are placeholders until
//! balancing (`tools/balance` tunes them). Tables indexed by an enum (`Race`, `Class`, ...) have
//! that enum's `COUNT` as their length, so a new variant does not compile until it has numbers.

use deadroll_combat::{Balance, SupportBalance};
use deadroll_skillcheck::SkillTuning;
use deadroll_types::{
    Aspect, Class, EquipSlot, Form, ItemBase, Race, RangeTag, StateKind, PARTY_MAX, STATS_COUNT,
};

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
    /// Patch 10: the session key's fee prepayment and lifetime (the website reads them). Missing
    /// in a bench file saved before it: the defaults.
    #[cfg_attr(feature = "serde", serde(default))]
    pub fees: FeeTuning,
    /// Patch 11: chests, reward budget, reward rolls, the bag (`crate::loot`). Missing in a bench
    /// file saved before it: the defaults.
    #[cfg_attr(feature = "serde", serde(default))]
    pub loot: LootTuning,
}

impl BalanceConfig {
    pub const DEFAULT: BalanceConfig = BalanceConfig {
        combat: Balance::DEFAULT,
        support: SupportBalance::DEFAULT,
        skills: SkillTuning::DEFAULT,
        players: PlayerTuning::DEFAULT,
        monsters: MonsterScaling::DEFAULT,
        items: ItemTuning::DEFAULT,
        fees: FeeTuning::DEFAULT,
        loot: LootTuning::DEFAULT,
    };
}

impl Default for BalanceConfig {
    fn default() -> Self {
        Self::DEFAULT
    }
}

/// Trait tiers, `1..=14` (1 is best): tables indexed by tier have this length, index `tier - 1`.
pub const TIERS: usize = 14;
/// Character levels in the first version, `1..=LEVELS` (docs/CHARACTER.md, section 5).
pub const LEVELS: usize = 4;

/// Hero numbers (docs/CHARACTER.md, sections 2-6 and 8). The weapon and item bonuses come from
/// [`ItemTuning`].
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PlayerTuning {
    /// Action points per turn before Dexterity and items, by race then class (CHARACTER 3.1).
    pub ap_base: [[u8; Class::COUNT]; Race::COUNT],
    /// One extra action point per this much Dexterity.
    pub ap_dex_div: u16,
    pub ap_min: u8,
    pub ap_max: u8,
    /// Initiative = Intelligence / this, plus items (CHARACTER 3).
    pub initiative_div: u16,
    /// Action points of the basic attack (players and the monsters' natural attack). An active
    /// skill costs its own (`SkillTuning`, stored with the skill).
    pub basic_attack_ap: u8,
    /// Reach in cells (Chebyshev distance) of melee and ranged skills and attacks.
    pub melee_range: u8,
    pub ranged_range: u8,
    /// "A nearby ally" of a reactive trigger (`AllyHpBelow`), in cells (Chebyshev distance).
    pub nearby_ally_cells: u8,
    /// Native state chance of a skill and of the basic attack, percent.
    pub skill_state_chance_pct: u32,
    pub basic_state_chance_pct: u32,
    /// Class trait: tag increase on the class's own source, by class then tier (0 = none).
    pub class_increase_by_tier_bp: [[u32; TIERS]; Class::COUNT],
    /// Class trait: increase on heal, buff and debuff power, by class then tier.
    pub support_increase_by_tier_bp: [[u32; TIERS]; Class::COUNT],
    /// HP a heal restores per point of the weapon's range, bp.
    pub heal_weapon_bp: u32,
    /// Race trait: flat added damage `[min, max]` of the predisposition aspect, by tier.
    pub predisposition_added_by_tier: [[u32; 2]; TIERS],
    /// Accuracy (the `d20` bonus) = base + Dexterity / div, plus items (CHARACTER 3).
    pub accuracy_base: u32,
    pub accuracy_dex_div: u32,
    /// Evasion (the hit threshold) = base + Dexterity / div, plus items (CHARACTER 3).
    pub defense_base: u32,
    pub defense_dex_div: u32,
    /// Damage increase per point, bp: Strength on martial strikes, Dexterity on martial
    /// projectiles, Intelligence on every magical packet (CHARACTER 2).
    pub str_damage_bp: u32,
    pub dex_projectile_bp: u32,
    pub int_magic_bp: u32,
    /// State and debuff chance increase per point, bp: Strength for martial skills,
    /// Intelligence for magical skills, Dexterity for tempo effects (Haste, Slow).
    pub str_state_chance_bp: u32,
    pub int_state_chance_bp: u32,
    pub dex_state_chance_bp: u32,
    /// Own buffs on self last one more turn per this much Endurance, at most `self_buff_turns_max`
    /// more (CHARACTER 2).
    pub self_buff_end_div: u16,
    pub self_buff_turns_max: u8,
    /// Patch 9. State resistance per point of Endurance, bp: the counterweight on the chance of
    /// every state and effect landing on the hero (CHARACTER 2-3, COMBAT_MATH 3).
    pub end_state_resist_bp: u32,
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
    /// Race trait (predisposition) tier at creation (1 best .. 14 worst).
    pub start_tier: u8,
    /// Class trait tier at creation (CHARACTER 6.3: starts low).
    pub class_start_tier: u8,
    /// Total experience needed for each level, level 1 first (CHARACTER 5).
    pub level_xp: [u32; LEVELS],
    /// Stat points per level gained (CHARACTER 5).
    pub stat_points_per_level: u8,
}

/// `2_000 + (14 - tier) x 250` bp: the class increase of the first version at the starting tier,
/// growing with better tiers.
const CLASS_TIERS: [u32; TIERS] = [
    5_250, 5_000, 4_750, 4_500, 4_250, 4_000, 3_750, 3_500, 3_250, 3_000, 2_750, 2_500, 2_250,
    2_000,
];

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
        basic_attack_ap: 2,
        melee_range: 1,
        ranged_range: 6,
        nearby_ally_cells: 3,
        skill_state_chance_pct: 20,
        basic_state_chance_pct: 10,
        class_increase_by_tier_bp: [CLASS_TIERS, CLASS_TIERS, [0; TIERS]],
        support_increase_by_tier_bp: [[0; TIERS], [0; TIERS], CLASS_TIERS],
        heal_weapon_bp: 20_000,
        // tier 1 .. tier 14
        predisposition_added_by_tier: [
            [4, 8],
            [4, 7],
            [4, 7],
            [3, 6],
            [3, 6],
            [3, 5],
            [3, 5],
            [2, 4],
            [2, 4],
            [2, 3],
            [2, 3],
            [1, 3],
            [1, 2],
            [1, 2],
        ],
        accuracy_base: 2,
        accuracy_dex_div: 5,
        defense_base: 10,
        defense_dex_div: 5,
        str_damage_bp: 200,
        dex_projectile_bp: 200,
        int_magic_bp: 200,
        str_state_chance_bp: 300,
        int_state_chance_bp: 300,
        dex_state_chance_bp: 300,
        self_buff_end_div: 5,
        self_buff_turns_max: 2,
        // Mirrors the stat bonus on the chance: one point of Endurance holds one of Strength.
        end_state_resist_bp: 300,
        // With the starter armor (10, 2, 5) the totals stay 30, 5, 5.
        armor: [20, 3, 0],
        hp_base: [30, 18, 22],
        hp_per_endurance: 2,
        hp_per_level: [6, 3, 4],
        stat_base: 5,
        stat_points_start: 10,
        stat_max_start: 15,
        start_tier: 14,
        class_start_tier: 14,
        level_xp: [0, 100, 300, 600],
        stat_points_per_level: 2,
    };
}

impl Default for PlayerTuning {
    fn default() -> Self {
        Self::DEFAULT
    }
}

/// How a monster template grows with its level, what an elite version gets and how many skills
/// each rank takes (docs/BESTIARY.md, sections 3.2, 4, 5). The template itself is registry data:
/// [`crate::monster::MonsterTemplate`].
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
    /// The "resistance" level of a bestiary record (docs/DAMAGE_TYPES_AND_MIXES.md 9): the
    /// counterweight on each aspect the monster resists, bp.
    pub resist_bp: u32,
    /// Patch 6. An elite's HP increase, bp (BESTIARY 5: "more HP").
    pub elite_hp_bp: u32,
    /// Skill slots by `monster::Rank` (regular, elite, boss), each by `Category` (active,
    /// reactive, passive): BESTIARY 3.2.
    pub slots: [[u8; deadroll_skillcheck::Category::COUNT]; crate::monster::Rank::COUNT],
    /// Monster AI (the simulator and the website's auto mode): heal an ally below this percent.
    pub ai_heal_below_pct: u8,
}

impl MonsterScaling {
    pub const DEFAULT: MonsterScaling = MonsterScaling {
        weapon_min_per_level_bp: 5_000,
        weapon_max_per_level_bp: 10_000,
        attack_per_level_bp: 10_000,
        ap_min: 1,
        ap_max: 12,
        resist_bp: 5_000,
        elite_hp_bp: 5_000,
        //      active reactive passive
        slots: [
            [3, 0, 1], // regular
            [4, 0, 1], // elite
            [5, 1, 1], // boss
        ],
        ai_heal_below_pct: 60,
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
    /// Tier of the item as a carrier of `aspect` (a resonance tint, docs/SKILL_BUILDER.md 5.2,
    /// DAMAGE_TYPES_AND_MIXES.md 4), `1..=14`; 0 = not a carrier.
    #[cfg_attr(feature = "serde", serde(default))]
    pub aspect_tier: u8,
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
    /// Crit chance besides a natural 20, percent (CHARACTER 3: items are a crit source).
    pub crit_pct: u32,
    /// Added to speed before the counterweights (light trousers and footwear, CHARACTER 3.1).
    pub speed: u16,
    /// Speed counterweight, bp: heavy armor divides speed, never to zero (CHARACTER 3.1).
    pub speed_counter_bp: u32,
    /// Added to initiative.
    pub initiative: u16,
    /// A weapon's basic attack (docs/SKILL_BUILDER.md 3.1: "a sword strike, a bow shot, a wand
    /// shot"): its range and form. Ignored on items that are not weapons.
    #[cfg_attr(feature = "serde", serde(default, with = "names::variant"))]
    pub basic_range: RangeTag,
    #[cfg_attr(feature = "serde", serde(default, with = "names::variant"))]
    pub basic_form: Form,
    /// Patch 9. Resistance per aspect, bp: a counterweight on the damage of that aspect
    /// (CHARACTER 3: items are a source of resistances).
    #[cfg_attr(feature = "serde", serde(default, with = "names::by_aspect"))]
    pub resist_bp: [u32; Aspect::COUNT],
    /// Patch 9. State resistance, bp: the counterweight on the chance of every state and effect.
    #[cfg_attr(feature = "serde", serde(default))]
    pub state_resist_bp: u32,
    /// Patch 9b. Resistance to one state, bp, by `StateKind`: adds to `state_resist_bp` for
    /// that state only.
    #[cfg_attr(feature = "serde", serde(default, with = "names::by_state"))]
    pub state_kind_resist_bp: [u32; StateKind::COUNT],
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

/// The tier of a starter weapon's aspect (CHARACTER 6.3: carriers start low).
const STARTER_ASPECT_TIER: u8 = 14;

/// A weapon whose basic attack is a melee strike.
const fn weapon(aspect: Aspect, damage: [u32; 2]) -> ItemBaseStats {
    ItemBaseStats {
        aspect,
        aspect_tier: STARTER_ASPECT_TIER,
        ..base(EquipSlot::MainHand, damage, 0)
    }
}

/// A weapon that shoots: a ranged projectile basic attack.
const fn shooter(aspect: Aspect, damage: [u32; 2]) -> ItemBaseStats {
    ItemBaseStats {
        basic_range: RangeTag::Ranged,
        basic_form: Form::Projectile,
        ..weapon(aspect, damage)
    }
}

const fn base(slot: EquipSlot, damage: [u32; 2], armor: u32) -> ItemBaseStats {
    ItemBaseStats {
        slot,
        level: 1,
        // Not a carrier: `aspect` is the weapon damage aspect only.
        aspect: Aspect::Blunt,
        aspect_tier: 0,
        damage,
        armor,
        defense: 0,
        attack: 0,
        stats: [0; STATS_COUNT],
        hp: 0,
        crit_pct: 0,
        speed: 0,
        speed_counter_bp: 0,
        initiative: 0,
        basic_range: RangeTag::Melee,
        basic_form: Form::Strike,
        resist_bp: [0; Aspect::COUNT],
        state_resist_bp: 0,
        state_kind_resist_bp: [0; StateKind::COUNT],
    }
}

impl ItemTuning {
    //                                                   Sword  Staff  Wand  Leather  Robe  Light
    pub const DEFAULT: ItemTuning = ItemTuning {
        bases: [
            // A Piercing sword, a Blunt staff, a Lightning wand (SKILL_BUILDER 5.1). The sword
            // strikes, the staff and the wand shoot (3.1).
            weapon(Aspect::Piercing, [4, 8]),
            shooter(Aspect::Blunt, [3, 6]),
            shooter(Aspect::Lightning, [2, 5]),
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

/// The temporary key of a party (docs/GAME_COMMISSIONS.md, 3 and 6.2): what the main wallet moves
/// to it for turn and VRF fees, and how long its MagicBlock token lives. Lamports.
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FeeTuning {
    /// Moved to the key when joining (or opening) a party.
    pub prepay_lamports: u64,
    /// The website warns below this balance.
    pub low_balance_lamports: u64,
    /// The website does not top the key up above this balance.
    pub max_key_lamports: u64,
    /// `SessionTokenV2.valid_until` = now + this, seconds (the Session Keys program allows a week).
    pub key_validity_secs: u32,
}

impl FeeTuning {
    pub const DEFAULT: FeeTuning = FeeTuning {
        prepay_lamports: 10_000_000,
        low_balance_lamports: 2_000_000,
        max_key_lamports: 50_000_000,
        key_validity_secs: 3 * 60 * 60,
    };
}

impl Default for FeeTuning {
    fn default() -> Self {
        Self::DEFAULT
    }
}

/// Map tiers with numbers (`deadroll_mapcheck::limits`): tables by map tier have this length,
/// index `tier - 1`.
pub const MAP_TIERS: usize = deadroll_mapcheck::limits::TIER_MAX as usize;
/// Steps of [`LootTuning::over_tier_xp_bp`]: 1, 2 or more levels above the map tier.
pub const OVER_TIER_STEPS: usize = 2;

/// Patch 11: rewards (docs/PARTY_AND_TURNS.md 9, docs/MAP_DESCRIPTION.md 7.1 and 10.2,
/// docs/ITEMS_AND_CRAFT.md 6.1). The reward tables themselves are registry data.
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LootTuning {
    /// Visible chests a map may have, by map tier.
    pub chests: [u8; MAP_TIERS],
    /// Reward budget of a map (chests and boss table costs), by map tier, then by the map's
    /// minimum players (index `min_players - 1`): it grows with party size.
    pub budget: [[u16; PARTY_MAX]; MAP_TIERS],
    /// Registry reward table ids a DM may put on a map, by map tier, bit `1 << id`. A table
    /// whose top level is above the tier is refused anyway.
    pub allowed_tables: [u64; MAP_TIERS],
    /// Chance, in basis points, that a reward event's level goes one step above the table's
    /// base level; rolled again for every further step (the base level drops most often).
    pub level_step_bp: u16,
    /// Action points to open a chest (docs/PARTY_AND_TURNS.md 5.1).
    pub chest_open_ap: u8,
    /// The radius a DM gives a map (`Map.chest_radius`): no living monster may be this close
    /// (Chebyshev distance) to a chest or to any living player when a chest opens. Inclusive.
    pub chest_radius_min: u8,
    pub chest_radius_max: u8,
    /// The time a DM gives a map (`Map.settle_timeout_secs`) to issue the rewards after a party
    /// ends; after it every player claims their own cache and gets their contribution back.
    /// Inclusive; a `mock-vrf` session build (localnet only) accepts from 1 s.
    pub settle_timeout_min_secs: u32,
    pub settle_timeout_max_secs: u32,
    /// Experience of a character above the map tier's top level, by levels above (1, then 2 or
    /// more), basis points (docs/PARTY_AND_TURNS.md 9.5).
    pub over_tier_xp_bp: [u16; OVER_TIER_STEPS],
    /// From this many levels above the map tier no rewards are rolled for the character.
    pub over_tier_no_loot: u8,
    /// Units in one bag resource slot (docs/ITEMS_AND_CRAFT.md 6.1).
    pub resource_stack_max: u8,
    /// Identical consumables in one bag consumable slot.
    pub consumable_stack_max: u8,
}

impl LootTuning {
    pub const DEFAULT: LootTuning = LootTuning {
        chests: [1, 2],
        budget: [[6, 8, 10, 12], [10, 13, 16, 20]],
        // Tier 1: the tier 1 stub chest and boss tables; tier 2 also its own
        // (assets/registry/reward_tables.json).
        allowed_tables: [0b0011, 0b1111],
        level_step_bp: 2_500,
        chest_open_ap: 1,
        chest_radius_min: 1,
        chest_radius_max: 10,
        // 10 minutes to 10 hours.
        settle_timeout_min_secs: 10 * 60,
        settle_timeout_max_secs: 10 * 60 * 60,
        over_tier_xp_bp: [5_000, 0],
        over_tier_no_loot: 2,
        resource_stack_max: 10,
        consumable_stack_max: 5,
    };
}

impl Default for LootTuning {
    fn default() -> Self {
        Self::DEFAULT
    }
}

/// Enums by variant name in JSON (the name is the `Debug` text, the list is the enum's `ALL`).
#[cfg(feature = "serde")]
pub mod names {
    use std::{collections::BTreeMap, fmt::Debug, format, string::String, vec::Vec};

    use deadroll_types::{
        Aspect, Class, EquipSlot, Form, ItemBase, RangeTag, Stat, StateKind, STATS_COUNT,
    };
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
    variant!(Aspect, Class, EquipSlot, Form, ItemBase, RangeTag, Stat, StateKind);

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

    /// `[u32; Aspect::COUNT]` keyed by `Aspect`.
    pub mod by_aspect {
        use super::*;

        pub fn serialize<S: Serializer>(v: &[u32; Aspect::COUNT], s: S) -> Result<S::Ok, S::Error> {
            ser_keyed::<Aspect, _, _>(v, s)
        }

        pub fn deserialize<'de, D: Deserializer<'de>>(
            d: D,
        ) -> Result<[u32; Aspect::COUNT], D::Error> {
            de_keyed::<Aspect, _, { Aspect::COUNT }, _>(d)
        }
    }

    /// `[u32; StateKind::COUNT]` keyed by `StateKind`.
    pub mod by_state {
        use super::*;

        pub fn serialize<S: Serializer>(
            v: &[u32; StateKind::COUNT],
            s: S,
        ) -> Result<S::Ok, S::Error> {
            ser_keyed::<StateKind, _, _>(v, s)
        }

        pub fn deserialize<'de, D: Deserializer<'de>>(
            d: D,
        ) -> Result<[u32; StateKind::COUNT], D::Error> {
            de_keyed::<StateKind, _, { StateKind::COUNT }, _>(d)
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

    /// Presets written before the resonance pass have no Exposed curve.
    fn exposed_default() -> [u32; 14] {
        Balance::DEFAULT.res_exposed_pct
    }

    #[derive(Serialize, Deserialize)]
    #[serde(remote = "Balance")]
    pub struct BalanceDef {
        pub crit_more_bp: u32,
        pub armor_k: u32,
        pub resist_cap_bp: u32,
        pub chance_min_pct: u32,
        pub chance_max_pct: u32,
        pub min_damage: u32,
        pub tint_chance_pct: [u32; 14],
        pub resonance_more_bp: [u32; 14],
        pub pure_pair_bp: [u32; 14],
        pub state_turns: [u8; StateKind::COUNT],
        pub state_turns_max: u8,
        pub dot_share_bp: u32,
        pub chill_buildup_bp: u32,
        pub freeze_threshold_bp: u32,
        pub chill_slow_share_bp: u32,
        pub shock_vuln_bp: u32,
        pub penetration_power_bp: u32,
        pub keenness_pen_bp: [u32; 14],
        pub extra_state_chance_pct: u32,
        pub res_turns: [u8; 14],
        pub res_power_bp: [u32; 14],
        pub res_mod_bp: [u32; 14],
        pub res_mod_turns: u8,
        pub tint_added: [[u32; 2]; 14],
        pub res_accuracy: [u32; 14],
        pub res_accuracy_turns: u8,
        pub burning_cell_rounds: u8,
        pub aspect_state_resist_share_bp: u32,
        #[serde(default = "exposed_default")]
        pub res_exposed_pct: [u32; 14],
    }

    #[derive(Serialize, Deserialize)]
    #[serde(remote = "SupportBalance")]
    pub struct SupportBalanceDef {
        pub mod_min_bp: [u32; deadroll_combat::MODS_MAX],
        pub mod_max_bp: [u32; deadroll_combat::MODS_MAX],
        pub mod_duration: [u8; deadroll_combat::MODS_MAX],
        pub rider_chance_pct: u32,
        pub passive_point_bp: u32,
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
        pub ap_base: u8,
        pub ap_line: u8,
        pub ap_third_aspect: u8,
        pub ap_effect: u8,
        pub ap_hard_control: u8,
        pub hp_below_min_pct: u8,
        pub hp_below_max_pct: u8,
    }
}
