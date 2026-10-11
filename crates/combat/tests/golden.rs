//! Golden hits: fixed inputs, fixed VRF seeds, pinned breakdowns. The session program and
//! the website (WASM) must reproduce these numbers exactly. Every number below was checked
//! by hand against docs/COMBAT_MATH.md, section 6 (comments show the arithmetic).

use deadroll_combat::*;
use deadroll_types::{ActiveState, Aspect, Form, Source, StateKind};

pub const SEED: [u8; 32] = [7; 32];
/// A seed on which both tints of the mage succeed: two resonances in one cast.
pub const RESONANCE_SEED: [u8; 32] = [9; 32];

/// Human warrior, Piercing predisposition (race, tier 10), sword Piercing 4-8 (item tint
/// Piercing, tier 12), +1-2 Piercing added, basic attack, vs a wolf.
pub fn warrior_vs_wolf() -> HitInput {
    let mut input = HitInput::default();
    let a = &mut input.attacker;
    a.slot = 0;
    a.attack_bonus = 3;
    a.crit_chance_bp = 500;
    a.weapon = DamageRange::new(Aspect::Piercing, 4, 8);
    a.skill = Skill::basic(Source::Combat, Form::Strike, 0);
    a.added[0] = DamageRange::new(Aspect::Piercing, 1, 2);
    a.increased[0] = Mod::new(ModFilter::Source(Source::Combat), 1_000);
    a.tints[0] = Tint::new(Aspect::Piercing, 10, Carrier::Race);
    a.tints[1] = Tint::new(Aspect::Piercing, 12, Carrier::Item);
    let d = &mut input.defender;
    d.defense = 11;
    d.armor = 20;
    d.resist_bp[Aspect::Cold as usize] = 2_000;
    input
}

/// Forest elf mage, Cold predisposition (race, tier 8), Lightning wand 5-8 (item tint
/// Lightning, tier 11), skill "Frost Bolt": Cold 70% + Lightning 30%, power x1.5, vs a goblin.
pub fn mage_vs_goblin() -> HitInput {
    let mut input = HitInput::default();
    let a = &mut input.attacker;
    a.slot = 1;
    a.attack_bonus = 4;
    a.crit_chance_bp = 500;
    a.weapon = DamageRange::new(Aspect::Lightning, 5, 8);
    a.skill = Skill {
        aspects: [Aspect::Cold, Aspect::Lightning, Aspect::Fire],
        aspect_count: 2,
        shares_bp: [7_000, 3_000, 0],
        power_bp: 15_000,
        source: Source::Magical,
        form: Form::Projectile,
        state_chance_bp: 4_000,
    };
    a.increased[0] = Mod::new(ModFilter::Source(Source::Magical), 2_000);
    a.tints[0] = Tint::new(Aspect::Cold, 8, Carrier::Race);
    a.tints[1] = Tint::new(Aspect::Lightning, 11, Carrier::Item);
    let d = &mut input.defender;
    d.defense = 10;
    d.armor = 10;
    d.resist_bp[Aspect::Lightning as usize] = 1_000;
    input
}

/// `(aspect, base_min, base_max, increased, more, min, max, rolled, armor, resist, after_defense)`
type PacketRow = (Aspect, u64, u64, u32, u32, u64, u64, u64, u32, u32, u64);

fn rows(r: &HitResult) -> Vec<PacketRow> {
    r.packets()
        .iter()
        .map(|p| {
            (
                p.aspect,
                p.base_min,
                p.base_max,
                p.increased_bp,
                p.more_bp,
                p.min,
                p.max,
                p.rolled,
                p.armor,
                p.resist_bp,
                p.after_defense,
            )
        })
        .collect()
}

fn active(r: &HitResult) -> Vec<ActiveState> {
    r.states.iter().copied().filter(|s| s.turns > 0).collect()
}

