use super::*;
use crate::resonance::Carrier;
use crate::rng::{ScriptedRoll, VrfRoll};
use crate::states::has;
use crate::tables::{
    AspectEntry, PairEntry, TraitEntry, TripleEntry, NATIVE, TRIPLES, TRIPLE_RECORD,
};
use crate::{ModFilter, TimedMods};
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
        triples: [[0; TRIPLE_RECORD]; TRIPLES],
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
fn aspect_resistance_cuts_damage_and_a_share_of_the_chill_chance_not_the_buildup() {
    let mut input = basic(Aspect::Cold, 10, 10, 50);
    let plain = resolve_hit(&input, &B, &T, &mut ScriptedRoll::new([14, 0, 0]));
    input.defender.resist_bp[Aspect::Cold as usize] = BP;
    let r = resolve_hit(&input, &B, &T, &mut ScriptedRoll::new([14, 0, 0]));
    assert!(r.damage < plain.damage);
    // 100% Cold resistance counters its native Chill by a quarter (patch 9b): 50% / 1.25.
    assert_eq!(B.aspect_state_resist_share_bp, 2_500);
    assert_eq!(
        (
            r.packets()[0].state.chance_pct,
            r.packets()[0].state.success
        ),
        (40, true)
    );
    assert_eq!(
        r.packets()[0].state.resist_bp,
        2_500,
        "the counterweight is recorded"
    );
    assert_eq!(
        find(&r.states, StateKind::Chill).power,
        find(&plain.states, StateKind::Chill).power
    );
    // The state resistance counters in full and sums with the aspect's share: 50% / 2.25.
    input.defender.state_resist_bp[StateKind::Chill as usize] = BP;
    let r = resolve_hit(&input, &B, &T, &mut ScriptedRoll::new([14, 0, 0]));
    assert_eq!(r.packets()[0].state.chance_pct, 22);
    assert_eq!(r.packets()[0].state.resist_bp, BP + 2_500);
    // Resistance to another aspect does nothing to Chill.
    let mut fire = basic(Aspect::Cold, 10, 10, 50);
    fire.defender.resist_bp[Aspect::Fire as usize] = BP;
    let r = resolve_hit(&fire, &B, &T, &mut ScriptedRoll::new([14, 0, 0]));
    assert_eq!(r.packets()[0].state.chance_pct, 50);
}

