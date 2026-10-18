//! One attack, in the fixed order of docs/COMBAT_MATH.md, section 6.
//!
//! Roll order (the website replays it from the VRF seed, keep it stable):
//! 1. `d20` hit roll (none if the target is immune to every packet);
//! 2. crit chance roll, only on a hit that is not a natural 20 and with `crit_chance_bp > 0`;
//! 3. tint rolls: bases in skill order, tints in slot order ([`crate::resonance::roll_tints`]);
//! 4. one `d100` for the point in every packet's range;
//! 5. native state rolls, one per packet in packet order (only where a roll is needed);
//! 6. resonance state rolls, bases in skill order, effects in table order.
//!
//! A miss stops after roll 1. Rolls that are not needed are not made.

use deadroll_types::{ActiveState, Aspect, StateKind, BP, STATES_MAX};

use crate::balance::Balance;
use crate::modifiers::{
    armor_cut, counter, effective_armor, is_physical, more, product_matching, ratio, resist_cut,
    sum_matching, Mod,
};
use crate::packet::{build_packets, DamageRange, Skill, ADDED_MAX, PACKETS_MAX, SKILL_ASPECTS_MAX};
use crate::resonance::{
    entry, multiplier_bp, roll_tints, Resonance, ResonanceEffect, TargetView, Tint, TintRoll,
    TINTS_MAX, TINT_ROLLS_MAX,
};
use crate::rng::{chance, d100, d20, RollSource};
use crate::states::{
    apply_chill, apply_state, aspect_bit, freeze, native_state, state_bit, Applied, States,
};

#[cfg(feature = "borsh")]
use borsh::{BorshDeserialize, BorshSerialize};

/// "Increase" modifier slots of an attacker.
pub const INCREASED_MAX: usize = 8;
/// "More" modifier slots of an attacker (crit and resonance are added by the crate).
pub const MORE_MAX: usize = 4;

/// Everything about the attacker that matters for one hit.
#[cfg_attr(feature = "borsh", derive(BorshSerialize, BorshDeserialize))]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Attacker {
    /// Turn-order slot, written as the source of applied states.
    pub slot: u8,
    /// Added to the d20.
    pub attack_bonus: u32,
    /// Crit chance besides a natural 20, bp. 0 = only natural 20.
    pub crit_chance_bp: u32,
    pub weapon: DamageRange,
    pub skill: Skill,
    pub added: [DamageRange; ADDED_MAX],
    pub increased: [Mod; INCREASED_MAX],
    pub more: [Mod; MORE_MAX],
    /// Summed Weakenings on the attacker (counterweight on damage).
    pub weaken_bp: u32,
    /// Armor penetration (counterweight on target armor).
    pub penetration_bp: u32,
    /// "Increase" on native state chances.
    pub state_chance_bonus_bp: u32,
    pub tints: [Tint; TINTS_MAX],
}

/// Everything about the target that matters for one hit.
#[cfg_attr(feature = "borsh", derive(BorshSerialize, BorshDeserialize))]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Defender {
    /// Threshold for `d20 + attack_bonus`.
    pub defense: u32,
    pub armor: u32,
    /// Resistance per aspect (counterweight, capped by `Balance::resist_cap_bp`).
    pub resist_bp: [u32; Aspect::COUNT],
    /// Vulnerability per aspect (multiplier).
    pub vuln_bp: [u32; Aspect::COUNT],
    /// Immunity per aspect, bit `1 << aspect`.
    pub immune_aspects: u8,
    /// Immunity per state, bit `1 << state`.
    pub immune_states: u8,
    /// State resistance per state kind (counterweight on the chance).
    pub state_resist_bp: [u32; StateKind::COUNT],
    /// States before the hit.
    pub states: States,
}

#[cfg_attr(feature = "borsh", derive(BorshSerialize, BorshDeserialize))]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct HitInput {
    pub attacker: Attacker,
    pub defender: Defender,
}

#[cfg_attr(feature = "borsh", derive(BorshSerialize, BorshDeserialize))]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
#[repr(u8)]
pub enum Outcome {
    #[default]
    Miss,
    Hit,
    /// The target is immune to every packet: no rolls, no damage.
    Immune,
}

