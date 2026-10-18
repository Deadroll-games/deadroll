//! Builder checks (docs/SKILL_BUILDER.md, section 13) and the class limits on starting skills
//! (section 10.1).

use deadroll_types::{Aspect, Class, Source};

use crate::{
    Category, Effect, Purpose, SkillBlocks, SkillExt, SkillTuning, Target, Trigger,
    SKILL_ASPECTS_MAX,
};

/// Largest aura radius in cells. BALANCE: placeholder.
pub const AURA_RADIUS_MAX: u8 = 3;
/// Slots per map (section 3): 4 active, 1 reactive, 1 passive.
pub const ACTIVE_MAX: usize = 4;
pub const REACTIVE_MAX: usize = 1;
pub const PASSIVE_MAX: usize = 1;
/// Starting skills: one full loadout.
pub const STARTING_MAX: usize = ACTIVE_MAX + REACTIVE_MAX + PASSIVE_MAX;
/// An empty slot of a map selection.
pub const NO_SKILL: u8 = 0xFF;
/// The skills taken onto a map: `SkillSet` indexes, slots `0..4` active, `4` reactive,
/// `5` passive, [`NO_SKILL`] for an empty slot.
pub type MapSelection = [u8; STARTING_MAX];
/// Each category's first slot in a [`MapSelection`] and its number of slots.
pub const SELECTION_SLOTS: [(Category, usize, usize); 3] = [
    (Category::Active, 0, ACTIVE_MAX),
    (Category::Reactive, ACTIVE_MAX, REACTIVE_MAX),
    (Category::Passive, ACTIVE_MAX + REACTIVE_MAX, PASSIVE_MAX),
];

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
    /// Map selection: an index past the character's skills.
    SelectionUnknownSkill,
    /// Map selection: the same skill twice.
    SelectionRepeated,
    /// Map selection: a skill in a slot of another category.
    SelectionCategory,
    /// Map selection: not exactly min(slots, owned) skills of a category (section 13).
    SelectionCount,
    /// Heal is not in the builder yet: healing comes with its own class (CHARACTER.md, 8.1).
    HealNotAvailable,
    /// `HpBelow` / `AllyHpBelow` without an HP threshold (section 3.2).
    MissingThreshold,
    /// An HP threshold on a skill whose trigger has none.
    UnexpectedThreshold,
    /// HP threshold outside `SkillTuning::hp_below_min_pct..=hp_below_max_pct`.
    ThresholdRange,
    /// A passive debuff works only as an aura on the enemies around it.
    PassiveDebuffNeedsAura,
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
            SkillError::SelectionUnknownSkill => "the character has no such skill",
            SkillError::SelectionRepeated => "the same skill is chosen twice",
            SkillError::SelectionCategory => "a skill is in a slot of another category",
            SkillError::SelectionCount => {
                "take 4 active, 1 reactive and 1 passive (all of a kind if there are fewer)"
            }
            SkillError::HealNotAvailable => "healing is not available yet",
            SkillError::MissingThreshold => "this trigger needs an HP threshold",
            SkillError::UnexpectedThreshold => "only an HP trigger has a threshold",
            SkillError::ThresholdRange => "the HP threshold is out of range",
            SkillError::PassiveDebuffNeedsAura => "a passive debuff must be an aura",
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

    if s.effect == Effect::Heal {
        return Err(SkillError::HealNotAvailable);
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
    if s.category == Category::Passive && s.purpose == Purpose::Debuff && !s.is_aura() {
        return Err(SkillError::PassiveDebuffNeedsAura);
    }

    if s.is_aura() {
        if s.category == Category::Active {
            return Err(SkillError::AuraCategory);
        }
        // One aura = one effect: one stat, one resistance, one increase (section 3.5). Hard
        // control is none of these.
        if s.purpose == Purpose::Damage || s.effect.hard_control_aspect().is_some() {
            return Err(SkillError::AuraEffect);
        }
        if s.aura_radius > AURA_RADIUS_MAX {
            return Err(SkillError::AuraRadius);
        }
    }
    Ok(())
}

/// [`validate`] plus the rules of the patch 5 blocks, with the default numbers.
pub fn validate_skill(s: &SkillBlocks, e: &SkillExt) -> Result<(), SkillError> {
    validate_skill_with(s, e, &SkillTuning::DEFAULT)
}

/// [`validate_skill`] with the limits of `t`: an HP threshold exactly on the HP triggers, within
/// `t.hp_below_min_pct..=t.hp_below_max_pct`. The range and timing tags have no forbidden pairs
/// (DAMAGE_TYPES_AND_MIXES.md, 2.1).
pub fn validate_skill_with(
    s: &SkillBlocks,
    e: &SkillExt,
    t: &SkillTuning,
) -> Result<(), SkillError> {
    validate(s)?;
    match (s.trigger.uses_threshold(), e.hp_below_pct) {
        (true, 0) => Err(SkillError::MissingThreshold),
        (true, pct) if pct < t.hp_below_min_pct || pct > t.hp_below_max_pct => {
            Err(SkillError::ThresholdRange)
        }
        (false, pct) if pct != 0 => Err(SkillError::UnexpectedThreshold),
        _ => Ok(()),
    }
}

/// Builder rules plus the class limits on the first six skills (section 10.1).
pub fn validate_starting(s: &SkillBlocks, e: &SkillExt, class: Class) -> Result<(), SkillError> {
    validate_skill(s, e)?;
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

/// The skills taken onto a map (sections 3.4, 13): every slot holds a skill of its own category
/// or [`NO_SKILL`], no skill twice, and each category is full: exactly `min(slots, owned)`.
/// `owned` is the category of every `SkillSet` skill, by index.
pub fn check_map_selection(owned: &[Category], selection: &MapSelection) -> Result<(), SkillError> {
    for (i, &index) in selection.iter().enumerate() {
        if index != NO_SKILL && selection[i + 1..].contains(&index) {
            return Err(SkillError::SelectionRepeated);
        }
    }
    for (category, start, slots) in SELECTION_SLOTS {
        let mut chosen = 0usize;
        for &index in &selection[start..start + slots] {
            if index == NO_SKILL {
                continue;
            }
            match owned.get(index as usize) {
                None => return Err(SkillError::SelectionUnknownSkill),
                Some(c) if *c != category => return Err(SkillError::SelectionCategory),
                Some(_) => chosen += 1,
            }
        }
        let have = owned.iter().filter(|c| **c == category).count();
        if chosen != have.min(slots) {
            return Err(SkillError::SelectionCount);
        }
    }
    Ok(())
}
