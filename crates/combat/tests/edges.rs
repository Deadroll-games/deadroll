//! Targeted edge cases for the public `deadroll-combat` API.

use deadroll_combat::*;
use deadroll_types::{ActiveState, Aspect, Form, Source, StateKind, BP, STATES_MAX};

const B: Balance = Balance::DEFAULT;

fn st(kind: StateKind, turns: u8, power: u32) -> ActiveState {
    ActiveState {
        kind,
        turns,
        power,
        source: 1,
    }
}

fn has(states: &States, kind: StateKind) -> bool {
    states.iter().any(|s| s.turns > 0 && s.kind == kind)
}

fn find(states: &States, kind: StateKind) -> ActiveState {
    *states
        .iter()
        .find(|s| s.turns > 0 && s.kind == kind)
        .expect("state present")
}

fn fill_distinct(states: &mut States) {
    // Six slots, seven kinds: the last kind waits for a free slot.
    for (i, &kind) in StateKind::ALL.iter().take(STATES_MAX).enumerate() {
        states[i] = st(kind, 2, 1);
    }
}

fn skill(aspects: &[Aspect], shares: &[u32], power_bp: u32, aspect_count: u8) -> Skill {
    let mut s = Skill::basic(Source::Magical, Form::Projectile, 0);
    for (i, (a, sh)) in aspects.iter().zip(shares).enumerate() {
        s.aspects[i] = *a;
        s.shares_bp[i] = *sh;
    }
    s.aspect_count = aspect_count;
    s.power_bp = power_bp;
    s
}

fn hit_input(weapon: DamageRange, skill: Skill) -> HitInput {
    HitInput {
        attacker: Attacker {
            weapon,
            skill,
            ..Attacker::default()
        },
        defender: Defender {
            defense: 1,
            ..Defender::default()
        },
    }
}

#[test]
fn skill_three_aspects_shares_sum_to_bp() {
    let weapon = DamageRange::new(Aspect::Blunt, 10, 10);
    let s = skill(
        &[Aspect::Fire, Aspect::Cold, Aspect::Lightning],
        &[5_000, 3_000, 2_000],
        BP,
        3,
    );
    let p = build_packets(weapon, &s, &[]);
    assert_eq!(p.count, 3);
    assert_eq!(
        p.as_slice(),
        &[
            Packet {
                aspect: Aspect::Fire,
                min: 50_000,
                max: 50_000
            },
            Packet {
                aspect: Aspect::Cold,
                min: 30_000,
                max: 30_000
            },
            Packet {
                aspect: Aspect::Lightning,
                min: 20_000,
                max: 20_000
            },
        ]
    );
    let r = resolve_hit(
        &hit_input(weapon, s),
        &B,
        &mut ScriptedRoll::new([14u64, 0]),
    );
    assert_eq!(r.outcome, Outcome::Hit);
    assert_eq!(r.packet_count, 3);
    assert_eq!(r.damage, 10);
}

#[test]
fn skill_aspect_count_above_three_is_clamped() {
    let weapon = DamageRange::new(Aspect::Blunt, 10, 10);
    let s = skill(
        &[Aspect::Fire, Aspect::Cold, Aspect::Lightning],
        &[BP, 0, 0],
        BP,
        5,
    );
    let p = build_packets(weapon, &s, &[]);
    assert_eq!(p.count, 3);
    assert_eq!(p.as_slice()[0].aspect, Aspect::Fire);
    assert_eq!(p.as_slice()[0].min, 100_000);
    assert_eq!(p.as_slice()[1].min, 0);
    assert_eq!(p.as_slice()[2].min, 0);
    let r = resolve_hit(
        &hit_input(weapon, s),
        &B,
        &mut ScriptedRoll::new([19u64, 0]),
    );
    assert_eq!(r.outcome, Outcome::Hit);
    assert_eq!(r.packet_count, 3);
}

