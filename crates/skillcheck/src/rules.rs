//! Builder checks (docs/SKILL_BUILDER.md, section 13) and the class limits on starting skills
//! (section 10.1).

use deadroll_types::{Aspect, Class, Source};

use crate::{Category, Effect, Purpose, SkillBlocks, Target, Trigger, SKILL_ASPECTS_MAX};

/// Largest aura radius in cells. BALANCE: placeholder.
pub const AURA_RADIUS_MAX: u8 = 3;
/// Slots per map (section 3): 4 active, 1 reactive, 1 passive.
pub const ACTIVE_MAX: usize = 4;
pub const REACTIVE_MAX: usize = 1;
pub const PASSIVE_MAX: usize = 1;
/// Starting skills: one full loadout.
pub const STARTING_MAX: usize = ACTIVE_MAX + REACTIVE_MAX + PASSIVE_MAX;

/// Why a skill was rejected. One variant per rule.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum SkillError {
    /// The number of aspects does not fit the purpose (section 4.1).
    AspectCount,
    /// The same aspect twice in one skill.
    RepeatedAspect,
    /// An unused aspect slot is not at the default value (keeps one canonical encoding).
    UnusedAspectSet,
    /// A third aspect needs a single target (section 4.3).
    ThirdAspectTarget,
    /// An effect needs exactly one damage aspect (section 13).
    EffectNeedsOneAspect,
    /// The effect kind does not fit the purpose (buff effect on a debuff, ...).
    EffectKind,
    /// A buff or debuff without its effect.
    MissingEffect,
    /// Stun only with Blunt, Freeze only with Cold (section 6.1).
    HardControlAspect,
    /// A reactive skill without a trigger condition (section 3.2).
    MissingTrigger,
    /// A trigger on a skill that is not reactive.
    UnexpectedTrigger,
    /// A passive skill must be a buff or a debuff: it works constantly (section 3.3).
    PassiveDamage,
    /// An aura on an active skill (section 3.5).
    AuraCategory,
    /// An aura must give exactly one buff or debuff effect (section 3.5).
    AuraEffect,
    /// Aura radius above `AURA_RADIUS_MAX`.
    AuraRadius,
    /// Starting skill: power source not allowed by the class (section 10.1).
    ClassSource,
    /// Starting skill: purpose not allowed by the class (section 10.1).
    ClassPurpose,
    /// More skills of a category than the slots (section 3.4).
    TooManyActive,
    TooManyReactive,
    TooManyPassive,
}

impl SkillError {
    /// A short English explanation for builder UIs.
    pub const fn message(self) -> &'static str {
        match self {
            SkillError::AspectCount => "the number of aspects does not fit the purpose",
            SkillError::RepeatedAspect => "the same aspect twice",
            SkillError::UnusedAspectSet => "an unused aspect slot is set",
            SkillError::ThirdAspectTarget => "a third aspect needs a single target",
            SkillError::EffectNeedsOneAspect => "an effect needs exactly one damage aspect",
            SkillError::EffectKind => "the effect kind does not fit the purpose",
            SkillError::MissingEffect => "a buff or debuff needs an effect",
            SkillError::HardControlAspect => "Stun needs Blunt, Freeze needs Cold",
            SkillError::MissingTrigger => "a reactive skill needs a trigger",
            SkillError::UnexpectedTrigger => "only reactive skills have a trigger",
            SkillError::PassiveDamage => "a passive skill must be a buff or a debuff",
            SkillError::AuraCategory => "an active skill cannot be an aura",
            SkillError::AuraEffect => "an aura must give a buff or debuff effect",
            SkillError::AuraRadius => "aura radius is too large",
            SkillError::ClassSource => "the class does not allow this power source",
            SkillError::ClassPurpose => "the class does not allow this purpose",
            SkillError::TooManyActive => "more than 4 active skills",
            SkillError::TooManyReactive => "more than 1 reactive skill",
            SkillError::TooManyPassive => "more than 1 passive skill",
        }
    }
}

