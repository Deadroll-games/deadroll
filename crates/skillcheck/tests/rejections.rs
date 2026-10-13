//! One rejection of `validate`, `validate_starting` or `check_loadout` per test.
//! Each case starts from a valid preset and changes only what the rule looks at.
//! Happy paths live in `tests/presets.rs`.

use deadroll_skillcheck::{
    check_loadout, presets, validate, validate_starting, Category, Effect, Purpose, SkillBlocks,
    SkillError, Target, Trigger, AURA_RADIUS_MAX,
};
use deadroll_types::{Aspect, Class, Form, Source};

#[test]
fn aspect_count_zero() {
    let mut s = presets::WARRIOR[0].blocks;
    s.aspect_count = 0;
    assert_eq!(validate(&s), Err(SkillError::AspectCount));
}

#[test]
fn aspect_count_four() {
    let mut s = presets::WARRIOR[0].blocks;
    s.aspect_count = 4;
    assert_eq!(validate(&s), Err(SkillError::AspectCount));
}

#[test]
fn aspect_count_buff_with_two() {
    // Quicken is a one-aspect buff. A second aspect is still `AspectCount`.
    let mut s = presets::SUPPORT[0].blocks;
    s.aspect_count = 2;
    assert_eq!(validate(&s), Err(SkillError::AspectCount));
}

#[test]
fn aspect_count_debuff_with_two() {
    // Sap is a one-aspect debuff.
    let mut s = presets::SUPPORT[1].blocks;
    s.aspect_count = 2;
    assert_eq!(validate(&s), Err(SkillError::AspectCount));
}

#[test]
fn repeated_aspect_cold_cold() {
    // Frost Burn is [Cold, Fire] with count 2. The second slot becomes a repeat.
    let mut s = presets::MAGE[1].blocks;
    s.aspects[1] = Aspect::Cold;
    assert_eq!(validate(&s), Err(SkillError::RepeatedAspect));
}

#[test]
fn repeated_aspect_fire_cold_fire() {
    // Count 3, single target, no effect: the repeated Fire is the only break.
    let mut s = presets::MAGE[0].blocks;
    s.aspects = [Aspect::Fire, Aspect::Cold, Aspect::Fire];
    s.aspect_count = 3;
    assert_eq!(validate(&s), Err(SkillError::RepeatedAspect));
}

#[test]
fn unused_aspect_slot_set() {
    // Piercing Thrust uses one aspect. Slot 1 must stay at the default (Fire).
    let mut s = presets::WARRIOR[0].blocks;
    s.aspects[1] = Aspect::Cold;
    assert_eq!(validate(&s), Err(SkillError::UnusedAspectSet));
}

#[test]
fn unused_last_aspect_slot_set() {
    // Same rule one slot further: count 1, slot 2 set, slot 1 still default.
    let mut s = presets::WARRIOR[0].blocks;
    s.aspects[2] = Aspect::Cold;
    assert_eq!(validate(&s), Err(SkillError::UnusedAspectSet));
}

#[test]
fn third_aspect_line_target() {
    let mut s = presets::MAGE[0].blocks;
    s.aspects = [Aspect::Cold, Aspect::Lightning, Aspect::Fire];
    s.aspect_count = 3;
    s.target = Target::Line;
    assert_eq!(validate(&s), Err(SkillError::ThirdAspectTarget));

    // The same three aspects on a single target are legal.
    s.target = Target::Single;
    assert_eq!(validate(&s), Ok(()));
    // A line with two aspects is legal (Bleeding Cleave).
    assert_eq!(validate(&presets::WARRIOR[3].blocks), Ok(()));
}

#[test]
fn effect_on_two_aspect_damage() {
    // Frost Burn has two aspects and no effect.
    let mut s = presets::MAGE[1].blocks;
    s.effect = Effect::Slow;
    assert_eq!(validate(&s), Err(SkillError::EffectNeedsOneAspect));
}

