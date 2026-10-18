use super::*;
use crate::resonance::Carrier;
use crate::rng::{ScriptedRoll, VrfRoll};
use crate::states::has;
use crate::tables::{AspectEntry, PairEntry, TraitEntry, NATIVE};
use crate::ModFilter;
use deadroll_types::{Family, Form, Source};

const B: Balance = Balance::DEFAULT;

/// Native states only: no pair records, no traits.
const fn natives() -> ResonanceTables {
    const NONE_PAIR: PairEntry = PairEntry {
        condition: crate::ResonanceCondition::Always,
        set: false,
        ops: [Op::NONE; 2],
    };
    let mut t = ResonanceTables {
        aspects: [AspectEntry {
            family: Family::Elemental,
            flags: 0,
            native: None,
            trait_id: 0,
            condition: crate::ResonanceCondition::Always,
        }; Aspect::COUNT],
        traits: [TraitEntry {
            present: false,
            hit: Op::NONE,
            template: [Op::NONE; 2],
        }; Aspect::COUNT],
        pairs: [[NONE_PAIR; Aspect::COUNT]; Aspect::COUNT],
    };
    let natives = [
        StateKind::Burning,
        StateKind::Chill,
        StateKind::Shock,
        StateKind::Bleeding,
        StateKind::Stun,
        StateKind::Penetration,
    ];
    let mut i = 0;
    while i < Aspect::COUNT {
        t.aspects[i].native = Some(natives[i]);
        t.aspects[i].trait_id = i as u8;
        i += 1;
    }
    t
}

const T: ResonanceTables = natives();

/// `T` with one pair record.
fn pair(
    base: Aspect,
    tint: Aspect,
    condition: crate::ResonanceCondition,
    ops: [Op; 2],
) -> ResonanceTables {
    let mut t = T;
    t.pairs[base as usize][tint as usize] = PairEntry {
        condition,
        set: true,
        ops,
    };
    t
}

/// `t` with a trait for `aspect`.
fn with_trait(
    mut t: ResonanceTables,
    aspect: Aspect,
    hit: Op,
    template: [Op; 2],
) -> ResonanceTables {
    t.traits[aspect as usize] = TraitEntry {
        present: true,
        hit,
        template,
    };
    t
}

fn st(kind: StateKind) -> u8 {
    kind as u8
}

fn basic(aspect: Aspect, min: u32, max: u32, state_chance_pct: u32) -> HitInput {
    let mut input = HitInput::default();
    input.attacker.weapon = DamageRange::new(aspect, min, max);
    input.attacker.skill = Skill::basic(Source::Combat, Form::Strike, state_chance_pct);
    input.defender.defense = 10;
    input
}

fn cold_bolt(tint: Aspect) -> HitInput {
    let mut input = basic(Aspect::Lightning, 10, 10, 0);
    let s = &mut input.attacker.skill;
    s.aspects[0] = Aspect::Cold;
    s.aspect_count = 1;
    s.shares_bp[0] = BP;
    s.source = Source::Magical;
    s.form = Form::Projectile;
    input.attacker.tints[0] = Tint::new(tint, 1, Carrier::Race);
    input
}

fn find(states: &States, kind: StateKind) -> ActiveState {
    *states
        .iter()
        .find(|s| s.kind == kind && s.turns > 0)
        .unwrap()
}

#[test]
fn plain_hit() {
    let input = basic(Aspect::Piercing, 4, 8, 0);
    // d20 = 15, d100 = 50
    let r = resolve_hit(&input, &B, &T, &mut ScriptedRoll::new([14, 50]));
    assert_eq!(r.outcome, Outcome::Hit);
    assert_eq!(
        (r.d20, r.attack_total, r.crit, r.crit_roll),
        (15, 15, false, None)
    );
    assert_eq!(r.d100, 50);
    let p = r.packets()[0];
    assert_eq!(
        (p.base_min, p.base_max, p.min, p.max),
        (40_000, 80_000, 40_000, 80_000)
    );
    assert_eq!((p.rolled, p.after_defense), (60_000, 60_000));
    assert_eq!(p.more_bp, BP);
    assert_eq!(r.damage, 6);
    assert_eq!(r.rolls_used, 2);
    assert_eq!(r.states, input.defender.states);
}

#[test]
fn misses() {
    let mut input = basic(Aspect::Piercing, 4, 8, 0);
    input.defender.defense = 16;
    let r = resolve_hit(&input, &B, &T, &mut ScriptedRoll::new([14, 50]));
    assert_eq!((r.outcome, r.damage, r.rolls_used), (Outcome::Miss, 0, 1));
    // Natural 1 misses whatever the bonus.
    input.attacker.attack_bonus = 1_000;
    let r = resolve_hit(&input, &B, &T, &mut ScriptedRoll::new([0]));
    assert_eq!((r.outcome, r.d20), (Outcome::Miss, 1));
}

#[test]
fn natural_20_hits_and_crits() {
    let mut input = basic(Aspect::Piercing, 10, 10, 0);
    input.defender.defense = u32::MAX;
    input.attacker.crit_chance_pct = 5_000;
    let r = resolve_hit(&input, &B, &T, &mut ScriptedRoll::new([19, 0]));
    assert_eq!((r.outcome, r.crit, r.crit_roll), (Outcome::Hit, true, None));
    assert_eq!(r.packets()[0].more_bp, 15_000);
    assert_eq!(r.damage, 15);
    assert_eq!(r.rolls_used, 2);
}

