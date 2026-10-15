//! Pair resonances (docs/DAMAGE_TYPES_AND_MIXES.md, section 6).
//!
//! Every skill aspect is a base. For each base the target is not immune to, every tint
//! (another aspect of the character: race, class, item) rolls its own chance from its tier.
//! The best success makes a pair resonance; there is no cap on resonances per use.
//! A tint equal to the base makes no roll: it is a pure pair and boosts the native state.
//!
//! Mechanics are data: [`RESONANCES`] maps `(base, tint)` to a target condition and a short
//! list of [`ResonanceEffect`]s. It is the in-crate copy of the future registry
//! `ResonanceTable`; numbers are placeholder, tune in sim.

use deadroll_types::{ActiveState, Aspect, StateKind, Tier, BP};

use crate::balance::Balance;
use crate::packet::SKILL_ASPECTS_MAX;
use crate::rng::{chance, RollSource};
use crate::states::state_bit;

#[cfg(feature = "borsh")]
use borsh::{BorshDeserialize, BorshSerialize};

/// Tint slots of one attacker.
pub const TINTS_MAX: usize = 4;
/// Tint rolls of one attack: every base x every tint.
pub const TINT_ROLLS_MAX: usize = SKILL_ASPECTS_MAX * TINTS_MAX;

/// Where a tint comes from. Higher discriminant wins a tie: Item > Class > Race.
#[cfg_attr(feature = "borsh", derive(BorshSerialize, BorshDeserialize))]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord)]
#[repr(u8)]
pub enum Carrier {
    #[default]
    Race,
    Class,
    Item,
}

/// An aspect of the attacker that can tint a resonance. `tier` outside `1..=14` (0) is an
/// empty slot.
#[cfg_attr(feature = "borsh", derive(BorshSerialize, BorshDeserialize))]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Tint {
    pub aspect: Aspect,
    pub tier: u8,
    pub carrier: Carrier,
}

impl Tint {
    pub const fn new(aspect: Aspect, tier: u8, carrier: Carrier) -> Self {
        Self {
            aspect,
            tier,
            carrier,
        }
    }
}

/// What the target must satisfy for the resonance to trigger.
#[cfg_attr(feature = "borsh", derive(BorshSerialize, BorshDeserialize))]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
#[repr(u8)]
pub enum ResonanceCondition {
    #[default]
    Always,
    CanBurn,
    CanBeChilled,
    CanBeFrozen,
    /// Frozen before this attack.
    IsFrozen,
    CanBeShocked,
    CanBleed,
    CanBeStunned,
    HasArmor,
}

/// One mechanic of a resonance.
#[cfg_attr(feature = "borsh", derive(BorshSerialize, BorshDeserialize))]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ResonanceEffect {
    #[default]
    None,
    /// "More" on the base packet: `Balance::resonance_more(tier) * scale_bp / BP`.
    Multiplier(u32),
    /// On a crit the state is applied without a roll (if the target is not immune).
    StateOnCrit(StateKind),
    /// One more state roll. The chance is `Balance::extra_state_chance_pct`.
    ExtraStateChance(StateKind),
}

/// A row of the resonance table.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ResonanceEntry {
    pub condition: ResonanceCondition,
    pub effects: [ResonanceEffect; 2],
}

const fn generic(condition: ResonanceCondition) -> ResonanceEntry {
    ResonanceEntry {
        condition,
        effects: [ResonanceEffect::Multiplier(BP), ResonanceEffect::None],
    }
}

const fn with(condition: ResonanceCondition, extra: ResonanceEffect) -> ResonanceEntry {
    ResonanceEntry {
        condition,
        effects: [ResonanceEffect::Multiplier(BP), extra],
    }
}

use ResonanceCondition as C;

/// Pure pair placeholder on the diagonal: never looked up as a resonance.
const PURE: ResonanceEntry = ResonanceEntry {
    condition: C::Always,
    effects: [ResonanceEffect::None; 2],
};