#[test]
fn vrf_rolls_are_pinned() {
    // sha256(seed ‖ index u32 LE), first 8 bytes LE. A TS replay must match these.
    assert_eq!(VrfRoll::roll_at(&SEED, 0), 0x0d70_0235_478b_e3d9);
    assert_eq!(VrfRoll::roll_at(&SEED, 1), 0x8ea1_504a_a556_6f46);
    assert_eq!(VrfRoll::roll_at(&SEED, 2), 0xd45b_ec64_28c8_aa07);
    assert_eq!(VrfRoll::roll_at(&SEED, 3), 0xc662_133b_338c_7890);
    assert_eq!(VrfRoll::roll_at(&RESONANCE_SEED, 0), 0x0d60_61ea_8126_d559);
    assert_eq!(VrfRoll::roll_at(&RESONANCE_SEED, 1), 0x3c4b_3144_0021_8eb1);
}

#[test]
fn golden_warrior_vs_wolf() {
    let r = resolve_hit(
        &warrior_vs_wolf(),
        &Balance::DEFAULT,
        &mut VrfRoll::new(SEED),
    );
    assert_eq!(r.outcome, Outcome::Hit);
    // d20 10 + 3 = 13 >= 11; crit chance 5%, roll 9734: no crit.
    assert_eq!((r.d20, r.attack_total, r.defense), (10, 13, 11));
    assert_eq!((r.crit, r.crit_roll), (false, Some(9_734)));
    // Both tints are Piercing = pure pairs with base Piercing: no tint dice.
    assert_eq!(r.tint_roll_count, 0);
    assert_eq!(r.resonances, [None; 3]);
    assert_eq!(r.d100, 84);
    // base 4-8 + 1-2 = 5-10; +10% -> 5.5-11; d100 84 -> 5.5 + 5.5 x 0.84 = 10.12;
    // armor 20, K 100: 10.12 x 100 / 120 = 8.4333
    assert_eq!(
        rows(&r),
        [(
            Aspect::Piercing,
            50_000,
            100_000,
            1_000,
            10_000,
            55_000,
            110_000,
            101_200,
            20,
            0,
            84_333
        )]
    );
    // Piercing has no P0 native state.
    assert!(!r.packets()[0].state.rolled);
    assert_eq!(r.damage, 8);
    assert!(active(&r).is_empty());
    // d20, crit, d100
    assert_eq!(r.rolls_used, 3);
}

#[test]
fn golden_mage_vs_goblin() {
    let r = resolve_hit(
        &mage_vs_goblin(),
        &Balance::DEFAULT,
        &mut VrfRoll::new(SEED),
    );
    assert_eq!(r.outcome, Outcome::Hit);
    assert_eq!(
        (r.d20, r.attack_total, r.crit, r.crit_roll),
        (10, 14, false, Some(9_734))
    );
    // Base Cold: the Lightning item tint rolls (tier 11, 14.50%); the Cold race tint is a pure pair.
    // Base Lightning: the Cold race tint rolls (tier 8, 22.00%); the Lightning item tint is a pure pair.
    let tints: Vec<_> = r
        .tint_rolls()
        .iter()
        .map(|t| {
            (
                t.base,
                t.tint,
                t.carrier,
                t.tier,
                t.chance_bp,
                t.roll,
                t.success,
            )
        })
        .collect();
    assert_eq!(
        tints,
        [
            (
                Aspect::Cold,
                Aspect::Lightning,
                Carrier::Item,
                11,
                1_450,
                6_231,
                false
            ),
            (
                Aspect::Lightning,
                Aspect::Cold,
                Carrier::Race,
                8,
                2_200,
                7_744,
                false
            ),
        ]
    );
    assert_eq!(r.resonances, [None; 3]);
    assert_eq!(r.d100, 84);
    // Cold: 5-8 x1.5 x70% = 5.25-8.4; +20% -> 6.3-10.08; d100 84 -> 9.4752; no armor (elemental).
    // Lightning: 5-8 x1.5 x30% = 2.25-3.6; +20% -> 2.7-4.32; d100 84 -> 4.0608; resist 10% -> 3.6916.
    assert_eq!(
        rows(&r),
        [
            (
                Aspect::Cold,
                52_500,
                84_000,
                2_000,
                10_000,
                63_000,
                100_800,
                94_752,
                0,
                0,
                94_752
            ),
            (
                Aspect::Lightning,
                22_500,
                36_000,
                2_000,
                10_000,
                27_000,
                43_200,
                40_608,
                0,
                1_000,
                36_916
            ),
        ]
    );
    // 9.4752 + 3.6916 = 13.1668 -> 13
    assert_eq!(r.damage, 13);
    // Chill: 40% x (1 + pure pair tier 8 = 38%) = 55.20%, roll 1188 succeeds;
    // buildup 50% x 1.38 = 69%.
    let s = r.packets()[0].state;
    assert_eq!(
        (s.kind, s.rolled, s.chance_bp, s.roll, s.success, s.applied),
        (StateKind::Chill, true, 5_520, 1_188, true, Applied::Added)
    );
    assert!(!r.packets()[1].state.rolled);
    assert_eq!(
        active(&r),
        [ActiveState {
            kind: StateKind::Chill,
            turns: 3,
            power: 6_900,
            source: 1
        }]
    );
    // d20, crit, 2 tints, d100, chill
    assert_eq!(r.rolls_used, 6);
}