#[test]
fn crit_chance_is_clamped_and_rolled() {
    let mut input = basic(Aspect::Piercing, 10, 10, 0);
    input.attacker.crit_chance_pct = 100;
    // Clamped to 95. Roll 94 passes, roll 95 does not.
    let r = resolve_hit(&input, &B, &T, &mut ScriptedRoll::new([14, 94, 0]));
    assert!(r.crit);
    assert_eq!(r.crit_roll, Some(94));
    let r = resolve_hit(&input, &B, &T, &mut ScriptedRoll::new([14, 95, 0]));
    assert!(!r.crit);
    assert_eq!(r.rolls_used, 3);
}

#[test]
fn armor_and_penetration() {
    let mut input = basic(Aspect::Piercing, 6, 6, 0);
    input.defender.armor = 100;
    let r = resolve_hit(&input, &B, &T, &mut ScriptedRoll::new([14, 0]));
    assert_eq!((r.packets()[0].armor, r.damage), (100, 3));
    input.attacker.penetration_bp = BP;
    let r = resolve_hit(&input, &B, &T, &mut ScriptedRoll::new([14, 0]));
    assert_eq!((r.packets()[0].armor, r.damage), (50, 4));
}

#[test]
fn resistance_vulnerability_elemental() {
    let mut input = basic(Aspect::Fire, 10, 10, 0);
    // Armor never cuts elemental packets.
    input.defender.armor = 1_000;
    input.defender.resist_bp[Aspect::Fire as usize] = BP;
    let r = resolve_hit(&input, &B, &T, &mut ScriptedRoll::new([14, 0]));
    assert_eq!((r.packets()[0].armor, r.damage), (0, 5));
    input.defender.vuln_bp[Aspect::Fire as usize] = BP;
    assert_eq!(
        resolve_hit(&input, &B, &T, &mut ScriptedRoll::new([14, 0])).damage,
        10
    );
    // Resistance is capped: 75% at most.
    input.defender.vuln_bp = [0; 6];
    input.defender.resist_bp[Aspect::Fire as usize] = u32::MAX;
    let r = resolve_hit(&input, &B, &T, &mut ScriptedRoll::new([14, 0]));
    assert_eq!((r.packets()[0].resist_bp, r.damage), (30_000, 2));
}

#[test]
fn increase_more_weaken() {
    let mut input = basic(Aspect::Blunt, 10, 10, 0);
    input.attacker.increased[0] = Mod::new(ModFilter::Any, 5_000);
    input.attacker.increased[1] = Mod::new(ModFilter::Source(Source::Combat), 5_000);
    input.attacker.more[0] = Mod::new(ModFilter::Aspect(Aspect::Blunt), 5_000);
    input.attacker.weaken_bp = 20_000;
    // 10 x2.0 x1.5 / 3.0 = 10
    let r = resolve_hit(&input, &B, &T, &mut ScriptedRoll::new([14, 0]));
    let p = r.packets()[0];
    assert_eq!(
        (p.increased_bp, p.more_bp, p.weaken_bp),
        (10_000, 15_000, 20_000)
    );
    assert_eq!(r.damage, 10);
}

#[test]
fn minimum_damage_on_hit() {
    let mut input = basic(Aspect::Fire, 1, 1, 0);
    input.attacker.weaken_bp = u32::MAX;
    assert_eq!(
        resolve_hit(&input, &B, &T, &mut ScriptedRoll::new([14, 0])).damage,
        1
    );
}

#[test]
fn full_immunity_rolls_nothing() {
    let mut input = basic(Aspect::Fire, 10, 10, 5_000);
    input.defender.immune_aspects = aspect_bit(Aspect::Fire);
    let r = resolve_hit(&input, &B, &T, &mut ScriptedRoll::new([14, 0, 0]));
    assert_eq!(
        (r.outcome, r.damage, r.rolls_used, r.d20),
        (Outcome::Immune, 0, 0, 0)
    );
}

#[test]
fn partial_immunity() {
    let mut input = basic(Aspect::Fire, 10, 10, 0);
    input.attacker.added[0] = DamageRange::new(Aspect::Cold, 5, 5);
    input.defender.immune_aspects = aspect_bit(Aspect::Cold);
    let r = resolve_hit(&input, &B, &T, &mut ScriptedRoll::new([14, 0]));
    assert_eq!(r.packet_count, 2);
    assert!(r.packets()[1].immune);
    assert_eq!(r.packets()[1].after_defense, 0);
    assert_eq!(r.damage, 10);
}

#[test]
fn native_state_burning_power_from_damage() {
    let input = basic(Aspect::Fire, 10, 10, 50);
    let r = resolve_hit(&input, &B, &T, &mut ScriptedRoll::new([14, 0, 0]));
    let s = r.packets()[0].state;
    assert_eq!(
        (s.kind, s.rolled, s.chance_pct, s.success),
        (StateKind::Burning, true, 50, true)
    );
    assert_eq!(s.applied, Applied::Added);
    let burning = find(&r.states, StateKind::Burning);
    assert_eq!((burning.power, burning.turns), (3, 3));
    assert_eq!(r.rolls_used, 3);
}

#[test]
fn native_state_failure_resistance_and_immunity() {
    let mut input = basic(Aspect::Blood, 10, 10, 50);
    input.defender.state_resist_bp[StateKind::Bleeding as usize] = BP;
    // 50 * BP / (BP + BP) = 25; roll 25 fails.
    let r = resolve_hit(&input, &B, &T, &mut ScriptedRoll::new([14, 0, 25]));
    let s = r.packets()[0].state;
    assert_eq!(
        (s.chance_pct, s.success, s.applied),
        (25, false, Applied::None)
    );
    assert!(!has(&r.states, StateKind::Bleeding));
    input.defender.immune_states = state_bit(StateKind::Bleeding);
    let r = resolve_hit(&input, &B, &T, &mut ScriptedRoll::new([14, 0, 0]));
    assert!(!r.packets()[0].state.rolled);
    assert_eq!(r.rolls_used, 2);
}

