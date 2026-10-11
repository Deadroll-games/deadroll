//! Damage packets (docs/COMBAT_MATH.md, section 4.1; docs/SKILL_BUILDER.md, section 5).
//!
//! The weapon base range x skill power is converted into the skill's aspects by shares;
//! added damage (predisposition, items) merges into the packet of its aspect. One packet
//! per aspect. Packet values are scaled by `BP` (`5 damage == 50_000`) so the whole hit
//! rounds down only once, at the end.

use deadroll_types::{Aspect, Form, Source, BP};

#[cfg(feature = "borsh")]
use borsh::{BorshDeserialize, BorshSerialize};

/// Most skill aspects (docs/SKILL_BUILDER.md, section 4.1).
pub const SKILL_ASPECTS_MAX: usize = 3;
/// Added damage slots of one attack.
pub const ADDED_MAX: usize = 3;
/// One packet per aspect.
pub const PACKETS_MAX: usize = Aspect::COUNT;

const BP128: u128 = BP as u128;

/// An unscaled damage range of one aspect. `min == max == 0` is an empty slot.
#[cfg_attr(feature = "borsh", derive(BorshSerialize, BorshDeserialize))]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct DamageRange {
    pub aspect: Aspect,
    pub min: u32,
    pub max: u32,
}

impl DamageRange {
    pub const fn new(aspect: Aspect, min: u32, max: u32) -> Self {
        Self { aspect, min, max }
    }

    pub const fn is_empty(&self) -> bool {
        self.min == 0 && self.max == 0
    }
}

/// The skill used for the attack. `aspect_count == 0` is the weapon's basic attack: the
/// weapon's own aspect, no conversion, power x1.
#[cfg_attr(feature = "borsh", derive(BorshSerialize, BorshDeserialize))]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Skill {
    /// In order: first is the main aspect (largest share).
    pub aspects: [Aspect; SKILL_ASPECTS_MAX],
    pub aspect_count: u8,
    /// Share of the converted base per aspect, bp; the skill builder makes them sum to `BP`.
    pub shares_bp: [u32; SKILL_ASPECTS_MAX],
    /// Multiplier on the weapon base, bp (`BP` = x1).
    pub power_bp: u32,
    pub source: Source,
    pub form: Form,
    /// Base chance of each packet's native state, bp.
    pub state_chance_bp: u32,
}

impl Skill {
    /// The weapon's basic attack.
    pub const fn basic(source: Source, form: Form, state_chance_bp: u32) -> Self {
        Self {
            aspects: [Aspect::Fire; SKILL_ASPECTS_MAX],
            aspect_count: 0,
            shares_bp: [0; SKILL_ASPECTS_MAX],
            power_bp: BP,
            source,
            form,
            state_chance_bp,
        }
    }

    /// Bases of the attack: the skill's aspects, or the weapon aspect for a basic attack.
    pub fn bases(&self, weapon: Aspect) -> ([Aspect; SKILL_ASPECTS_MAX], usize) {
        if self.aspect_count == 0 {
            ([weapon; SKILL_ASPECTS_MAX], 1)
        } else {
            (
                self.aspects,
                (self.aspect_count as usize).min(SKILL_ASPECTS_MAX),
            )
        }
    }
}

/// A damage packet, values scaled by `BP`.
#[cfg_attr(feature = "borsh", derive(BorshSerialize, BorshDeserialize))]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Packet {
    pub aspect: Aspect,
    pub min: u64,
    pub max: u64,
}

/// Up to one packet per aspect, in order: skill aspects first, then added-only aspects
/// in the order of the added slots.
#[cfg_attr(feature = "borsh", derive(BorshSerialize, BorshDeserialize))]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Packets {
    pub list: [Packet; PACKETS_MAX],
    pub count: u8,
}

impl Packets {
    pub fn as_slice(&self) -> &[Packet] {
        &self.list[..self.count as usize]
    }

    /// Adds a scaled range into the packet of its aspect, creating it if needed.
    pub fn add(&mut self, aspect: Aspect, min: u64, max: u64) {
        let count = self.count as usize;
        if let Some(p) = self.list[..count].iter_mut().find(|p| p.aspect == aspect) {
            p.min = p.min.saturating_add(min);
            p.max = p.max.saturating_add(max);
        } else if count < PACKETS_MAX {
            self.list[count] = Packet { aspect, min, max };
            self.count += 1;
        }
    }

    /// Hint for the player: minimums and maximums summed, rounded down (docs 4.1).
    pub fn total_range(&self) -> (u64, u64) {
        self.as_slice().iter().fold((0u64, 0u64), |(lo, hi), p| {
            (lo.saturating_add(p.min), hi.saturating_add(p.max))
        })
    }
}