#[test]
fn duplicate_skill_aspects_merge() {
    let weapon = DamageRange::new(Aspect::Blunt, 10, 10);
    let s = skill(
        &[Aspect::Fire, Aspect::Fire, Aspect::Cold],
        &[4_000, 4_000, 2_000],
        BP,
        3,
    );
    let p = build_packets(weapon, &s, &[]);
    assert_eq!(p.count, 2);
    assert_eq!(p.as_slice()[0].aspect, Aspect::Fire);
    assert_eq!(p.as_slice()[0].min, 80_000);
    assert_eq!(p.as_slice()[1].aspect, Aspect::Cold);
    assert_eq!(p.as_slice()[1].min, 20_000);
}

#[test]
fn zero_weapon_range_still_min_damage_on_hit() {
    let input = hit_input(
        DamageRange::new(Aspect::Fire, 0, 0),
        Skill::basic(Source::Combat, Form::Strike, 0),
    );
    let r = resolve_hit(&input, &B, &mut ScriptedRoll::new([19u64, 0]));
    assert_eq!(r.outcome, Outcome::Hit);
    assert_eq!(r.damage, B.min_damage);
    assert_eq!(r.damage, 1);
}

#[test]
fn u32_max_weapon_power_shares_increased_more_saturates() {
    let mut input = hit_input(
        DamageRange::new(Aspect::Fire, u32::MAX, u32::MAX),
        skill(
            &[Aspect::Fire, Aspect::Cold, Aspect::Lightning],
            &[u32::MAX, u32::MAX, u32::MAX],
            u32::MAX,
            3,
        ),
    );
    for slot in &mut input.attacker.increased {
        *slot = Mod::new(ModFilter::Any, u32::MAX);
    }
    for slot in &mut input.attacker.more {
        *slot = Mod::new(ModFilter::Any, u32::MAX);
    }
    let r = resolve_hit(&input, &B, &mut ScriptedRoll::new([19u64, 0]));
    assert_eq!(r.outcome, Outcome::Hit);
    assert_eq!(r.damage, u32::MAX);
    let (lo, hi) = preview_range(&input, &B);
    assert!(lo <= hi);
    assert_eq!(hi, u32::MAX);
}

#[test]
fn freeze_when_six_distinct_slots_are_full() {
    let mut states = States::default();
    fill_distinct(&mut states);
    assert_eq!(states.iter().filter(|s| s.turns > 0).count(), STATES_MAX);
    let applied = states::freeze(&mut states, 3, &B);
    assert_eq!(applied, Applied::Froze);
    assert!(has(&states, StateKind::Freeze));
    assert!(!has(&states, StateKind::Chill));

    let mut input = hit_input(
        DamageRange::new(Aspect::Lightning, 10, 10),
        skill(&[Aspect::Cold], &[BP], BP, 1),
    );
    input.attacker.tints[0] = Tint::new(Aspect::Piercing, 1, Carrier::Race);
    fill_distinct(&mut input.defender.states);
    let r = resolve_hit(&input, &B, &mut ScriptedRoll::new([19u64, 0, 0]));
    assert_eq!(r.outcome, Outcome::Hit);
    assert!(r.crit);
    assert_eq!(r.resonance_states[0].kind, StateKind::Freeze);
    assert_eq!(r.resonance_states[0].applied, Applied::Froze);
    assert!(has(&r.states, StateKind::Freeze));
    assert!(!has(&r.states, StateKind::Chill));
}

#[test]
fn chill_buildup_reaches_freeze_then_tick_skips_and_expires() {
    let mut input = hit_input(
        DamageRange::new(Aspect::Cold, 10, 10),
        Skill::basic(Source::Magical, Form::Projectile, 5_000),
    );
    let r1 = resolve_hit(&input, &B, &mut ScriptedRoll::new([14u64, 0, 0]));
    assert_eq!(r1.outcome, Outcome::Hit);
    assert_eq!(r1.packets()[0].state.applied, Applied::Added);
    assert!(has(&r1.states, StateKind::Chill));
    assert!(!has(&r1.states, StateKind::Freeze));
    assert_eq!(find(&r1.states, StateKind::Chill).power, B.chill_buildup_bp);

    input.defender.states = r1.states;
    let r2 = resolve_hit(&input, &B, &mut ScriptedRoll::new([14u64, 0, 0]));
    assert_eq!(r2.packets()[0].state.applied, Applied::Froze);
    assert!(!has(&r2.states, StateKind::Chill));
    assert!(has(&r2.states, StateKind::Freeze));
    assert_eq!(
        find(&r2.states, StateKind::Freeze).turns,
        B.turns(StateKind::Freeze)
    );

    let mut after = r2.states;
    let tick = tick_turn_start(&mut after);
    assert!(tick.skip_action);
    assert_eq!(tick.expired, 1 << StateKind::Freeze as u8);
    assert!(!has(&after, StateKind::Freeze));
}

