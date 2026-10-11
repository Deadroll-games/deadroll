//! Every balance number of the crate, in one place.
//!
//! The design docs define only the logic, never the numbers. All values here are
//! placeholder, tune in sim (`tools/sim`); later they move to registry tables.

use deadroll_types::{StateKind, Tier, BP};

/// Balance numbers used by [`crate::resolve_hit`] and the state helpers.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Balance {
    /// Crit "more" bonus: damage x `(BP + crit_more_bp) / BP`.
    pub crit_more_bp: u32,
    /// Armor constant `K`: physical damage x `K / (armor + K)`. Armor equal to `K` halves it.
    pub armor_k: u32,
    /// Resistance counterweight cap. 30_000 = at most a 75% cut, so no resistance is immunity.
    pub resist_cap_bp: u32,
    /// Every rolled chance is clamped into `[chance_min_bp, chance_max_bp]`.
    pub chance_min_bp: u32,
    pub chance_max_bp: u32,
    /// Damage of a hit on a non-immune target is never below this.
    pub min_damage: u32,
    /// Tint chance by tier, index `tier - 1` (tier 1 is best).
    pub tint_chance_bp: [u32; 14],
    /// Generic resonance "more" bonus by the tint's tier, index `tier - 1`.
    pub resonance_more_bp: [u32; 14],
    /// Pure pair (tint == base): "increase" bonus to the base's native state chance and power,
    /// by the tint's tier, index `tier - 1`.
    pub pure_pair_bp: [u32; 14],
    /// Duration in the target's turns, indexed by `StateKind`.
    pub state_turns: [u8; StateKind::COUNT],
    /// Cap on any state duration.
    pub state_turns_max: u8,
    /// Damage over time per turn as a share of the packet damage that applied it.
    pub dot_share_bp: u32,
    /// Chill buildup added by one successful Cold application.
    pub chill_buildup_bp: u32,
    /// Buildup at which Chill turns into Freeze.
    pub freeze_threshold_bp: u32,
    /// Speed counterweight per point of Chill buildup, in bp of the buildup (10_000 = buildup itself).
    pub chill_slow_share_bp: u32,
}

impl Balance {
    /// Placeholder, tune in sim.
    pub const DEFAULT: Balance = Balance {
        crit_more_bp: 5_000,
        armor_k: 100,
        resist_cap_bp: 30_000,
        chance_min_bp: 500,
        chance_max_bp: 9_500,
        min_damage: 1,
        // tier 1 .. tier 14
        tint_chance_bp: [
            3_950, 3_700, 3_450, 3_200, 2_950, 2_700, 2_450, 2_200, 1_950, 1_700, 1_450, 1_200,
            950, 700,
        ],
        resonance_more_bp: [
            3_600, 3_400, 3_200, 3_000, 2_800, 2_600, 2_400, 2_200, 2_000, 1_800, 1_600, 1_400,
            1_200, 1_000,
        ],
        pure_pair_bp: [
            5_900, 5_600, 5_300, 5_000, 4_700, 4_400, 4_100, 3_800, 3_500, 3_200, 2_900, 2_600,
            2_300, 2_000,
        ],
        // Burning, Chill, Freeze, Bleeding, Shock, Stun
        state_turns: [3, 3, 1, 3, 2, 1],
        state_turns_max: 5,
        dot_share_bp: 3_000,
        chill_buildup_bp: 5_000,
        freeze_threshold_bp: 10_000,
        chill_slow_share_bp: 10_000,
    };

    pub const fn tint_chance(&self, tier: Tier) -> u32 {
        self.tint_chance_bp[tier.get() as usize - 1]
    }

    pub const fn resonance_more(&self, tier: Tier) -> u32 {
        self.resonance_more_bp[tier.get() as usize - 1]
    }

    pub const fn pure_pair(&self, tier: Tier) -> u32 {
        self.pure_pair_bp[tier.get() as usize - 1]
    }

    pub const fn turns(&self, kind: StateKind) -> u8 {
        let turns = self.state_turns[kind as usize];
        if turns > self.state_turns_max {
            self.state_turns_max
        } else {
            turns
        }
    }

    /// Clamp a chance into `[chance_min_bp, chance_max_bp]`.
    pub const fn clamp_chance(&self, bp: u32) -> u32 {
        if bp < self.chance_min_bp {
            self.chance_min_bp
        } else if bp > self.chance_max_bp {
            self.chance_max_bp
        } else {
            bp
        }
    }
}

impl Default for Balance {
    fn default() -> Self {
        Self::DEFAULT
    }
}

const _: () = assert!(Balance::DEFAULT.chance_max_bp <= BP);
const _: () = assert!(Balance::DEFAULT.armor_k > 0);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tier_tables_improve_with_better_tier() {
        let b = Balance::DEFAULT;
        for i in 0..13 {
            assert!(b.tint_chance_bp[i] > b.tint_chance_bp[i + 1]);
            assert!(b.resonance_more_bp[i] > b.resonance_more_bp[i + 1]);
            assert!(b.pure_pair_bp[i] > b.pure_pair_bp[i + 1]);
        }
        assert_eq!(b.tint_chance(Tier::BEST), 3_950);
        assert_eq!(b.tint_chance(Tier::WORST), 700);
    }

    #[test]
    fn clamp_chance_bounds() {
        let b = Balance::DEFAULT;
        assert_eq!(b.clamp_chance(0), 500);
        assert_eq!(b.clamp_chance(5_000), 5_000);
        assert_eq!(b.clamp_chance(BP), 9_500);
    }

    #[test]
    fn turns_capped() {
        let mut b = Balance::DEFAULT;
        b.state_turns[StateKind::Burning as usize] = 200;
        assert_eq!(b.turns(StateKind::Burning), b.state_turns_max);
        assert_eq!(b.turns(StateKind::Freeze), 1);
    }
}
