//! Happy paths: every preset is a valid starting set for its class, derived values are sane.
//! Rejections live in `tests/rejections.rs`.

use deadroll_skillcheck::{
    check_loadout, derive, fusion_id, presets, validate, validate_starting, Category, Effect,
    SkillBlocks, SkillError, SkillExt, Trigger,
};
use deadroll_types::{Aspect, Class, RangeTag, TimeTag, BP};

#[test]
fn every_preset_is_a_valid_starting_set() {
    for &class in Class::ALL {
        let set = presets::for_class(class);
        for p in set {
            assert_eq!(
                validate_starting(&p.blocks, &p.ext, class),
                Ok(()),
                "{class:?} {}",
                p.name
            );
            assert!(p.name.len() <= 24, "{} too long", p.name);
        }
        assert_eq!(check_loadout(set.iter().map(|p| &p.blocks)), Ok(()));
        let count = |c: Category| set.iter().filter(|p| p.blocks.category == c).count();
        assert_eq!(
            (
                count(Category::Active),
                count(Category::Reactive),
                count(Category::Passive)
            ),
            (4, 1, 1)
        );
    }
}

#[test]
fn presets_are_class_bound() {
    // Warrior skills are martial, so a mage cannot take them as starting skills, and back.
    assert_eq!(
        validate_starting(&presets::WARRIOR[0].blocks, &SkillExt::MELEE, Class::Mage),
        Err(SkillError::ClassSource)
    );
    assert_eq!(
        validate_starting(&presets::MAGE[0].blocks, &SkillExt::MELEE, Class::Warrior),
        Err(SkillError::ClassSource)
    );
    assert_eq!(
        validate_starting(&presets::MAGE[0].blocks, &SkillExt::MELEE, Class::Support),
        Err(SkillError::ClassPurpose)
    );
    // Support buffs are fine for anyone as cut skills.
    for p in &presets::SUPPORT {
        assert_eq!(validate(&p.blocks), Ok(()));
    }
}

#[test]
fn shares_always_sum_to_bp() {
    for &class in Class::ALL {
        for p in presets::for_class(class) {
            let d = derive(&p.blocks);
            assert_eq!(d.shares_bp.iter().sum::<u32>(), BP, "{}", p.name);
            assert!(d.power_bp > 0 && d.power_bp <= BP, "{}", p.name);
        }
    }
}

#[test]
fn derived_values_of_known_skills() {
    let frost_burn = derive(&presets::MAGE[1].blocks);
    assert_eq!(frost_burn.fusion, fusion_id(Aspect::Cold, Aspect::Fire));
    assert_eq!(frost_burn.shares_bp, [6_500, 3_500, 0]);
    assert_eq!(frost_burn.power_bp, BP);
    assert_eq!(frost_burn.cooldown, 1);

    // Freeze: effect counterweight and hard-control cooldown.
    let ice_prison = derive(&presets::MAGE[2].blocks);
    assert_eq!(ice_prison.fusion, 0);
    assert_eq!(ice_prison.power_bp, 8_000);
    assert_eq!(ice_prison.cooldown, 2);

    // Line target with a fusion.
    let cleave = derive(&presets::WARRIOR[3].blocks);
    assert_eq!(cleave.power_bp, 6_666);
    assert_eq!(cleave.cooldown, 2);

    assert_eq!(derive(&presets::WARRIOR[4].blocks).cooldown, 2); // reactive
    assert_eq!(derive(&presets::WARRIOR[5].blocks).cooldown, 0); // passive
}

#[test]
fn heal_stays_a_buff_but_the_builder_refuses_it() {
    // The mechanic stays (a buff kind), the builder has no Heal until the healing class
    // (docs/CHARACTER.md, 8.1).
    let mut s = presets::SUPPORT[0].blocks;
    s.effect = Effect::Heal;
    assert!(Effect::Heal.is_buff() && !Effect::Heal.is_debuff());
    assert_eq!(Effect::Heal.hard_control_aspect(), None);
    for &aspect in Aspect::ALL.iter() {
        s.aspects[0] = aspect;
        assert_eq!(
            validate_starting(&s, &SkillExt::MELEE, Class::Support),
            Err(SkillError::HealNotAvailable)
        );
    }
    let d = derive(&s);
    assert_eq!((d.power_bp, d.cooldown), (BP, 1));
}

#[test]
fn third_aspect_single_target_is_valid() {
    let mut s = presets::MAGE[0].blocks; // Frost Bolt, single target, no effect
    s.aspects = [Aspect::Cold, Aspect::Lightning, Aspect::Fire];
    s.aspect_count = 3;
    assert_eq!(validate_starting(&s, &SkillExt::MELEE, Class::Mage), Ok(()));
    let d = derive(&s);
    assert_eq!(d.fusion, 0);
    assert_eq!(d.shares_bp, [5_500, 3_000, 1_500]);
    assert_eq!(d.power_bp, 8_333);
    assert_eq!(d.cooldown, 2);
}

#[test]
fn fusion_ids_are_unique_per_ordered_pair() {
    let mut seen = [false; 64];
    for &a in Aspect::ALL {
        for &b in Aspect::ALL {
            let id = fusion_id(a, b);
            if a == b {
                assert_eq!(id, 0);
                continue;
            }
            assert!(id > 0 && !seen[id as usize]);
            seen[id as usize] = true;
        }
    }
}