#[test]
fn weaker_burning_refresh_keeps_stronger_power() {
    let mut input = hit_input(
        DamageRange::new(Aspect::Fire, 100, 100),
        Skill::basic(Source::Magical, Form::Projectile, 5_000),
    );
    let r1 = resolve_hit(&input, &B, &mut ScriptedRoll::new([14u64, 0, 0]));
    assert_eq!(r1.packets()[0].state.applied, Applied::Added);
    let strong = find(&r1.states, StateKind::Burning);
    assert!(strong.power > 1);

    input.attacker.weapon = DamageRange::new(Aspect::Fire, 1, 1);
    input.defender.states = r1.states;
    let r2 = resolve_hit(&input, &B, &mut ScriptedRoll::new([14u64, 0, 0]));
    assert_eq!(r2.packets()[0].state.applied, Applied::Refreshed);
    let kept = find(&r2.states, StateKind::Burning);
    assert_eq!(kept.power, strong.power);
    assert_eq!(kept.turns, strong.turns.max(B.turns(StateKind::Burning)));
}

#[test]
fn tick_turn_start_dot_u32_max_saturates() {
    let mut states = States::default();
    assert_eq!(
        apply_state(&mut states, st(StateKind::Burning, 1, u32::MAX)),
        Applied::Added
    );
    assert_eq!(
        apply_state(&mut states, st(StateKind::Bleeding, 1, u32::MAX)),
        Applied::Added
    );
    assert_eq!(tick_turn_start(&mut states).dot_damage, u32::MAX);
}

#[test]
fn tint_tier_0_or_15_is_ignored() {
    let mut input = hit_input(
        DamageRange::new(Aspect::Fire, 10, 10),
        Skill::basic(Source::Magical, Form::Projectile, 0),
    );
    input.attacker.tints[0] = Tint::new(Aspect::Cold, 0, Carrier::Race);
    input.attacker.tints[1] = Tint::new(Aspect::Lightning, 15, Carrier::Class);
    let r = resolve_hit(&input, &B, &mut ScriptedRoll::new([14u64, 0]));
    assert_eq!(r.outcome, Outcome::Hit);
    assert_eq!(r.tint_roll_count, 0);
    assert_eq!(r.rolls_used, 2);
    assert_eq!(r.resonances, [None; 3]);
}

#[test]
fn resonance_is_frozen_only_when_defender_already_has_freeze() {
    let mut input = hit_input(
        DamageRange::new(Aspect::Lightning, 10, 10),
        skill(&[Aspect::Cold], &[BP], BP, 1),
    );
    input.attacker.tints[0] = Tint::new(Aspect::Blunt, 1, Carrier::Race);

    let cold = resolve_hit(&input, &B, &mut ScriptedRoll::new([14u64, 0, 0]));
    assert_eq!(cold.outcome, Outcome::Hit);
    let res = cold.resonances[0].expect("pair rolled");
    assert_eq!((res.base, res.tint), (Aspect::Cold, Aspect::Blunt));
    assert_eq!(res.id, resonance_id(Aspect::Cold, Aspect::Blunt));
    assert!(!res.active);

    input.defender.states[0] = st(StateKind::Freeze, 1, 0);
    let frozen = resolve_hit(&input, &B, &mut ScriptedRoll::new([14u64, 0, 0]));
    let res = frozen.resonances[0].expect("pair rolled");
    assert!(res.active);
}
