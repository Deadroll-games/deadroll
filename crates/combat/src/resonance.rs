//! Tint rolls and resonance selection (docs/DAMAGE_TYPES_AND_MIXES.md, section 6).
//!
//! Every skill aspect is a base. For each base the target is not immune to, every tint
//! (another aspect of the character: race, class, item) rolls its own chance from its tier.
//! One success makes a pair resonance, two or more a triple from the two best rolls; there is
//! no cap on resonances per use. A tint equal to the base makes no roll: it is a pure pair and
//! boosts the native state.
//!
//! What a resonance does is data ([`crate::tables::ResonanceTables`]): this module only picks
//! the tints and checks the target condition.

use deadroll_types::{ActiveState, Aspect, StateKind, Tier};

use crate::balance::Balance;
use crate::packet::SKILL_ASPECTS_MAX;
use crate::rng::{chance, RollSource};
use crate::states::state_bit;
use crate::tables::ResonanceTables;

#[cfg(feature = "borsh")]
use borsh::{BorshDeserialize, BorshSerialize};

/// Tint slots of one attacker: race, class, then items.
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

/// What the target must satisfy for the resonance to trigger. Discriminants are the registry
/// format: append only.
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

    /// The target had `kind` before the attack.
    pub fn has(&self, kind: StateKind) -> bool {
        self.states.iter().any(|s| s.turns > 0 && s.kind == kind)
    }
}

