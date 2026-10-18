//! Skill blocks (docs/SKILL_BUILDER.md, section 2). Mechanics come only from these.

use deadroll_types::{Aspect, Form, RangeTag, Source, TimeTag};

u8_enum!(
    /// Section 3: active (4 slots), reactive (1), passive (1).
    Category { Active, Reactive, Passive }
);
u8_enum!(
    /// Buff targets self or an ally, debuff targets an enemy.
    Purpose { Damage, Buff, Debuff }
);
u8_enum!(
    /// Area comes later (append).
    Target { Single, Line }
);
u8_enum!(
    /// Section 6. `None` only for pure damage. Debuffs and hard control first, then buffs;
    /// `Heal` was appended later (a buff kind: restores HP up to the maximum).
    Effect { None, Weakening, Slow, Vulnerability, Stun, Freeze, Haste, Empowerment, Protection, Heal }
);
u8_enum!(
    /// Section 3.2, reactive skills only.
    Trigger { None, Hit, CritHit, HpBelow, AllyHpBelow, Controlled, EnemyAdjacent }
);

impl Trigger {
    /// `HpBelow` and `AllyHpBelow` need an HP threshold ([`SkillExt::hp_below_pct`]).
    pub const fn uses_threshold(self) -> bool {
        matches!(self, Trigger::HpBelow | Trigger::AllyHpBelow)
    }
}

impl Effect {
    /// Weakening, Slow, Vulnerability and hard control: allowed on damage and debuff skills.
    pub const fn is_debuff(self) -> bool {
        matches!(
            self,
            Effect::Weakening
                | Effect::Slow
                | Effect::Vulnerability
                | Effect::Stun
                | Effect::Freeze
        )
    }

    /// Haste, Empowerment, Protection (section 6.2) and Heal.
    pub const fn is_buff(self) -> bool {
        matches!(
            self,
            Effect::Haste | Effect::Empowerment | Effect::Protection | Effect::Heal
        )
    }

    /// Stun and Freeze: tied to their own aspect in the builder (section 6.1).
    pub const fn hard_control_aspect(self) -> Option<Aspect> {
        match self {
            Effect::Stun => Some(Aspect::Blunt),
            Effect::Freeze => Some(Aspect::Cold),
            _ => None,
        }
    }
}

/// Most aspects in one skill (section 4.1).
pub const SKILL_ASPECTS_MAX: usize = 3;

shared! {
    /// Everything that gives a skill its mechanics. Text (name, description) is not here.
    #[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
    pub struct SkillBlocks {
        pub category: Category,
        pub purpose: Purpose,
        /// Power source tag. One value, so magical and martial never mix.
        pub source: Source,
        /// Form tag. One value, so strike and projectile never mix.
        pub form: Form,
        pub target: Target,
        /// In order: the first is the main one (largest share). Only `aspect_count` are used;
        /// unused slots must stay at the default value.
        pub aspects: [Aspect; SKILL_ASPECTS_MAX],
        pub aspect_count: u8,
        pub effect: Effect,
        pub trigger: Trigger,
        /// Aura radius in cells, `0` = not an aura (section 3.5).
        pub aura_radius: u8,
    }
}

shared! {
    /// Blocks added in patch 5, stored next to [`SkillBlocks`] (a `SkillSet` record cannot grow
    /// in place): the range and timing tags (docs/DAMAGE_TYPES_AND_MIXES.md, section 2) and the
    /// HP threshold of a reactive trigger (docs/SKILL_BUILDER.md, section 3.2).
    #[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
    pub struct SkillExt {
        /// Melee or ranged: how far the skill reaches.
        pub range: RangeTag,
        /// Instant or over time. Only a tag for now (rules by tag come later).
        pub timing: TimeTag,
        /// `HpBelow` / `AllyHpBelow`: the trigger fires when HP drops below this percent of the
        /// maximum. `0` for every other skill.
        pub hp_below_pct: u8,
    }
}

impl SkillExt {
    /// Borsh size in bytes.
    pub const SIZE: usize = 3;
    /// Melee, instant, no threshold.
    pub const MELEE: SkillExt = SkillExt {
        range: RangeTag::Melee,
        timing: TimeTag::Instant,
        hp_below_pct: 0,
    };
}

impl SkillBlocks {
    /// Borsh size in bytes.
    pub const SIZE: usize = 5 + SKILL_ASPECTS_MAX + 1 + 3;

    /// The used aspects, in order. Clamped to the array size.
    pub fn used_aspects(&self) -> &[Aspect] {
        &self.aspects[..(self.aspect_count as usize).min(SKILL_ASPECTS_MAX)]
    }

    pub const fn is_aura(&self) -> bool {
        self.aura_radius > 0
    }
}