#[test]
fn tag_immunity_and_tag_resistance() {
    let input = basic(Aspect::Fire, 10, 10, 0);
    let plain = resolve_hit(&input, &B, &T, &mut ScriptedRoll::new([14, 0, 0]));
    // Immune to the hit's source: every packet, no rolls.
    let mut immune = input;
    immune.defender.immune_tags = source_bit(input.attacker.skill.source);
    let r = resolve_hit(&immune, &B, &T, &mut ScriptedRoll::new([14, 0, 0]));
    assert_eq!((r.outcome, r.damage, r.rolls_used), (Outcome::Immune, 0, 0));
    assert_eq!(preview_range(&immune, &B), (0, 0));
    // Immune to the other form: nothing changes.
    let other = match input.attacker.skill.form {
        Form::Strike => Form::Projectile,
        Form::Projectile => Form::Strike,
    };
    let mut miss = input;
    miss.defender.immune_tags = form_bit(other);
    assert_eq!(
        resolve_hit(&miss, &B, &T, &mut ScriptedRoll::new([14, 0, 0])).damage,
        plain.damage
    );
    // Resistance by the hit's form sums with the aspect's, then the cap.
    let mut res = input;
    res.defender.resist_bp[Aspect::Fire as usize] = 2_000;
    res.defender.tag_resist_bp[Source::COUNT + input.attacker.skill.form as usize] = 3_000;
    let r = resolve_hit(&res, &B, &T, &mut ScriptedRoll::new([14, 0, 0]));
    assert_eq!(r.packets()[0].resist_bp, 5_000);
    res.defender.tag_resist_bp[input.attacker.skill.source as usize] = u32::MAX;
    let r = resolve_hit(&res, &B, &T, &mut ScriptedRoll::new([14, 0, 0]));
    assert_eq!(r.packets()[0].resist_bp, B.resist_cap_bp);
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

fn on_target(kind: StateKind, power: u32) -> ActiveState {
    ActiveState {
        kind,
        turns: 2,
        power,
        source: 9,
    }
}

#[test]
fn force_crit_makes_the_hit_a_crit_and_the_crit_ops_act() {
    use crate::ResonanceCondition as C;
    // Piercing Frost (resonance pass): the hit is a crit and the target freezes.
    let t = pair(
        Aspect::Cold,
        Aspect::Piercing,
        C::CanBeFrozen,
        [
            Op::new(OpKind::ForceCrit, 0, 0),
            Op::new(OpKind::StateOnCrit, st(StateKind::Freeze), 0),
        ],
    );
    let input = cold_bolt(Aspect::Piercing);
    let r = resolve_hit(&input, &B, &t, &mut ScriptedRoll::new([14, 0, 0]));
    assert!(r.crit && r.crit_roll.is_none());
    assert_eq!(r.packets()[0].more_bp, BP + B.crit_more_bp);
    assert!(op(&r, 0).acted && op(&r, 1).acted);
    assert!(has(&r.states, StateKind::Freeze));
    // The tint does not trigger: a plain hit.
    let r = resolve_hit(&input, &B, &t, &mut ScriptedRoll::new([14, 99, 0]));
    assert!(!r.crit && !has(&r.states, StateKind::Freeze));
    // The preview shows the forced crit's damage for that tint.
    let p = preview_hit(&input, &B, &t);
    assert_eq!((p.tints[0].min, p.tints[0].max), (p.crit_min, p.crit_max));
}

#[test]
fn exposed_on_the_target_adds_to_every_attackers_crit_chance() {
    let mut input = basic(Aspect::Piercing, 4, 8, 0);
    // No crit chance at all: no roll.
    let r = resolve_hit(&input, &B, &T, &mut ScriptedRoll::new([14, 50]));
    assert_eq!(r.crit_roll, None);
    let mut mods = TimedMods::default();
    crate::support::apply_mod(
        &mut mods,
        TimedMod {
            kind: ModKind::Exposed,
            turns: 2,
            power_bp: 2_000,
            ..TimedMod::default()
        },
    );
    crate::support::apply_to_defender(&mut input.defender, &mods);
    assert_eq!(input.defender.crit_taken_pct, 20);
    // d20 15, crit roll 10 < 20: a crit.
    let r = resolve_hit(&input, &B, &T, &mut ScriptedRoll::new([14, 10, 50]));
    assert_eq!((r.crit, r.crit_roll), (true, Some(10)));
    assert!(preview_hit(&input, &B, &T).crit_bp > BP / 20);
}

#[test]
fn state_if_had_needs_the_native_state_before_the_hit() {
    use crate::ResonanceCondition as C;
    // Incinerating Discharge (resonance pass): Burning only on a target that was already shocked.
    let t = pair(
        Aspect::Lightning,
        Aspect::Fire,
        C::CanBeShocked,
        [
            Op::new(OpKind::StateIfHad, st(StateKind::Burning), BP),
            Op::NONE,
        ],
    );
    let mut input = skill_hit(Aspect::Lightning, Aspect::Fire);
    let r = resolve_hit(&input, &B, &t, &mut rolls(0));
    assert!(has(&r.states, StateKind::Shock) && !has(&r.states, StateKind::Burning));
    assert!(!op(&r, 0).acted);
    input.defender.states[0] = on_target(StateKind::Shock, 2_000);
    // The native state fails its roll: Burning still lands, without a roll.
    let r = resolve_hit(&input, &B, &t, &mut ScriptedRoll::new([14, 0, 0, 99]));
    let o = op(&r, 0);
    assert!(o.acted && !o.state.rolled);
    assert_eq!(o.state.kind, StateKind::Burning);
    assert!(has(&r.states, StateKind::Burning));
    // Immune to Burning: nothing.
    input.defender.immune_states = state_bit(StateKind::Burning);
    let r = resolve_hit(&input, &B, &t, &mut rolls(0));
    assert!(!op(&r, 0).acted);
}

#[test]
fn splash_is_a_share_of_the_damage_when_the_target_had_the_state() {
    use crate::ResonanceCondition as C;
    let t = pair(
        Aspect::Lightning,
        Aspect::Cold,
        C::CanBeShocked,
        [
            Op::new(OpKind::Splash, st(StateKind::Freeze), 5_000),
            Op::NONE,
        ],
    );
    let mut input = skill_hit(Aspect::Lightning, Aspect::Cold);
    let r = resolve_hit(&input, &B, &t, &mut rolls(0));
    assert_eq!(r.splash, Splash::default());
    assert!(!op(&r, 0).acted);
    input.defender.states[0] = frozen();
    let r = resolve_hit(&input, &B, &t, &mut rolls(0));
    assert_eq!(r.splash.damage, r.damage / 2);
    assert_eq!(r.splash.aspect, Aspect::Lightning);
    assert_eq!(op(&r, 0).amount, r.damage / 2);
}

#[test]
fn apply_state_lands_without_a_roll_and_is_bleeding_is_the_condition() {
    use crate::ResonanceCondition as C;
    // Blackout (resonance pass): on a bleeding target, Stun and the Bleeding refreshed.
    let t = pair(
        Aspect::Blood,
        Aspect::Blunt,
        C::IsBleeding,
        [
            Op::new(OpKind::ApplyState, st(StateKind::Stun), BP),
            Op::new(OpKind::ApplyState, NATIVE, BP),
        ],
    );
    let mut input = skill_hit(Aspect::Blood, Aspect::Blunt);
    let r = resolve_hit(&input, &B, &t, &mut rolls(0));
    assert!(!r.resonances[0].unwrap().active);
    assert!(!has(&r.states, StateKind::Stun));
    input.defender.states[0] = ActiveState {
        kind: StateKind::Bleeding,
        turns: 1,
        power: 1,
        source: 9,
    };
    // The native roll fails (99): the resonance still stuns and refreshes the Bleeding.
    let r = resolve_hit(&input, &B, &t, &mut ScriptedRoll::new([14, 0, 0, 99]));
    assert!(r.resonances[0].unwrap().active);
    assert!(has(&r.states, StateKind::Stun));
    assert_eq!(
        find(&r.states, StateKind::Bleeding).turns,
        B.turns(StateKind::Bleeding)
    );
    assert!(op(&r, 0).acted && op(&r, 1).acted);
}

#[test]
fn apply_mod_marks_the_target_and_follows_the_jumps() {
    use crate::ResonanceCondition as C;
    // Lightning Needle (resonance pass): Shock jumps, both are exposed to crits.
    let t = pair(
        Aspect::Lightning,
        Aspect::Piercing,
        C::CanBeShocked,
        [
            Op::new(OpKind::Jump, NATIVE, 0),
            Op::new(OpKind::ApplyMod, ModKind::Exposed as u8, BP),
        ],
    );
    let input = skill_hit(Aspect::Lightning, Aspect::Piercing);
    let r = resolve_hit(&input, &B, &t, &mut rolls(0));
    assert_eq!(r.jump_count, 1);
    let m = r.target_mods()[0];
    assert_eq!(
        (m.kind, m.power_bp, m.turns),
        (
            ModKind::Exposed,
            B.res_exposed_pct[0] * 100,
            mod_turns(B.res_mod_turns)
        )
    );
    assert_eq!(r.jump_mods(), &[m]);
    // The native state fails: no jump, the target is still marked.
    let r = resolve_hit(&input, &B, &t, &mut ScriptedRoll::new([14, 0, 0, 99]));
    assert_eq!((r.jump_count, r.target_mod_count), (0, 1));
    // Root has no strength, only presence.
    let t = pair(
        Aspect::Piercing,
        Aspect::Blunt,
        C::Always,
        [Op::new(OpKind::ApplyMod, ModKind::Root as u8, BP), Op::NONE],
    );
    let input = skill_hit(Aspect::Piercing, Aspect::Blunt);
    let r = resolve_hit(&input, &B, &t, &mut rolls(0));
    let mut mods = TimedMods::default();
    crate::support::apply_mod(&mut mods, r.target_mods()[0]);
    assert!(crate::support::rooted(&mods));
    assert!(!crate::support::rooted(&TimedMods::default()));
}

#[test]
fn mod_after_native_lasts_beyond_the_state_and_nova_spreads_it() {
    use crate::ResonanceCondition as C;
    // Frozen Ram (resonance pass): Stun longer, then Slow.
    let t = pair(
        Aspect::Blunt,
        Aspect::Cold,
        C::CanBeStunned,
        [
            Op::new(OpKind::NativeTurns, 0, BP),
            Op::new(OpKind::ModAfterNative, ModKind::Slow as u8, BP),
        ],
    );
    let input = skill_hit(Aspect::Blunt, Aspect::Cold);
    let r = resolve_hit(&input, &B, &t, &mut rolls(0));
    let stun = find(&r.states, StateKind::Stun).turns;
    assert_eq!(stun, B.turns(StateKind::Stun) + B.res_turns[0]);
    let m = r.target_mods()[0];
    assert_eq!(
        (m.kind, m.turns),
        (ModKind::Slow, mod_turns(B.res_mod_turns + stun))
    );
    let r = resolve_hit(&input, &B, &t, &mut ScriptedRoll::new([14, 0, 0, 99]));
    assert_eq!(r.target_mod_count, 0);
    // Dread Pulse (resonance pass): after the Stun lands, the enemies around are stunned and weakened.
    let t = pair(
        Aspect::Blunt,
        Aspect::Blood,
        C::CanBeStunned,
        [Op::new(OpKind::Nova, ModKind::Weakening as u8, 2), Op::NONE],
    );
    let input = skill_hit(Aspect::Blunt, Aspect::Blood);
    let r = resolve_hit(&input, &B, &t, &mut rolls(0));
    assert_eq!(r.nova.radius, 2);
    assert_eq!(r.nova.state, find(&r.states, StateKind::Stun));
    assert_eq!(
        (r.nova.modifier.kind, r.nova.modifier.power_bp),
        (ModKind::Weakening, B.res_mod_bp[0])
    );
    // The target itself is not weakened.
    assert_eq!(r.target_mod_count, 0);
    let r = resolve_hit(&input, &B, &t, &mut ScriptedRoll::new([14, 0, 0, 99]));
    assert_eq!(r.nova, Nova::default());
}

// ---------------------------------------------------------------------------------------------
// Triples

use crate::states::{tick_turn_start_with, MARK_DORMANT, MARK_ESCALATE};
use crate::tables::{triple_index, triple_of};

/// `T` with a pair record (always) and one triple record.
fn tri(
    base: Aspect,
    tint: Aspect,
    second: Aspect,
    pair_ops: [Op; 2],
    ops: [Op; 3],
) -> ResonanceTables {
    let mut t = pair(base, tint, crate::ResonanceCondition::Always, pair_ops);
    assert!(t.set_triple(base, tint, second, &TripleEntry { set: true, ops }));
    t
}

/// A hit with base `base` and the tints `tint` (best) and `second`.
fn tri_input(base: Aspect, tint: Aspect, second: Aspect) -> HitInput {
    let mut input = skill_hit(base, tint);
    input.attacker.tints[1] = Tint::new(second, 2, Carrier::Item);
    input
}

/// d20 15, both tints succeed (the first by more), d100 0, native roll 0, then zeros.
fn tri_rolls() -> ScriptedRoll<5> {
    ScriptedRoll::new([14, 0, 1, 0, 0])
}

fn tri_op(r: &HitResult, k: usize) -> OpResult {
    r.res_ops[0][k]
}

fn gen(kind: OpKind, arg: u8, value: u32) -> Op {
    Op::new(kind, arg, value)
}

fn just(op: Op) -> [Op; 3] {
    [op, Op::NONE, Op::NONE]
}

fn two(a: Op, b: Op) -> [Op; 3] {
    [a, b, Op::NONE]
}

const NO_PAIR: [Op; 2] = [Op::NONE; 2];

#[test]
fn triple_index_is_a_bijection_over_the_120_triples() {
    let mut seen = [false; TRIPLES];
    for b in Aspect::ALL.iter().copied() {
        for t in Aspect::ALL.iter().copied() {
            for s in Aspect::ALL.iter().copied() {
                match triple_index(b, t, s) {
                    Some(i) => {
                        assert!(!seen[i]);
                        seen[i] = true;
                        assert_eq!(triple_of(i), Some([b, t, s]));
                    }
                    None => assert!(b == t || b == s || t == s),
                }
            }
        }
    }
    assert!(seen.iter().all(|x| *x));
    assert_eq!(triple_of(TRIPLES), None);
}

#[test]
fn a_triple_record_replaces_the_second_trait_template() {
    let mut t = tri(
        Aspect::Cold,
        Aspect::Piercing,
        Aspect::Lightning,
        NO_PAIR,
        just(gen(OpKind::ApplyState, st(StateKind::Bleeding), BP)),
    );
    t = with_trait(
        t,
        Aspect::Lightning,
        Op::NONE,
        [gen(OpKind::Jump, NATIVE, 0), Op::NONE],
    );
    let input = tri_input(Aspect::Cold, Aspect::Piercing, Aspect::Lightning);
    let r = resolve_hit(&input, &B, &t, &mut tri_rolls());
    let o = tri_op(&r, 0);
    assert_eq!((o.kind, o.origin), (OpKind::ApplyState, OpOrigin::Triple));
    assert!(o.acted && has(&r.states, StateKind::Bleeding));
    // The template did not run.
    assert_eq!(r.jump_count, 0);
    assert_eq!(r.res_ops[0][1].kind, OpKind::None);
    // Another second tint has no record: the template stays.
    let input = tri_input(Aspect::Cold, Aspect::Piercing, Aspect::Fire);
    let t2 = with_trait(
        t,
        Aspect::Fire,
        Op::NONE,
        [gen(OpKind::Jump, NATIVE, 0), Op::NONE],
    );
    let r = resolve_hit(&input, &B, &t2, &mut tri_rolls());
    assert_eq!(tri_op(&r, 0).origin, OpOrigin::Template);
    // Two equal tints are no triple of three aspects.
    let same = tri_input(Aspect::Cold, Aspect::Piercing, Aspect::Piercing);
    let t3 = with_trait(
        t,
        Aspect::Piercing,
        Op::NONE,
        [gen(OpKind::Jump, NATIVE, 0), Op::NONE],
    );
    let r = resolve_hit(&same, &B, &t3, &mut tri_rolls());
    assert_eq!(tri_op(&r, 0).origin, OpOrigin::Template);
}

fn fire_triple(ops: [Op; 3]) -> (ResonanceTables, HitInput) {
    (
        tri(Aspect::Fire, Aspect::Cold, Aspect::Lightning, NO_PAIR, ops),
        tri_input(Aspect::Fire, Aspect::Cold, Aspect::Lightning),
    )
}

#[test]
fn state_slower_lasts_longer_and_ticks_less() {
    let apply = gen(OpKind::ApplyState, st(StateKind::Burning), BP);
    let (t0, input) = fire_triple(just(apply));
    let plain = find(
        &resolve_hit(&input, &B, &t0, &mut tri_rolls()).states,
        StateKind::Burning,
    );
    let (t, _) = fire_triple(two(
        apply,
        gen(OpKind::StateSlower, st(StateKind::Burning), 2),
    ));
    let r = resolve_hit(&input, &B, &t, &mut tri_rolls());
    let slow = find(&r.states, StateKind::Burning);
    assert_eq!(slow.power, (plain.power / 2).max(1));
    assert_eq!(slow.turns, (plain.turns * 2).min(B.state_turns_max));
    assert!(tri_op(&r, 1).acted);
    // Stun is no damage over time: it only lasts longer.
    let t = tri(
        Aspect::Blunt,
        Aspect::Fire,
        Aspect::Cold,
        NO_PAIR,
        just(gen(OpKind::StateSlower, st(StateKind::Stun), 2)),
    );
    let input = tri_input(Aspect::Blunt, Aspect::Fire, Aspect::Cold);
    let r = resolve_hit(&input, &B, &t, &mut tri_rolls());
    assert_eq!(find(&r.states, StateKind::Stun).turns, 2);
    // The state did not land: nothing to slow.
    let r = resolve_hit(&input, &B, &t, &mut ScriptedRoll::new([14, 0, 1, 0, 99]));
    assert!(!tri_op(&r, 0).acted && !has(&r.states, StateKind::Stun));
}

#[test]
fn state_faster_ticks_twice_per_turn() {
    let apply = gen(OpKind::ApplyState, st(StateKind::Bleeding), BP);
    let (t0, input) = fire_triple(just(apply));
    let plain = find(
        &resolve_hit(&input, &B, &t0, &mut tri_rolls()).states,
        StateKind::Bleeding,
    );
    let (t, _) = fire_triple(two(
        apply,
        gen(OpKind::StateFaster, st(StateKind::Bleeding), 2),
    ));
    let r = resolve_hit(&input, &B, &t, &mut tri_rolls());
    let fast = find(&r.states, StateKind::Bleeding);
    assert_eq!(fast.power, plain.power * 2);
    assert_eq!(fast.turns as u32, (plain.turns as u32).div_ceil(2));
    // Nothing landed to speed up.
    let (t, _) = fire_triple(just(gen(
        OpKind::StateFaster,
        st(StateKind::Bleeding),
        2,
    )));
    let r = resolve_hit(&input, &B, &t, &mut tri_rolls());
    assert!(!tri_op(&r, 0).acted && !has(&r.states, StateKind::Bleeding));
}

#[test]
fn tick_now_hits_with_the_state_power_right_away() {
    let apply = gen(OpKind::ApplyState, st(StateKind::Bleeding), 60_000);
    let (t0, input) = fire_triple(just(apply));
    let base = resolve_hit(&input, &B, &t0, &mut tri_rolls());
    let power = find(&base.states, StateKind::Bleeding).power;
    assert!(power > 0);
    let (t, _) = fire_triple(two(
        apply,
        gen(OpKind::TickNow, st(StateKind::Bleeding), 2),
    ));
    let r = resolve_hit(&input, &B, &t, &mut tri_rolls());
    assert_eq!(r.tick_damage, power * 2);
    assert_eq!(r.damage, base.damage + power * 2);
    // Once by default.
    let (t, _) = fire_triple(two(
        apply,
        gen(OpKind::TickNow, st(StateKind::Bleeding), 0),
    ));
    assert_eq!(
        resolve_hit(&input, &B, &t, &mut tri_rolls()).tick_damage,
        power
    );
    // No Bleeding on the target: nothing ticks.
    let (t, _) = fire_triple(just(gen(OpKind::TickNow, st(StateKind::Bleeding), 1)));
    let r = resolve_hit(&input, &B, &t, &mut tri_rolls());
    assert_eq!((r.tick_damage, tri_op(&r, 0).acted), (0, false));
}

#[test]
fn escalate_marks_the_state_and_each_tick_grows() {
    let apply = gen(OpKind::ApplyState, st(StateKind::Burning), 60_000);
    let (t, input) = fire_triple(two(
        apply,
        gen(OpKind::Escalate, st(StateKind::Burning), 0),
    ));
    let r = resolve_hit(&input, &B, &t, &mut tri_rolls());
    assert!(tri_op(&r, 1).acted);
    let burning = find(&r.states, StateKind::Burning);
    assert!(burning.source & MARK_ESCALATE != 0);
    let mut states = r.states;
    let first = tick_turn_start_with(&mut states, &B).dot_damage;
    let second = tick_turn_start_with(&mut states, &B).dot_damage;
    assert_eq!(first, burning.power);
    assert_eq!(
        second,
        burning.power + burning.power * B.escalate_step_bp / BP
    );
    // Not on the target: nothing to mark.
    let (t, _) = fire_triple(just(gen(OpKind::Escalate, st(StateKind::Bleeding), 0)));
    let r = resolve_hit(&input, &B, &t, &mut tri_rolls());
    assert!(!tri_op(&r, 0).acted);
}

#[test]
fn delayed_waits_for_the_control_to_end() {
    let delayed = gen(
        OpKind::Delayed,
        st(StateKind::Bleeding),
        st(StateKind::Freeze) as u32 | 150 << 8,
    );
    let t = tri(
        Aspect::Cold,
        Aspect::Fire,
        Aspect::Blood,
        NO_PAIR,
        just(delayed),
    );
    let mut input = tri_input(Aspect::Cold, Aspect::Fire, Aspect::Blood);
    input.defender.states[0] = ActiveState {
        kind: StateKind::Freeze,
        turns: 2,
        power: 0,
        source: 9,
    };
    let r = resolve_hit(&input, &B, &t, &mut tri_rolls());
    let bleeding = find(&r.states, StateKind::Bleeding);
    assert!(bleeding.source & MARK_DORMANT != 0 && bleeding.source & 0x1F == 0);
    let mut states = r.states;
    // Frozen: the bleeding does not tick or count down.
    let first = tick_turn_start_with(&mut states, &B);
    assert_eq!(first.dot_damage, 0);
    assert_eq!(find(&states, StateKind::Bleeding).turns, bleeding.turns);
    // The Freeze thawed in that tick (2 turns: one more). Frozen again for the next one.
    let second = tick_turn_start_with(&mut states, &B);
    assert_eq!(second.dot_damage, 0);
    assert!(!has(&states, StateKind::Freeze));
    // Now it starts.
    let third = tick_turn_start_with(&mut states, &B);
    assert_eq!(third.dot_damage, bleeding.power);
    assert_eq!(find(&states, StateKind::Bleeding).source & MARK_DORMANT, 0);
    // The target is not under control: it lands at once.
    input.defender.states[0] = ActiveState::default();
    let r = resolve_hit(&input, &B, &t, &mut tri_rolls());
    assert_eq!(
        find(&r.states, StateKind::Bleeding).source & MARK_DORMANT,
        0
    );
    assert!(tri_op(&r, 0).acted && tri_op(&r, 0).amount == 0);
    // 150% of the base packet: stronger than 100%.
    let plain = gen(
        OpKind::Delayed,
        st(StateKind::Bleeding),
        st(StateKind::Freeze) as u32,
    );
    let t1 = tri(
        Aspect::Cold,
        Aspect::Fire,
        Aspect::Blood,
        NO_PAIR,
        just(plain),
    );
    let p1 = find(
        &resolve_hit(&input, &B, &t1, &mut tri_rolls()).states,
        StateKind::Bleeding,
    )
    .power;
    assert!(find(&r.states, StateKind::Bleeding).power > p1);
}

#[test]
fn tick_aspect_rebuilds_the_tick_through_the_new_aspects_defenses() {
    let t = tri(
        Aspect::Blood,
        Aspect::Lightning,
        Aspect::Fire,
        NO_PAIR,
        just(gen(OpKind::TickAspect, Aspect::Fire as u8, 0)),
    );
    let mut input = tri_input(Aspect::Blood, Aspect::Lightning, Aspect::Fire);
    input.attacker.skill.state_chance_pct = 100;
    let plain = resolve_hit(&input, &B, &T, &mut tri_rolls());
    let r = resolve_hit(&input, &B, &t, &mut tri_rolls());
    // Armor 100 (K 100) halved the Bleeding; as Fire it meets no resistance.
    assert!(
        find(&r.states, StateKind::Bleeding).power
            > find(&plain.states, StateKind::Bleeding).power
    );
    assert!(tri_op(&r, 0).acted);
    // Fire resistance cuts it again.
    input.defender.resist_bp[Aspect::Fire as usize] = 10_000;
    let r2 = resolve_hit(&input, &B, &t, &mut tri_rolls());
    assert!(
        find(&r2.states, StateKind::Bleeding).power
            < find(&r.states, StateKind::Bleeding).power
    );
    // Immune to Fire: the tick stays what it was.
    input.defender.resist_bp[Aspect::Fire as usize] = 0;
    input.defender.immune_aspects = aspect_bit(Aspect::Fire);
    let r3 = resolve_hit(&input, &B, &t, &mut tri_rolls());
    assert!(!tri_op(&r3, 0).acted);
    assert_eq!(
        find(&r3.states, StateKind::Bleeding).power,
        find(&plain.states, StateKind::Bleeding).power
    );
}

#[test]
fn remove_mod_takes_back_what_the_pair_put_on() {
    let pair_ops = [
        gen(OpKind::LinkedMod, ModKind::Weakening as u8, BP),
        Op::NONE,
    ];
    let t = tri(
        Aspect::Cold,
        Aspect::Blood,
        Aspect::Fire,
        pair_ops,
        just(gen(OpKind::RemoveMod, ModKind::Weakening as u8, 0)),
    );
    let input = tri_input(Aspect::Cold, Aspect::Blood, Aspect::Fire);
    let r = resolve_hit(&input, &B, &t, &mut tri_rolls());
    assert_eq!(r.target_mod_count, 0);
    assert_eq!(r.remove_mods, crate::support::mod_bit(ModKind::Weakening));
    assert!(tri_op(&r, 1).acted);
    // Another kind: the Weakening stays.
    let t = tri(
        Aspect::Cold,
        Aspect::Blood,
        Aspect::Fire,
        pair_ops,
        just(gen(OpKind::RemoveMod, ModKind::Slow as u8, 0)),
    );
    let r = resolve_hit(&input, &B, &t, &mut tri_rolls());
    assert_eq!(r.target_mods()[0].kind, ModKind::Weakening);
}

#[test]
fn native_shorter_divides_the_native_duration() {
    let (t, input) = fire_triple(just(gen(OpKind::NativeShorter, 0, 3)));
    let r = resolve_hit(&input, &B, &t, &mut tri_rolls());
    assert_eq!(find(&r.states, StateKind::Burning).turns, 1);
    assert!(tri_op(&r, 0).acted);
    // Never below one turn, and nothing acts when the native state fails.
    let (t, _) = fire_triple(just(gen(OpKind::NativeShorter, 0, 100)));
    let r = resolve_hit(&input, &B, &t, &mut ScriptedRoll::new([14, 0, 1, 0, 99]));
    assert!(!tri_op(&r, 0).acted && !has(&r.states, StateKind::Burning));
}

#[test]
fn thaw_splash_and_splash_line() {
    let pair_ops = [
        gen(OpKind::ApplyState, st(StateKind::Freeze), BP),
        gen(OpKind::ThawDamage, 0, BP),
    ];
    let t = tri(
        Aspect::Cold,
        Aspect::Fire,
        Aspect::Lightning,
        pair_ops,
        two(
            gen(OpKind::ThawSplash, 0, BP),
            gen(OpKind::SplashLine, 0, 0),
        ),
    );
    let input = tri_input(Aspect::Cold, Aspect::Fire, Aspect::Lightning);
    let r = resolve_hit(&input, &B, &t, &mut tri_rolls());
    let thaw = find(&r.states, StateKind::Freeze).power;
    assert!(thaw > 0);
    assert_eq!(
        (r.splash.damage, r.splash.aspect, r.splash.line),
        (thaw, Aspect::Cold, true)
    );
    // No Freeze landed: no thaw to share, and no splash to line up.
    let t = tri(
        Aspect::Cold,
        Aspect::Fire,
        Aspect::Lightning,
        NO_PAIR,
        two(
            gen(OpKind::ThawSplash, 0, BP),
            gen(OpKind::SplashLine, 0, 0),
        ),
    );
    let r = resolve_hit(&input, &B, &t, &mut tri_rolls());
    assert_eq!((r.splash.damage, r.splash.line), (0, false));
    assert!(!tri_op(&r, 0).acted && !tri_op(&r, 1).acted);
}

#[test]
fn chain_radius_reaches_every_enemy_with_the_filter() {
    let value = 2 | (st(StateKind::Bleeding) as u32) << 8;
    let (t, input) = fire_triple(two(
        gen(OpKind::ApplyState, st(StateKind::Shock), BP),
        gen(OpKind::ChainRadius, st(StateKind::Shock), value),
    ));
    let r = resolve_hit(&input, &B, &t, &mut tri_rolls());
    let j = r.jumps()[0];
    assert_eq!(
        (j.mode, j.limit, j.filter, j.filter_any, j.state.kind),
        (
            JumpMode::Radius,
            2,
            StateKind::Bleeding,
            false,
            StateKind::Shock
        )
    );
    // Any enemy.
    let (t, _) = fire_triple(two(
        gen(OpKind::ApplyState, st(StateKind::Shock), BP),
        gen(
            OpKind::ChainRadius,
            st(StateKind::Shock),
            1 | (ANY_STATE as u32) << 8,
        ),
    ));
    assert!(resolve_hit(&input, &B, &t, &mut tri_rolls()).jumps()[0].filter_any);
    // The state did not land.
    let (t, _) = fire_triple(just(gen(
        OpKind::ChainRadius,
        st(StateKind::Shock),
        2 | 0xFF00,
    )));
    let r = resolve_hit(&input, &B, &t, &mut tri_rolls());
    assert_eq!((r.jump_count, tri_op(&r, 0).acted), (0, false));
}

#[test]
fn mod_after_state_lasts_beyond_the_state() {
    let (t, input) = fire_triple(two(
        gen(OpKind::ApplyState, st(StateKind::Stun), BP),
        gen(
            OpKind::ModAfterState,
            ModKind::Slow as u8,
            st(StateKind::Stun) as u32,
        ),
    ));
    let r = resolve_hit(&input, &B, &t, &mut tri_rolls());
    let stun = find(&r.states, StateKind::Stun).turns;
    let m = r.target_mods()[0];
    assert_eq!(m.kind, ModKind::Slow);
    assert_eq!(m.turns, mod_turns(B.res_mod_turns + stun));
    // No Stun landed: no modifier.
    let (t, _) = fire_triple(just(gen(
        OpKind::ModAfterState,
        ModKind::Slow as u8,
        st(StateKind::Stun) as u32,
    )));
    let r = resolve_hit(&input, &B, &t, &mut tri_rolls());
    assert_eq!((r.target_mod_count, tri_op(&r, 0).acted), (0, false));
}

#[test]
fn on_miss_rolls_even_when_the_attack_misses() {
    let t = tri(
        Aspect::Piercing,
        Aspect::Lightning,
        Aspect::Blunt,
        NO_PAIR,
        just(gen(OpKind::OnMiss, st(StateKind::Shock), BP)),
    );
    let mut input = tri_input(Aspect::Piercing, Aspect::Lightning, Aspect::Blunt);
    input.defender.defense = 100;
    // d20 2 misses; both tints succeed; the Shock roll 0 lands.
    let r = resolve_hit(&input, &B, &t, &mut ScriptedRoll::new([1, 0, 1, 0]));
    assert_eq!((r.outcome, r.damage, r.rolls_used), (Outcome::Miss, 0, 4));
    assert!(has(&r.states, StateKind::Shock));
    assert!(tri_op(&r, 0).acted && tri_op(&r, 0).state.rolled);
    // The roll can fail.
    let r = resolve_hit(&input, &B, &t, &mut ScriptedRoll::new([1, 0, 1, 99]));
    assert!(!has(&r.states, StateKind::Shock) && tri_op(&r, 0).state.rolled);
    // Without the second tint no such triple exists: a miss stops after the d20.
    input.attacker.tints[1] = Tint::default();
    let r = resolve_hit(&input, &B, &t, &mut ScriptedRoll::new([1, 0, 1, 0]));
    assert_eq!(
        (r.outcome, r.rolls_used, r.tint_roll_count),
        (Outcome::Miss, 1, 0)
    );
    // On a hit it rolls last, after the native state.
    let mut input = tri_input(Aspect::Piercing, Aspect::Lightning, Aspect::Blunt);
    input.defender.defense = 1;
    let r = resolve_hit(&input, &B, &t, &mut ScriptedRoll::new([14, 0, 1, 0, 99, 0]));
    assert_eq!(r.outcome, Outcome::Hit);
    assert!(!has(&r.states, StateKind::Penetration));
    assert!(has(&r.states, StateKind::Shock));
    assert_eq!(r.rolls_used, 6);
}

#[test]
fn blinded_divides_the_d20_and_sundered_the_armor_against_piercing() {
    let mut input = basic(Aspect::Piercing, 10, 10, 0);
    input.defender.defense = 10;
    input.defender.armor = 100;
    let hit = |input: &HitInput| resolve_hit(input, &B, &T, &mut ScriptedRoll::new([14, 0]));
    assert_eq!(hit(&input).outcome, Outcome::Hit);
    input.attacker.blinded_bp = 10_000;
    let r = hit(&input);
    assert_eq!((r.d20, r.attack_total, r.outcome), (15, 7, Outcome::Miss));
    // A natural 20 still hits, and the total never drops below 1.
    let r = resolve_hit(&input, &B, &T, &mut ScriptedRoll::new([19, 0]));
    assert_eq!(r.outcome, Outcome::Hit);
    input.attacker.blinded_bp = u32::MAX;
    let r = resolve_hit(&input, &B, &T, &mut ScriptedRoll::new([1, 0]));
    assert_eq!(r.attack_total, 1);
    // Sundered: more damage from Piercing, none more from Blunt.
    let mut piercing = basic(Aspect::Piercing, 100, 100, 0);
    piercing.defender.armor = 100;
    let plain = hit(&piercing).damage;
    piercing.defender.sunder_bp = 10_000;
    let sundered = hit(&piercing);
    assert!(sundered.damage > plain);
    assert_eq!(sundered.packets()[0].armor, 50);
    let mut blunt = basic(Aspect::Blunt, 100, 100, 0);
    blunt.defender.armor = 100;
    let plain = hit(&blunt).damage;
    blunt.defender.sunder_bp = 10_000;
    assert_eq!(hit(&blunt).damage, plain);
    // The preview agrees.
    let p = preview_hit(&piercing, &B, &T);
    assert_eq!(p.min, sundered.damage);
}

#[test]
fn root_scales_its_duration_with_the_value() {
    let apply = |value| {
        let t = tri(
            Aspect::Piercing,
            Aspect::Blunt,
            Aspect::Fire,
            [gen(OpKind::ApplyMod, ModKind::Root as u8, value), Op::NONE],
            [Op::NONE; 3],
        );
        let input = skill_hit(Aspect::Piercing, Aspect::Blunt);
        resolve_hit(&input, &B, &t, &mut rolls(0)).target_mods()[0]
    };
    assert_eq!(apply(BP).turns, mod_turns(B.res_mod_turns));
    assert_eq!(apply(2 * BP).turns, mod_turns(B.res_mod_turns * 2));
}

#[test]
fn the_new_mods_get_their_strength_from_the_tier_table() {
    for (kind, power) in [
        (ModKind::Blinded, B.res_counter_bp[0]),
        (ModKind::OffGuard, B.res_counter_bp[0]),
        (ModKind::Sundered, B.res_counter_bp[0]),
        (ModKind::Dulled, B.res_counter_bp[0]),
        (ModKind::Dazed, BP),
        (ModKind::Silenced, BP),
        (ModKind::NoHeal, BP),
    ] {
        let t = tri(
            Aspect::Fire,
            Aspect::Cold,
            Aspect::Lightning,
            [gen(OpKind::ApplyMod, kind as u8, BP), Op::NONE],
            [Op::NONE; 3],
        );
        let input = skill_hit(Aspect::Fire, Aspect::Cold);
        let m = resolve_hit(&input, &B, &t, &mut rolls(0)).target_mods()[0];
        assert_eq!(
            (m.kind, m.power_bp, m.turns),
            (kind, power, mod_turns(B.res_mod_turns))
        );
    }
}

#[test]
fn no_op_of_a_triple_panics_whatever_its_numbers() {
    let mut b = B;
    b.state_turns_max = u8::MAX;
    b.escalate_step_bp = u32::MAX;
    for kind in 1..OpKind::COUNT {
        let kind = OpKind::from_u8(kind).unwrap();
        for arg in 0..=16u8 {
            for value in [0, 1, 2, 0xFFFF, 0x7FFF_FFFF, u32::MAX] {
                let op = Op {
                    kind,
                    crit_only: false,
                    arg,
                    value,
                };
                // A triple record holds 16-bit values; a pair record any.
                let small = Op {
                    value: value.min(0xFFFF),
                    ..op
                };
                for base in [Aspect::Fire, Aspect::Blood] {
                    let t = tri(
                        base,
                        Aspect::Cold,
                        Aspect::Lightning,
                        [op, Op::NONE],
                        [small, small, small],
                    );
                    let mut input = tri_input(base, Aspect::Cold, Aspect::Lightning);
                    input.attacker.skill.state_chance_pct = 100;
                    input.defender.states[0] = frozen();
                    for rolls in [[14, 0, 1, 0, 0], [1, 0, 1, 0, 0], [19, 0, 1, 100, 100]] {
                        let r = resolve_hit(&input, &b, &t, &mut ScriptedRoll::new(rolls));
                        let mut states = r.states;
                        tick_turn_start_with(&mut states, &b);
                        tick_turn_start_with(&mut states, &b);
                    }
                }
            }
        }
    }
}
