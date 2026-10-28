//! Property tests for the public `deadroll-combat` API.

use deadroll_combat::modifiers::{armor_cut, counter, more, ratio, resist_cut};
use deadroll_combat::*;
use deadroll_types::{ActiveState, Aspect, Family, Form, Source, StateKind, STATES_MAX};
use proptest::prelude::*;

const B: Balance = Balance::DEFAULT;

fn proptest_config() -> ProptestConfig {
    ProptestConfig {
        cases: 256,
        ..ProptestConfig::default()
    }
}

/// Native states only.
fn natives() -> ResonanceTables {
    let mut t = ResonanceTables::default();
    let natives = [
        StateKind::Burning,
        StateKind::Chill,
        StateKind::Shock,
        StateKind::Bleeding,
        StateKind::Stun,
        StateKind::Penetration,
    ];
    for (i, e) in t.aspects.iter_mut().enumerate() {
        e.native = Some(natives[i]);
        e.trait_id = i as u8;
    }
    t
}

/// A valid op of any kind, or none.
fn arb_op() -> impl Strategy<Value = Op> {
    (
        0u8..OpKind::COUNT,
        any::<bool>(),
        prop_oneof![0u8..8, Just(0xFFu8)],
        prop_oneof![0u32..40_000, Just(u32::MAX)],
    )
        .prop_map(|(kind, crit, arg, value)| {
            let v = value.to_le_bytes();
            let k = kind | if crit { tables::CRIT_ONLY } else { 0 };
            Op::decode(&[k, arg, v[0], v[1], v[2], v[3]]).unwrap_or(Op::NONE)
        })
}

/// Natives plus random pair records and traits: every op kind the decoder accepts.
fn arb_tables() -> impl Strategy<Value = ResonanceTables> {
    (
        proptest::collection::vec((0u8..9, any::<bool>(), arb_op(), arb_op()), 36),
        proptest::collection::vec((arb_op(), arb_op(), arb_op()), 6),
    )
        .prop_map(|(pairs, traits)| {
            let mut t = natives();
            for (i, (cond, set, a, b)) in pairs.into_iter().enumerate() {
                let r = [cond, set as u8, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0];
                let mut e = tables::decode_pair(&r).expect("valid record");
                e.ops = [a, b];
                t.pairs[i / 6][i % 6] = e;
            }
            for (i, (hit, a, b)) in traits.into_iter().enumerate() {
                t.traits[i] = tables::TraitEntry {
                    present: true,
                    hit,
                    template: [a, b],
                };
            }
            t
        })
}

fn arb_aspect() -> impl Strategy<Value = Aspect> {
    (0..Aspect::COUNT).prop_map(|i| Aspect::ALL[i])
}

fn arb_family() -> impl Strategy<Value = Family> {
    (0..Family::COUNT).prop_map(|i| Family::ALL[i])
}

fn arb_source() -> impl Strategy<Value = Source> {
    (0..Source::COUNT).prop_map(|i| Source::ALL[i])
}

fn arb_form() -> impl Strategy<Value = Form> {
    (0..Form::COUNT).prop_map(|i| Form::ALL[i])
}

fn arb_carrier() -> impl Strategy<Value = Carrier> {
    prop_oneof![
        Just(Carrier::Race),
        Just(Carrier::Class),
        Just(Carrier::Item)
    ]
}

fn arb_kind() -> impl Strategy<Value = StateKind> {
    (0..StateKind::COUNT).prop_map(|i| StateKind::ALL[i])
}

fn arb_mod_filter() -> impl Strategy<Value = ModFilter> {
    prop_oneof![
        Just(ModFilter::Any),
        arb_aspect().prop_map(ModFilter::Aspect),
        arb_family().prop_map(ModFilter::Family),
        arb_source().prop_map(ModFilter::Source),
        arb_form().prop_map(ModFilter::Form),
        (arb_source(), arb_form()).prop_map(|(s, f)| ModFilter::SourceForm(s, f)),
    ]
}

fn arb_mod() -> impl Strategy<Value = Mod> {
    (arb_mod_filter(), any::<u32>()).prop_map(|(filter, bp)| Mod::new(filter, bp))
}

fn arb_range() -> impl Strategy<Value = DamageRange> {
    (arb_aspect(), any::<u32>(), any::<u32>())
        .prop_map(|(aspect, min, max)| DamageRange::new(aspect, min, max))
}

fn arb_tint() -> impl Strategy<Value = Tint> {
    (arb_aspect(), 0u8..=15, arb_carrier()).prop_map(|(aspect, tier, carrier)| Tint {
        aspect,
        tier,
        carrier,
    })
}