#[test]
fn effect_on_three_aspect_damage() {
    let mut s = presets::MAGE[0].blocks;
    s.aspects = [Aspect::Cold, Aspect::Lightning, Aspect::Fire];
    s.aspect_count = 3;
    s.effect = Effect::Slow;
    assert_eq!(validate(&s), Err(SkillError::EffectNeedsOneAspect));
}

#[test]
fn damage_with_buff_effect() {
    let mut s = presets::WARRIOR[0].blocks;
    s.effect = Effect::Haste;
    assert_eq!(validate(&s), Err(SkillError::EffectKind));
}

#[test]
fn buff_with_debuff_effect() {
    // Quicken is a buff (Haste). Slow is a debuff effect.
    let mut s = presets::SUPPORT[0].blocks;
    s.effect = Effect::Slow;
    assert_eq!(validate(&s), Err(SkillError::EffectKind));
}

#[test]
fn debuff_with_buff_effect() {
    // Sap is a debuff (Weakening). Protection is a buff effect.
    let mut s = presets::SUPPORT[1].blocks;
    s.effect = Effect::Protection;
    assert_eq!(validate(&s), Err(SkillError::EffectKind));
}

#[test]
fn damage_with_heal_effect() {
    let mut s = presets::WARRIOR[0].blocks;
    s.effect = Effect::Heal;
    assert_eq!(validate(&s), Err(SkillError::EffectKind));
}

#[test]
fn debuff_with_heal_effect() {
    let mut s = presets::SUPPORT[1].blocks;
    s.effect = Effect::Heal;
    assert_eq!(validate(&s), Err(SkillError::EffectKind));
}

#[test]
fn buff_missing_effect() {
    let mut s = presets::SUPPORT[0].blocks;
    s.effect = Effect::None;
    assert_eq!(validate(&s), Err(SkillError::MissingEffect));
}

#[test]
fn debuff_missing_effect() {
    let mut s = presets::SUPPORT[1].blocks;
    s.effect = Effect::None;
    assert_eq!(validate(&s), Err(SkillError::MissingEffect));
}

#[test]
fn stun_with_piercing() {
    // Piercing Thrust is piercing damage. Stun belongs to Blunt.
    let mut s = presets::WARRIOR[0].blocks;
    s.effect = Effect::Stun;
    assert_eq!(validate(&s), Err(SkillError::HardControlAspect));
}

#[test]
fn freeze_with_fire() {
    // Ice Prison is Freeze on Cold. Fire cannot freeze.
    let mut s = presets::MAGE[2].blocks;
    s.aspects[0] = Aspect::Fire;
    assert_eq!(validate(&s), Err(SkillError::HardControlAspect));
}

#[test]
fn debuff_freeze_with_lightning() {
    // Cold Snap is a Cold debuff. Freeze still requires Cold, including on a debuff.
    let mut s = presets::MAGE[4].blocks;
    s.aspects[0] = Aspect::Lightning;
    s.effect = Effect::Freeze;
    assert_eq!(validate(&s), Err(SkillError::HardControlAspect));
}

#[test]
fn hard_control_with_its_aspect_is_ok() {
    // Shield Bash is Stun with Blunt. Ice Prison is Freeze with Cold.
    assert_eq!(validate(&presets::WARRIOR[1].blocks), Ok(()));
    assert_eq!(validate(&presets::MAGE[2].blocks), Ok(()));
    // A debuff Freeze on Cold is the same pair.
    let mut s = presets::MAGE[4].blocks;
    s.effect = Effect::Freeze;
    assert_eq!(validate(&s), Ok(()));
}

#[test]
fn reactive_missing_trigger() {
    // Riposte is reactive on Hit.
    let mut s = presets::WARRIOR[4].blocks;
    s.trigger = Trigger::None;
    assert_eq!(validate(&s), Err(SkillError::MissingTrigger));
}

