//! Modifier groups (docs/COMBAT_MATH.md, sections 3-4) and target defense (section 6.5).
//!
//! Bonuses multiply, counterweights divide, nothing ever subtracts. All functions work on
//! values scaled by `BP` (see [`crate::packet`]), compute in `u128`, multiply first, divide
//! last, and saturate at `u64::MAX`.

use deadroll_types::{Aspect, Family, Form, Source, BP};

#[cfg(feature = "borsh")]
use borsh::{BorshDeserialize, BorshSerialize};

const BP128: u128 = BP as u128;

fn sat(value: u128) -> u64 {
    u64::try_from(value).unwrap_or(u64::MAX)
}

/// Family of a starting aspect (docs/DAMAGE_TYPES_AND_MIXES.md, section 3.2).
pub const fn family_of(aspect: Aspect) -> Family {
    match aspect {
        Aspect::Fire | Aspect::Cold | Aspect::Lightning => Family::Elemental,
        Aspect::Blood | Aspect::Blunt | Aspect::Piercing => Family::Physical,
    }
}

/// Physical packets are cut by armor, the rest by resistance.
pub const fn is_physical(aspect: Aspect) -> bool {
    matches!(family_of(aspect), Family::Physical)
}

/// `value * (BP + bonus) / (BP + counter)`: the opposing-pairs rule.
/// The divisor is at least `BP`, so a counterweight never divides by zero.
pub fn ratio(value: u64, bonus_bp: u32, counter_bp: u32) -> u64 {
    sat(value as u128 * (BP128 + bonus_bp as u128) / (BP128 + counter_bp as u128))
}

/// `value * (BP + bonus) / BP`: one multiplier or a summed increase.
pub fn more(value: u64, bonus_bp: u32) -> u64 {
    ratio(value, bonus_bp, 0)
}

/// `value * BP / (BP + counter)`: a summed counterweight.
pub fn counter(value: u64, counter_bp: u32) -> u64 {
    ratio(value, 0, counter_bp)
}

/// Armor with diminishing returns, never 100%.
/// Penetration is the counterweight of armor: `armor_eff = armor * BP / (BP + pen)`.
/// Result: `value * K / (armor_eff + K)`, computed without intermediate division.
pub fn armor_cut(value: u64, armor: u32, penetration_bp: u32, armor_k: u32) -> u64 {
    let k = armor_k.max(1) as u128;
    let pen = BP128 + penetration_bp as u128;
    // value * K / (armor * BP / pen + K) == value * K * pen / (armor * BP + K * pen)
    sat(value as u128 * k * pen / (armor as u128 * BP128 + k * pen))
}

/// Effective armor after penetration, rounded down (for the breakdown only).
pub fn effective_armor(armor: u32, penetration_bp: u32) -> u32 {
    (armor as u128 * BP128 / (BP128 + penetration_bp as u128)) as u32
}

/// Resistance as a counterweight, capped so it never becomes immunity.
pub fn resist_cut(value: u64, resist_bp: u32, cap_bp: u32) -> u64 {
    counter(value, resist_bp.min(cap_bp))
}

/// Which hits a modifier applies to.
#[cfg_attr(feature = "borsh", derive(BorshSerialize, BorshDeserialize))]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ModFilter {
    /// Every packet.
    #[default]
    Any,
    Aspect(Aspect),
    Family(Family),
    /// Every packet of a skill with this power source.
    Source(Source),
    /// Every packet of a skill with this form.
    Form(Form),
    /// Every packet of a skill with both this power source and this form (a stat bonus such as
    /// Strength on martial strikes, docs/CHARACTER.md section 2).
    SourceForm(Source, Form),
}

impl ModFilter {
    pub fn matches(self, aspect: Aspect, source: Source, form: Form) -> bool {
        match self {
            ModFilter::Any => true,
            ModFilter::Aspect(a) => a == aspect,
            ModFilter::Family(f) => f == family_of(aspect),
            ModFilter::Source(s) => s == source,
            ModFilter::Form(f) => f == form,
            ModFilter::SourceForm(s, f) => s == source && f == form,
        }
    }
}

/// One modifier: `bp == 0` is an empty slot.
#[cfg_attr(feature = "borsh", derive(BorshSerialize, BorshDeserialize))]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Mod {
    pub filter: ModFilter,
    pub bp: u32,
}

impl Mod {
    pub const fn new(filter: ModFilter, bp: u32) -> Self {
        Self { filter, bp }
    }
}

/// Increase group: matching bonuses are summed into one total.
pub fn sum_matching(mods: &[Mod], aspect: Aspect, source: Source, form: Form) -> u32 {
    mods.iter()
        .filter(|m| m.bp > 0 && m.filter.matches(aspect, source, form))
        .fold(0u32, |acc, m| acc.saturating_add(m.bp))
}

