//! Dice. Every roll of an action comes from one [`RollSource`] (CLAUDE.md, hard rule 5).
//!
//! Production rolls use [`VrfRoll`]: one VRF value per action, roll number `i` is the first
//! 8 bytes (little endian) of `sha256(seed ‖ i as u32 LE)`. The website replays a hit from the
//! seed in the `HitResolved` event with the same code (WASM).

use deadroll_types::BP;
use sha2::{Digest, Sha256};

/// A stream of uniform `u64` values, one per roll of the current action.
pub trait RollSource {
    fn next_u64(&mut self) -> u64;
}

/// The production source, built from the VRF value of one action.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct VrfRoll {
    pub seed: [u8; 32],
    /// Number of the next roll.
    pub index: u32,
}

impl VrfRoll {
    pub const fn new(seed: [u8; 32]) -> Self {
        Self { seed, index: 0 }
    }

    /// Roll number `index` for `seed`, without a stream.
    pub fn roll_at(seed: &[u8; 32], index: u32) -> u64 {
        let mut hasher = Sha256::new();
        hasher.update(seed);
        hasher.update(index.to_le_bytes());
        let digest = hasher.finalize();
        let mut first = [0u8; 8];
        first.copy_from_slice(&digest[..8]);
        u64::from_le_bytes(first)
    }
}

impl RollSource for VrfRoll {
    fn next_u64(&mut self) -> u64 {
        let value = Self::roll_at(&self.seed, self.index);
        self.index = self.index.wrapping_add(1);
        value
    }
}

/// splitmix64 for tests and the simulator. Never used on-chain.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SeededRoll {
    pub state: u64,
}

impl SeededRoll {
    pub const fn new(seed: u64) -> Self {
        Self { state: seed }
    }
}

impl RollSource for SeededRoll {
    fn next_u64(&mut self) -> u64 {
        self.state = self.state.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.state;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }
}

/// Fixed answers in order, then zeros. For unit tests that need exact dice.
#[derive(Clone, Copy, Debug)]
pub struct ScriptedRoll<const N: usize> {
    pub values: [u64; N],
    pub index: usize,
}

impl<const N: usize> ScriptedRoll<N> {
    pub const fn new(values: [u64; N]) -> Self {
        Self { values, index: 0 }
    }
}

impl<const N: usize> RollSource for ScriptedRoll<N> {
    fn next_u64(&mut self) -> u64 {
        let value = self.values.get(self.index).copied().unwrap_or(0);
        self.index += 1;
        value
    }
}

/// `1..=20`.
pub fn d20(src: &mut impl RollSource) -> u8 {
    (src.next_u64() % 20) as u8 + 1
}

/// `0..=100`: a point in a range, 0 = minimum, 100 = maximum.
pub fn d100(src: &mut impl RollSource) -> u8 {
    (src.next_u64() % 101) as u8
}

/// A chance check in basis points. Returns `(success, roll)` with `roll` in `0..BP`;
/// success when `roll < chance_bp`.
pub fn chance(src: &mut impl RollSource, chance_bp: u32) -> (bool, u16) {
    let roll = (src.next_u64() % BP as u64) as u16;
    ((roll as u32) < chance_bp, roll)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn vrf_roll_is_sha256_of_seed_and_index() {
        // sha256([0; 32] ‖ 00 00 00 00), first 8 bytes LE.
        let mut src = VrfRoll::new([0; 32]);
        let a = src.next_u64();
        let b = src.next_u64();
        assert_eq!(src.index, 2);
        assert_eq!(a, VrfRoll::roll_at(&[0; 32], 0));
        assert_eq!(b, VrfRoll::roll_at(&[0; 32], 1));
        assert_ne!(a, b);
        let mut hasher = Sha256::new();
        hasher.update([0u8; 36]);
        let digest = hasher.finalize();
        assert_eq!(a.to_le_bytes(), digest[..8]);
    }

    #[test]
    fn vrf_roll_depends_on_seed() {
        assert_ne!(VrfRoll::roll_at(&[0; 32], 0), VrfRoll::roll_at(&[1; 32], 0));
    }

    #[test]
    fn seeded_roll_is_deterministic() {
        let mut a = SeededRoll::new(42);
        let mut b = SeededRoll::new(42);
        for _ in 0..100 {
            assert_eq!(a.next_u64(), b.next_u64());
        }
        // splitmix64 reference value for seed 0.
        assert_eq!(SeededRoll::new(0).next_u64(), 0xE220_A839_7B1D_CDAF);
    }

    #[test]
    fn dice_ranges() {
        let mut src = SeededRoll::new(1);
        let (mut seen20, mut seen1, mut seen0, mut seen100) = (false, false, false, false);
        for _ in 0..20_000 {
            let d = d20(&mut src);
            assert!((1..=20).contains(&d));
            seen1 |= d == 1;
            seen20 |= d == 20;
            let p = d100(&mut src);
            assert!(p <= 100);
            seen0 |= p == 0;
            seen100 |= p == 100;
        }
        assert!(seen1 && seen20 && seen0 && seen100);
    }

    #[test]
    fn dice_edges_from_script() {
        let mut src = ScriptedRoll::new([0, 19, 20, 0, 100, 101]);
        assert_eq!(d20(&mut src), 1);
        assert_eq!(d20(&mut src), 20);
        assert_eq!(d20(&mut src), 1);
        assert_eq!(d100(&mut src), 0);
        assert_eq!(d100(&mut src), 100);
        assert_eq!(d100(&mut src), 0);
    }

    #[test]
    fn chance_bounds() {
        let mut src = ScriptedRoll::new([0, 9_999, 2_499, 2_500]);
        assert_eq!(chance(&mut src, 0), (false, 0));
        assert_eq!(chance(&mut src, BP), (true, 9_999));
        assert_eq!(chance(&mut src, 2_500), (true, 2_499));
        assert_eq!(chance(&mut src, 2_500), (false, 2_500));
    }
}