#[test]
fn active_with_trigger() {
    let mut s = presets::WARRIOR[0].blocks;
    s.trigger = Trigger::Hit;
    assert_eq!(validate(&s), Err(SkillError::UnexpectedTrigger));
}

#[test]
fn passive_with_trigger() {
    // Iron Skin is a passive buff with no trigger.
    let mut s = presets::WARRIOR[5].blocks;
    s.trigger = Trigger::HpBelow;
    assert_eq!(validate(&s), Err(SkillError::UnexpectedTrigger));
}

#[test]
fn passive_damage() {
    // Piercing Thrust has no trigger and no effect. Only the category changes.
    let mut s = presets::WARRIOR[0].blocks;
    s.category = Category::Passive;
    assert_eq!(validate(&s), Err(SkillError::PassiveDamage));
}

#[test]
fn aura_on_active_buff() {
    // Quicken is an active buff.
    let mut s = presets::SUPPORT[0].blocks;
    s.aura_radius = 1;
    assert_eq!(validate(&s), Err(SkillError::AuraCategory));
}

#[test]
fn aura_on_reactive_damage() {
    // Riposte is reactive damage. An aura has to be a buff or a debuff.
    let mut s = presets::WARRIOR[4].blocks;
    s.aura_radius = 1;
    assert_eq!(validate(&s), Err(SkillError::AuraEffect));
}

#[test]
fn aura_radius_max_boundary() {
    // Last Stand is a reactive buff aura.
    let mut s = presets::SUPPORT[4].blocks;
    s.aura_radius = 1;
    assert_eq!(validate(&s), Ok(()));
    s.aura_radius = AURA_RADIUS_MAX;
    assert_eq!(validate(&s), Ok(()));
    s.aura_radius = AURA_RADIUS_MAX + 1;
    assert_eq!(validate(&s), Err(SkillError::AuraRadius));
}

#[test]
fn warrior_magical_source() {
    let mut s = presets::WARRIOR[0].blocks;
    s.source = Source::Magical;
    assert_eq!(
        validate_starting(&s, Class::Warrior),
        Err(SkillError::ClassSource)
    );
}

#[test]
fn mage_combat_source() {
    let mut s = presets::MAGE[0].blocks;
    s.source = Source::Combat;
    assert_eq!(
        validate_starting(&s, Class::Mage),
        Err(SkillError::ClassSource)
    );
}

#[test]
fn support_damage_purpose() {
    // Frost Bolt is a legal damage skill. Support starting skills cannot deal damage.
    assert_eq!(
        validate_starting(&presets::MAGE[0].blocks, Class::Support),
        Err(SkillError::ClassPurpose)
    );
}

#[test]
fn support_combat_buff_is_ok() {
    // Shelter is a Combat-source buff. Support may use either power source.
    assert_eq!(
        validate_starting(&presets::SUPPORT[3].blocks, Class::Support),
        Ok(())
    );
}

#[test]
fn five_active_skills() {
    let skills = [
        presets::WARRIOR[0].blocks,
        presets::WARRIOR[1].blocks,
        presets::WARRIOR[2].blocks,
        presets::WARRIOR[3].blocks,
        presets::WARRIOR[0].blocks,
    ];
    assert_eq!(check_loadout(skills.iter()), Err(SkillError::TooManyActive));
}

#[test]
fn two_reactive_skills() {
    let skills = [presets::WARRIOR[4].blocks, presets::MAGE[4].blocks];
    assert_eq!(
        check_loadout(skills.iter()),
        Err(SkillError::TooManyReactive)
    );

    // Four active is the cap, so the extra reactive is the error that fires.
    let mixed = [
        presets::WARRIOR[0].blocks,
        presets::WARRIOR[1].blocks,
        presets::WARRIOR[2].blocks,
        presets::WARRIOR[3].blocks,
        presets::WARRIOR[4].blocks,
        presets::MAGE[4].blocks,
    ];
    assert_eq!(
        check_loadout(mixed.iter()),
        Err(SkillError::TooManyReactive)
    );
}