pub fn condition_met(condition: ResonanceCondition, target: &TargetView) -> bool {
    use ResonanceCondition as C;
    match condition {
        C::Always => true,
        C::CanBurn => target.can(StateKind::Burning),
        C::CanBeChilled => target.can(StateKind::Chill),
        C::CanBeFrozen => target.can(StateKind::Freeze),
        C::IsFrozen => target.has(StateKind::Freeze),
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

/// A triggered (or condition-cancelled) resonance of one base.
#[cfg_attr(feature = "borsh", derive(BorshSerialize, BorshDeserialize))]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Resonance {
    pub base: Aspect,
    /// The best tint: it picks the pair.
    pub tint: Aspect,
    pub tier: u8,
    pub carrier: Carrier,
    /// `resonance_id(base, tint)`.
    pub id: u8,
    /// False: the target condition cancelled it (no effects).
    pub active: bool,
    /// The second successful tint: a triple (section 6.5), its trait joins the pair.
    pub second_tint: Option<Aspect>,
    pub second_tier: u8,
    pub second_carrier: Carrier,
    /// The pair has no record: its mechanics come from the tint's trait.
    pub generated: bool,
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

/// Rolls the tints for every base (step 6), picks the pair or triple (step 7) and checks the
/// target condition (step 8). Bases the target is immune to (`immune_aspects` bit set) and
/// repeated bases roll nothing. Roll order: bases in order, tints in slot order.
pub fn roll_tints(
    bases: &[Aspect],
    tints: &[Tint],
    immune_aspects: u8,
    target: &TargetView,
    balance: &Balance,
    tables: &ResonanceTables,
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
            let mechanics = tables.pair(base, t.aspect);
            let second = second.map(|(_, _, k)| tints[k]);
            out.resonances[i] = Some(Resonance {
                base,
                tint: t.aspect,
                tier: t.tier,
                carrier: t.carrier,
                id: resonance_id(base, t.aspect),
                active: condition_met(mechanics.condition, target),
                second_tint: second.map(|s| s.aspect),
                second_tier: second.map_or(0, |s| s.tier),
                second_carrier: second.map_or(Carrier::Race, |s| s.carrier),
                generated: mechanics.generated,
            });
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rng::ScriptedRoll;
    use crate::tables::{PairEntry, ResonanceTables};

    const B: Balance = Balance::DEFAULT;

    fn view(states: &[ActiveState]) -> TargetView<'_> {
        TargetView {
            immune_states: 0,
            armor: 10,
            states,
        }
    }

    /// Every pair overridden with `condition`.
    fn tables(condition: ResonanceCondition) -> ResonanceTables {
        let mut t = ResonanceTables::default();
        for row in t.pairs.iter_mut() {
            for p in row.iter_mut() {
                *p = PairEntry {
                    condition,
                    set: true,
                    ..PairEntry::default()
                };
            }
        }
        t
    }

    #[test]
    fn ids() {
        assert_eq!(resonance_id(Aspect::Cold, Aspect::Piercing), 11);
        assert_eq!(resonance_id(Aspect::Piercing, Aspect::Cold), 31);
    }

    #[test]
    fn conditions() {
        use ResonanceCondition as C;
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
        let t = tables(ResonanceCondition::Always);
        let out = roll_tints(&[Aspect::Cold], &tints, 0, &view(&[]), &B, &t, &mut src);
        assert_eq!(out.roll_count, 1);
        assert_eq!(out.rolls[0].chance_pct, 40);
        assert!(out.rolls[0].success);
        let r = out.resonances[0].unwrap();
        assert_eq!(
            (r.base, r.tint, r.tier, r.active, r.generated),
            (Aspect::Cold, Aspect::Piercing, 1, true, false)
        );
        assert_eq!(r.second_tint, None);
    }

    #[test]
    fn no_pair_on_failure() {
        let tints = [Tint::new(Aspect::Piercing, 14, Carrier::Race)];
        let mut src = ScriptedRoll::new([700]);
        let t = tables(ResonanceCondition::Always);
        let out = roll_tints(&[Aspect::Cold], &tints, 0, &view(&[]), &B, &t, &mut src);
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
        let t = tables(ResonanceCondition::Always);
        let out = roll_tints(
            &[Aspect::Cold, Aspect::Fire],
            &tints,
            immune,
            &view(&[]),
            &B,
            &t,
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
    fn best_margin_wins_then_carrier_priority_and_the_second_makes_a_triple() {
        let tints = [
            Tint::new(Aspect::Fire, 14, Carrier::Race),     // chance 7
            Tint::new(Aspect::Blood, 1, Carrier::Class),    // chance 40
            Tint::new(Aspect::Lightning, 1, Carrier::Item), // chance 40
            Tint::default(),                                // empty (tier 0)
        ];
        // margins: 7-0=7, 40-10=30, 40-10=30 -> tie, Item beats Class.
        let mut src = ScriptedRoll::new([0, 10, 10]);
        let t = tables(ResonanceCondition::Always);
        let out = roll_tints(&[Aspect::Cold], &tints, 0, &view(&[]), &B, &t, &mut src);
        assert_eq!(out.roll_count, 3);
        let r = out.resonances[0].unwrap();
        assert_eq!((r.tint, r.carrier), (Aspect::Lightning, Carrier::Item));
        assert_eq!(
            (r.second_tint, r.second_tier, r.second_carrier),
            (Some(Aspect::Blood), 1, Carrier::Class)
        );
    }

    #[test]
    fn no_cap_across_bases_and_repeats_skipped() {
        let tints = [Tint::new(Aspect::Piercing, 1, Carrier::Race)];
        let mut src = ScriptedRoll::new([0, 0, 0]);
        let t = tables(ResonanceCondition::Always);
        let out = roll_tints(
            &[Aspect::Cold, Aspect::Fire, Aspect::Cold],
            &tints,
            0,
            &view(&[]),
            &B,
            &t,
            &mut src,
        );
        assert_eq!(out.roll_count, 2);
        assert!(out.resonances[0].is_some() && out.resonances[1].is_some());
        assert_eq!(out.resonances[2], None);
    }

    #[test]
    fn condition_cancels_and_generated_pairs_take_the_base_condition() {
        let tints = [Tint::new(Aspect::Blunt, 1, Carrier::Race)];
        let mut src = ScriptedRoll::new([0]);
        // Cold + Blunt (Cleaving Ice) needs a frozen target.
        let t = tables(ResonanceCondition::IsFrozen);
        let out = roll_tints(&[Aspect::Cold], &tints, 0, &view(&[]), &B, &t, &mut src);
        assert!(!out.resonances[0].unwrap().active);
        // No record: the base's condition (default `Always`) and the trait's template.
        let mut src = ScriptedRoll::new([0]);
        let t = ResonanceTables::default();
        let out = roll_tints(&[Aspect::Cold], &tints, 0, &view(&[]), &B, &t, &mut src);
        let r = out.resonances[0].unwrap();
        assert!(r.active && r.generated);
    }
}
