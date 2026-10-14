//! `check_map_selection`: the skills taken onto a map (docs/SKILL_BUILDER.md, sections 3.4, 13).
//! Happy paths first, then one test per rejection.

use deadroll_skillcheck::{check_map_selection, Category, SkillError, NO_SKILL};

use Category::{Active as A, Passive as P, Reactive as R};

const N: u8 = NO_SKILL;

/// A class starter set: 4 active, 1 reactive, 1 passive.
const FULL: [Category; 6] = [A, A, A, A, R, P];

#[test]
fn full_set_in_order_and_shuffled() {
    assert_eq!(check_map_selection(&FULL, &[0, 1, 2, 3, 4, 5]), Ok(()));
    assert_eq!(check_map_selection(&FULL, &[3, 0, 2, 1, 4, 5]), Ok(()));
}

#[test]
fn more_skills_than_slots_take_any_four() {
    let owned = [A, R, A, A, P, A, A, R];
    assert_eq!(check_map_selection(&owned, &[0, 2, 5, 6, 7, 4]), Ok(()));
    assert_eq!(check_map_selection(&owned, &[3, 2, 5, 6, 1, 4]), Ok(()));
}

#[test]
fn fewer_skills_take_all_of_them() {
    // Two active, no reactive, one passive: empty slots anywhere in their category.
    let owned = [A, P, A];
    assert_eq!(check_map_selection(&owned, &[0, N, 2, N, N, 1]), Ok(()));
    assert_eq!(check_map_selection(&owned, &[N, 2, N, 0, N, 1]), Ok(()));
    // No skills at all: nothing to take.
    assert_eq!(check_map_selection(&[], &[N; 6]), Ok(()));
}

#[test]
fn unknown_skill() {
    assert_eq!(
        check_map_selection(&FULL, &[0, 1, 2, 6, 4, 5]),
        Err(SkillError::SelectionUnknownSkill)
    );
    assert_eq!(
        check_map_selection(&[A], &[0, N, N, N, 0x7F, N]),
        Err(SkillError::SelectionUnknownSkill)
    );
}

#[test]
fn repeated_skill() {
    assert_eq!(
        check_map_selection(&FULL, &[0, 1, 2, 2, 4, 5]),
        Err(SkillError::SelectionRepeated)
    );
    // Across categories too: the repeat is reported before the category.
    assert_eq!(
        check_map_selection(&FULL, &[0, 1, 2, 3, 4, 4]),
        Err(SkillError::SelectionRepeated)
    );
}

#[test]
fn skill_in_a_slot_of_another_category() {
    assert_eq!(
        check_map_selection(&FULL, &[0, 1, 2, 4, 3, 5]),
        Err(SkillError::SelectionCategory)
    );
    assert_eq!(
        check_map_selection(&FULL, &[0, 1, 2, 3, 5, 4]),
        Err(SkillError::SelectionCategory)
    );
}

#[test]
fn not_full() {
    // Has 4 active, takes 3.
    assert_eq!(
        check_map_selection(&FULL, &[0, 1, 2, N, 4, 5]),
        Err(SkillError::SelectionCount)
    );
    // Has a reactive, leaves it out.
    assert_eq!(
        check_map_selection(&FULL, &[0, 1, 2, 3, N, 5]),
        Err(SkillError::SelectionCount)
    );
    // Has a passive, leaves it out.
    assert_eq!(
        check_map_selection(&FULL, &[0, 1, 2, 3, 4, N]),
        Err(SkillError::SelectionCount)
    );
    // Has two active, takes one.
    assert_eq!(
        check_map_selection(&[A, A], &[0, N, N, N, N, N]),
        Err(SkillError::SelectionCount)
    );
}