/// Builder rules for any skill (starting or cut from an uncut skill).
pub fn validate(s: &SkillBlocks) -> Result<(), SkillError> {
    let count = s.aspect_count as usize;
    if count == 0 || count > SKILL_ASPECTS_MAX {
        return Err(SkillError::AspectCount);
    }
    let used = s.used_aspects();
    for (i, a) in used.iter().enumerate() {
        if used[i + 1..].contains(a) {
            return Err(SkillError::RepeatedAspect);
        }
    }
    if s.aspects[count..].iter().any(|a| *a != Aspect::default()) {
        return Err(SkillError::UnusedAspectSet);
    }

    match s.purpose {
        Purpose::Damage => {
            if count == 3 && s.target != Target::Single {
                return Err(SkillError::ThirdAspectTarget);
            }
            if s.effect != Effect::None {
                // Fusion (2 aspects) and the third aspect both exclude an effect.
                if count != 1 {
                    return Err(SkillError::EffectNeedsOneAspect);
                }
                if !s.effect.is_debuff() {
                    return Err(SkillError::EffectKind);
                }
            }
        }
        Purpose::Buff | Purpose::Debuff => {
            if count != 1 {
                return Err(SkillError::AspectCount);
            }
            if s.effect == Effect::None {
                return Err(SkillError::MissingEffect);
            }
            let fits = match s.purpose {
                Purpose::Buff => s.effect.is_buff(),
                _ => s.effect.is_debuff(),
            };
            if !fits {
                return Err(SkillError::EffectKind);
            }
        }
    }

    if let Some(aspect) = s.effect.hard_control_aspect() {
        if s.aspects[0] != aspect {
            return Err(SkillError::HardControlAspect);
        }
    }

    match (s.category, s.trigger) {
        (Category::Reactive, Trigger::None) => return Err(SkillError::MissingTrigger),
        (Category::Reactive, _) | (_, Trigger::None) => {}
        _ => return Err(SkillError::UnexpectedTrigger),
    }

    if s.category == Category::Passive && s.purpose == Purpose::Damage {
        return Err(SkillError::PassiveDamage);
    }

    if s.is_aura() {
        if s.category == Category::Active {
            return Err(SkillError::AuraCategory);
        }
        if s.purpose == Purpose::Damage {
            return Err(SkillError::AuraEffect);
        }
        if s.aura_radius > AURA_RADIUS_MAX {
            return Err(SkillError::AuraRadius);
        }
    }
    Ok(())
}

/// Builder rules plus the class limits on the first six skills (section 10.1).
pub fn validate_starting(s: &SkillBlocks, class: Class) -> Result<(), SkillError> {
    validate(s)?;
    match class {
        Class::Warrior if s.source != Source::Combat => Err(SkillError::ClassSource),
        Class::Mage if s.source != Source::Magical => Err(SkillError::ClassSource),
        Class::Support if s.purpose == Purpose::Damage => Err(SkillError::ClassPurpose),
        _ => Ok(()),
    }
}

/// At most 4 active, 1 reactive, 1 passive (fewer is fine).
pub fn check_loadout<'a>(
    skills: impl IntoIterator<Item = &'a SkillBlocks>,
) -> Result<(), SkillError> {
    let (mut active, mut reactive, mut passive) = (0usize, 0usize, 0usize);
    for s in skills {
        match s.category {
            Category::Active => active += 1,
            Category::Reactive => reactive += 1,
            Category::Passive => passive += 1,
        }
    }
    if active > ACTIVE_MAX {
        return Err(SkillError::TooManyActive);
    }
    if reactive > REACTIVE_MAX {
        return Err(SkillError::TooManyReactive);
    }
    if passive > PASSIVE_MAX {
        return Err(SkillError::TooManyPassive);
    }
    Ok(())
}
