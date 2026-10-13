//! Support and control: timed modifiers (buffs and debuffs), heals, and the effect of a damage
//! skill (docs/SKILL_BUILDER.md, section 6; docs/COMBAT_MATH.md, sections 3-4 and 7.5).
//!
//! A buff or debuff is a [`TimedMod`] on the target that feeds the existing modifier groups for as
//! long as it lasts. Nothing ever subtracts:
//!
//! | Modifier      | Group          | Where                                         |
//! |---------------|----------------|-----------------------------------------------|
//! | Empowerment   | increase       | one `Attacker.increased` slot, every packet   |
//! | Weakening     | counterweight  | summed into `Attacker.weaken_bp`              |
//! | Protection    | counterweight  | added to every `Defender.resist_bp` (capped)  |
//! | Vulnerability | multiplier     | `Defender.vuln_bp` of the modifier's aspect   |
//! | Haste / Slow  | opposing pair  | action points: `ap * (BP + haste) / (BP + slow)` |
//!
//! Roll order of one support action (the website replays it from the VRF seed, keep it stable):
//! 1. hostile only (a debuff on an enemy): `d20` hit roll against the target's defense,
//!    natural 1 misses, natural 20 lands and crits; a miss stops here;
//! 2. crit chance roll, unless the d20 was a natural 20 and only with `crit_chance_bp > 0`;
//! 3. one `d100` for the point in the strength range, only when there is a range (hard control
//!    has none). A crit multiplies the point by `Balance::crit_more_bp`.
//!
//! The effect of a damage skill ([`resolve_rider`]) rolls after the hit, on the next indices of
//! the same VRF value: 1. effect chance roll (none on a crit: guaranteed); 2. the `d100` point.

use deadroll_types::{Aspect, BP};

use crate::balance::Balance;
use crate::hit::{Attacker, Defender, Outcome};
use crate::modifiers::{more, ratio, Mod, ModFilter};
use crate::rng::{chance, d100, d20, RollSource};
use crate::states::Applied;

#[cfg(feature = "borsh")]
use borsh::{BorshDeserialize, BorshSerialize};

/// A buff or debuff kind that lives on a unit for some turns.
#[cfg_attr(feature = "borsh", derive(BorshSerialize, BorshDeserialize))]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
#[repr(u8)]
pub enum ModKind {
    #[default]
    Haste,
    Slow,
    Empowerment,
    Weakening,
    Protection,
    Vulnerability,
}

impl ModKind {
    pub const COUNT: usize = 6;
    pub const ALL: [ModKind; Self::COUNT] = [
        ModKind::Haste,
        ModKind::Slow,
        ModKind::Empowerment,
        ModKind::Weakening,
        ModKind::Protection,
        ModKind::Vulnerability,
    ];

    /// Slow, Weakening, Vulnerability: harmful, applied to enemies.
    pub const fn is_harmful(self) -> bool {
        matches!(
            self,
            ModKind::Slow | ModKind::Weakening | ModKind::Vulnerability
        )
    }
}

/// Modifier slots on one unit: one instance per kind.
pub const MODS_MAX: usize = ModKind::COUNT;

/// A buff or debuff on a unit. `turns == 0` marks an empty slot.
#[cfg_attr(feature = "borsh", derive(BorshSerialize, BorshDeserialize))]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct TimedMod {
    pub kind: ModKind,
    /// Aspect of the effect: the skill's aspect (Vulnerability is to this aspect; reactions later).
    pub aspect: Aspect,
    /// Own turn starts of the bearer until it ends ([`mod_turns`]).
    pub turns: u8,
    /// Strength in bp.
    pub power_bp: u32,
    /// Turn-order id of the unit that applied it.
    pub source: u8,
}

impl TimedMod {
    pub const fn is_empty(&self) -> bool {
        self.turns == 0
    }
}

pub type TimedMods = [TimedMod; MODS_MAX];

/// Bit of a modifier kind in an expiry mask.
pub const fn mod_bit(kind: ModKind) -> u8 {
    1 << kind as u8
}

/// Stored `turns` for a modifier that lasts `duration` of the bearer's own turns: each own turn
/// start counts it down by one before the turn is played, so it covers the rest of the current
/// turn (if any) and exactly the next `duration` own turns.
pub const fn mod_turns(duration: u8) -> u8 {
    match duration {
        0 => 0,
        d => d.saturating_add(1),
    }
}

