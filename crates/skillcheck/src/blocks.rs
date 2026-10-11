//! Skill blocks (docs/SKILL_BUILDER.md, section 2). Mechanics come only from these.

use deadroll_types::{Aspect, Form, Source};

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
    /// Section 6. `None` only for pure damage. Debuffs and hard control first, then buffs.
    Effect { None, Weakening, Slow, Vulnerability, Stun, Freeze, Haste, Empowerment, Protection }
);
u8_enum!(
    /// Section 3.2, reactive skills only.
    Trigger { None, Hit, CritHit, HpBelow, AllyHpBelow, Controlled, EnemyAdjacent }
);

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

    /// Haste, Empowerment, Protection (section 6.2).
    pub const fn is_buff(self) -> bool {
        matches!(
            self,
            Effect::Haste | Effect::Empowerment | Effect::Protection
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