#[test]
fn blocks_round_trip_through_borsh() {
    for p in presets::MAGE
        .iter()
        .chain(&presets::WARRIOR)
        .chain(&presets::SUPPORT)
    {
        let bytes = borsh::to_vec(&p.blocks).unwrap();
        assert_eq!(bytes.len(), SkillBlocks::SIZE);
        assert_eq!(borsh::from_slice::<SkillBlocks>(&bytes).unwrap(), p.blocks);
    }
    // An out-of-range enum byte does not decode.
    let mut bytes = borsh::to_vec(&presets::MAGE[0].blocks).unwrap();
    bytes[0] = 9;
    assert!(borsh::from_slice::<SkillBlocks>(&bytes).is_err());
}

#[test]
fn tuning_drives_derive() {
    use deadroll_skillcheck::{derive_with, SkillTuning};
    let all = presets::WARRIOR
        .iter()
        .chain(&presets::MAGE)
        .chain(&presets::SUPPORT);
    for p in all.clone() {
        assert_eq!(
            derive_with(&p.blocks, &SkillTuning::DEFAULT),
            derive(&p.blocks)
        );
    }
    let tuned = SkillTuning {
        line_counter_bp: 0,
        active_cooldown: 3,
        ..SkillTuning::DEFAULT
    };
    // "Bleeding Cleave": a line, two aspects.
    let cleave = derive_with(&presets::WARRIOR[3].blocks, &tuned);
    assert_eq!((cleave.power_bp, cleave.cooldown), (10_000, 4));
    assert_eq!(cleave.shares_bp, SkillTuning::DEFAULT.shares_bp[1]);
}

#[test]
fn preset_ext_and_descriptions() {
    for &class in Class::ALL {
        for p in presets::for_class(class) {
            assert!(
                !p.description.is_empty() && p.description.len() <= 200,
                "{}",
                p.name
            );
            assert!(
                p.description.bytes().all(|b| (0x20..0x7f).contains(&b)),
                "{}",
                p.name
            );
            assert_eq!(p.ext.timing, TimeTag::Instant, "{}", p.name);
            assert_eq!(
                p.ext.hp_below_pct > 0,
                p.blocks.trigger.uses_threshold(),
                "{}",
                p.name
            );
        }
    }
    assert_eq!(presets::SUPPORT[4].ext.hp_below_pct, 40);
    assert_eq!(presets::MAGE[0].ext.range, RangeTag::Ranged);
    assert_eq!(presets::WARRIOR[0].ext.range, RangeTag::Melee);
    // A line keeps its reach: both starter lines are ranged.
    assert_eq!(presets::WARRIOR[3].ext.range, RangeTag::Ranged);
    assert_eq!(presets::MAGE[3].ext.range, RangeTag::Ranged);
    let bytes = borsh::to_vec(&presets::SUPPORT[4].ext).unwrap();
    assert_eq!(bytes, [0, 0, 40]);
    assert_eq!(bytes.len(), SkillExt::SIZE);
}

#[test]
fn ap_cost_of_known_skills() {
    // Base 2; a line, a third aspect and hard control add one each; reactive and passive: none.
    assert_eq!(derive(&presets::WARRIOR[0].blocks).ap_cost, 2);
    assert_eq!(derive(&presets::WARRIOR[1].blocks).ap_cost, 3); // Stun
    assert_eq!(derive(&presets::WARRIOR[3].blocks).ap_cost, 3); // line
    assert_eq!(derive(&presets::MAGE[2].blocks).ap_cost, 3); // Freeze
    assert_eq!(derive(&presets::SUPPORT[0].blocks).ap_cost, 2); // buff
    assert_eq!(derive(&presets::WARRIOR[4].blocks).ap_cost, 0); // reactive
    assert_eq!(derive(&presets::WARRIOR[5].blocks).ap_cost, 0); // passive
    let mut third = presets::MAGE[0].blocks;
    third.aspects = [Aspect::Cold, Aspect::Lightning, Aspect::Fire];
    third.aspect_count = 3;
    assert_eq!(derive(&third).ap_cost, 3);

    use deadroll_skillcheck::{derive_with, SkillTuning};
    let tuned = SkillTuning {
        ap_base: 1,
        ap_line: 2,
        ap_effect: 4,
        ap_hard_control: 0,
        ..SkillTuning::DEFAULT
    };
    assert_eq!(derive_with(&presets::WARRIOR[3].blocks, &tuned).ap_cost, 3);
    assert_eq!(derive_with(&presets::WARRIOR[1].blocks, &tuned).ap_cost, 5);
    // A buff carries an effect but is not a damage skill with one.
    assert_eq!(derive_with(&presets::SUPPORT[0].blocks, &tuned).ap_cost, 1);
}

#[test]
fn hp_threshold_limits_are_inclusive() {
    use deadroll_skillcheck::validate_skill;
    let p = &presets::SUPPORT[4];
    for pct in [10, 40, 90] {
        let ext = SkillExt {
            hp_below_pct: pct,
            ..p.ext
        };
        assert_eq!(validate_skill(&p.blocks, &ext), Ok(()), "{pct}");
    }
    // AllyHpBelow takes a threshold the same way.
    let mut ally = p.blocks;
    ally.trigger = Trigger::AllyHpBelow;
    assert_eq!(validate_skill(&ally, &p.ext), Ok(()));
}