/// `RESONANCES[base][tint]`, rows and columns in `Aspect` order:
/// Fire, Cold, Lightning, Blood, Blunt, Piercing.
///
/// Every pair gets the generic tier multiplier. Demo pairs carry their state mechanics:
/// - Cold + Piercing (Piercing Frost): a crit can freeze.
/// - Piercing + Cold (Icy Point): Penetration chills.
///
/// TODO(post-demo): the other pairs' mechanics from section 6.4 (jumps, knockback, armor
/// penetration, longer states) as more effect kinds and table entries.
pub const RESONANCES: [[ResonanceEntry; Aspect::COUNT]; Aspect::COUNT] = [
    // Base Fire: target can burn.
    [
        PURE,
        generic(C::CanBurn),
        generic(C::CanBurn),
        generic(C::CanBurn),
        generic(C::CanBurn),
        generic(C::CanBurn),
    ],
    // Base Cold.
    [
        generic(C::CanBeFrozen),
        PURE,
        generic(C::CanBeFrozen),
        generic(C::CanBeChilled),
        generic(C::IsFrozen),
        with(
            C::CanBeFrozen,
            ResonanceEffect::StateOnCrit(StateKind::Freeze),
        ),
    ],
    // Base Lightning: target can be shocked.
    [
        generic(C::CanBeShocked),
        generic(C::CanBeShocked),
        PURE,
        generic(C::CanBeShocked),
        generic(C::CanBeShocked),
        generic(C::CanBeShocked),
    ],
    // Base Blood: target can bleed.
    [
        generic(C::CanBleed),
        generic(C::CanBleed),
        generic(C::CanBleed),
        PURE,
        generic(C::CanBleed),
        generic(C::CanBleed),
    ],
    // Base Blunt: target can be stunned.
    [
        generic(C::CanBeStunned),
        generic(C::CanBeStunned),
        generic(C::CanBeStunned),
        generic(C::CanBeStunned),
        PURE,
        generic(C::CanBeStunned),
    ],
    // Base Piercing.
    [
        generic(C::HasArmor),
        with(
            C::HasArmor,
            ResonanceEffect::ExtraStateChance(StateKind::Chill),
        ),
        generic(C::HasArmor),
        generic(C::CanBleed),
        generic(C::HasArmor),
        PURE,
    ],
];

pub const fn entry(base: Aspect, tint: Aspect) -> &'static ResonanceEntry {
    &RESONANCES[base as usize][tint as usize]
}

/// Id of a pair for names and lore off-chain: `base * 6 + tint`.
pub const fn resonance_id(base: Aspect, tint: Aspect) -> u8 {
    base as u8 * Aspect::COUNT as u8 + tint as u8
}

/// The parts of the target a condition looks at.
#[derive(Clone, Copy, Debug)]
pub struct TargetView<'a> {
    pub immune_states: u8,
    pub armor: u32,
    /// States before the attack.
    pub states: &'a [ActiveState],
}

impl TargetView<'_> {
    fn can(&self, kind: StateKind) -> bool {
        self.immune_states & state_bit(kind) == 0
    }
}

pub fn condition_met(condition: ResonanceCondition, target: &TargetView) -> bool {
    match condition {
        C::Always => true,
        C::CanBurn => target.can(StateKind::Burning),
        C::CanBeChilled => target.can(StateKind::Chill),
        C::CanBeFrozen => target.can(StateKind::Freeze),
        C::IsFrozen => target
            .states
            .iter()
            .any(|s| s.turns > 0 && s.kind == StateKind::Freeze),
        C::CanBeShocked => target.can(StateKind::Shock),
        C::CanBleed => target.can(StateKind::Bleeding),
        C::CanBeStunned => target.can(StateKind::Stun),
        C::HasArmor => target.armor > 0,
    }
}

/// One tint die in the breakdown.
#[cfg_attr(feature = "borsh", derive(BorshSerialize, BorshDeserialize))]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct TintRoll {
    pub base: Aspect,
    pub tint: Aspect,
    pub carrier: Carrier,
    pub tier: u8,
    /// Percent. Success when `roll < chance_pct` (`roll` is a d100, `0..=100`).
    pub chance_pct: u32,
    pub roll: u16,
    pub success: bool,
}

/// A triggered (or condition-cancelled) pair resonance of one base.
#[cfg_attr(feature = "borsh", derive(BorshSerialize, BorshDeserialize))]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Resonance {
    pub base: Aspect,
    pub tint: Aspect,
    pub tier: u8,
    pub carrier: Carrier,
    /// `resonance_id(base, tint)`.
    pub id: u8,
    /// False: the target condition cancelled it (no effects).
    pub active: bool,
    /// A second successful tint, recorded for triple resonances (P2), no effect now.
    pub second_tint: Option<Aspect>,
}

/// Tint dice of one attack.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct TintOutcome {
    pub rolls: [TintRoll; TINT_ROLLS_MAX],
    pub roll_count: u8,
    /// Per base slot (same index as the bases passed in).
    pub resonances: [Option<Resonance>; SKILL_ASPECTS_MAX],
    /// Pure pair "increase" on the base's native state chance and power, per base slot.
    pub pure_pair_bp: [u32; SKILL_ASPECTS_MAX],
}

