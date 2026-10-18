//! Values computed once at creation and stored with the skill (docs/SKILL_BUILDER.md,
//! sections 4.2, 5.3, 7, 8). Every number here is a BALANCE placeholder for `tools/sim`.
//!
//! Penalties are counterweights (`x * BP / (BP + c)`), never subtractions (COMBAT_MATH.md, 3).

use deadroll_types::{Aspect, BP};

use crate::{Category, Effect, Purpose, SkillBlocks, Target, SKILL_ASPECTS_MAX};

/// Shares of the converted weapon base by aspect count, bp, summing to `BP` (section 4.2).
/// BALANCE: placeholder.
pub const SHARES_BP: [[u32; SKILL_ASPECTS_MAX]; SKILL_ASPECTS_MAX] =
    [[BP, 0, 0], [6_500, 3_500, 0], [5_500, 3_000, 1_500]];
/// Counterweights on power, bp. BALANCE: placeholders.
pub const LINE_COUNTER_BP: u32 = 5_000;
pub const EFFECT_COUNTER_BP: u32 = 2_500;
pub const THIRD_ASPECT_COUNTER_BP: u32 = 2_000;
/// Cooldowns in the character's turns. BALANCE: placeholders.
pub const ACTIVE_COOLDOWN: u8 = 1;
pub const REACTIVE_COOLDOWN: u8 = 2;
/// Action points of an active skill (docs/PARTY_AND_TURNS.md, section 5.1): a base plus what the
/// blocks add. BALANCE: placeholders.
pub const AP_BASE: u8 = 2;
pub const AP_LINE: u8 = 1;
pub const AP_THIRD_ASPECT: u8 = 1;
pub const AP_EFFECT: u8 = 0;
pub const AP_HARD_CONTROL: u8 = 1;
/// Limits of the HP threshold of a reactive trigger, percent of the maximum. BALANCE: placeholders.
pub const HP_BELOW_MIN_PCT: u8 = 10;
pub const HP_BELOW_MAX_PCT: u8 = 90;

/// Stored next to the blocks; `deadroll_combat::Skill` is built from these by `session`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Derived {
    /// Share of the converted weapon base per aspect, bp; sums to `BP`.
    pub shares_bp: [u32; SKILL_ASPECTS_MAX],
    /// Multiplier on the weapon base (damage) or on the effect (buff, debuff), bp.
    pub power_bp: u32,
    /// [`fusion_id`] of the first two aspects, `0` = none.
    pub fusion: u8,
    /// Cooldown in the character's turns, `0` = none (passive).
    pub cooldown: u8,
    /// Action points to use it, `0` for reactive and passive skills (they are not actions).
    pub ap_cost: u8,
}

/// Fusion of an ordered aspect pair (section 7): `1 + first * 6 + second`, `0` = none.
/// The fusion damage takes the native state of `second`; names live in the registry table.
pub const fn fusion_id(first: Aspect, second: Aspect) -> u8 {
    if first as u8 == second as u8 {
        return 0;
    }
    1 + first as u8 * Aspect::COUNT as u8 + second as u8
}

const fn counter(value: u32, counter_bp: u32) -> u32 {
    ((value as u64 * BP as u64) / (BP as u64 + counter_bp as u64)) as u32
}

/// Every balance number of [`derive`] in one table, so `tools/balance` can tune them. The
/// consts above are its defaults.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SkillTuning {
    /// Shares of the converted weapon base by aspect count, bp, each row summing to `BP`.
    pub shares_bp: [[u32; SKILL_ASPECTS_MAX]; SKILL_ASPECTS_MAX],
    pub line_counter_bp: u32,
    pub effect_counter_bp: u32,
    pub third_aspect_counter_bp: u32,
    pub active_cooldown: u8,
    pub reactive_cooldown: u8,
    /// Action points of an active skill: `ap_base`, plus `ap_line` on a line, `ap_third_aspect`
    /// with three aspects, `ap_effect` for a damage skill's effect, `ap_hard_control` for Stun or
    /// Freeze.
    pub ap_base: u8,
    pub ap_line: u8,
    pub ap_third_aspect: u8,
    pub ap_effect: u8,
    pub ap_hard_control: u8,
    /// Limits of the HP threshold of `HpBelow` / `AllyHpBelow`, percent.
    pub hp_below_min_pct: u8,
    pub hp_below_max_pct: u8,
}

impl SkillTuning {
    pub const DEFAULT: SkillTuning = SkillTuning {
        shares_bp: SHARES_BP,
        line_counter_bp: LINE_COUNTER_BP,
        effect_counter_bp: EFFECT_COUNTER_BP,
        third_aspect_counter_bp: THIRD_ASPECT_COUNTER_BP,
        active_cooldown: ACTIVE_COOLDOWN,
        reactive_cooldown: REACTIVE_COOLDOWN,
        ap_base: AP_BASE,
        ap_line: AP_LINE,
        ap_third_aspect: AP_THIRD_ASPECT,
        ap_effect: AP_EFFECT,
        ap_hard_control: AP_HARD_CONTROL,
        hp_below_min_pct: HP_BELOW_MIN_PCT,
        hp_below_max_pct: HP_BELOW_MAX_PCT,
    };
}

impl Default for SkillTuning {
    fn default() -> Self {
        Self::DEFAULT
    }
}

/// Derived values of a skill that passed [`crate::validate`], with the default numbers.
pub fn derive(s: &SkillBlocks) -> Derived {
    derive_with(s, &SkillTuning::DEFAULT)
}

/// [`derive`] with the numbers of `t`.
pub fn derive_with(s: &SkillBlocks, t: &SkillTuning) -> Derived {
    let count = (s.aspect_count as usize).clamp(1, SKILL_ASPECTS_MAX);
    let mut power = BP;
    if s.target == Target::Line {
        power = counter(power, t.line_counter_bp);
    }
    if s.purpose == Purpose::Damage && s.effect != Effect::None {
        power = counter(power, t.effect_counter_bp);
    }
    if count == 3 {
        power = counter(power, t.third_aspect_counter_bp);
    }

    let fusion = if count == 2 {
        fusion_id(s.aspects[0], s.aspects[1])
    } else {
        0
    };

    let line = s.target == Target::Line;
    let third = count == 3;
    let hard = s.effect.hard_control_aspect().is_some();
    let cooldown = match s.category {
        Category::Passive => 0,
        Category::Reactive => t.reactive_cooldown,
        Category::Active => t.active_cooldown + line as u8 + third as u8 + hard as u8,
    };

    let add = |on: bool, ap: u8| if on { ap } else { 0 };
    let ap_cost = match s.category {
        Category::Reactive | Category::Passive => 0,
        Category::Active => t
            .ap_base
            .saturating_add(add(line, t.ap_line))
            .saturating_add(add(third, t.ap_third_aspect))
            .saturating_add(add(
                s.purpose == Purpose::Damage && s.effect != Effect::None,
                t.ap_effect,
            ))
            .saturating_add(add(hard, t.ap_hard_control)),
    };

    Derived {
        shares_bp: t.shares_bp[count - 1],
        power_bp: power,
        fusion,
        cooldown,
        ap_cost,
    }
}