#[test]
fn pure_pair_boosts_native_state() {
    let mut input = cold_bolt(Aspect::Cold);
    input.attacker.skill.state_chance_pct = 20;
    let r = resolve_hit(&input, &B, &T, &mut ScriptedRoll::new([14, 0, 0]));
    assert_eq!(r.tint_roll_count, 0);
    // 20% x1.59 = 31%; buildup 5000 x1.59 = 7950
    let s = r.packets()[0].state;
    assert_eq!((s.chance_pct, s.success), (31, true));
    assert_eq!(find(&r.states, StateKind::Chill).power, 7_950);
}

#[test]
fn deterministic_per_seed() {
    let mut input = cold_bolt(Aspect::Piercing);
    input.attacker.skill.state_chance_pct = 4_000;
    input.attacker.crit_chance_pct = 1_000;
    let a = resolve_hit(&input, &B, &T, &mut VrfRoll::new([3; 32]));
    let b = resolve_hit(&input, &B, &T, &mut VrfRoll::new([3; 32]));
    assert_eq!(a, b);
}

#[test]
fn preview() {
    let mut input = basic(Aspect::Piercing, 4, 8, 0);
    assert_eq!(preview_range(&input, &B), (4, 8));
    input.defender.armor = 100;
    assert_eq!(preview_range(&input, &B), (2, 4));
    input.defender.immune_aspects = aspect_bit(Aspect::Piercing);
    assert_eq!(preview_range(&input, &B), (0, 0));
}

#[test]
fn preview_hit_chances_and_ranges() {
    let mut input = basic(Aspect::Piercing, 4, 8, 0);
    // Faces 10..=19 and the natural 20 hit defense 10: 11 of 20.
    let p = preview_hit(&input, &B, &T);
    assert_eq!((p.immune, p.hit_bp, p.crit_bp), (false, 5_500, 500));
    assert_eq!((p.min, p.max, p.crit_min, p.crit_max), (4, 8, 6, 12));
    // 10 plain hits x 10% + the natural 20.
    input.attacker.crit_chance_pct = 10;
    assert_eq!(preview_hit(&input, &B, &T).crit_bp, 1_000);
    // The crit roll is clamped like in the hit.
    input.attacker.crit_chance_pct = BP;
    assert_eq!(preview_hit(&input, &B, &T).crit_bp, (BP + 10 * 9_500) / 20);
    // A natural 1 always misses, a natural 20 always hits.
    input.attacker.attack_bonus = 1_000;
    assert_eq!(preview_hit(&input, &B, &T).hit_bp, 9_500);
    input.attacker.attack_bonus = 0;
    input.defender.defense = u32::MAX;
    assert_eq!(preview_hit(&input, &B, &T).hit_bp, 500);
    input.defender.immune_aspects = aspect_bit(Aspect::Piercing);
    assert_eq!(
        preview_hit(&input, &B, &T),
        HitPreview {
            immune: true,
            ..HitPreview::default()
        }
    );
}

#[test]
fn preview_hit_matches_every_d20_face() {
    for (bonus, defense) in [(0, 10), (3, 14), (7, 2), (0, 25), (12, 30)] {
        let mut input = basic(Aspect::Blunt, 3, 9, 0);
        input.attacker.attack_bonus = bonus;
        input.defender.defense = defense;
        let hits = (0..20u64)
            .filter(|face| {
                let r = resolve_hit(&input, &B, &T, &mut ScriptedRoll::new([*face, 0]));
                r.outcome == Outcome::Hit
            })
            .count() as u32;
        assert_eq!(
            preview_hit(&input, &B, &T).hit_bp,
            hits * BP / 20,
            "{bonus} vs {defense}"
        );
    }
}

#[test]
fn preview_hit_damage_matches_the_hit_ends() {
    let mut input = cold_bolt(Aspect::Fire);
    input.attacker.added[0] = DamageRange::new(Aspect::Piercing, 2, 5);
    input.attacker.increased[0] = Mod::new(ModFilter::Any, 3_000);
    input.defender.armor = 40;
    input.defender.resist_bp[Aspect::Cold as usize] = 5_000;
    let p = preview_hit(&input, &B, &T);
    // Tint chance of tier 1 is 40; roll 40 fails. d100 0 and 100 are the range ends.
    let at = |face: u64, point: u64| {
        resolve_hit(&input, &B, &T, &mut ScriptedRoll::new([face, 40, point])).damage
    };
    assert_eq!((p.min, p.max), (at(14, 0), at(14, 100)));
    let nat20 =
        |point: u64| resolve_hit(&input, &B, &T, &mut ScriptedRoll::new([19, 40, point])).damage;
    assert_eq!((p.crit_min, p.crit_max), (nat20(0), nat20(100)));
}

#[test]
fn shock_multiplies_the_next_hit_and_the_preview() {
    let input = basic(Aspect::Lightning, 10, 10, 50);
    let r = resolve_hit(&input, &B, &T, &mut ScriptedRoll::new([14, 0, 0]));
    let shock = find(&r.states, StateKind::Shock);
    assert_eq!((shock.power, shock.turns), (B.shock_vuln_bp, 2));
    let mut next = basic(Aspect::Fire, 10, 10, 0);
    next.defender.states = r.states;
    let hit = resolve_hit(&next, &B, &T, &mut ScriptedRoll::new([14, 0]));
    assert_eq!(hit.packets()[0].vuln_shock_bp, B.shock_vuln_bp);
    assert_eq!(hit.damage, 12);
    assert_eq!(preview_range(&next, &B), (12, 12));
}