/// Rolls the tints for every base (step 6), picks the pair (step 7) and checks the target
/// condition (step 8). Bases the target is immune to (`immune_aspects` bit set) and repeated
/// bases roll nothing. Roll order: bases in order, tints in slot order.
pub fn roll_tints(
    bases: &[Aspect],
    tints: &[Tint],
    immune_aspects: u8,
    target: &TargetView,
    balance: &Balance,
    src: &mut impl RollSource,
) -> TintOutcome {
    let mut out = TintOutcome::default();
    for (i, &base) in bases.iter().enumerate().take(SKILL_ASPECTS_MAX) {
        if immune_aspects & (1 << base as u8) != 0 || bases[..i].contains(&base) {
            continue;
        }
        // (margin, carrier, slot) of the best and second-best success.
        let mut best: Option<(u32, Carrier, usize)> = None;
        let mut second: Option<(u32, Carrier, usize)> = None;
        for (j, t) in tints.iter().enumerate().take(TINTS_MAX) {
            let Ok(tier) = Tier::new(t.tier) else {
                continue;
            };
            if t.aspect == base {
                out.pure_pair_bp[i] = out.pure_pair_bp[i].max(balance.pure_pair(tier));
                continue;
            }
            let chance_pct = balance.clamp_chance(balance.tint_chance(tier));
            let (success, roll) = chance(src, chance_pct);
            if (out.roll_count as usize) < TINT_ROLLS_MAX {
                out.rolls[out.roll_count as usize] = TintRoll {
                    base,
                    tint: t.aspect,
                    carrier: t.carrier,
                    tier: t.tier,
                    chance_pct,
                    roll,
                    success,
                };
                out.roll_count += 1;
            }
            if !success {
                continue;
            }
            let candidate = (chance_pct - roll as u32, t.carrier, j);
            let better =
                |a: &(u32, Carrier, usize), b: &(u32, Carrier, usize)| (a.0, a.1) > (b.0, b.1);
            match best {
                Some(b) if !better(&candidate, &b) => {
                    if second.is_none_or(|s| better(&candidate, &s)) {
                        second = Some(candidate);
                    }
                }
                _ => {
                    second = best;
                    best = Some(candidate);
                }
            }
        }
        if let Some((_, _, j)) = best {
            let t = tints[j];
            out.resonances[i] = Some(Resonance {
                base,
                tint: t.aspect,
                tier: t.tier,
                carrier: t.carrier,
                id: resonance_id(base, t.aspect),
                active: condition_met(entry(base, t.aspect).condition, target),
                second_tint: second.map(|(_, _, k)| tints[k].aspect),
            });
        }
    }
    out
}