fn arb_array<T, S, const N: usize>(elem: S) -> impl Strategy<Value = [T; N]>
where
    T: Clone + core::fmt::Debug,
    S: Strategy<Value = T>,
{
    proptest::collection::vec(elem, N).prop_map(|v| {
        let ok: [T; N] = v.try_into().expect("fixed length");
        ok
    })
}

fn arb_skill() -> impl Strategy<Value = Skill> {
    (
        arb_array::<_, _, 3>(arb_aspect()),
        any::<u8>(),
        any::<[u32; 3]>(),
        any::<u32>(),
        arb_source(),
        arb_form(),
        any::<u32>(),
    )
        .prop_map(
            |(aspects, aspect_count, shares_bp, power_bp, source, form, state_chance_pct)| Skill {
                aspects,
                aspect_count,
                shares_bp,
                power_bp,
                source,
                form,
                state_chance_pct,
            },
        )
}

fn arb_states() -> impl Strategy<Value = States> {
    (
        any::<[u8; STATES_MAX]>(),
        any::<[u32; STATES_MAX]>(),
        any::<[u8; STATES_MAX]>(),
    )
        .prop_map(|(turns, power, source)| {
            let mut states = States::default();
            for i in 0..STATES_MAX {
                states[i] = ActiveState {
                    kind: StateKind::ALL[i],
                    turns: turns[i],
                    power: power[i],
                    source: source[i],
                };
            }
            states
        })
}

fn arb_attacker() -> impl Strategy<Value = Attacker> {
    (
        (
            any::<u8>(),
            any::<u32>(),
            any::<u32>(),
            arb_range(),
            arb_skill(),
        ),
        (
            arb_array::<_, _, 3>(arb_range()),
            arb_array::<_, _, 8>(arb_mod()),
            arb_array::<_, _, 4>(arb_mod()),
            any::<u32>(),
            any::<u32>(),
            any::<u32>(),
            arb_array::<_, _, 4>(arb_tint()),
        ),
    )
        .prop_map(
            |(
                (slot, attack_bonus, crit_chance_pct, weapon, skill),
                (added, increased, more, weaken_bp, penetration_bp, state_chance_bonus_bp, tints),
            )| Attacker {
                slot,
                attack_bonus,
                crit_chance_pct,
                weapon,
                skill,
                added,
                increased,
                more,
                weaken_bp,
                penetration_bp,
                state_chance_bonus_bp,
                tints,
                tempo_bonus_bp: 0,
            },
        )
}

fn arb_defender() -> impl Strategy<Value = Defender> {
    (
        any::<u32>(),
        any::<u32>(),
        any::<[u32; 6]>(),
        any::<[u32; 6]>(),
        any::<u8>(),
        any::<u8>(),
        any::<[u32; StateKind::COUNT]>(),
        arb_states(),
        any::<[u32; TAGS]>(),
        any::<u8>(),
    )
        .prop_map(
            |(
                defense,
                armor,
                resist_bp,
                vuln_bp,
                immune_aspects,
                immune_states,
                state_resist_bp,
                states,
                tag_resist_bp,
                immune_tags,
            )| Defender {
                defense,
                armor,
                resist_bp,
                vuln_bp,
                immune_aspects,
                immune_states,
                state_resist_bp,
                states,
                tag_resist_bp,
                immune_tags,
                crit_taken_pct: 0,
            },
        )
}

fn arb_hit_input() -> impl Strategy<Value = HitInput> {
    (arb_attacker(), arb_defender())
        .prop_map(|(attacker, defender)| HitInput { attacker, defender })
}

fn unique_active_kinds(states: &States) -> bool {
    let mut seen = [false; StateKind::COUNT];
    for s in states.iter().filter(|s| s.turns > 0) {
        let i = s.kind as usize;
        if seen[i] {
            return false;
        }
        seen[i] = true;
    }
    true
}

fn reported_dice(r: &HitResult) -> u32 {
    let mut n = 1;
    if r.crit_roll.is_some() {
        n += 1;
    }
    n += u32::from(r.tint_roll_count);
    n += 1;
    n += r.packets().iter().filter(|p| p.state.rolled).count() as u32;
    // Every op of every resonance that rolled a die.
    n += op_rolls(r).count() as u32;
    n
}

fn op_rolls(r: &HitResult) -> impl Iterator<Item = &StateRoll> {
    r.res_ops
        .iter()
        .flatten()
        .map(|o| &o.state)
        .filter(|s| s.rolled)
}