#[test]
fn penetration_lowers_armor_on_the_next_physical_hit() {
    let mut input = basic(Aspect::Piercing, 10, 10, 50);
    input.defender.armor = 100;
    let r = resolve_hit(&input, &B, &T, &mut ScriptedRoll::new([14, 0, 0]));
    assert_eq!(
        find(&r.states, StateKind::Penetration).power,
        B.penetration_power_bp
    );
    // The hit that applies it does not benefit.
    assert_eq!(
        (r.packets()[0].pen_state_bp, r.packets()[0].armor),
        (0, 100)
    );
    assert_eq!(r.damage, 5);
    let mut next = basic(Aspect::Blunt, 10, 10, 0);
    next.defender.armor = 100;
    next.defender.states = r.states;
    let hit = resolve_hit(&next, &B, &T, &mut ScriptedRoll::new([14, 0]));
    let p = hit.packets()[0];
    assert_eq!((p.pen_state_bp, p.pen_attacker_bp, p.armor), (5_000, 0, 66));
    assert_eq!(hit.damage, 6);
    assert_eq!(preview_range(&next, &B), (6, 6));
}

#[test]
fn aspect_resistance_lowers_chill_chance_and_buildup() {
    let mut input = basic(Aspect::Cold, 10, 10, 50);
    input.defender.resist_bp[Aspect::Cold as usize] = BP;
    let r = resolve_hit(&input, &B, &T, &mut ScriptedRoll::new([14, 0, 0]));
    assert_eq!(
        (
            r.packets()[0].state.chance_pct,
            r.packets()[0].state.success
        ),
        (25, true)
    );
    assert_eq!(find(&r.states, StateKind::Chill).power, 2_500);
}

#[test]
fn chill_scales_with_the_tempo_bonus() {
    let mut input = basic(Aspect::Cold, 10, 10, 20);
    input.attacker.state_chance_bonus_bp = BP;
    let plain = resolve_hit(&input, &B, &T, &mut ScriptedRoll::new([14, 0, 0]));
    assert_eq!(plain.packets()[0].state.chance_pct, 20);
    // Half again: still under the freeze threshold (a full doubling of buildup freezes).
    input.attacker.tempo_bonus_bp = 5_000;
    let r = resolve_hit(&input, &B, &T, &mut ScriptedRoll::new([14, 0, 0]));
    assert_eq!(r.packets()[0].state.chance_pct, 30);
    assert_eq!(find(&r.states, StateKind::Chill).power, 7_500);
}

#[test]
fn blunt_stun_still_skips_the_turn() {
    let input = basic(Aspect::Blunt, 10, 10, 50);
    let r = resolve_hit(&input, &B, &T, &mut ScriptedRoll::new([14, 0, 0]));
    assert_eq!(find(&r.states, StateKind::Stun).power, 0);
    let mut states = r.states;
    assert!(crate::tick_turn_start(&mut states).skip_action);
}

/// A skill of `base` (weapon 10-10 Lightning, converted), native state chance 50, tinted by
/// `tint` at tier 1 (chance 40: roll 0 triggers).
fn skill_hit(base: Aspect, tint: Aspect) -> HitInput {
    let mut input = cold_bolt(tint);
    input.attacker.skill.aspects[0] = base;
    input.attacker.skill.state_chance_pct = 50;
    input.defender.armor = 100;
    input
}

/// d20 15, tint 0, d100 0, native state 0 (lands), then `next`.
fn rolls(next: u64) -> ScriptedRoll<5> {
    ScriptedRoll::new([14, 0, 0, 0, next])
}

fn op(r: &HitResult, k: usize) -> OpResult {
    r.res_ops[0][k]
}

fn frozen() -> ActiveState {
    ActiveState {
        kind: StateKind::Freeze,
        turns: 1,
        power: 0,
        source: 9,
    }
}

#[test]
fn multiplier_on_the_base_and_its_state_requirement() {
    use crate::ResonanceCondition as C;
    let t = pair(
        Aspect::Cold,
        Aspect::Blunt,
        C::Always,
        [Op::new(OpKind::Multiplier, ANY_STATE, BP), Op::NONE],
    );
    let mut input = skill_hit(Aspect::Cold, Aspect::Blunt);
    input.attacker.skill.state_chance_pct = 0;
    let r = resolve_hit(&input, &B, &t, &mut ScriptedRoll::new([14, 0, 0]));
    // x1.36 at tier 1.
    assert_eq!(r.packets()[0].more_bp, 13_600);
    assert_eq!((op(&r, 0).acted, op(&r, 0).amount), (true, 3_600));
    // Only on a frozen target: not frozen, no bonus.
    let t = pair(
        Aspect::Cold,
        Aspect::Blunt,
        C::Always,
        [
            Op::new(OpKind::Multiplier, st(StateKind::Freeze), BP),
            Op::NONE,
        ],
    );
    let r = resolve_hit(&input, &B, &t, &mut ScriptedRoll::new([14, 0, 0]));
    assert_eq!((r.packets()[0].more_bp, op(&r, 0).acted), (BP, false));
    input.defender.states[0] = frozen();
    let r = resolve_hit(&input, &B, &t, &mut ScriptedRoll::new([14, 0, 0]));
    assert_eq!(r.packets()[0].more_bp, 13_600);
}