/// Weapon base x power, split by shares, plus added damage. `max < min` is read as `max = min`.
pub fn build_packets(weapon: DamageRange, skill: &Skill, added: &[DamageRange]) -> Packets {
    let mut packets = Packets::default();
    let w_min = weapon.min;
    let w_max = weapon.max.max(weapon.min);
    if skill.aspect_count == 0 {
        packets.add(
            weapon.aspect,
            scale(w_min, skill.power_bp, BP),
            scale(w_max, skill.power_bp, BP),
        );
    } else {
        let (aspects, count) = skill.bases(weapon.aspect);
        for (&aspect, &share) in aspects.iter().zip(&skill.shares_bp).take(count) {
            packets.add(
                aspect,
                scale(w_min, skill.power_bp, share),
                scale(w_max, skill.power_bp, share),
            );
        }
    }
    for range in added.iter().filter(|r| !r.is_empty()) {
        let max = range.max.max(range.min);
        packets.add(
            range.aspect,
            range.min as u64 * BP as u64,
            max as u64 * BP as u64,
        );
    }
    packets
}

/// `amount * power * share / BP`, i.e. `amount` x power x share, scaled by `BP`.
fn scale(amount: u32, power_bp: u32, share_bp: u32) -> u64 {
    let v = amount as u128 * power_bp as u128 * share_bp as u128 / BP128;
    u64::try_from(v).unwrap_or(u64::MAX)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn skill(aspects: &[Aspect], shares: &[u32], power_bp: u32) -> Skill {
        let mut s = Skill::basic(Source::Magical, Form::Strike, 0);
        for (i, (a, sh)) in aspects.iter().zip(shares).enumerate() {
            s.aspects[i] = *a;
            s.shares_bp[i] = *sh;
        }
        s.aspect_count = aspects.len() as u8;
        s.power_bp = power_bp;
        s
    }

    #[test]
    fn basic_attack_keeps_weapon_aspect() {
        let w = DamageRange::new(Aspect::Piercing, 4, 7);
        let p = build_packets(w, &Skill::basic(Source::Combat, Form::Strike, 0), &[]);
        assert_eq!(p.count, 1);
        assert_eq!(
            p.list[0],
            Packet {
                aspect: Aspect::Piercing,
                min: 40_000,
                max: 70_000
            }
        );
    }

    #[test]
    fn conversion_by_shares_and_power() {
        // Lightning wand 5-8, skill Cold 60% + Lightning 40%, power x2.
        let w = DamageRange::new(Aspect::Lightning, 5, 8);
        let s = skill(&[Aspect::Cold, Aspect::Lightning], &[6_000, 4_000], 20_000);
        let p = build_packets(w, &s, &[]);
        assert_eq!(p.count, 2);
        assert_eq!(
            p.list[0],
            Packet {
                aspect: Aspect::Cold,
                min: 60_000,
                max: 96_000
            }
        );
        assert_eq!(
            p.list[1],
            Packet {
                aspect: Aspect::Lightning,
                min: 40_000,
                max: 64_000
            }
        );
        // Fractions are kept: 5 * 2 * 0.6 = 6.0, 8 * 2 * 0.4 = 6.4.
        assert_eq!(p.total_range(), (100_000, 160_000));
    }

    #[test]
    fn weapon_aspect_is_not_kept_when_converted() {
        let w = DamageRange::new(Aspect::Piercing, 3, 3);
        let s = skill(&[Aspect::Fire], &[BP], BP);
        let p = build_packets(w, &s, &[]);
        assert_eq!(
            p.as_slice(),
            &[Packet {
                aspect: Aspect::Fire,
                min: 30_000,
                max: 30_000
            }]
        );
    }

    #[test]
    fn added_damage_merges_by_aspect() {
        let w = DamageRange::new(Aspect::Blunt, 2, 4);
        let s = skill(&[Aspect::Cold], &[BP], BP);
        let added = [
            DamageRange::new(Aspect::Cold, 1, 2),
            DamageRange::new(Aspect::Piercing, 1, 3),
            DamageRange::default(),
        ];
        let p = build_packets(w, &s, &added);
        assert_eq!(p.count, 2);
        assert_eq!(
            p.list[0],
            Packet {
                aspect: Aspect::Cold,
                min: 30_000,
                max: 60_000
            }
        );
        assert_eq!(
            p.list[1],
            Packet {
                aspect: Aspect::Piercing,
                min: 10_000,
                max: 30_000
            }
        );
    }

    #[test]
    fn inverted_range_reads_as_flat() {
        let w = DamageRange::new(Aspect::Blunt, 6, 2);
        let p = build_packets(w, &Skill::basic(Source::Combat, Form::Strike, 0), &[]);
        assert_eq!(p.list[0].min, 60_000);
        assert_eq!(p.list[0].max, 60_000);
    }

    #[test]
    fn duplicate_skill_aspects_merge_and_overflow_saturates() {
        let w = DamageRange::new(Aspect::Fire, u32::MAX, u32::MAX);
        let s = skill(
            &[Aspect::Fire, Aspect::Fire],
            &[u32::MAX, u32::MAX],
            u32::MAX,
        );
        let p = build_packets(w, &s, &[]);
        assert_eq!(p.count, 1);
        assert_eq!(p.list[0].max, u64::MAX);
    }
}