pub fn find_mod(mods: &TimedMods, kind: ModKind) -> Option<usize> {
    mods.iter().position(|m| m.turns > 0 && m.kind == kind)
}

/// Strength of `kind` on the unit, 0 if absent.
pub fn mod_power(mods: &TimedMods, kind: ModKind) -> u32 {
    find_mod(mods, kind).map_or(0, |i| mods[i].power_bp)
}

/// One instance per kind, like states: the stronger power (with its aspect and source) stays,
/// the duration is refreshed to the longer one.
pub fn apply_mod(mods: &mut TimedMods, new: TimedMod) -> Applied {
    if new.turns == 0 {
        return Applied::None;
    }
    if let Some(i) = find_mod(mods, new.kind) {
        let old = &mut mods[i];
        if new.power_bp > old.power_bp {
            old.power_bp = new.power_bp;
            old.aspect = new.aspect;
            old.source = new.source;
        }
        old.turns = old.turns.max(new.turns);
        return Applied::Refreshed;
    }
    match mods.iter().position(|m| m.turns == 0) {
        Some(i) => {
            mods[i] = new;
            Applied::Added
        }
        None => Applied::None,
    }
}

/// Start of the bearer's own turn: every duration goes down by one; returns the [`mod_bit`]s of
/// the modifiers that ended.
pub fn tick_mods(mods: &mut TimedMods) -> u8 {
    let mut expired = 0;
    for m in mods.iter_mut().filter(|m| m.turns > 0) {
        m.turns -= 1;
        if m.turns == 0 {
            expired |= mod_bit(m.kind);
            *m = TimedMod::default();
        }
    }
    expired
}

/// Empowerment joins the attacker's increase group, Weakening its counterweight.
pub fn apply_to_attacker(attacker: &mut Attacker, mods: &TimedMods) {
    let empower = mod_power(mods, ModKind::Empowerment);
    if empower > 0 {
        match attacker.increased.iter_mut().find(|m| m.bp == 0) {
            Some(slot) => *slot = Mod::new(ModFilter::Any, empower),
            // Every slot taken (never in P0): sum into the last universal one.
            None => {
                let last = attacker.increased.len() - 1;
                let slot = &mut attacker.increased[last];
                *slot = Mod::new(ModFilter::Any, slot.bp.saturating_add(empower));
            }
        }
    }
    attacker.weaken_bp = attacker
        .weaken_bp
        .saturating_add(mod_power(mods, ModKind::Weakening));
}

/// Protection is a counterweight on every aspect (through resistance, so the cap still holds);
/// Vulnerability multiplies the damage of its aspect.
pub fn apply_to_defender(defender: &mut Defender, mods: &TimedMods) {
    let protection = mod_power(mods, ModKind::Protection);
    if protection > 0 {
        for r in defender.resist_bp.iter_mut() {
            *r = r.saturating_add(protection);
        }
    }
    if let Some(i) = find_mod(mods, ModKind::Vulnerability) {
        let v = &mut defender.vuln_bp[mods[i].aspect as usize];
        *v = v.saturating_add(mods[i].power_bp);
    }
}

/// Action points of a turn: Haste multiplies, Slow divides (opposing pair), rounded down, never
/// below 1 (hard control is a separate rule).
pub fn speed_ap(base: u8, mods: &TimedMods) -> u8 {
    let haste = mod_power(mods, ModKind::Haste);
    let slow = mod_power(mods, ModKind::Slow);
    ratio(base as u64, haste, slow).clamp(1, u8::MAX as u64) as u8
}

// ---------------------------------------------------------------------------------------------
// Rolls

/// Balance numbers of support actions. TODO(balance): every number, tune in sim.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SupportBalance {
    /// Strength range in bp before skill power, indexed by `ModKind`.
    pub mod_min_bp: [u32; MODS_MAX],
    pub mod_max_bp: [u32; MODS_MAX],
    /// Duration in the bearer's own turns, indexed by `ModKind`.
    pub mod_duration: [u8; MODS_MAX],
    /// Native chance of a damage skill's effect on a hit (a crit makes it certain).
    pub rider_chance_bp: u32,
}