#[test]
fn two_passive_skills() {
    let skills = [presets::WARRIOR[5].blocks, presets::MAGE[5].blocks];
    assert_eq!(
        check_loadout(skills.iter()),
        Err(SkillError::TooManyPassive)
    );

    // Active and reactive are at their caps. The second passive is the error.
    let mixed = [
        presets::WARRIOR[0].blocks,
        presets::WARRIOR[1].blocks,
        presets::WARRIOR[2].blocks,
        presets::WARRIOR[3].blocks,
        presets::WARRIOR[4].blocks,
        presets::WARRIOR[5].blocks,
        presets::MAGE[5].blocks,
    ];
    assert_eq!(check_loadout(mixed.iter()), Err(SkillError::TooManyPassive));
}

#[test]
fn empty_loadout_and_three_skill_loadout_are_ok() {
    let empty: [SkillBlocks; 0] = [];
    assert_eq!(check_loadout(empty.iter()), Ok(()));

    let three = [
        presets::WARRIOR[0].blocks,
        presets::WARRIOR[1].blocks,
        presets::WARRIOR[2].blocks,
    ];
    assert_eq!(check_loadout(three.iter()), Ok(()));

    // The slot cap itself (4 active, 1 reactive, 1 passive) is still legal.
    let full = [
        presets::WARRIOR[0].blocks,
        presets::WARRIOR[1].blocks,
        presets::WARRIOR[2].blocks,
        presets::WARRIOR[3].blocks,
        presets::WARRIOR[4].blocks,
        presets::WARRIOR[5].blocks,
    ];
    assert_eq!(check_loadout(full.iter()), Ok(()));
}

#[test]
fn builder_error_before_class_error() {
    // Magical source would be `ClassSource` for a warrior, but the aspect count is also wrong.
    let mut s = presets::WARRIOR[0].blocks;
    s.source = Source::Magical;
    s.aspect_count = 0;
    assert_eq!(
        validate_starting(&s, Class::Warrior),
        Err(SkillError::AspectCount)
    );
    // Once the builder rule holds, the class rule is what remains.
    s.aspect_count = 1;
    assert_eq!(
        validate_starting(&s, Class::Warrior),
        Err(SkillError::ClassSource)
    );
}

#[test]
fn out_of_range_enum_bytes_do_not_decode() {
    // Field order is the struct order in `blocks.rs`. Fixed arrays have no length prefix.
    // 0 category, 1 purpose, 2 source, 3 form, 4 target,
    // 5 aspects[0], 6 aspects[1], 7 aspects[2], 8 aspect_count,
    // 9 effect, 10 trigger, 11 aura_radius.
    let base = borsh::to_vec(&presets::MAGE[0].blocks).unwrap();
    assert_eq!(base.len(), SkillBlocks::SIZE);
    assert_eq!(
        borsh::from_slice::<SkillBlocks>(&base).unwrap(),
        presets::MAGE[0].blocks
    );

    let bad: [(usize, u8); 8] = [
        (0, Category::COUNT as u8),
        (1, Purpose::COUNT as u8),
        (2, Source::COUNT as u8),
        (3, Form::COUNT as u8),
        (4, Target::COUNT as u8),
        (5, Aspect::COUNT as u8),
        (9, Effect::COUNT as u8),
        (10, Trigger::COUNT as u8),
    ];
    for (index, byte) in bad {
        let mut bytes = base.clone();
        bytes[index] = byte;
        assert!(
            borsh::from_slice::<SkillBlocks>(&bytes).is_err(),
            "offset {index} byte {byte}"
        );
    }
}

