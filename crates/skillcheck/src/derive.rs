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
}

impl SkillTuning {
    pub const DEFAULT: SkillTuning = SkillTuning {
        shares_bp: SHARES_BP,
        line_counter_bp: LINE_COUNTER_BP,
        effect_counter_bp: EFFECT_COUNTER_BP,
        third_aspect_counter_bp: THIRD_ASPECT_COUNTER_BP,
        active_cooldown: ACTIVE_COOLDOWN,
        reactive_cooldown: REACTIVE_COOLDOWN,
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

    let cooldown = match s.category {
        Category::Passive => 0,
        Category::Reactive => t.reactive_cooldown,
        Category::Active => {
            t.active_cooldown
                + (s.target == Target::Line) as u8
                + (count == 3) as u8
                + s.effect.hard_control_aspect().is_some() as u8
        }
    };

    Derived {
        shares_bp: t.shares_bp[count - 1],
        power_bp: power,
        fusion,
        cooldown,
    }
}