fn chance_in_bounds(r: &HitResult) -> bool {
    let lo = B.chance_min_pct;
    let hi = B.chance_max_pct;
    let ok = |pct: u32| pct >= lo && pct <= hi;
    r.tint_rolls().iter().all(|t| ok(t.chance_pct))
        && r.packets()
            .iter()
            .filter(|p| p.state.rolled)
            .all(|p| ok(p.state.chance_pct))
        && op_rolls(r).all(|s| ok(s.chance_pct))
}

fn check_result(input: &HitInput, r: &HitResult) -> Result<(), TestCaseError> {
    prop_assert!(u32::from(r.tint_roll_count) <= 12);
    prop_assert!(unique_active_kinds(&r.states));
    let all_immune = r.packets().iter().all(|p| p.immune);
    prop_assert_eq!(r.outcome == Outcome::Immune, all_immune);
    match r.outcome {
        Outcome::Immune => {
            prop_assert_eq!(r.damage, 0);
            prop_assert_eq!(r.rolls_used, 0);
            prop_assert_eq!(r.states, input.defender.states);
        }
        Outcome::Miss => {
            prop_assert_eq!(r.damage, 0);
            prop_assert_eq!(r.rolls_used, 1);
            prop_assert_eq!(r.states, input.defender.states);
            if r.d20 == 1 {
                prop_assert_eq!(r.outcome, Outcome::Miss);
            }
        }
        Outcome::Hit => {
            prop_assert!(r.damage >= B.min_damage);
            prop_assert_eq!(r.rolls_used, reported_dice(r));
            prop_assert!(chance_in_bounds(r));
            if r.d20 == 20 {
                prop_assert!(r.crit);
            }
        }
    }
    if r.d20 == 1 {
        prop_assert_eq!(r.outcome, Outcome::Miss);
    }
    Ok(())
}