#[test]
fn piercing_frost_crit_on_a_frozen_target_is_stronger_and_freezes() {
    use crate::ResonanceCondition as C;
    let t = pair(
        Aspect::Cold,
        Aspect::Piercing,
        C::CanBeFrozen,
        [
            Op::new(OpKind::Multiplier, st(StateKind::Freeze), BP).on_crit(),
            Op::new(OpKind::StateOnCrit, st(StateKind::Freeze), BP),
        ],
    );
    let mut input = cold_bolt(Aspect::Piercing);
    // No crit: nothing.
    let r = resolve_hit(&input, &B, &t, &mut ScriptedRoll::new([14, 0, 0]));
    assert!(r.resonances[0].unwrap().active);
    assert_eq!(r.packets()[0].more_bp, BP);
    assert!(!op(&r, 0).acted && !op(&r, 1).acted);
    assert!(!has(&r.states, StateKind::Freeze));
    // Crit on a target that is not frozen: the crit only, then it freezes.
    let r = resolve_hit(&input, &B, &t, &mut ScriptedRoll::new([19, 0, 0]));
    assert_eq!(r.packets()[0].more_bp, 15_000);
    let s = op(&r, 1).state;
    assert_eq!(
        (s.kind, s.rolled, s.success, s.applied),
        (StateKind::Freeze, false, true, Applied::Froze)
    );
    assert_eq!(r.resonance_states[0], s);
    assert!(has(&r.states, StateKind::Freeze));
    assert_eq!(r.rolls_used, 3);
    // Crit on a frozen target: x1.5 x1.36.
    input.defender.states[0] = frozen();
    let r = resolve_hit(&input, &B, &t, &mut ScriptedRoll::new([19, 0, 0]));
    assert_eq!(r.packets()[0].more_bp, 20_400);
    // A target immune to Freeze fails the condition: no ops at all.
    input.defender.immune_states = state_bit(StateKind::Freeze);
    let r = resolve_hit(&input, &B, &t, &mut ScriptedRoll::new([19, 0, 0]));
    assert!(!r.resonances[0].unwrap().active);
    assert_eq!(r.res_ops[0][0].kind, OpKind::None);
}

#[test]
fn linked_rides_on_the_native_state_without_a_roll() {
    use crate::ResonanceCondition as C;
    // Icy Point: Penetration chills.
    let t = pair(
        Aspect::Piercing,
        Aspect::Cold,
        C::HasArmor,
        [Op::new(OpKind::Linked, st(StateKind::Chill), BP), Op::NONE],
    );
    let input = skill_hit(Aspect::Piercing, Aspect::Cold);
    let r = resolve_hit(&input, &B, &t, &mut rolls(0));
    assert_eq!(r.packets()[0].state.applied, Applied::Added);
    let o = op(&r, 0);
    assert!(o.acted && !o.state.rolled && o.state.success);
    assert_eq!(
        (o.state.kind, o.state.applied),
        (StateKind::Chill, Applied::Added)
    );
    // Tier 1 power: buildup 5000 x (1 + 0.59).
    assert_eq!(find(&r.states, StateKind::Chill).power, 7_950);
    assert_eq!(
        r.landed,
        state_bit(StateKind::Penetration) | state_bit(StateKind::Chill)
    );
    assert_eq!(r.rolls_used, 4);
    // The native state did not land (roll 50 fails 50%): no Chill.
    let r = resolve_hit(&input, &B, &t, &mut ScriptedRoll::new([14, 0, 0, 50]));
    assert!(!op(&r, 0).acted);
    assert!(!has(&r.states, StateKind::Chill));
    // Immune to Chill: no Chill even when Penetration lands.
    let mut immune = input;
    immune.defender.immune_states = state_bit(StateKind::Chill);
    let r = resolve_hit(&immune, &B, &t, &mut rolls(0));
    assert!(!op(&r, 0).acted && has(&r.states, StateKind::Penetration));
}

#[test]
fn linked_dot_power_is_a_share_of_the_base_packet() {
    use crate::ResonanceCondition as C;
    // Seething: half of the Blood packet as Burning.
    let t = pair(
        Aspect::Blood,
        Aspect::Fire,
        C::CanBleed,
        [
            Op::new(OpKind::Linked, st(StateKind::Burning), 5_000),
            Op::NONE,
        ],
    );
    let mut input = skill_hit(Aspect::Blood, Aspect::Fire);
    input.defender.armor = 0;
    input.attacker.weapon = DamageRange::new(Aspect::Lightning, 40, 40);
    let r = resolve_hit(&input, &B, &t, &mut rolls(0));
    // Bleeding: 40 x 30% = 12. Burning: 20 x 1.59 x 30% = 9.
    assert_eq!(find(&r.states, StateKind::Bleeding).power, 12);
    assert_eq!(find(&r.states, StateKind::Burning).power, 9);
}

#[test]
fn chance_ops_roll_and_scale_with_the_stat() {
    use crate::ResonanceCondition as C;
    // Thunderclap: Shock with a chance of Stun.
    let t = pair(
        Aspect::Lightning,
        Aspect::Blunt,
        C::CanBeShocked,
        [
            Op::new(OpKind::LinkedChance, st(StateKind::Stun), BP),
            Op::NONE,
        ],
    );
    let mut input = skill_hit(Aspect::Lightning, Aspect::Blunt);
    input.attacker.state_chance_bonus_bp = BP;
    // Native Shock: 50 x2 = 95 (clamped), roll 0 lands. Stun: 30 x2 = 60, roll 59 lands.
    let r = resolve_hit(&input, &B, &t, &mut rolls(59));
    let o = op(&r, 0);
    assert_eq!(
        (o.state.rolled, o.state.chance_pct, o.state.success),
        (true, 60, true)
    );
    assert!(has(&r.states, StateKind::Stun));
    assert_eq!(r.rolls_used, 5);
    let r = resolve_hit(&input, &B, &t, &mut rolls(60));
    assert!(!op(&r, 0).state.success && !has(&r.states, StateKind::Stun));
    // No Shock: no roll.
    let r = resolve_hit(&input, &B, &t, &mut ScriptedRoll::new([14, 0, 0, 100]));
    assert_eq!(r.rolls_used, 4);
    assert!(!op(&r, 0).acted);
    // An independent extra roll needs no native state.
    let t = pair(
        Aspect::Lightning,
        Aspect::Blunt,
        C::Always,
        [
            Op::new(OpKind::ExtraStateChance, st(StateKind::Stun), BP),
            Op::NONE,
        ],
    );
    let r = resolve_hit(&input, &B, &t, &mut ScriptedRoll::new([14, 0, 0, 100, 0]));
    assert!(op(&r, 0).state.success && r.rolls_used == 5);
}

