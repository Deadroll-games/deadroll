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
//! | Resistance    | counterweight  | `Defender.resist_bp` by its aspect weights    |
//! | Haste / Slow  | opposing pair  | action points: `ap * (BP + haste) / (BP + slow)` |
//! | Exposed       | addition       | `Defender.crit_taken_pct`, every attacker     |
//! | Root          | rule           | the bearer cannot move ([`rooted`])           |
//! | Blinded       | counterweight  | divides the bearer's d20 (`Attacker.blinded_bp`) |
//! | OffGuard      | counterweight  | divides the bearer's defense                  |
//! | Sundered      | counterweight  | divides the armor against Piercing (`Defender.sunder_bp`) |
//! | Dazed         | rule           | no reactive skills ([`dazed`])                |
//! | Silenced      | rule           | no skills, basic attack and move only ([`silenced`]) |
//! | NoHeal        | rule           | heals on the bearer do nothing ([`no_heal`])  |
//! | Dulled        | counterweight  | divides the bearer's crit chance              |
//!
//! Roll order of one support action (the website replays it from the VRF seed, keep it stable):
//! 1. hostile only (a debuff on an enemy): `d20` hit roll against the target's defense,
//!    natural 1 misses, natural 20 lands and crits; a miss stops here;
//! 2. crit chance roll, unless the d20 was a natural 20 and only with `crit_chance_pct > 0`;
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
    /// Patch 9: resistance to the aspects of its skill and its bearer's predisposition, each by
    /// its weight (`TimedMod::weights_pct`).
    Resistance,
    /// Resonance only: every attacker's crit chance against the bearer grows by `power_bp`
    /// (bp of a percent point, `100` = +1%).
    Exposed,
    /// Resonance only: the bearer cannot move (it can still act).
    Root,
    /// Triples only: the bearer's d20 is divided by a counterweight of `power_bp`.
    Blinded,
    /// Triples only: the bearer's defense is divided by a counterweight of `power_bp`.
    OffGuard,
    /// Triples only: the bearer's armor against Piercing packets is divided by a counterweight of
    /// `power_bp`.
    Sundered,
    /// Triples only: the bearer uses no reactive skill.
    Dazed,
    /// Triples only: the bearer uses no skill (a basic attack and a move are still allowed).
    Silenced,
    /// Triples only: heals on the bearer do nothing.
    NoHeal,
    /// Triples only: the bearer's crit chance is divided by a counterweight of `power_bp`.
    Dulled,
}

impl ModKind {
    pub const COUNT: usize = 16;
    pub const ALL: [ModKind; Self::COUNT] = [
        ModKind::Haste,
        ModKind::Slow,
        ModKind::Empowerment,
        ModKind::Weakening,
        ModKind::Protection,
        ModKind::Vulnerability,
        ModKind::Resistance,
        ModKind::Exposed,
        ModKind::Root,
        ModKind::Blinded,
        ModKind::OffGuard,
        ModKind::Sundered,
        ModKind::Dazed,
        ModKind::Silenced,
        ModKind::NoHeal,
        ModKind::Dulled,
    ];

    /// Slow, Weakening, Vulnerability and every resonance-only kind: harmful, applied to enemies.
    pub const fn is_harmful(self) -> bool {
        matches!(
            self,
            ModKind::Slow
                | ModKind::Weakening
                | ModKind::Vulnerability
                | ModKind::Exposed
                | ModKind::Root
                | ModKind::Blinded
                | ModKind::OffGuard
                | ModKind::Sundered
                | ModKind::Dazed
                | ModKind::Silenced
                | ModKind::NoHeal
                | ModKind::Dulled
        )
    }
}

/// Modifier slots on one unit: one instance per kind, each in the slot of its kind
/// (`kind as usize`), so the stored layout is the same everywhere.
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
    /// Patch 9, Resistance only: the share of `power_bp` each aspect gets, percent (`0..=100`,
    /// by `Aspect`). Zero for every other kind.
    pub weights_pct: [u8; Aspect::COUNT],
}

impl TimedMod {
    pub const fn is_empty(&self) -> bool {
        self.turns == 0
    }
}

pub type TimedMods = [TimedMod; MODS_MAX];