proptest! {
    #![proptest_config(proptest_config())]

    #[test]
    fn resolve_hit_never_panics_and_is_deterministic(
        input in arb_hit_input(),
        tables in arb_tables(),
        vrf_seed in any::<[u8; 32]>(),
        seeded in any::<u64>(),
    ) {
        let a = resolve_hit(&input, &B, &tables, &mut VrfRoll::new(vrf_seed));
        let b = resolve_hit(&input, &B, &tables, &mut VrfRoll::new(vrf_seed));
        prop_assert_eq!(a, b);
        check_result(&input, &a)?;

        let c = resolve_hit(&input, &B, &tables, &mut SeededRoll::new(seeded));
        let d = resolve_hit(&input, &B, &tables, &mut SeededRoll::new(seeded));
        prop_assert_eq!(c, d);
        check_result(&input, &c)?;
    }

    #[test]
    fn hit_ordering_nat1_miss_nat20_crit(input in arb_hit_input()) {
        let miss = resolve_hit(&input, &B, &natives(), &mut ScriptedRoll::new([0u64]));
        if miss.outcome != Outcome::Immune {
            prop_assert_eq!(miss.d20, 1);
            prop_assert_eq!(miss.outcome, Outcome::Miss);
            prop_assert_eq!(miss.damage, 0);
            prop_assert_eq!(miss.rolls_used, 1);
            prop_assert_eq!(miss.states, input.defender.states);
        }

        let hit = resolve_hit(&input, &B, &natives(), &mut ScriptedRoll::new([19u64]));
        if hit.outcome != Outcome::Immune {
            prop_assert_eq!(hit.d20, 20);
            prop_assert_eq!(hit.outcome, Outcome::Hit);
            prop_assert!(hit.crit);
            prop_assert!(hit.damage >= B.min_damage);
        }
    }

    #[test]
    fn monotonic_counterweights(
        input in arb_hit_input(),
        seed in any::<[u8; 32]>(),
        extra in 1u32..=u32::MAX,
    ) {
        let base = resolve_hit(&input, &B, &natives(), &mut VrfRoll::new(seed));

        let mut weaker = input;
        weaker.attacker.weaken_bp = input.attacker.weaken_bp.saturating_add(extra);
        let rw = resolve_hit(&weaker, &B, &natives(), &mut VrfRoll::new(seed));
        prop_assert_eq!(rw.d20, base.d20);
        prop_assert_eq!(rw.outcome, base.outcome);
        prop_assert!(rw.damage <= base.damage);

        let mut resist = input;
        for v in resist.defender.resist_bp.iter_mut() {
            *v = v.saturating_add(extra);
        }
        let rr = resolve_hit(&resist, &B, &natives(), &mut VrfRoll::new(seed));
        prop_assert_eq!(rr.d20, base.d20);
        prop_assert_eq!(rr.outcome, base.outcome);
        prop_assert!(rr.damage <= base.damage);

        if input.defender.armor > 0 {
            let mut armored = input;
            armored.defender.armor = input.defender.armor.saturating_add(extra);
            let ra = resolve_hit(&armored, &B, &natives(), &mut VrfRoll::new(seed));
            prop_assert_eq!(ra.d20, base.d20);
            prop_assert_eq!(ra.outcome, base.outcome);
            prop_assert!(ra.damage <= base.damage);
        }
    }

    #[test]
    fn modifiers_never_panic(
        value in any::<u64>(),
        bonus in any::<u32>(),
        counter_bp in any::<u32>(),
        armor in any::<u32>(),
        pen in any::<u32>(),
        k in any::<u32>(),
        resist in any::<u32>(),
        cap in any::<u32>(),
    ) {
        let r = ratio(value, bonus, counter_bp);
        let c = counter(value, counter_bp);
        let m = more(value, bonus);
        let a = armor_cut(value, armor, pen, k);
        let s = resist_cut(value, resist, cap);
        let _ = (r, m);
        prop_assert!(c <= value);
        prop_assert!(a <= value);
        prop_assert!(s <= value);
    }

    #[test]
    fn modifiers_stay_positive_at_sane_sizes(
        value in (10_000u64 * 10_000)..=u64::MAX,
        bonus in any::<u32>(),
        counter_bp in 0u32..=1_000_000,
        armor in 0u32..=1_000_000,
        pen in 0u32..=1_000_000,
        k in 1u32..=1_000_000,
        resist in 0u32..=1_000_000,
        cap in 0u32..=1_000_000,
    ) {
        prop_assert!(value > 0);
        prop_assert!(ratio(value, bonus, counter_bp) > 0);
        prop_assert!(counter(value, counter_bp) > 0);
        prop_assert!(more(value, bonus) > 0);
        prop_assert!(armor_cut(value, armor, pen, k) > 0);
        prop_assert!(resist_cut(value, resist, cap) > 0);
        prop_assert!(counter(value, counter_bp) <= value);
        prop_assert!(armor_cut(value, armor, pen, k) <= value);
        prop_assert!(resist_cut(value, resist, cap) <= value);
    }

    #[test]
    fn apply_state_keeps_one_instance_and_maxima(
        kind in arb_kind(),
        t1 in 1u8..=20,
        t2 in 1u8..=20,
        p1 in any::<u32>(),
        p2 in any::<u32>(),
        src1 in any::<u8>(),
        src2 in any::<u8>(),
        other in arb_kind(),
    ) {
        let mut states = States::default();
        let first = apply_state(
            &mut states,
            ActiveState {
                kind,
                turns: t1,
                power: p1,
                source: src1,
            },
        );
        prop_assert_eq!(first, Applied::Added);
        let second = apply_state(
            &mut states,
            ActiveState {
                kind,
                turns: t2,
                power: p2,
                source: src2,
            },
        );
        prop_assert_eq!(second, Applied::Refreshed);
        let active: Vec<_> = states.iter().copied().filter(|s| s.turns > 0).collect();
        prop_assert_eq!(active.len(), 1);
        prop_assert_eq!(active[0].kind, kind);
        prop_assert_eq!(active[0].power, p1.max(p2));
        prop_assert_eq!(active[0].turns, t1.max(t2));
        if p2 > p1 {
            prop_assert_eq!(active[0].source, src2);
        } else {
            prop_assert_eq!(active[0].source, src1);
        }

        if other != kind {
            let added = apply_state(
                &mut states,
                ActiveState {
                    kind: other,
                    turns: 1,
                    power: 1,
                    source: 0,
                },
            );
            prop_assert_eq!(added, Applied::Added);
            prop_assert!(unique_active_kinds(&states));
            prop_assert_eq!(states.iter().filter(|s| s.turns > 0).count(), 2);
        }

        let before = states;
        let ignored = apply_state(
            &mut states,
            ActiveState {
                kind,
                turns: 0,
                power: u32::MAX,
                source: 9,
            },
        );
        prop_assert_eq!(ignored, Applied::None);
        prop_assert_eq!(states, before);
    }

    #[test]
    fn preview_range_ordered_and_full_immunity_is_zero(input in arb_hit_input()) {
        let (lo, hi) = preview_range(&input, &B);
        prop_assert!(lo <= hi);
        let mut immune = input;
        immune.defender.immune_aspects = u8::MAX;
        prop_assert_eq!(preview_range(&immune, &B), (0, 0));
    }

    #[test]
    fn vrf_roll_at_matches_stream(seed in any::<[u8; 32]>(), n in 0u32..32) {
        let mut src = VrfRoll::new(seed);
        for i in 0..n {
            prop_assert_eq!(src.next_u64(), VrfRoll::roll_at(&seed, i));
        }
    }
}