/// A state application check.
#[cfg_attr(feature = "borsh", derive(BorshSerialize, BorshDeserialize))]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct StateRoll {
    pub kind: StateKind,
    /// A die was rolled. False with `success` = guaranteed (e.g. on a crit).
    pub rolled: bool,
    pub chance_bp: u32,
    pub roll: u16,
    pub success: bool,
    pub applied: Applied,
}

/// One packet after every step. Values scaled by `BP` unless named otherwise.
#[cfg_attr(feature = "borsh", derive(BorshSerialize, BorshDeserialize))]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct PacketResult {
    pub aspect: Aspect,
    pub immune: bool,
    /// Step 9: converted base + added.
    pub base_min: u64,
    pub base_max: u64,
    /// Step 10: summed increase, bp.
    pub increased_bp: u32,
    /// Step 11: combined multiplier, bp (`BP` = x1): modifiers x crit x resonance.
    pub more_bp: u32,
    /// Step 12: attacker counterweight, bp.
    pub weaken_bp: u32,
    /// After steps 10-12.
    pub min: u64,
    pub max: u64,
    /// Step 13: the d100 point.
    pub rolled: u64,
    /// Step 14: effective armor after penetration (physical packets, else 0).
    pub armor: u32,
    /// Step 15: applied resistance (after cap), bp.
    pub resist_bp: u32,
    /// Step 16: vulnerability, bp.
    pub vuln_bp: u32,
    /// After target defense.
    pub after_defense: u64,
    /// Step 20: native state check (`rolled == false && !success` = no check).
    pub state: StateRoll,
}

/// The full hit breakdown (docs/COMBAT_MATH.md, section 12).
#[cfg_attr(feature = "borsh", derive(BorshSerialize, BorshDeserialize))]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct HitResult {
    pub outcome: Outcome,
    pub d20: u8,
    /// `d20 + attack_bonus`, saturating.
    pub attack_total: u32,
    pub defense: u32,
    pub crit: bool,
    /// The crit chance roll, if one was made.
    pub crit_roll: Option<u16>,
    pub tint_rolls: [TintRoll; TINT_ROLLS_MAX],
    pub tint_roll_count: u8,
    /// Per skill base slot.
    pub resonances: [Option<Resonance>; SKILL_ASPECTS_MAX],
    pub d100: u8,
    pub packets: [PacketResult; PACKETS_MAX],
    pub packet_count: u8,
    /// Resonance state checks, per skill base slot (one state effect per resonance).
    pub resonance_states: [StateRoll; SKILL_ASPECTS_MAX],
    /// Target states after the hit.
    pub states: States,
    /// Final damage, rounded down once.
    pub damage: u32,
    /// Dice used from the roll source.
    pub rolls_used: u32,
}

impl HitResult {
    pub fn packets(&self) -> &[PacketResult] {
        &self.packets[..self.packet_count as usize]
    }

    pub fn tint_rolls(&self) -> &[TintRoll] {
        &self.tint_rolls[..self.tint_roll_count as usize]
    }
}

/// Counts the rolls taken from the inner source.
struct Counted<'a, R: RollSource> {
    inner: &'a mut R,
    used: u32,
}

impl<R: RollSource> RollSource for Counted<'_, R> {
    fn next_u64(&mut self) -> u64 {
        self.used += 1;
        self.inner.next_u64()
    }
}

/// Resolves one attack. See the module docs for the roll order.
pub fn resolve_hit(input: &HitInput, balance: &Balance, src: &mut impl RollSource) -> HitResult {
    let mut out = HitResult::default();
    resolve_hit_into(input, balance, src, &mut out);
    out
}