#[test]
fn linked_mod_goes_to_the_target_with_the_tempo_stat_for_slow() {
    use crate::ResonanceCondition as C;
    // Chilling Wounds: Bleeding slows.
    let t = pair(
        Aspect::Blood,
        Aspect::Cold,
        C::CanBleed,
        [
            Op::new(OpKind::LinkedMod, ModKind::Slow as u8, BP),
            Op::NONE,
        ],
    );
    let mut input = skill_hit(Aspect::Blood, Aspect::Cold);
    input.attacker.tempo_bonus_bp = BP;
    input.attacker.state_chance_bonus_bp = 50_000;
    let r = resolve_hit(&input, &B, &t, &mut rolls(0));
    assert_eq!(r.target_mod_count, 1);
    let m = r.target_mods()[0];
    // 5000 x2 (Dexterity), not the source stat.
    assert_eq!(
        (m.kind, m.aspect, m.power_bp),
        (ModKind::Slow, Aspect::Blood, 10_000)
    );
    assert_eq!(m.turns, mod_turns(B.res_mod_turns));
    assert_eq!(op(&r, 0).amount, 10_000);
    // Chilled Blood: Chill weakens, by the source stat.
    let t = pair(
        Aspect::Cold,
        Aspect::Blood,
        C::CanBeChilled,
        [
            Op::new(OpKind::LinkedMod, ModKind::Weakening as u8, BP),
            Op::NONE,
        ],
    );
    let input = skill_hit(Aspect::Cold, Aspect::Blood);
    let r = resolve_hit(&input, &B, &t, &mut rolls(0));
    assert_eq!(r.target_mods()[0].kind, ModKind::Weakening);
    assert_eq!(r.target_mods()[0].power_bp, 5_000);
}

#[test]
fn native_ops_change_what_lands() {
    use crate::ResonanceCondition as C;
    // Frozen Ram: Stun lasts longer.
    let t = pair(
        Aspect::Blunt,
        Aspect::Cold,
        C::CanBeStunned,
        [Op::new(OpKind::NativeTurns, 0, BP), Op::NONE],
    );
    let input = skill_hit(Aspect::Blunt, Aspect::Cold);
    let r = resolve_hit(&input, &B, &t, &mut rolls(0));
    assert_eq!(find(&r.states, StateKind::Stun).turns, 1 + 2);
    assert_eq!((op(&r, 0).acted, op(&r, 0).amount), (true, 2));
    // Capped by the longest duration.
    let long = pair(
        Aspect::Blunt,
        Aspect::Cold,
        C::CanBeStunned,
        [Op::new(OpKind::NativeTurns, 0, 50_000), Op::NONE],
    );
    let r = resolve_hit(&input, &B, &long, &mut rolls(0));
    assert_eq!(find(&r.states, StateKind::Stun).turns, B.state_turns_max);
    // Internal Bleeding: stronger.
    let t = pair(
        Aspect::Blood,
        Aspect::Blunt,
        C::CanBleed,
        [Op::new(OpKind::NativePower, 0, BP), Op::NONE],
    );
    let mut input = skill_hit(Aspect::Blood, Aspect::Blunt);
    input.defender.armor = 0;
    input.attacker.weapon = DamageRange::new(Aspect::Lightning, 40, 40);
    let r = resolve_hit(&input, &B, &t, &mut rolls(0));
    // 40 x 1.59 x 30% = 19.
    assert_eq!(find(&r.states, StateKind::Bleeding).power, 19);
    // Frenzied Pulse: twice per turn, half as long (3 turns -> 2).
    let t = pair(
        Aspect::Blood,
        Aspect::Lightning,
        C::CanBleed,
        [Op::new(OpKind::NativeFaster, 0, 2), Op::NONE],
    );
    input.attacker.tints[0].aspect = Aspect::Lightning;
    let r = resolve_hit(&input, &B, &t, &mut rolls(0));
    let b = find(&r.states, StateKind::Bleeding);
    assert_eq!((b.power, b.turns), (24, 2));
    // Deep Wound: only on a crit.
    let t = pair(
        Aspect::Blood,
        Aspect::Piercing,
        C::CanBleed,
        [
            Op::new(OpKind::NativeTurns, 0, BP).on_crit(),
            Op::new(OpKind::NativePower, 0, BP).on_crit(),
        ],
    );
    input.attacker.tints[0].aspect = Aspect::Piercing;
    let r = resolve_hit(&input, &B, &t, &mut rolls(0));
    assert_eq!(find(&r.states, StateKind::Bleeding).turns, 3);
    assert!(!op(&r, 0).acted && !op(&r, 1).acted);
    let r = resolve_hit(&input, &B, &t, &mut ScriptedRoll::new([19, 0, 0, 0]));
    assert_eq!(find(&r.states, StateKind::Bleeding).turns, 5);
    assert!(op(&r, 0).acted && op(&r, 1).acted);
}