/// Bit of a modifier kind in an expiry mask.
pub const fn mod_bit(kind: ModKind) -> u16 {
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
/// the duration is refreshed to the longer one. A new kind goes into its own slot.
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
            old.weights_pct = new.weights_pct;
        }
        old.turns = old.turns.max(new.turns);
        return Applied::Refreshed;
    }
    match free_slot(mods, new.kind) {
        Some(i) => {
            mods[i] = new;
            Applied::Added
        }
        None => Applied::None,
    }
}

/// The slot a new modifier of `kind` takes: its own, else (data written before patch 9, which
/// filled the first free slot) the first free one.
pub fn free_slot(mods: &TimedMods, kind: ModKind) -> Option<usize> {
    let own = kind as usize;
    if mods[own].is_empty() {
        Some(own)
    } else {
        mods.iter().position(|m| m.is_empty())
    }
}

/// Start of the bearer's own turn: every duration goes down by one; returns the [`mod_bit`]s of
/// the modifiers that ended.
pub fn tick_mods(mods: &mut TimedMods) -> u16 {
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
    // Triples: a Blinded bearer's d20 and a Dulled bearer's crit chance are divided.
    attacker.blinded_bp = attacker
        .blinded_bp
        .saturating_add(mod_power(mods, ModKind::Blinded));
    let dulled = mod_power(mods, ModKind::Dulled);
    if dulled > 0 {
        attacker.crit_chance_pct = ratio(attacker.crit_chance_pct as u64, 0, dulled)
            .min(u32::MAX as u64) as u32;
    }
}

/// Protection is a counterweight on every aspect (through resistance, so the cap still holds);
/// Resistance on the aspects it weighs; Vulnerability multiplies the damage of its aspect.
pub fn apply_to_defender(defender: &mut Defender, mods: &TimedMods) {
    let protection = mod_power(mods, ModKind::Protection);
    if protection > 0 {
        for r in defender.resist_bp.iter_mut() {
            *r = r.saturating_add(protection);
        }
    }
    if let Some(i) = find_mod(mods, ModKind::Resistance) {
        let m = &mods[i];
        for (r, w) in defender.resist_bp.iter_mut().zip(m.weights_pct) {
            *r = r.saturating_add(resistance_share(m.power_bp, w));
        }
    }
    if let Some(i) = find_mod(mods, ModKind::Vulnerability) {
        let v = &mut defender.vuln_bp[mods[i].aspect as usize];
        *v = v.saturating_add(mods[i].power_bp);
    }
    defender.crit_taken_pct = defender
        .crit_taken_pct
        .saturating_add(mod_power(mods, ModKind::Exposed) / 100);
    // Triples: OffGuard divides the defense, Sundered the armor against Piercing.
    let off_guard = mod_power(mods, ModKind::OffGuard);
    if off_guard > 0 {
        defender.defense = ratio(defender.defense as u64, 0, off_guard).min(u32::MAX as u64) as u32;
    }
    defender.sunder_bp = defender
        .sunder_bp
        .saturating_add(mod_power(mods, ModKind::Sundered));
}

/// The bearer uses no reactive skill: a Dazed on it.
pub fn dazed(mods: &TimedMods) -> bool {
    find_mod(mods, ModKind::Dazed).is_some()
}

/// The bearer uses no skill, only a basic attack and a move: a Silenced on it.
pub fn silenced(mods: &TimedMods) -> bool {
    find_mod(mods, ModKind::Silenced).is_some()
}

/// Heals on the bearer do nothing: a NoHeal on it.
pub fn no_heal(mods: &TimedMods) -> bool {
    find_mod(mods, ModKind::NoHeal).is_some()
}

/// The bearer cannot move: a Root on it.
pub fn rooted(mods: &TimedMods) -> bool {
    find_mod(mods, ModKind::Root).is_some()
}

/// The part of a Resistance of `power_bp` that an aspect of weight `weight_pct` gets.
pub const fn resistance_share(power_bp: u32, weight_pct: u8) -> u32 {
    let w = if weight_pct > 100 { 100 } else { weight_pct };
    (power_bp as u64 * w as u64 / 100) as u32
}

/// Action points of a turn: Haste multiplies, Slow and `slow_extra_bp` (Chill) divide as one
/// counterweight, rounded down, never below 1 (hard control is a separate rule).
pub fn speed_ap(base: u8, mods: &TimedMods, slow_extra_bp: u32) -> u8 {
    let haste = mod_power(mods, ModKind::Haste);
    let slow = mod_power(mods, ModKind::Slow).saturating_add(slow_extra_bp);
    ratio(base as u64, haste, slow).clamp(1, u8::MAX as u64) as u8
}