/// Same as [`resolve_hit`], writing into `out` (lets on-chain callers keep the large result
/// off the 4 KB stack frame, e.g. in a `Box`).
pub fn resolve_hit_into(
    input: &HitInput,
    balance: &Balance,
    src: &mut impl RollSource,
    out: &mut HitResult,
) {
    *out = HitResult::default();
    let mut src = Counted {
        inner: src,
        used: 0,
    };
    let a = &input.attacker;
    let d = &input.defender;
    out.defense = d.defense;
    out.states = d.states;

    // Steps 3, 9: packets and immunity.
    let packets = build_packets(a.weapon, &a.skill, &a.added);
    out.packet_count = packets.count;
    for (r, p) in out.packets.iter_mut().zip(packets.as_slice()) {
        r.aspect = p.aspect;
        r.immune = d.immune_aspects & aspect_bit(p.aspect) != 0;
        r.base_min = p.min;
        r.base_max = p.max;
    }
    if out.packets().iter().all(|p| p.immune) {
        out.outcome = Outcome::Immune;
        return;
    }

    // Steps 4-5: hit and crit.
    out.d20 = d20(&mut src);
    out.attack_total = (out.d20 as u32).saturating_add(a.attack_bonus);
    let hit = match out.d20 {
        1 => false,
        20 => true,
        _ => out.attack_total >= d.defense,
    };
    if !hit {
        out.outcome = Outcome::Miss;
        out.rolls_used = src.used;
        return;
    }
    out.outcome = Outcome::Hit;
    out.crit = out.d20 == 20;
    if !out.crit && a.crit_chance_bp > 0 {
        let (crit, roll) = chance(&mut src, balance.clamp_chance(a.crit_chance_bp));
        out.crit = crit;
        out.crit_roll = Some(roll);
    }

    // Steps 6-8: resonance.
    let (bases, base_count) = a.skill.bases(a.weapon.aspect);
    let bases = &bases[..base_count];
    let view = TargetView {
        immune_states: d.immune_states,
        armor: d.armor,
        states: &d.states,
    };
    let tints = roll_tints(bases, &a.tints, d.immune_aspects, &view, balance, &mut src);
    out.tint_rolls = tints.rolls;
    out.tint_roll_count = tints.roll_count;
    out.resonances = tints.resonances;

    // Steps 10-12: attacker modifiers, per packet.
    let (source, form) = (a.skill.source, a.skill.form);
    for r in out
        .packets
        .iter_mut()
        .take(packets.count as usize)
        .filter(|r| !r.immune)
    {
        r.increased_bp = sum_matching(&a.increased, r.aspect, source, form);
        let mut mult = product_matching(&a.more, r.aspect, source, form) as u64;
        if out.crit {
            mult = more(mult, balance.crit_more_bp);
        }
        for res in tints.resonances.iter().flatten() {
            if res.active && res.base == r.aspect {
                mult = more(mult, multiplier_bp(res, balance));
            }
        }
        r.more_bp = mult.min(u32::MAX as u64) as u32;
        r.weaken_bp = a.weaken_bp;
        let apply = |v: u64| {
            let v = more(v, r.increased_bp);
            let v = (v as u128 * r.more_bp as u128 / BP as u128).min(u64::MAX as u128) as u64;
            counter(v, r.weaken_bp)
        };
        r.min = apply(r.base_min);
        r.max = apply(r.base_max).max(r.min);
    }

    // Step 13: one d100 for every packet.
    out.d100 = d100(&mut src);
    // Steps 14-16: target defense, per packet.
    let mut total: u128 = 0;
    for r in out
        .packets
        .iter_mut()
        .take(packets.count as usize)
        .filter(|r| !r.immune)
    {
        let span = (r.max - r.min) as u128 * out.d100 as u128 / 100;
        r.rolled = (r.min as u128 + span).min(u64::MAX as u128) as u64;
        let mut v = r.rolled;
        if is_physical(r.aspect) {
            r.armor = effective_armor(d.armor, a.penetration_bp);
            v = armor_cut(v, d.armor, a.penetration_bp, balance.armor_k);
        }
        r.resist_bp = d.resist_bp[r.aspect as usize].min(balance.resist_cap_bp);
        v = resist_cut(v, r.resist_bp, balance.resist_cap_bp);
        r.vuln_bp = d.vuln_bp[r.aspect as usize];
        v = more(v, r.vuln_bp);
        r.after_defense = v;
        total += v as u128;
    }

    // Steps 17-18: round down once, minimum damage on a hit.
    let damage = (total / BP as u128).min(u32::MAX as u128) as u32;
    out.damage = damage.max(balance.min_damage);

    // Step 20: native states, one check per packet.
    let mut states = d.states;
    for i in 0..packets.count as usize {
        let r = out.packets[i];
        if r.immune {
            continue;
        }
        let Some(kind) = native_state(r.aspect) else {
            continue;
        };
        if d.immune_states & state_bit(kind) != 0 || a.skill.state_chance_bp == 0 {
            continue;
        }
        let pure = bases
            .iter()
            .position(|b| *b == r.aspect)
            .map(|slot| tints.pure_pair_bp[slot])
            .unwrap_or(0);
        let bonus = a.state_chance_bonus_bp.saturating_add(pure);
        let raw = ratio(
            a.skill.state_chance_bp as u64,
            bonus,
            d.state_resist_bp[kind as usize],
        );
        let chance_bp = balance.clamp_chance(raw.min(u32::MAX as u64) as u32);
        let (success, roll) = chance(&mut src, chance_bp);
        let mut roll_result = StateRoll {
            kind,
            rolled: true,
            chance_bp,
            roll,
            success,
            applied: Applied::None,
        };
        if success {
            roll_result.applied =
                apply_kind(&mut states, kind, r.after_defense, pure, a.slot, balance);
        }
        out.packets[i].state = roll_result;
    }

    // Step 21: resonance states.
    for (slot, res) in tints.resonances.iter().enumerate() {
        let Some(res) = res.filter(|r| r.active) else {
            continue;
        };
        let base_damage = out
            .packets()
            .iter()
            .find(|p| p.aspect == res.base)
            .map(|p| p.after_defense)
            .unwrap_or(0);
        for effect in entry(res.base, res.tint).effects {
            let roll_result = match effect {
                ResonanceEffect::StateOnCrit(kind)
                    if out.crit && d.immune_states & state_bit(kind) == 0 =>
                {
                    let applied = apply_kind(&mut states, kind, base_damage, 0, a.slot, balance);
                    StateRoll {
                        kind,
                        rolled: false,
                        chance_bp: BP,
                        roll: 0,
                        success: true,
                        applied,
                    }
                }
                ResonanceEffect::ExtraStateChance(kind, bp)
                    if d.immune_states & state_bit(kind) == 0 =>
                {
                    let raw = ratio(
                        bp as u64,
                        a.state_chance_bonus_bp,
                        d.state_resist_bp[kind as usize],
                    );
                    let chance_bp = balance.clamp_chance(raw.min(u32::MAX as u64) as u32);
                    let (success, roll) = chance(&mut src, chance_bp);
                    let applied = if success {
                        apply_kind(&mut states, kind, base_damage, 0, a.slot, balance)
                    } else {
                        Applied::None
                    };
                    StateRoll {
                        kind,
                        rolled: true,
                        chance_bp,
                        roll,
                        success,
                        applied,
                    }
                }
                _ => continue,
            };
            out.resonance_states[slot] = roll_result;
        }
    }

    out.states = states;
    out.rolls_used = src.used;
}