impl SupportBalance {
    pub const DEFAULT: SupportBalance = SupportBalance {
        // Haste, Slow, Empowerment, Weakening, Protection, Vulnerability
        mod_min_bp: [2_500, 2_500, 2_000, 2_500, 2_000, 2_000],
        mod_max_bp: [5_000, 5_000, 4_000, 5_000, 4_000, 4_000],
        mod_duration: [2, 2, 2, 2, 2, 2],
        rider_chance_bp: 3_000,
    };

    pub const fn duration(&self, kind: ModKind) -> u8 {
        self.mod_duration[kind as usize]
    }
}

impl Default for SupportBalance {
    fn default() -> Self {
        Self::DEFAULT
    }
}

/// One support action: a heal, a buff or a debuff.
#[cfg_attr(feature = "borsh", derive(BorshSerialize, BorshDeserialize))]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct SupportInput {
    /// A debuff on an enemy: lands only on `d20 + attack_bonus >= defense`.
    pub hostile: bool,
    pub attack_bonus: u32,
    pub defense: u32,
    /// Crit chance besides a natural 20, bp.
    pub crit_chance_bp: u32,
    /// Strength range: HP for a heal, bp for a modifier. `max == 0`: no strength (hard control).
    pub min: u32,
    pub max: u32,
}

/// The rolls of a support action or of a damage skill's effect.
#[cfg_attr(feature = "borsh", derive(BorshSerialize, BorshDeserialize))]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct EffectRoll {
    /// `Hit`: the effect lands. `Miss`: the d20 or the effect chance failed.
    pub outcome: Outcome,
    /// The hit roll of a hostile action, 0 = none.
    pub d20: u8,
    pub attack_total: u32,
    pub defense: u32,
    pub crit: bool,
    pub crit_roll: Option<u16>,
    /// Effect chance of a damage skill's effect (0 for support actions).
    pub chance_bp: u32,
    /// The chance roll, `None` when not rolled (support action, or certain on a crit).
    pub chance_roll: Option<u16>,
    pub d100: u8,
    /// Strength: the d100 point in the range, x crit. 0 without a range or on a miss.
    pub amount: u32,
    /// Dice used from the roll source.
    pub rolls_used: u32,
}

/// The `d100` point in `min..=max`, raised by the crit multiplier on a crit.
fn strength(
    min: u32,
    max: u32,
    crit: bool,
    balance: &Balance,
    src: &mut impl RollSource,
    out: &mut EffectRoll,
) {
    if max == 0 {
        return;
    }
    let max = max.max(min);
    out.d100 = d100(src);
    let point = min as u64 + (max - min) as u64 * out.d100 as u64 / 100;
    let point = if crit {
        more(point, balance.crit_more_bp)
    } else {
        point
    };
    out.amount = point.min(u32::MAX as u64) as u32;
}

/// Resolves a heal, buff or debuff. See the module docs for the roll order.
pub fn resolve_support(
    input: &SupportInput,
    balance: &Balance,
    src: &mut impl RollSource,
) -> EffectRoll {
    let mut src = Counted {
        inner: src,
        used: 0,
    };
    let mut out = EffectRoll {
        defense: input.defense,
        ..EffectRoll::default()
    };
    if input.hostile {
        out.d20 = d20(&mut src);
        out.attack_total = (out.d20 as u32).saturating_add(input.attack_bonus);
        let lands = match out.d20 {
            1 => false,
            20 => true,
            _ => out.attack_total >= input.defense,
        };
        if !lands {
            out.outcome = Outcome::Miss;
            out.rolls_used = src.used;
            return out;
        }
        out.crit = out.d20 == 20;
    }
    out.outcome = Outcome::Hit;
    if !out.crit && input.crit_chance_bp > 0 {
        let (crit, roll) = chance(&mut src, balance.clamp_chance(input.crit_chance_bp));
        out.crit = crit;
        out.crit_roll = Some(roll);
    }
    strength(input.min, input.max, out.crit, balance, &mut src, &mut out);
    out.rolls_used = src.used;
    out
}