/// A timed bonus to the d20 of the bearer's attacks that carry a packet of `aspect` (a resonance,
/// docs/DAMAGE_TYPES_AND_MIXES.md 6.4, Pale Flame). `turns == 0` is none.
#[cfg_attr(feature = "borsh", derive(BorshSerialize, BorshDeserialize))]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct AccuracyBuff {
    pub aspect: Aspect,
    /// Own turn starts of the bearer until it ends, as [`TimedMod::turns`].
    pub turns: u8,
    /// Added to the d20.
    pub bonus: u32,
}

impl AccuracyBuff {
    pub const fn is_empty(&self) -> bool {
        self.turns == 0
    }

    /// One buff: a new one of another aspect or a stronger one replaces it, the same strength
    /// refreshes the duration to the longer one.
    pub fn apply(&mut self, new: AccuracyBuff) -> Applied {
        if new.turns == 0 {
            return Applied::None;
        }
        if self.is_empty() {
            *self = new;
            return Applied::Added;
        }
        if new.aspect != self.aspect || new.bonus > self.bonus {
            let turns = if new.aspect == self.aspect {
                self.turns.max(new.turns)
            } else {
                new.turns
            };
            *self = AccuracyBuff { turns, ..new };
        } else {
            self.turns = self.turns.max(new.turns);
        }
        Applied::Refreshed
    }

    /// Start of the bearer's own turn; true when it ended.
    pub fn tick(&mut self) -> bool {
        if self.turns == 0 {
            return false;
        }
        self.turns -= 1;
        if self.turns == 0 {
            *self = AccuracyBuff::default();
            return true;
        }
        false
    }

    /// Its d20 bonus for an attack with these `bases` (the skill's aspects, or the weapon's).
    pub fn bonus_for(&self, bases: &[Aspect]) -> u32 {
        if !self.is_empty() && bases.contains(&self.aspect) {
            self.bonus
        } else {
            0
        }
    }
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
    /// Native chance of a damage skill's effect on a hit (a crit makes it certain), percent.
    pub rider_chance_pct: u32,
    /// A passive skill works all the time and rolls nothing (docs/SKILL_BUILDER.md 3.3,
    /// COMBAT_MATH.md 5: strength is not rolled): its strength is this point of the range, bp
    /// (`0` = the minimum, `BP` = the maximum).
    pub passive_point_bp: u32,
}

impl SupportBalance {
    pub const DEFAULT: SupportBalance = SupportBalance {
        // Haste, Slow, Empowerment, Weakening, Protection, Vulnerability, Resistance, then the
        // resonance-only kinds (Exposed, Root, and the triple kinds): no skill effect gives
        // those, so no numbers.
        mod_min_bp: [2_500, 2_500, 2_000, 2_500, 2_000, 2_000, 2_000, 0, 0, 0, 0, 0, 0, 0, 0, 0],
        mod_max_bp: [5_000, 5_000, 4_000, 5_000, 4_000, 4_000, 4_000, 0, 0, 0, 0, 0, 0, 0, 0, 0],
        mod_duration: [2, 2, 2, 2, 2, 2, 2, 0, 0, 0, 0, 0, 0, 0, 0, 0],
        rider_chance_pct: 30,
        passive_point_bp: 5_000,
    };

    pub const fn duration(&self, kind: ModKind) -> u8 {
        self.mod_duration[kind as usize]
    }