#[test]
fn golden_mage_double_resonance() {
    let r = resolve_hit(
        &mage_vs_goblin(),
        &Balance::DEFAULT,
        &mut VrfRoll::new(RESONANCE_SEED),
    );
    assert_eq!(r.outcome, Outcome::Hit);
    assert_eq!(
        (r.d20, r.attack_total, r.crit, r.crit_roll),
        (10, 14, false, Some(1_729))
    );
    let tints: Vec<_> = r
        .tint_rolls()
        .iter()
        .map(|t| (t.chance_bp, t.roll, t.success))
        .collect();
    assert_eq!(tints, [(1_450, 759, true), (2_200, 1_745, true)]);
    // No cap: each base gets its own pair. Cold + Lightning (Ringing Ice, id 8) and
    // Lightning + Cold (Frost Spark, id 13).
    let res: Vec<_> = r
        .resonances
        .iter()
        .flatten()
        .map(|x| {
            (
                x.base,
                x.tint,
                x.tier,
                x.carrier,
                x.id,
                x.active,
                x.second_tint,
            )
        })
        .collect();
    assert_eq!(
        res,
        [
            (
                Aspect::Cold,
                Aspect::Lightning,
                11,
                Carrier::Item,
                8,
                true,
                None
            ),
            (
                Aspect::Lightning,
                Aspect::Cold,
                8,
                Carrier::Race,
                13,
                true,
                None
            ),
        ]
    );
    assert_eq!(r.d100, 72);
    // Cold: 6.3-10.08 x1.16 (tier 11) = 7.308-11.6928; d100 72 -> 7.308 + 4.3848 x 0.72 = 10.4650
    // Lightning: 2.7-4.32 x1.22 (tier 8) = 3.294-5.2704; d100 72 -> 4.7170; resist 10% -> 4.2881
    assert_eq!(
        rows(&r),
        [
            (
                Aspect::Cold,
                52_500,
                84_000,
                2_000,
                11_600,
                73_080,
                116_928,
                104_650,
                0,
                0,
                104_650
            ),
            (
                Aspect::Lightning,
                22_500,
                36_000,
                2_000,
                12_200,
                32_940,
                52_704,
                47_170,
                0,
                1_000,
                42_881
            ),
        ]
    );
    // 10.4650 + 4.2881 = 14.7531 -> 14
    assert_eq!(r.damage, 14);
    // Generic pairs carry no state effect in P0.
    assert_eq!(r.resonance_states, [StateRoll::default(); 3]);
    let s = r.packets()[0].state;
    assert_eq!(
        (s.chance_bp, s.roll, s.applied),
        (5_520, 1_669, Applied::Added)
    );
    assert_eq!(
        active(&r),
        [ActiveState {
            kind: StateKind::Chill,
            turns: 3,
            power: 6_900,
            source: 1
        }]
    );
    assert_eq!(r.rolls_used, 6);
}

#[test]
fn golden_preview() {
    // Warrior: 5.5-11 after armor 20 (x100/120) -> 4.58-9.16
    assert_eq!(preview_range(&warrior_vs_wolf(), &Balance::DEFAULT), (4, 9));
    // Mage: 6.3 + 2.7/1.1 = 8.75 .. 10.08 + 4.32/1.1 = 14.007
    assert_eq!(preview_range(&mage_vs_goblin(), &Balance::DEFAULT), (8, 14));
}