/// Applies a state of `kind` from a packet that dealt `packet_damage` (scaled), with a pure
/// pair bonus `pure_bp` on its strength.
fn apply_kind(
    states: &mut [ActiveState; STATES_MAX],
    kind: StateKind,
    packet_damage: u64,
    pure_bp: u32,
    source: u8,
    balance: &Balance,
) -> Applied {
    match kind {
        StateKind::Chill => {
            let buildup =
                more(balance.chill_buildup_bp as u64, pure_bp).min(u32::MAX as u64) as u32;
            apply_chill(states, buildup, source, balance)
        }
        StateKind::Freeze => freeze(states, source, balance),
        StateKind::Burning | StateKind::Bleeding => {
            // Strength from the hit (section 7.1): a share of the packet damage per turn.
            let per_turn = more(packet_damage, pure_bp) as u128 * balance.dot_share_bp as u128
                / (BP as u128 * BP as u128);
            let power = (per_turn.min(u32::MAX as u128) as u32).max(1);
            apply_state(
                states,
                ActiveState {
                    kind,
                    turns: balance.turns(kind),
                    power,
                    source,
                },
            )
        }
        StateKind::Shock | StateKind::Stun => apply_state(
            states,
            ActiveState {
                kind,
                turns: balance.turns(kind),
                power: 0,
                source,
            },
        ),
    }
}

