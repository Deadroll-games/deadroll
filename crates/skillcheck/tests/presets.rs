//! Happy paths: every preset is a valid starting set for its class, derived values are sane.
//! Rejections live in `tests/rejections.rs`.

use deadroll_skillcheck::{
    check_loadout, derive, fusion_id, presets, validate, validate_starting, Category, Effect,
    SkillBlocks, SkillError,
};
use deadroll_types::{Aspect, Class, BP};

#[test]
fn every_preset_is_a_valid_starting_set() {
    for &class in Class::ALL {
        let set = presets::for_class(class);
        for p in set {
            assert_eq!(
                validate_starting(&p.blocks, class),
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
        validate_starting(&presets::WARRIOR[0].blocks, Class::Mage),
        Err(SkillError::ClassSource)
    );
    assert_eq!(
        validate_starting(&presets::MAGE[0].blocks, Class::Warrior),
        Err(SkillError::ClassSource)
    );
    assert_eq!(
        validate_starting(&presets::MAGE[0].blocks, Class::Support),
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
fn heal_is_a_buff_for_any_aspect() {
    // Quicken with Heal instead of Haste: a Support starting skill, full power, active cooldown.
    let mut s = presets::SUPPORT[0].blocks;
    s.effect = Effect::Heal;
    assert!(Effect::Heal.is_buff() && !Effect::Heal.is_debuff());
    assert_eq!(Effect::Heal.hard_control_aspect(), None);
    for &aspect in Aspect::ALL.iter() {
        s.aspects[0] = aspect;
        assert_eq!(validate_starting(&s, Class::Support), Ok(()));
    }
    let d = derive(&s);
    assert_eq!((d.power_bp, d.cooldown), (BP, 1));
}

#[test]
fn third_aspect_single_target_is_valid() {
    let mut s = presets::MAGE[0].blocks; // Frost Bolt, single target, no effect
    s.aspects = [Aspect::Cold, Aspect::Lightning, Aspect::Fire];
    s.aspect_count = 3;
    assert_eq!(validate_starting(&s, Class::Mage), Ok(()));
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
