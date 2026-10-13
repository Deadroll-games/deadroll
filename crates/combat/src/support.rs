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

use deadroll_types::Aspect;

use crate::hit::{Attacker, Defender};
use crate::modifiers::{ratio, Mod, ModFilter};
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::balance::Balance;
    use crate::hit::{resolve_hit, HitInput};
    use crate::packet::{DamageRange, Skill};
    use crate::rng::ScriptedRoll;
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
    fn bits_and_harm() {
        assert!(ModKind::Slow.is_harmful() && !ModKind::Haste.is_harmful());
        assert_eq!(mod_bit(ModKind::Vulnerability), 0b10_0000);
        assert_eq!(ModKind::ALL.len(), MODS_MAX);
    }
}