    /// The fixed strength of a passive skill in `min..=max` (see `passive_point_bp`).
    pub const fn passive_point(&self, min: u32, max: u32) -> u32 {
        let max = if max < min { min } else { max };
        let point = self.passive_point_bp as u64;
        let point = if point > BP as u64 { BP as u64 } else { point };
        (min as u64 + (max - min) as u64 * point / BP as u64) as u32
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
    /// Crit chance besides a natural 20, percent.
    pub crit_chance_pct: u32,
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
    /// Effect chance of a damage skill's effect (0 for support actions), percent.
    pub chance_pct: u32,
    /// The chance roll, `None` when not rolled (support action, or certain on a crit).
    pub chance_roll: Option<u16>,
    pub d100: u8,
    /// Strength: the d100 point in the range, x crit. 0 without a range or on a miss.
    pub amount: u32,
    /// Dice used from the roll source.
    pub rolls_used: u32,
    /// Patch 9b: the target's counterweight the effect chance met, bp (set by the caller, which
    /// built the chance; 0 for support actions).
    pub resist_bp: u32,
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
    if !out.crit && input.crit_chance_pct > 0 {
        let (crit, roll) = chance(&mut src, balance.clamp_chance(input.crit_chance_pct));
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
    chance_pct: u32,
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
        chance_pct: balance.clamp_chance(chance_pct),
        ..EffectRoll::default()
    };
    let lands = if hit_crit {
        true
    } else {
        let (ok, roll) = chance(&mut src, out.chance_pct);
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
            weights_pct: [0; Aspect::COUNT],
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
    fn resistance_weighs_its_aspects_and_the_stronger_one_stays() {
        let mut w = [0; Aspect::COUNT];
        w[Aspect::Piercing as usize] = 65;
        w[Aspect::Blood as usize] = 35;
        let piercing = TimedMod {
            weights_pct: w,
            ..m(ModKind::Resistance, 3, 4_000)
        };
        let mut d = Defender::default();
        d.resist_bp[Aspect::Blood as usize] = 1_000;
        apply_to_defender(&mut d, &with(&[piercing]));
        assert_eq!(d.resist_bp[Aspect::Piercing as usize], 2_600);
        assert_eq!(d.resist_bp[Aspect::Blood as usize], 1_000 + 1_400);
        assert_eq!(d.resist_bp[Aspect::Fire as usize], 0);
        // One instance: a stronger Resistance replaces the weights too.
        let mut fire = [0; Aspect::COUNT];
        fire[Aspect::Fire as usize] = 100;
        let mut mods = with(&[piercing]);
        apply_mod(
            &mut mods,
            TimedMod {
                weights_pct: fire,
                ..m(ModKind::Resistance, 1, 5_000)
            },
        );
        let mut d = Defender::default();
        apply_to_defender(&mut d, &mods);
        assert_eq!(d.resist_bp[Aspect::Fire as usize], 5_000);
        assert_eq!(d.resist_bp[Aspect::Piercing as usize], 0);
        assert_eq!(resistance_share(1_000, 250), 1_000);
    }

    #[test]
    fn triple_mods_divide_defense_crit_and_d20_and_mark_rules() {
        let mods = with(&[
            m(ModKind::OffGuard, 2, 10_000),
            m(ModKind::Sundered, 2, 5_000),
        ]);
        let mut d = Defender {
            defense: 11,
            ..Defender::default()
        };
        apply_to_defender(&mut d, &mods);
        assert_eq!((d.defense, d.sunder_bp), (5, 5_000));
        let mods = with(&[
            m(ModKind::Dulled, 2, 10_000),
            m(ModKind::Blinded, 2, 3_000),
        ]);
        let mut a = Attacker {
            crit_chance_pct: 30,
            ..Attacker::default()
        };
        apply_to_attacker(&mut a, &mods);
        assert_eq!((a.crit_chance_pct, a.blinded_bp), (15, 3_000));
        // No such mods: nothing changes.
        let none = TimedMods::default();
        let mut d = Defender {
            defense: 11,
            ..Defender::default()
        };
        apply_to_defender(&mut d, &none);
        assert_eq!((d.defense, d.sunder_bp), (11, 0));
        // The rules: Dazed, Silenced, NoHeal.
        let rules = with(&[
            m(ModKind::Dazed, 1, 1),
            m(ModKind::Silenced, 1, 1),
            m(ModKind::NoHeal, 1, 1),
        ]);
        assert!(dazed(&rules) && silenced(&rules) && no_heal(&rules));
        assert!(!dazed(&none) && !silenced(&none) && !no_heal(&none));
        // Sixteen kinds fit the sixteen slots and the u16 expiry mask.
        assert_eq!(ModKind::COUNT, 16);
        assert_eq!(mod_bit(ModKind::Dulled), 1 << 15);
        assert!(ModKind::ALL[ModKind::Exposed as usize..]
            .iter()
            .all(|k| k.is_harmful()));
    }

    #[test]
    fn accuracy_buff_stacks_as_one_and_ticks() {
        let mut b = AccuracyBuff::default();
        let fire = |turns, bonus| AccuracyBuff {
            aspect: Aspect::Fire,
            turns,
            bonus,
        };
        assert_eq!(b.apply(fire(0, 3)), Applied::None);
        assert_eq!(b.apply(fire(3, 2)), Applied::Added);
        assert_eq!(b.apply(fire(2, 1)), Applied::Refreshed);
        assert_eq!((b.turns, b.bonus), (3, 2));
        b.apply(fire(1, 4));
        assert_eq!((b.turns, b.bonus), (3, 4));
        assert_eq!(b.bonus_for(&[Aspect::Cold, Aspect::Fire]), 4);
        assert_eq!(b.bonus_for(&[Aspect::Cold]), 0);
        assert!(!b.tick() && !b.tick());
        assert!(b.tick());
        assert!(b.is_empty() && !b.tick());
        assert_eq!(b.bonus_for(&[Aspect::Fire]), 0);
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
        assert_eq!(speed_ap(4, &TimedMods::default(), 0), 4);
        assert_eq!(speed_ap(4, &with(&[m(ModKind::Haste, 2, 5_000)]), 0), 6);
        assert_eq!(speed_ap(4, &with(&[m(ModKind::Slow, 2, 10_000)]), 0), 2);
        // Chill is the same counterweight as Slow: 100% halves speed.
        assert_eq!(speed_ap(4, &TimedMods::default(), 10_000), 2);
        let both = with(&[m(ModKind::Haste, 2, 4_000), m(ModKind::Slow, 2, 4_000)]);
        assert_eq!(speed_ap(5, &both, 0), 5);
        // Never below 1, never past u8.
        assert_eq!(speed_ap(1, &with(&[m(ModKind::Slow, 2, u32::MAX)]), 0), 1);
        assert_eq!(speed_ap(0, &TimedMods::default(), 0), 1);
        assert_eq!(
            speed_ap(200, &with(&[m(ModKind::Haste, 2, u32::MAX)]), 0),
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
        resolve_hit(
            input,
            &B,
            &crate::ResonanceTables::default(),
            &mut ScriptedRoll::new([10, 50, 50, 50]),
        )
        .damage
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
            crit_chance_pct: 10,
            min: 4,
            max: 12,
            ..SupportInput::default()
        };
        // crit roll 10 is not < 10, d100 = 50.
        let r = resolve_support(&heal, &B, &mut ScriptedRoll::new([10, 50]));
        assert_eq!(r.outcome, Outcome::Hit);
        assert_eq!((r.d20, r.crit, r.crit_roll), (0, false, Some(10)));
        assert_eq!((r.d100, r.amount, r.rolls_used), (50, 8, 2));
        // Crit roll 9 < 10: x1.5.
        let r = resolve_support(&heal, &B, &mut ScriptedRoll::new([9, 100]));
        assert!(r.crit);
        assert_eq!((r.d100, r.amount), (100, 18));
        // No crit chance: only the d100.
        let plain = SupportInput {
            crit_chance_pct: 0,
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
            crit_chance_pct: 0,
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
            crit_chance_pct: 50,
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
        let r = resolve_rider(30, false, 2_000, 4_000, &B, &mut ScriptedRoll::new([29, 0]));
        assert_eq!(
            (r.outcome, r.chance_pct, r.chance_roll),
            (Outcome::Hit, 30, Some(29))
        );
        assert_eq!((r.amount, r.rolls_used), (2_000, 2));
        let r = resolve_rider(30, false, 2_000, 4_000, &B, &mut ScriptedRoll::new([30]));
        assert_eq!((r.outcome, r.amount, r.rolls_used), (Outcome::Miss, 0, 1));
        let r = resolve_rider(30, true, 2_000, 4_000, &B, &mut ScriptedRoll::new([100]));
        assert_eq!(
            (r.outcome, r.chance_roll, r.amount),
            (Outcome::Hit, None, 6_000)
        );
        // Freeze / Stun riders: chance only.
        let r = resolve_rider(30, false, 0, 0, &B, &mut ScriptedRoll::new([0, 0]));
        assert_eq!((r.outcome, r.rolls_used), (Outcome::Hit, 1));
        // The chance is clamped like every rolled chance.
        assert_eq!(
            resolve_rider(0, false, 0, 0, &B, &mut ScriptedRoll::new([0])).chance_pct,
            B.chance_min_pct
        );
    }

    #[test]
    fn rolls_are_deterministic_and_bounded() {
        let input = SupportInput {
            hostile: true,
            attack_bonus: 4,
            defense: 11,
            crit_chance_pct: 20,
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
            crit_chance_pct: 100,
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