#[test]
fn armor_breaker_grows_with_the_armor() {
    use crate::ResonanceCondition as C;
    let t = pair(
        Aspect::Piercing,
        Aspect::Blunt,
        C::HasArmor,
        [Op::new(OpKind::NativePowerArmor, 0, BP), Op::NONE],
    );
    let mut input = skill_hit(Aspect::Piercing, Aspect::Blunt);
    // Armor 100 = K: half of 5900 = 2950 more on Penetration's 5000.
    let r = resolve_hit(&input, &B, &t, &mut rolls(0));
    assert_eq!(op(&r, 0).amount, 2_950);
    assert_eq!(find(&r.states, StateKind::Penetration).power, 6_475);
    input.defender.armor = 300;
    let r = resolve_hit(&input, &B, &t, &mut rolls(0));
    assert_eq!(op(&r, 0).amount, 4_425);
}

#[test]
fn burning_frost_stores_thaw_damage_in_the_freeze() {
    use crate::ResonanceCondition as C;
    let t = pair(
        Aspect::Cold,
        Aspect::Fire,
        C::CanBeFrozen,
        [Op::new(OpKind::ThawDamage, 0, BP), Op::NONE],
    );
    let mut input = skill_hit(Aspect::Cold, Aspect::Fire);
    input.defender.armor = 0;
    // Buildup near the threshold: this hit freezes.
    input.defender.states[0] = ActiveState {
        kind: StateKind::Chill,
        turns: 2,
        power: 9_000,
        source: 9,
    };
    let r = resolve_hit(&input, &B, &t, &mut rolls(0));
    assert_eq!(r.packets()[0].state.applied, Applied::Froze);
    // Cold packet 10 x 1.59 = 15.
    assert_eq!(find(&r.states, StateKind::Freeze).power, 15);
    assert_eq!(op(&r, 0).amount, 15);
    let mut states = r.states;
    let tick = crate::tick_turn_start(&mut states);
    assert_eq!((tick.thaw_damage, tick.skip_action), (15, true));
    // No freeze this hit: nothing stored.
    input.defender.states = States::default();
    let r = resolve_hit(&input, &B, &t, &mut rolls(0));
    assert!(!op(&r, 0).acted && !has(&r.states, StateKind::Freeze));
}

#[test]
fn cleaving_ice_removes_the_freeze_after_the_hit() {
    use crate::ResonanceCondition as C;
    let t = pair(
        Aspect::Cold,
        Aspect::Blunt,
        C::IsFrozen,
        [
            Op::new(OpKind::Multiplier, ANY_STATE, BP),
            Op::new(OpKind::RemoveState, st(StateKind::Freeze), 0),
        ],
    );
    let mut input = skill_hit(Aspect::Cold, Aspect::Blunt);
    input.attacker.skill.state_chance_pct = 0;
    input.defender.states[0] = frozen();
    let r = resolve_hit(&input, &B, &t, &mut ScriptedRoll::new([14, 0, 0]));
    assert_eq!(r.packets()[0].more_bp, 13_600);
    assert!(op(&r, 1).acted);
    assert!(!has(&r.states, StateKind::Freeze));
}

#[test]
fn jumps_are_returned_for_the_caller() {
    use crate::ResonanceCondition as C;
    // Stormfire: Burning jumps.
    let t = pair(
        Aspect::Fire,
        Aspect::Lightning,
        C::CanBurn,
        [Op::new(OpKind::Jump, st(StateKind::Burning), 0), Op::NONE],
    );
    let input = skill_hit(Aspect::Fire, Aspect::Lightning);
    let r = resolve_hit(&input, &B, &t, &mut rolls(0));
    assert_eq!(r.jump_count, 1);
    let j = r.jumps()[0];
    assert_eq!(
        (j.mode, j.limit, j.base),
        (JumpMode::Adjacent, 1, Aspect::Fire)
    );
    assert_eq!(j.state, find(&r.states, StateKind::Burning));
    // Nothing landed: nothing jumps.
    let r = resolve_hit(&input, &B, &t, &mut ScriptedRoll::new([14, 0, 0, 99]));
    assert_eq!(r.jump_count, 0);
    // Blood Arc along bleeding targets, up to 3; and the line behind the target.
    let t = pair(
        Aspect::Lightning,
        Aspect::Blood,
        C::CanBeShocked,
        [
            Op::new(OpKind::JumpChain, st(StateKind::Bleeding), 3),
            Op::new(OpKind::JumpLine, NATIVE, 0),
        ],
    );
    let input = skill_hit(Aspect::Lightning, Aspect::Blood);
    let r = resolve_hit(&input, &B, &t, &mut rolls(0));
    assert_eq!(r.jump_count, 2);
    let (chain, line) = (r.jumps()[0], r.jumps()[1]);
    assert_eq!(
        (chain.mode, chain.filter, chain.limit, chain.state.kind),
        (JumpMode::Chain, StateKind::Bleeding, 3, StateKind::Shock)
    );
    assert_eq!(
        (line.mode, line.state.kind),
        (JumpMode::Line, StateKind::Shock)
    );
}

#[test]
fn knockback_ignite_and_the_accuracy_buff() {
    use crate::ResonanceCondition as C;
    // Blasting Flame.
    let t = pair(
        Aspect::Fire,
        Aspect::Blunt,
        C::CanBurn,
        [
            Op::new(OpKind::Knockback, 0, 1),
            Op::new(OpKind::IgniteCells, 0, BP),
        ],
    );
    let mut input = skill_hit(Aspect::Fire, Aspect::Blunt);
    input.attacker.skill.state_chance_pct = 0;
    input.attacker.weapon = DamageRange::new(Aspect::Lightning, 20, 20);
    let r = resolve_hit(&input, &B, &t, &mut ScriptedRoll::new([14, 0, 0]));
    assert_eq!(r.knockback, 1);
    // 20 x 1.59 x 30% = 9.
    assert_eq!(r.ignite_power, 9);
    // Pale Flame's accuracy: +4 to Fire attacks for 2 own turns.
    let t = pair(
        Aspect::Fire,
        Aspect::Cold,
        C::CanBurn,
        [
            Op::new(OpKind::AccuracyBuff, Aspect::Fire as u8, BP),
            Op::NONE,
        ],
    );
    input.attacker.tints[0].aspect = Aspect::Cold;
    let r = resolve_hit(&input, &B, &t, &mut ScriptedRoll::new([14, 0, 0]));
    assert_eq!(
        r.accuracy_buff,
        AccuracyBuff {
            aspect: Aspect::Fire,
            turns: mod_turns(B.res_accuracy_turns),
            bonus: 4
        }
    );
}

