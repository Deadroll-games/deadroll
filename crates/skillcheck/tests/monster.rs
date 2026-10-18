//! The monster builder (docs/BESTIARY.md 3.1) and a spawn's skill choice (3.2, MAP_DESCRIPTION.md
//! 7.2): happy paths and one rejection per rule.

use deadroll_skillcheck::{
    check_monster_selection, presets, validate, validate_monster_skill, Category, Effect,
    SkillError, SkillExt, SkillTuning, Target, Trigger,
};

const T: SkillTuning = SkillTuning::DEFAULT;

#[test]
fn a_heal_is_a_monster_skill_but_not_a_player_one() {
    // Support's Quicken (a one-aspect buff) turned into a heal.
    let mut s = presets::SUPPORT[0].blocks;
    s.effect = Effect::Heal;
    assert_eq!(validate(&s), Err(SkillError::HealNotAvailable));
    assert_eq!(validate_monster_skill(&s, &SkillExt::MELEE, &T), Ok(()));
    s.target = Target::Line;
    assert_eq!(validate_monster_skill(&s, &SkillExt::MELEE, &T), Ok(()));
}

#[test]
fn a_heal_is_never_passive_or_an_aura() {
    let mut s = presets::SUPPORT[0].blocks;
    s.effect = Effect::Heal;
    s.category = Category::Passive;
    assert_eq!(
        validate_monster_skill(&s, &SkillExt::MELEE, &T),
        Err(SkillError::HealCategory)
    );
    s.category = Category::Reactive;
    s.trigger = Trigger::Hit;
    s.aura_radius = 2;
    assert_eq!(
        validate_monster_skill(&s, &SkillExt::MELEE, &T),
        Err(SkillError::HealCategory)
    );
    s.aura_radius = 0;
    assert_eq!(validate_monster_skill(&s, &SkillExt::MELEE, &T), Ok(()));
}

#[test]
fn the_other_builder_rules_still_hold_for_monsters() {
    let mut s = presets::SUPPORT[0].blocks;
    s.aspect_count = 2;
    assert_eq!(
        validate_monster_skill(&s, &SkillExt::MELEE, &T),
        Err(SkillError::AspectCount)
    );
    // An HP trigger without its threshold.
    let mut r = presets::SUPPORT[0].blocks;
    r.category = Category::Reactive;
    r.trigger = Trigger::HpBelow;
    assert_eq!(
        validate_monster_skill(&r, &SkillExt::MELEE, &T),
        Err(SkillError::MissingThreshold)
    );
    let e = SkillExt {
        hp_below_pct: 50,
        ..SkillExt::MELEE
    };
    assert_eq!(validate_monster_skill(&r, &e, &T), Ok(()));
}

const A: Category = Category::Active;
const R: Category = Category::Reactive;
const P: Category = Category::Passive;
/// Regular slots: 3 active, 0 reactive, 1 passive (in `Category` order).
const REGULAR: [u8; 3] = [3, 0, 1];

#[test]
fn a_full_choice_passes() {
    let pool = [A, A, A, A, P];
    assert_eq!(check_monster_selection(&pool, 0b1_0111, REGULAR), Ok(()));
    assert_eq!(check_monster_selection(&pool, 0b1_1110, REGULAR), Ok(()));
    // Fewer skills than slots: all of them.
    assert_eq!(check_monster_selection(&[A, A], 0b11, REGULAR), Ok(()));
    // No pool: nothing to choose.
    assert_eq!(check_monster_selection(&[], 0, REGULAR), Ok(()));
    // A 16-skill pool may use every bit.
    let mut big = [A; 16];
    big[15] = P;
    assert_eq!(
        check_monster_selection(&big, 0b1000_0000_0000_0111, REGULAR),
        Ok(())
    );
}

#[test]
fn a_bit_past_the_pool_is_refused() {
    assert_eq!(
        check_monster_selection(&[A, A], 0b111, REGULAR),
        Err(SkillError::MonsterSkillUnknown)
    );
    assert_eq!(
        check_monster_selection(&[], 1, REGULAR),
        Err(SkillError::MonsterSkillUnknown)
    );
}

#[test]
fn too_few_or_too_many_of_a_category_is_refused() {
    let pool = [A, A, A, A, P, R];
    // Two actives of four.
    assert_eq!(
        check_monster_selection(&pool, 0b1_0011, REGULAR),
        Err(SkillError::MonsterSkillCount)
    );
    // Four actives.
    assert_eq!(
        check_monster_selection(&pool, 0b1_1111, REGULAR),
        Err(SkillError::MonsterSkillCount)
    );
    // The passive left out.
    assert_eq!(
        check_monster_selection(&pool, 0b0_0111, REGULAR),
        Err(SkillError::MonsterSkillCount)
    );
    // A reactive without a reactive slot.
    assert_eq!(
        check_monster_selection(&pool, 0b11_0111, REGULAR),
        Err(SkillError::MonsterSkillCount)
    );
    // A boss takes it.
    assert_eq!(check_monster_selection(&pool, 0b11_1111, [5, 1, 1]), Ok(()));
}