/// The effect of a damage skill after a hit (`hit_crit` = that hit's crit). `min..=max` as in
/// [`SupportInput`]. Roll it on the same seed after the hit's `rolls_used`.
pub fn resolve_rider(
    chance_bp: u32,
    hit_crit: bool,
    min: u32,
    max: u32,
    balance: &Balance,
    src: &mut impl RollSource,
) -> EffectRoll {
    let mut src = Counted {
        inner: src,
        used: 0,
    };
    let mut out = EffectRoll {
        crit: hit_crit,
        chance_bp: balance.clamp_chance(chance_bp),
        ..EffectRoll::default()
    };
    let lands = if hit_crit {
        true
    } else {
        let (ok, roll) = chance(&mut src, out.chance_bp);
        out.chance_roll = Some(roll);
        ok
    };
    if !lands {
        out.outcome = Outcome::Miss;
        out.rolls_used = src.used;
        return out;
    }
    out.outcome = Outcome::Hit;
    strength(min, max, hit_crit, balance, &mut src, &mut out);
    out.rolls_used = src.used;
    out
}

/// A modifier's strength range for a skill of `power_bp` with `increase_bp` (class bonus to
/// buff and debuff power): `base * power / BP * (BP + increase) / BP`.
pub fn mod_range(kind: ModKind, power_bp: u32, increase_bp: u32, b: &SupportBalance) -> (u32, u32) {
    let scale = |v: u32| {
        let v = v as u64 * power_bp as u64 / BP as u64;
        more(v, increase_bp).min(u32::MAX as u64) as u32
    };
    let (min, max) = (
        scale(b.mod_min_bp[kind as usize]),
        scale(b.mod_max_bp[kind as usize]),
    );
    (min.max(1), max.max(min).max(1))
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::hit::{resolve_hit, HitInput};
    use crate::packet::{DamageRange, Skill};
    use crate::rng::{ScriptedRoll, SeededRoll};
    use deadroll_types::{Form, Source};

    const B: Balance = Balance::DEFAULT;

    fn m(kind: ModKind, turns: u8, power_bp: u32) -> TimedMod {
        TimedMod {
            kind,
            aspect: Aspect::Cold,
            turns,
            power_bp,
            source: 1,
        }
    }

    fn with(list: &[TimedMod]) -> TimedMods {
        let mut out = TimedMods::default();
        for &x in list {
            apply_mod(&mut out, x);
        }
        out
    }

    #[test]
    fn one_instance_per_kind_strongest_stays_longest_duration() {
        let mut mods = TimedMods::default();
        assert_eq!(
            apply_mod(&mut mods, m(ModKind::Haste, 2, 3_000)),
            Applied::Added
        );
        let weaker_longer = TimedMod {
            aspect: Aspect::Fire,
            source: 7,
            ..m(ModKind::Haste, 4, 1_000)
        };
        assert_eq!(apply_mod(&mut mods, weaker_longer), Applied::Refreshed);
        let i = find_mod(&mods, ModKind::Haste).unwrap();
        assert_eq!(
            (
                mods[i].power_bp,
                mods[i].turns,
                mods[i].aspect,
                mods[i].source
            ),
            (3_000, 4, Aspect::Cold, 1)
        );
        let stronger = TimedMod {
            aspect: Aspect::Fire,
            source: 7,
            ..m(ModKind::Haste, 1, 5_000)
        };
        apply_mod(&mut mods, stronger);
        assert_eq!(
            (
                mods[i].power_bp,
                mods[i].turns,
                mods[i].aspect,
                mods[i].source
            ),
            (5_000, 4, Aspect::Fire, 7)
        );
        assert_eq!(mods.iter().filter(|x| !x.is_empty()).count(), 1);
        assert_eq!(
            apply_mod(&mut mods, m(ModKind::Slow, 0, 9_000)),
            Applied::None
        );
        // Every kind fits.
        for kind in ModKind::ALL {
            assert_ne!(apply_mod(&mut mods, m(kind, 1, 1)), Applied::None);
        }
        assert!(mods.iter().all(|x| !x.is_empty()));
    }

    #[test]
    fn a_modifier_covers_exactly_its_duration_in_own_turns() {
        for duration in 0..=3u8 {
            let mut mods = with(&[m(ModKind::Haste, mod_turns(duration), 5_000)]);
            let mut covered = 0;
            let mut expired_at = None;
            for turn in 1..=10 {
                let expired = tick_mods(&mut mods);
                if expired != 0 {
                    assert_eq!(expired, mod_bit(ModKind::Haste));
                    expired_at = Some(turn);
                }
                if mod_power(&mods, ModKind::Haste) > 0 {
                    covered += 1;
                }
            }
            assert_eq!(covered, duration, "duration {duration}");
            if duration > 0 {
                assert_eq!(expired_at, Some(duration as u32 + 1));
            }
        }
        assert_eq!(mod_turns(u8::MAX), u8::MAX);
        assert!(mods_all_empty_after_ticks());
    }

    fn mods_all_empty_after_ticks() -> bool {
        let mut mods = with(&[m(ModKind::Slow, 1, 1), m(ModKind::Protection, 2, 1)]);
        assert_eq!(tick_mods(&mut mods), mod_bit(ModKind::Slow));
        assert_eq!(tick_mods(&mut mods), mod_bit(ModKind::Protection));
        mods.iter().all(|x| x.is_empty())
    }

    #[test]
    fn haste_and_slow_are_an_opposing_pair() {
        assert_eq!(speed_ap(4, &TimedMods::default()), 4);
        assert_eq!(speed_ap(4, &with(&[m(ModKind::Haste, 2, 5_000)])), 6);
        assert_eq!(speed_ap(4, &with(&[m(ModKind::Slow, 2, 10_000)])), 2);
        let both = with(&[m(ModKind::Haste, 2, 4_000), m(ModKind::Slow, 2, 4_000)]);
        assert_eq!(speed_ap(5, &both), 5);
        // Never below 1, never past u8.
        assert_eq!(speed_ap(1, &with(&[m(ModKind::Slow, 2, u32::MAX)])), 1);
        assert_eq!(speed_ap(0, &TimedMods::default()), 1);
        assert_eq!(
            speed_ap(200, &with(&[m(ModKind::Haste, 2, u32::MAX)])),
            u8::MAX
        );
    }

    fn attack() -> HitInput {
        HitInput {
            attacker: Attacker {
                attack_bonus: 100,
                weapon: DamageRange::new(Aspect::Blunt, 10, 10),
                skill: Skill::basic(Source::Combat, Form::Strike, 0),
                ..Attacker::default()
            },
            defender: Defender::default(),
        }
    }

    fn damage(input: &HitInput) -> u32 {
        resolve_hit(input, &B, &mut ScriptedRoll::new([10, 50, 50, 50])).damage
    }

    #[test]
    fn empowerment_increases_weakening_counterweighs() {
        let plain = attack();
        assert_eq!(damage(&plain), 10);

        let mut strong = attack();
        strong.attacker.increased[0] = Mod::new(ModFilter::Any, 1_000);
        apply_to_attacker(
            &mut strong.attacker,
            &with(&[m(ModKind::Empowerment, 2, 5_000)]),
        );
        assert_eq!(
            strong.attacker.increased[1],
            Mod::new(ModFilter::Any, 5_000)
        );
        // Increase is summed: 10 x (1 + 0.1 + 0.5).
        assert_eq!(damage(&strong), 16);

        let mut weak = attack();
        weak.attacker.weaken_bp = 5_000;
        apply_to_attacker(
            &mut weak.attacker,
            &with(&[m(ModKind::Weakening, 2, 5_000)]),
        );
        assert_eq!(weak.attacker.weaken_bp, 10_000);
        assert_eq!(damage(&weak), 5);
        // Any amount of Weakening leaves the minimum damage.
        let mut crushed = attack();
        apply_to_attacker(
            &mut crushed.attacker,
            &with(&[m(ModKind::Weakening, 2, u32::MAX)]),
        );
        assert_eq!(damage(&crushed), B.min_damage);

        // Every increase slot taken: Empowerment sums into the last one.
        let mut full = attack();
        full.attacker.increased = [Mod::new(ModFilter::Any, 100); crate::hit::INCREASED_MAX];
        apply_to_attacker(
            &mut full.attacker,
            &with(&[m(ModKind::Empowerment, 2, 900)]),
        );
        assert_eq!(
            full.attacker.increased[crate::hit::INCREASED_MAX - 1].bp,
            1_000
        );
    }

    #[test]
    fn protection_is_a_capped_counterweight_vulnerability_a_multiplier() {
        let mut guarded = attack();
        apply_to_defender(
            &mut guarded.defender,
            &with(&[m(ModKind::Protection, 2, 10_000)]),
        );
        assert!(guarded.defender.resist_bp.iter().all(|&r| r == 10_000));
        assert_eq!(damage(&guarded), 5);
        let mut wall = attack();
        apply_to_defender(
            &mut wall.defender,
            &with(&[m(ModKind::Protection, 2, u32::MAX)]),
        );
        // The resistance cap (30_000) holds: a quarter gets through.
        assert_eq!(damage(&wall), 2);

        let exposed_mod = TimedMod {
            aspect: Aspect::Blunt,
            ..m(ModKind::Vulnerability, 2, 5_000)
        };
        let mut exposed = attack();
        apply_to_defender(&mut exposed.defender, &with(&[exposed_mod]));
        assert_eq!(exposed.defender.vuln_bp[Aspect::Blunt as usize], 5_000);
        assert_eq!(exposed.defender.vuln_bp[Aspect::Fire as usize], 0);
        assert_eq!(damage(&exposed), 15);
        // Vulnerability to another aspect does nothing to this hit.
        let mut elsewhere = attack();
        apply_to_defender(
            &mut elsewhere.defender,
            &with(&[m(ModKind::Vulnerability, 2, 5_000)]),
        );
        assert_eq!(damage(&elsewhere), 10);
        // Equal Protection and Vulnerability cancel out (bar the single round-down).
        let mut both = attack();
        apply_to_defender(
            &mut both.defender,
            &with(&[exposed_mod, m(ModKind::Protection, 2, 5_000)]),
        );
        assert_eq!(damage(&both), 9);
    }

    #[test]
    fn friendly_support_rolls_crit_then_strength() {
        let heal = SupportInput {
            crit_chance_bp: 1_000,
            min: 4,
            max: 12,
            ..SupportInput::default()
        };
        // crit roll 5_000 (no crit), d100 = 50.
        let r = resolve_support(&heal, &B, &mut ScriptedRoll::new([5_000, 50]));
        assert_eq!(r.outcome, Outcome::Hit);
        assert_eq!((r.d20, r.crit, r.crit_roll), (0, false, Some(5_000)));
        assert_eq!((r.d100, r.amount, r.rolls_used), (50, 8, 2));
        // Crit roll 999 < 1_000: x1.5.
        let r = resolve_support(&heal, &B, &mut ScriptedRoll::new([999, 100]));
        assert!(r.crit);
        assert_eq!((r.d100, r.amount), (100, 18));
        // No crit chance: only the d100.
        let plain = SupportInput {
            crit_chance_bp: 0,
            ..heal
        };
        let r = resolve_support(&plain, &B, &mut ScriptedRoll::new([0]));
        assert_eq!(
            (r.crit_roll, r.d100, r.amount, r.rolls_used),
            (None, 0, 4, 1)
        );
    }

    #[test]
    fn hostile_support_rolls_to_land() {
        let sap = SupportInput {
            hostile: true,
            attack_bonus: 3,
            defense: 12,
            crit_chance_bp: 0,
            min: 2_000,
            max: 4_000,
        };
        // d20 = roll % 20 + 1.
        let miss = resolve_support(&sap, &B, &mut ScriptedRoll::new([7, 99]));
        assert_eq!(
            (miss.outcome, miss.d20, miss.attack_total),
            (Outcome::Miss, 8, 11)
        );
        assert_eq!((miss.amount, miss.rolls_used), (0, 1));
        let hit = resolve_support(&sap, &B, &mut ScriptedRoll::new([8, 100]));
        assert_eq!(
            (hit.outcome, hit.attack_total, hit.defense),
            (Outcome::Hit, 12, 12)
        );
        assert_eq!((hit.amount, hit.rolls_used), (4_000, 2));
        // Natural 1 misses whatever the bonus, natural 20 lands and crits without a crit roll.
        let sure = SupportInput {
            attack_bonus: 1_000,
            crit_chance_bp: 5_000,
            ..sap
        };
        assert_eq!(
            resolve_support(&sure, &B, &mut ScriptedRoll::new([0])).outcome,
            Outcome::Miss
        );
        let hopeless = SupportInput {
            attack_bonus: 0,
            defense: 1_000,
            ..sap
        };
        let r = resolve_support(&hopeless, &B, &mut ScriptedRoll::new([19, 0]));
        assert_eq!((r.outcome, r.crit, r.crit_roll), (Outcome::Hit, true, None));
        assert_eq!((r.amount, r.rolls_used), (3_000, 2));
        // Hard control has no strength: no d100.
        let stun = SupportInput {
            min: 0,
            max: 0,
            ..sap
        };
        let r = resolve_support(&stun, &B, &mut ScriptedRoll::new([15, 99]));
        assert_eq!((r.outcome, r.amount, r.rolls_used), (Outcome::Hit, 0, 1));
    }

    #[test]
    fn riders_roll_their_chance_unless_the_hit_crit() {
        let r = resolve_rider(
            3_000,
            false,
            2_000,
            4_000,
            &B,
            &mut ScriptedRoll::new([2_999, 0]),
        );
        assert_eq!(
            (r.outcome, r.chance_bp, r.chance_roll),
            (Outcome::Hit, 3_000, Some(2_999))
        );
        assert_eq!((r.amount, r.rolls_used), (2_000, 2));
        let r = resolve_rider(
            3_000,
            false,
            2_000,
            4_000,
            &B,
            &mut ScriptedRoll::new([3_000]),
        );
        assert_eq!((r.outcome, r.amount, r.rolls_used), (Outcome::Miss, 0, 1));
        let r = resolve_rider(3_000, true, 2_000, 4_000, &B, &mut ScriptedRoll::new([100]));
        assert_eq!(
            (r.outcome, r.chance_roll, r.amount),
            (Outcome::Hit, None, 6_000)
        );
        // Freeze / Stun riders: chance only.
        let r = resolve_rider(3_000, false, 0, 0, &B, &mut ScriptedRoll::new([0, 0]));
        assert_eq!((r.outcome, r.rolls_used), (Outcome::Hit, 1));
        // The chance is clamped like every rolled chance.
        assert_eq!(
            resolve_rider(0, false, 0, 0, &B, &mut ScriptedRoll::new([0])).chance_bp,
            B.chance_min_bp
        );
    }

    #[test]
    fn rolls_are_deterministic_and_bounded() {
        let input = SupportInput {
            hostile: true,
            attack_bonus: 4,
            defense: 11,
            crit_chance_bp: 2_000,
            min: 1_000,
            max: 3_000,
        };
        for seed in 0..300 {
            let a = resolve_support(&input, &B, &mut SeededRoll::new(seed));
            assert_eq!(a, resolve_support(&input, &B, &mut SeededRoll::new(seed)));
            match a.outcome {
                Outcome::Hit => {
                    let top = more(3_000, B.crit_more_bp) as u32;
                    assert!(a.amount >= 1_000 && a.amount <= top);
                    assert!(a.crit || a.amount <= 3_000);
                }
                _ => assert_eq!(a.amount, 0),
            }
            assert!(a.rolls_used <= 3);
        }
        let huge = SupportInput {
            min: u32::MAX,
            max: u32::MAX,
            crit_chance_bp: 10_000,
            ..SupportInput::default()
        };
        assert_eq!(
            resolve_support(&huge, &B, &mut ScriptedRoll::new([0, 100])).amount,
            u32::MAX
        );
    }

    #[test]
    fn mod_range_scales_with_power_and_class() {
        let s = SupportBalance::DEFAULT;
        assert_eq!(mod_range(ModKind::Haste, BP, 0, &s), (2_500, 5_000));
        assert_eq!(mod_range(ModKind::Haste, 5_000, 2_000, &s), (1_500, 3_000));
        assert_eq!(mod_range(ModKind::Slow, 0, 0, &s), (1, 1));
        assert_eq!(
            mod_range(ModKind::Weakening, u32::MAX, u32::MAX, &s).1,
            u32::MAX
        );
        assert_eq!(s.duration(ModKind::Protection), 2);
        assert!(ModKind::Slow.is_harmful() && !ModKind::Haste.is_harmful());
        assert_eq!(mod_bit(ModKind::Vulnerability), 0b10_0000);
    }
}