/// Damage hint without dice (website preview): total min and max after attacker modifiers
/// and target defense, no crit, no resonance, rounded down. Immune packets count as zero.
pub fn preview_range(input: &HitInput, balance: &Balance) -> (u32, u32) {
    damage_range(input, balance, false)
}

/// What an attack would do, before any die is rolled (docs/COMBAT_MATH.md, section 13).
#[cfg_attr(feature = "borsh", derive(BorshSerialize, BorshDeserialize))]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct HitPreview {
    /// The target is immune to every packet: no roll is made.
    pub immune: bool,
    /// Chance to hit, bp: the d20 faces that hit (a natural 1 always misses, a natural 20 always
    /// hits), out of 20.
    pub hit_bp: u32,
    /// Chance to crit, bp: a natural 20, or a hit on another face that wins the crit roll.
    pub crit_bp: u32,
    /// Damage of a hit without and with a crit; no resonance (its tints are rolled).
    pub min: u32,
    pub max: u32,
    pub crit_min: u32,
    pub crit_max: u32,
}

/// Chances and damage of `input` without dice, by the same rules as [`resolve_hit`].
pub fn preview_hit(input: &HitInput, balance: &Balance) -> HitPreview {
    let a = &input.attacker;
    let d = &input.defender;
    let packets = build_packets(a.weapon, &a.skill, &a.added);
    if packets
        .as_slice()
        .iter()
        .all(|p| d.immune_aspects & aspect_bit(p.aspect) != 0)
    {
        return HitPreview {
            immune: true,
            ..HitPreview::default()
        };
    }
    // Faces 2..=19 hit on the threshold; 20 always hits.
    let plain_hits = (2u32..=19)
        .filter(|face| face.saturating_add(a.attack_bonus) >= d.defense)
        .count() as u32;
    let crit_roll = if a.crit_chance_bp > 0 {
        balance.clamp_chance(a.crit_chance_bp)
    } else {
        0
    };
    let (min, max) = damage_range(input, balance, false);
    let (crit_min, crit_max) = damage_range(input, balance, true);
    HitPreview {
        immune: false,
        hit_bp: (plain_hits + 1) * BP / 20,
        crit_bp: (BP + plain_hits * crit_roll) / 20,
        min,
        max,
        crit_min,
        crit_max,
    }
}