#[test]
fn every_skill_error_variant_is_covered() {
    let mut zero = presets::WARRIOR[0].blocks;
    zero.aspect_count = 0;

    let mut repeated = presets::MAGE[1].blocks;
    repeated.aspects[1] = Aspect::Cold;

    let mut unused = presets::WARRIOR[0].blocks;
    unused.aspects[1] = Aspect::Cold;

    let mut third = presets::MAGE[0].blocks;
    third.aspects = [Aspect::Cold, Aspect::Lightning, Aspect::Fire];
    third.aspect_count = 3;
    third.target = Target::Line;

    let mut two_effect = presets::MAGE[1].blocks;
    two_effect.effect = Effect::Slow;

    let mut haste = presets::WARRIOR[0].blocks;
    haste.effect = Effect::Haste;

    let mut no_effect = presets::SUPPORT[0].blocks;
    no_effect.effect = Effect::None;

    let mut stun = presets::WARRIOR[0].blocks;
    stun.effect = Effect::Stun;

    let mut no_trigger = presets::WARRIOR[4].blocks;
    no_trigger.trigger = Trigger::None;

    let mut triggered = presets::WARRIOR[0].blocks;
    triggered.trigger = Trigger::Hit;

    let mut passive = presets::WARRIOR[0].blocks;
    passive.category = Category::Passive;

    let mut active_aura = presets::SUPPORT[0].blocks;
    active_aura.aura_radius = 1;

    let mut damage_aura = presets::WARRIOR[4].blocks;
    damage_aura.aura_radius = 1;

    let mut wide_aura = presets::SUPPORT[4].blocks;
    wide_aura.aura_radius = AURA_RADIUS_MAX + 1;

    let mut magical = presets::WARRIOR[0].blocks;
    magical.source = Source::Magical;

    let five_active = [presets::WARRIOR[0].blocks; 5];
    let two_reactive = [presets::WARRIOR[4].blocks; 2];
    let two_passive = [presets::WARRIOR[5].blocks; 2];

    let produced = [
        validate(&zero).unwrap_err(),
        validate(&repeated).unwrap_err(),
        validate(&unused).unwrap_err(),
        validate(&third).unwrap_err(),
        validate(&two_effect).unwrap_err(),
        validate(&haste).unwrap_err(),
        validate(&no_effect).unwrap_err(),
        validate(&stun).unwrap_err(),
        validate(&no_trigger).unwrap_err(),
        validate(&triggered).unwrap_err(),
        validate(&passive).unwrap_err(),
        validate(&active_aura).unwrap_err(),
        validate(&damage_aura).unwrap_err(),
        validate(&wide_aura).unwrap_err(),
        validate_starting(&magical, Class::Warrior).unwrap_err(),
        validate_starting(&presets::MAGE[0].blocks, Class::Support).unwrap_err(),
        check_loadout(five_active.iter()).unwrap_err(),
        check_loadout(two_reactive.iter()).unwrap_err(),
        check_loadout(two_passive.iter()).unwrap_err(),
    ];
    let all = [
        SkillError::AspectCount,
        SkillError::RepeatedAspect,
        SkillError::UnusedAspectSet,
        SkillError::ThirdAspectTarget,
        SkillError::EffectNeedsOneAspect,
        SkillError::EffectKind,
        SkillError::MissingEffect,
        SkillError::HardControlAspect,
        SkillError::MissingTrigger,
        SkillError::UnexpectedTrigger,
        SkillError::PassiveDamage,
        SkillError::AuraCategory,
        SkillError::AuraEffect,
        SkillError::AuraRadius,
        SkillError::ClassSource,
        SkillError::ClassPurpose,
        SkillError::TooManyActive,
        SkillError::TooManyReactive,
        SkillError::TooManyPassive,
    ];
    assert_eq!(produced.len(), 19);
    assert_eq!(all.len(), 19);
    for variant in all {
        assert!(produced.contains(&variant), "missing {variant:?}");
    }
    assert_eq!(produced, all);
}