/// "More" bonus of an active resonance on its base packet, bp.
pub fn multiplier_bp(res: &Resonance, balance: &Balance) -> u32 {
    let Ok(tier) = Tier::new(res.tier) else {
        return 0;
    };
    entry(res.base, res.tint)
        .effects
        .iter()
        .filter_map(|e| match e {
            ResonanceEffect::Multiplier(scale) => {
                Some((balance.resonance_more(tier) as u64 * *scale as u64 / BP as u64) as u32)
            }
            _ => None,
        })
        .fold(0u32, u32::saturating_add)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rng::ScriptedRoll;

    const B: Balance = Balance::DEFAULT;

    fn view(states: &[ActiveState]) -> TargetView<'_> {
        TargetView {
            immune_states: 0,
            armor: 10,
            states,
        }
    }

    #[test]
    fn table_shape() {
        for a in Aspect::ALL {
            assert_eq!(*entry(*a, *a), PURE);
            for t in Aspect::ALL.iter().filter(|t| *t != a) {
                assert_eq!(entry(*a, *t).effects[0], ResonanceEffect::Multiplier(BP));
            }
        }
        assert_eq!(
            entry(Aspect::Cold, Aspect::Piercing).effects[1],
            ResonanceEffect::StateOnCrit(StateKind::Freeze)
        );
        assert_eq!(
            entry(Aspect::Piercing, Aspect::Cold).effects[1],
            ResonanceEffect::ExtraStateChance(StateKind::Chill)
        );
        assert_eq!(resonance_id(Aspect::Cold, Aspect::Piercing), 11);
        assert_eq!(resonance_id(Aspect::Piercing, Aspect::Cold), 31);
    }

    #[test]
    fn conditions() {
        let frozen = [ActiveState {
            kind: StateKind::Freeze,
            turns: 1,
            power: 0,
            source: 0,
        }];
        let mut v = view(&[]);
        assert!(condition_met(C::Always, &v));
        assert!(condition_met(C::CanBurn, &v));
        assert!(condition_met(C::HasArmor, &v));
        assert!(!condition_met(C::IsFrozen, &v));
        v.states = &frozen;
        assert!(condition_met(C::IsFrozen, &v));
        v.armor = 0;
        assert!(!condition_met(C::HasArmor, &v));
        v.immune_states = state_bit(StateKind::Freeze) | state_bit(StateKind::Bleeding);
        assert!(!condition_met(C::CanBeFrozen, &v));
        assert!(!condition_met(C::CanBleed, &v));
        assert!(condition_met(C::CanBeChilled, &v));
        v.immune_states = 0xFF;
        for c in [
            C::CanBurn,
            C::CanBeChilled,
            C::CanBeShocked,
            C::CanBeStunned,
        ] {
            assert!(!condition_met(c, &v));
        }
    }

    #[test]
    fn pair_on_success() {
        let tints = [Tint::new(Aspect::Piercing, 1, Carrier::Race)];
        // chance tier 1 = 40; roll 0 succeeds.
        let mut src = ScriptedRoll::new([0]);
        let out = roll_tints(&[Aspect::Cold], &tints, 0, &view(&[]), &B, &mut src);
        assert_eq!(out.roll_count, 1);
        assert_eq!(out.rolls[0].chance_pct, 40);
        assert!(out.rolls[0].success);
        let r = out.resonances[0].unwrap();
        assert_eq!(
            (r.base, r.tint, r.tier, r.active),
            (Aspect::Cold, Aspect::Piercing, 1, true)
        );
        assert_eq!(r.second_tint, None);
        assert_eq!(multiplier_bp(&r, &B), 3_600);
    }

    #[test]
    fn no_pair_on_failure() {
        let tints = [Tint::new(Aspect::Piercing, 14, Carrier::Race)];
        let mut src = ScriptedRoll::new([700]);
        let out = roll_tints(&[Aspect::Cold], &tints, 0, &view(&[]), &B, &mut src);
        assert_eq!(out.roll_count, 1);
        assert!(!out.rolls[0].success);
        assert_eq!(out.resonances[0], None);
    }

    #[test]
    fn immune_base_rolls_nothing_and_pure_pair_does_not_roll() {
        let tints = [
            Tint::new(Aspect::Cold, 3, Carrier::Race),
            Tint::new(Aspect::Fire, 3, Carrier::Item),
        ];
        let mut src = ScriptedRoll::new([0, 0, 0]);
        let immune = 1 << Aspect::Fire as u8;
        let out = roll_tints(
            &[Aspect::Cold, Aspect::Fire],
            &tints,
            immune,
            &view(&[]),
            &B,
            &mut src,
        );
        // Only base Cold rolls, and only the Fire tint (Cold is a pure pair).
        assert_eq!(out.roll_count, 1);
        assert_eq!(src.index, 1);
        assert_eq!(out.pure_pair_bp, [B.pure_pair_bp[2], 0, 0]);
        assert_eq!(out.resonances[0].unwrap().tint, Aspect::Fire);
        assert_eq!(out.resonances[1], None);
    }

    #[test]
    fn best_margin_wins_then_carrier_priority() {
        let tints = [
            Tint::new(Aspect::Fire, 14, Carrier::Race),     // chance 7
            Tint::new(Aspect::Blood, 1, Carrier::Class),    // chance 40
            Tint::new(Aspect::Lightning, 1, Carrier::Item), // chance 40
            Tint::default(),                                // empty (tier 0)
        ];
        // margins: 7-0=7, 40-10=30, 40-10=30 -> tie, Item beats Class.
        let mut src = ScriptedRoll::new([0, 10, 10]);
        let out = roll_tints(&[Aspect::Cold], &tints, 0, &view(&[]), &B, &mut src);
        assert_eq!(out.roll_count, 3);
        let r = out.resonances[0].unwrap();
        assert_eq!(r.tint, Aspect::Lightning);
        assert_eq!(r.second_tint, Some(Aspect::Blood));
    }

    #[test]
    fn no_cap_across_bases_and_repeats_skipped() {
        let tints = [Tint::new(Aspect::Piercing, 1, Carrier::Race)];
        let mut src = ScriptedRoll::new([0, 0, 0]);
        let out = roll_tints(
            &[Aspect::Cold, Aspect::Fire, Aspect::Cold],
            &tints,
            0,
            &view(&[]),
            &B,
            &mut src,
        );
        assert_eq!(out.roll_count, 2);
        assert!(out.resonances[0].is_some() && out.resonances[1].is_some());
        assert_eq!(out.resonances[2], None);
    }

    #[test]
    fn condition_cancels() {
        let tints = [Tint::new(Aspect::Blunt, 1, Carrier::Race)];
        let mut src = ScriptedRoll::new([0]);
        // Cold + Blunt (Cleaving Ice) needs a frozen target.
        let out = roll_tints(&[Aspect::Cold], &tints, 0, &view(&[]), &B, &mut src);
        let r = out.resonances[0].unwrap();
        assert!(!r.active);
    }
}