/// Total min and max of a hit after attacker modifiers (with the crit multiplier when `crit`)
/// and target defense, rounded down once.
fn damage_range(input: &HitInput, balance: &Balance, crit: bool) -> (u32, u32) {
    let a = &input.attacker;
    let d = &input.defender;
    let packets = build_packets(a.weapon, &a.skill, &a.added);
    let (mut lo, mut hi) = (0u128, 0u128);
    for p in packets.as_slice() {
        if d.immune_aspects & aspect_bit(p.aspect) != 0 {
            continue;
        }
        let incr = sum_matching(&a.increased, p.aspect, a.skill.source, a.skill.form);
        let mut mult = product_matching(&a.more, p.aspect, a.skill.source, a.skill.form) as u64;
        if crit {
            mult = more(mult, balance.crit_more_bp);
        }
        // As `resolve_hit` stores it (`PacketResult::more_bp`).
        let mult = mult.min(u32::MAX as u64);
        let defend = |v: u64| {
            let v = more(v, incr);
            let v = (v as u128 * mult as u128 / BP as u128).min(u64::MAX as u128) as u64;
            let mut v = counter(v, a.weaken_bp);
            if is_physical(p.aspect) {
                v = armor_cut(v, d.armor, a.penetration_bp, balance.armor_k);
            }
            v = resist_cut(v, d.resist_bp[p.aspect as usize], balance.resist_cap_bp);
            more(v, d.vuln_bp[p.aspect as usize]) as u128
        };
        lo += defend(p.min);
        hi += defend(p.max.max(p.min));
    }
    let to_u32 = |v: u128| (v / BP as u128).min(u32::MAX as u128) as u32;
    (to_u32(lo), to_u32(hi))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::resonance::Carrier;
    use crate::rng::{ScriptedRoll, VrfRoll};
    use crate::states::has;
    use crate::ModFilter;
    use deadroll_types::{Form, Source};

    const B: Balance = Balance::DEFAULT;

    fn basic(aspect: Aspect, min: u32, max: u32, state_chance_bp: u32) -> HitInput {
        let mut input = HitInput::default();
        input.attacker.weapon = DamageRange::new(aspect, min, max);
        input.attacker.skill = Skill::basic(Source::Combat, Form::Strike, state_chance_bp);
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
        let r = resolve_hit(&input, &B, &mut ScriptedRoll::new([14, 50]));
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
        let r = resolve_hit(&input, &B, &mut ScriptedRoll::new([14, 50]));
        assert_eq!((r.outcome, r.damage, r.rolls_used), (Outcome::Miss, 0, 1));
        // Natural 1 misses whatever the bonus.
        input.attacker.attack_bonus = 1_000;
        let r = resolve_hit(&input, &B, &mut ScriptedRoll::new([0]));
        assert_eq!((r.outcome, r.d20), (Outcome::Miss, 1));
    }

    #[test]
    fn natural_20_hits_and_crits() {
        let mut input = basic(Aspect::Piercing, 10, 10, 0);
        input.defender.defense = u32::MAX;
        input.attacker.crit_chance_bp = 5_000;
        let r = resolve_hit(&input, &B, &mut ScriptedRoll::new([19, 0]));
        assert_eq!((r.outcome, r.crit, r.crit_roll), (Outcome::Hit, true, None));
        assert_eq!(r.packets()[0].more_bp, 15_000);
        assert_eq!(r.damage, 15);
        assert_eq!(r.rolls_used, 2);
    }

    #[test]
    fn crit_chance_is_clamped_and_rolled() {
        let mut input = basic(Aspect::Piercing, 10, 10, 0);
        input.attacker.crit_chance_bp = BP;
        let r = resolve_hit(&input, &B, &mut ScriptedRoll::new([14, 9_499, 0]));
        assert!(r.crit);
        assert_eq!(r.crit_roll, Some(9_499));
        let r = resolve_hit(&input, &B, &mut ScriptedRoll::new([14, 9_500, 0]));
        assert!(!r.crit);
        assert_eq!(r.rolls_used, 3);
    }

    #[test]
    fn armor_and_penetration() {
        let mut input = basic(Aspect::Piercing, 6, 6, 0);
        input.defender.armor = 100;
        let r = resolve_hit(&input, &B, &mut ScriptedRoll::new([14, 0]));
        assert_eq!((r.packets()[0].armor, r.damage), (100, 3));
        input.attacker.penetration_bp = BP;
        let r = resolve_hit(&input, &B, &mut ScriptedRoll::new([14, 0]));
        assert_eq!((r.packets()[0].armor, r.damage), (50, 4));
    }

    #[test]
    fn resistance_vulnerability_elemental() {
        let mut input = basic(Aspect::Fire, 10, 10, 0);
        // Armor never cuts elemental packets.
        input.defender.armor = 1_000;
        input.defender.resist_bp[Aspect::Fire as usize] = BP;
        let r = resolve_hit(&input, &B, &mut ScriptedRoll::new([14, 0]));
        assert_eq!((r.packets()[0].armor, r.damage), (0, 5));
        input.defender.vuln_bp[Aspect::Fire as usize] = BP;
        assert_eq!(
            resolve_hit(&input, &B, &mut ScriptedRoll::new([14, 0])).damage,
            10
        );
        // Resistance is capped: 75% at most.
        input.defender.vuln_bp = [0; 6];
        input.defender.resist_bp[Aspect::Fire as usize] = u32::MAX;
        let r = resolve_hit(&input, &B, &mut ScriptedRoll::new([14, 0]));
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
        let r = resolve_hit(&input, &B, &mut ScriptedRoll::new([14, 0]));
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
            resolve_hit(&input, &B, &mut ScriptedRoll::new([14, 0])).damage,
            1
        );
    }

    #[test]
    fn full_immunity_rolls_nothing() {
        let mut input = basic(Aspect::Fire, 10, 10, 5_000);
        input.defender.immune_aspects = aspect_bit(Aspect::Fire);
        let r = resolve_hit(&input, &B, &mut ScriptedRoll::new([14, 0, 0]));
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
        let r = resolve_hit(&input, &B, &mut ScriptedRoll::new([14, 0]));
        assert_eq!(r.packet_count, 2);
        assert!(r.packets()[1].immune);
        assert_eq!(r.packets()[1].after_defense, 0);
        assert_eq!(r.damage, 10);
    }

    #[test]
    fn native_state_burning_power_from_damage() {
        let input = basic(Aspect::Fire, 10, 10, 5_000);
        let r = resolve_hit(&input, &B, &mut ScriptedRoll::new([14, 0, 0]));
        let s = r.packets()[0].state;
        assert_eq!(
            (s.kind, s.rolled, s.chance_bp, s.success),
            (StateKind::Burning, true, 5_000, true)
        );
        assert_eq!(s.applied, Applied::Added);
        let burning = find(&r.states, StateKind::Burning);
        assert_eq!((burning.power, burning.turns), (3, 3));
        assert_eq!(r.rolls_used, 3);
    }

    #[test]
    fn native_state_failure_resistance_and_immunity() {
        let mut input = basic(Aspect::Blood, 10, 10, 5_000);
        input.defender.state_resist_bp[StateKind::Bleeding as usize] = BP;
        // 5000 / 2 = 2500; roll 2500 fails.
        let r = resolve_hit(&input, &B, &mut ScriptedRoll::new([14, 0, 2_500]));
        let s = r.packets()[0].state;
        assert_eq!(
            (s.chance_bp, s.success, s.applied),
            (2_500, false, Applied::None)
        );
        assert!(!has(&r.states, StateKind::Bleeding));
        input.defender.immune_states = state_bit(StateKind::Bleeding);
        let r = resolve_hit(&input, &B, &mut ScriptedRoll::new([14, 0, 0]));
        assert!(!r.packets()[0].state.rolled);
        assert_eq!(r.rolls_used, 2);
    }

    #[test]
    fn piercing_frost_crit_freezes() {
        let input = cold_bolt(Aspect::Piercing);
        // nat 20, tint roll 0 (success), d100 0.
        let r = resolve_hit(&input, &B, &mut ScriptedRoll::new([19, 0, 0]));
        let res = r.resonances[0].unwrap();
        assert_eq!(
            (res.base, res.tint, res.active, res.id),
            (Aspect::Cold, Aspect::Piercing, true, 11)
        );
        // x1.5 crit, x1.36 resonance
        assert_eq!(r.packets()[0].more_bp, 20_400);
        assert_eq!(r.damage, 20);
        let s = r.resonance_states[0];
        assert_eq!(
            (s.kind, s.rolled, s.success, s.applied),
            (StateKind::Freeze, false, true, Applied::Froze)
        );
        assert!(has(&r.states, StateKind::Freeze));
        assert_eq!(r.rolls_used, 3);
    }

    #[test]
    fn piercing_frost_without_crit_only_multiplies() {
        let input = cold_bolt(Aspect::Piercing);
        let r = resolve_hit(&input, &B, &mut ScriptedRoll::new([14, 0, 0]));
        assert_eq!(r.packets()[0].more_bp, 13_600);
        assert_eq!(r.damage, 13);
        assert_eq!(r.resonance_states[0], StateRoll::default());
        assert!(!has(&r.states, StateKind::Freeze));
    }

    #[test]
    fn icy_point_chills_through_armor() {
        let mut input = basic(Aspect::Piercing, 10, 10, 0);
        input.attacker.tints[0] = Tint::new(Aspect::Cold, 1, Carrier::Class);
        input.defender.armor = 100;
        // d20 15, tint 0, d100 0, extra state roll 0.
        let r = resolve_hit(&input, &B, &mut ScriptedRoll::new([14, 0, 0, 0]));
        assert!(r.resonances[0].unwrap().active);
        // 10 x1.36 = 13.6, armor halves: 6.8
        assert_eq!(r.damage, 6);
        let s = r.resonance_states[0];
        assert_eq!(
            (s.kind, s.rolled, s.chance_bp, s.applied),
            (StateKind::Chill, true, 3_000, Applied::Added)
        );
        assert_eq!(r.rolls_used, 4);
        // No armor: condition fails, no multiplier, no extra roll.
        input.defender.armor = 0;
        let r = resolve_hit(&input, &B, &mut ScriptedRoll::new([14, 0, 0, 0]));
        assert!(!r.resonances[0].unwrap().active);
        assert_eq!((r.damage, r.rolls_used), (10, 3));
    }

    #[test]
    fn pure_pair_boosts_native_state() {
        let mut input = cold_bolt(Aspect::Cold);
        input.attacker.skill.state_chance_bp = 2_000;
        let r = resolve_hit(&input, &B, &mut ScriptedRoll::new([14, 0, 0]));
        assert_eq!(r.tint_roll_count, 0);
        // 2000 x1.59 = 3180; buildup 5000 x1.59 = 7950
        let s = r.packets()[0].state;
        assert_eq!((s.chance_bp, s.success), (3_180, true));
        assert_eq!(find(&r.states, StateKind::Chill).power, 7_950);
    }

    #[test]
    fn deterministic_per_seed() {
        let mut input = cold_bolt(Aspect::Piercing);
        input.attacker.skill.state_chance_bp = 4_000;
        input.attacker.crit_chance_bp = 1_000;
        let a = resolve_hit(&input, &B, &mut VrfRoll::new([3; 32]));
        let b = resolve_hit(&input, &B, &mut VrfRoll::new([3; 32]));
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
        let p = preview_hit(&input, &B);
        assert_eq!((p.immune, p.hit_bp, p.crit_bp), (false, 5_500, 500));
        assert_eq!((p.min, p.max, p.crit_min, p.crit_max), (4, 8, 6, 12));
        // 10 plain hits x 10% + the natural 20.
        input.attacker.crit_chance_bp = 1_000;
        assert_eq!(preview_hit(&input, &B).crit_bp, 1_000);
        // The crit roll is clamped like in the hit.
        input.attacker.crit_chance_bp = BP;
        assert_eq!(preview_hit(&input, &B).crit_bp, (BP + 10 * 9_500) / 20);
        // A natural 1 always misses, a natural 20 always hits.
        input.attacker.attack_bonus = 1_000;
        assert_eq!(preview_hit(&input, &B).hit_bp, 9_500);
        input.attacker.attack_bonus = 0;
        input.defender.defense = u32::MAX;
        assert_eq!(preview_hit(&input, &B).hit_bp, 500);
        input.defender.immune_aspects = aspect_bit(Aspect::Piercing);
        assert_eq!(
            preview_hit(&input, &B),
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
                    let r = resolve_hit(&input, &B, &mut ScriptedRoll::new([*face, 0]));
                    r.outcome == Outcome::Hit
                })
                .count() as u32;
            assert_eq!(
                preview_hit(&input, &B).hit_bp,
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
        let p = preview_hit(&input, &B);
        // Tint roll fails (9_999), d100 0 or 100 (the range ends); crit on a natural 20.
        let at = |face: u64, d100: u64| {
            resolve_hit(&input, &B, &mut ScriptedRoll::new([face, 9_999, d100])).damage
        };
        assert_eq!((p.min, p.max), (at(14, 0), at(14, 100)));
        let nat20 =
            |d100: u64| resolve_hit(&input, &B, &mut ScriptedRoll::new([19, 9_999, d100])).damage;
        assert_eq!((p.crit_min, p.crit_max), (nat20(0), nat20(100)));
    }

    #[test]
    fn state_chance_bonus_reaches_resonance_states() {
        let mut input = basic(Aspect::Piercing, 10, 10, 0);
        input.attacker.tints[0] = Tint::new(Aspect::Cold, 1, Carrier::Class);
        input.attacker.state_chance_bonus_bp = BP;
        input.defender.armor = 100;
        let r = resolve_hit(&input, &B, &mut ScriptedRoll::new([14, 0, 0, 0]));
        // Icy Point: 3000 x2.
        assert_eq!(r.resonance_states[0].chance_bp, 6_000);
    }
}