#[test]
fn trait_hit_parts_add_fire_and_penetrate() {
    // Fury: a Fire packet of the tier (a generated pair, no record).
    let t = with_trait(
        T,
        Aspect::Fire,
        Op::new(OpKind::AddPacket, Aspect::Fire as u8, BP),
        [Op::NONE; 2],
    );
    let mut input = cold_bolt(Aspect::Fire);
    let r = resolve_hit(&input, &B, &t, &mut ScriptedRoll::new([14, 0, 0]));
    assert_eq!(r.packet_count, 2);
    let fire = r.packets()[1];
    assert_eq!(
        (fire.aspect, fire.base_min, fire.base_max),
        (Aspect::Fire, 40_000, 80_000)
    );
    let o = op(&r, 0);
    assert_eq!((o.origin, o.amount, o.amount_max), (OpOrigin::Trait, 4, 8));
    // d100 0: 10 Cold + 4 Fire.
    assert_eq!(r.damage, 14);
    // Immune to Fire: the added packet does nothing.
    input.defender.immune_aspects = aspect_bit(Aspect::Fire);
    let r = resolve_hit(&input, &B, &t, &mut ScriptedRoll::new([14, 0, 0]));
    assert!(r.packets()[1].immune);
    assert_eq!(r.damage, 10);

    // Keenness: penetration on the hit that triggered it.
    let t = with_trait(
        T,
        Aspect::Piercing,
        Op::new(OpKind::Penetrate, 0, BP),
        [Op::NONE; 2],
    );
    let mut input = basic(Aspect::Fire, 10, 10, 0);
    input.attacker.added[0] = DamageRange::new(Aspect::Piercing, 10, 10);
    input.attacker.tints[0] = Tint::new(Aspect::Piercing, 1, Carrier::Race);
    input.defender.armor = 100;
    let r = resolve_hit(&input, &B, &t, &mut ScriptedRoll::new([14, 0, 0]));
    let p = r.packets()[1];
    assert_eq!(p.pen_attacker_bp, B.keenness_pen(Tier::BEST));
    assert!(p.armor < 100);
    assert_eq!(r.packets()[0].pen_attacker_bp, 0);
}

#[test]
fn a_triple_adds_the_second_tints_trait_template() {
    use crate::ResonanceCondition as C;
    // Piercing Frost of the Storm: the pair + Swiftness (the native state jumps).
    let t = pair(
        Aspect::Cold,
        Aspect::Piercing,
        C::CanBeFrozen,
        [
            Op::new(OpKind::StateOnCrit, st(StateKind::Freeze), 0),
            Op::NONE,
        ],
    );
    let t = with_trait(
        t,
        Aspect::Lightning,
        Op::NONE,
        [Op::new(OpKind::Jump, NATIVE, 0), Op::NONE],
    );
    let mut input = cold_bolt(Aspect::Piercing);
    input.attacker.tints[1] = Tint::new(Aspect::Lightning, 2, Carrier::Item);
    input.attacker.skill.state_chance_pct = 50;
    // d20 15, Piercing 0 (margin 40), Lightning 1 (margin 36), d100 0, Chill 0.
    let r = resolve_hit(&input, &B, &t, &mut ScriptedRoll::new([14, 0, 1, 0, 0]));
    let res = r.resonances[0].unwrap();
    assert_eq!(
        (res.tint, res.second_tint),
        (Aspect::Piercing, Some(Aspect::Lightning))
    );
    let o = op(&r, 1);
    assert_eq!(
        (o.kind, o.origin, o.tint, o.tier),
        (OpKind::Jump, OpOrigin::Template, Aspect::Lightning, 2)
    );
    assert!(o.acted);
    assert_eq!(r.jumps()[0].state.kind, StateKind::Chill);
}

#[test]
fn preview_lists_the_tints_with_what_they_add() {
    let t = with_trait(
        T,
        Aspect::Fire,
        Op::new(OpKind::AddPacket, Aspect::Fire as u8, BP),
        [Op::NONE; 2],
    );
    let mut input = cold_bolt(Aspect::Fire);
    input.attacker.tints[1] = Tint::new(Aspect::Cold, 3, Carrier::Item);
    let p = preview_hit(&input, &B, &t);
    assert_eq!(p.tint_count, 2);
    let fire = p.tints[0];
    assert_eq!(
        (
            fire.tint,
            fire.carrier,
            fire.chance_pct,
            fire.pure,
            fire.generated
        ),
        (Aspect::Fire, Carrier::Race, 40, false, true)
    );
    assert_eq!((p.min, p.max), (10, 10));
    assert_eq!((fire.min, fire.max), (14, 18));
    // The hit agrees at both ends of the range.
    let at =
        |point: u64| resolve_hit(&input, &B, &t, &mut ScriptedRoll::new([14, 0, point])).damage;
    assert_eq!((fire.min, fire.max), (at(0), at(100)));
    let cold = p.tints[1];
    assert!(cold.pure && cold.chance_pct == 0);
    assert_eq!((cold.min, cold.max), (10, 10));
}