/// Multiplier group: each matching bonus multiplies separately.
/// Returns the combined multiplier in bp (`BP` = x1), saturating at `u32::MAX`.
pub fn product_matching(mods: &[Mod], aspect: Aspect, source: Source, form: Form) -> u32 {
    mods.iter()
        .filter(|m| m.bp > 0 && m.filter.matches(aspect, source, form))
        .fold(BP, |acc, m| {
            let next = acc as u128 * (BP128 + m.bp as u128) / BP128;
            u32::try_from(next).unwrap_or(u32::MAX)
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn families() {
        assert_eq!(family_of(Aspect::Fire), Family::Elemental);
        assert_eq!(family_of(Aspect::Cold), Family::Elemental);
        assert_eq!(family_of(Aspect::Lightning), Family::Elemental);
        assert!(is_physical(Aspect::Blood));
        assert!(is_physical(Aspect::Blunt));
        assert!(is_physical(Aspect::Piercing));
    }

    #[test]
    fn opposing_pairs() {
        // Equal haste and slow cancel out.
        assert_eq!(ratio(1_000, 5_000, 5_000), 1_000);
        // 100% slow halves, 300% slow quarters, never zero.
        assert_eq!(counter(1_000, 10_000), 500);
        assert_eq!(counter(1_000, 30_000), 250);
        assert!(counter(1_000_000, u32::MAX) > 0);
        assert_eq!(more(1_000, 5_000), 1_500);
    }

    #[test]
    fn saturates() {
        assert_eq!(more(u64::MAX, u32::MAX), u64::MAX);
        assert_eq!(ratio(u64::MAX, 0, 0), u64::MAX);
    }

    #[test]
    fn armor_diminishing_returns() {
        let k = 100;
        assert_eq!(armor_cut(10_000, 0, 0, k), 10_000);
        assert_eq!(armor_cut(10_000, 100, 0, k), 5_000);
        assert_eq!(armor_cut(10_000, 300, 0, k), 2_500);
        // Never 100%.
        assert!(armor_cut(10_000, u32::MAX, 0, k) < 10_000);
        assert!(armor_cut(10_000_000_000, u32::MAX, 0, k) > 0);
        // 100% penetration halves effective armor: 50 armor -> x100/150.
        assert_eq!(armor_cut(15_000, 100, 10_000, k), 10_000);
        assert_eq!(effective_armor(100, 10_000), 50);
        // Zero K is treated as 1, no division by zero.
        assert_eq!(armor_cut(10_000, 0, 0, 0), 10_000);
    }

    #[test]
    fn resistance_capped() {
        assert_eq!(resist_cut(10_000, 10_000, 30_000), 5_000);
        assert_eq!(resist_cut(10_000, 90_000, 30_000), 2_500);
        assert_eq!(resist_cut(10_000, 0, 30_000), 10_000);
    }

    #[test]
    fn filters() {
        let (a, s, f) = (Aspect::Cold, Source::Magical, Form::Projectile);
        assert!(ModFilter::Any.matches(a, s, f));
        assert!(ModFilter::Aspect(Aspect::Cold).matches(a, s, f));
        assert!(!ModFilter::Aspect(Aspect::Fire).matches(a, s, f));
        assert!(ModFilter::Family(Family::Elemental).matches(a, s, f));
        assert!(!ModFilter::Family(Family::Physical).matches(a, s, f));
        assert!(ModFilter::Source(Source::Magical).matches(a, s, f));
        assert!(!ModFilter::Source(Source::Combat).matches(a, s, f));
        assert!(ModFilter::Form(Form::Projectile).matches(a, s, f));
        assert!(!ModFilter::Form(Form::Strike).matches(a, s, f));
        assert!(ModFilter::SourceForm(Source::Magical, Form::Projectile).matches(a, s, f));
        assert!(!ModFilter::SourceForm(Source::Combat, Form::Projectile).matches(a, s, f));
        assert!(!ModFilter::SourceForm(Source::Magical, Form::Strike).matches(a, s, f));
    }

    #[test]
    fn increase_sums_more_multiplies() {
        let mods = [
            Mod::new(ModFilter::Any, 2_000),
            Mod::new(ModFilter::Aspect(Aspect::Cold), 3_000),
            Mod::new(ModFilter::Aspect(Aspect::Fire), 7_000),
            Mod::default(),
        ];
        let (a, s, f) = (Aspect::Cold, Source::Magical, Form::Strike);
        assert_eq!(sum_matching(&mods, a, s, f), 5_000);
        // x1.2 * x1.3 = x1.56
        assert_eq!(product_matching(&mods, a, s, f), 15_600);
        assert_eq!(product_matching(&[], a, s, f), BP);
        let huge = [Mod::new(ModFilter::Any, u32::MAX); 4];
        assert_eq!(sum_matching(&huge, a, s, f), u32::MAX);
        assert_eq!(product_matching(&huge, a, s, f), u32::MAX);
    }
}
