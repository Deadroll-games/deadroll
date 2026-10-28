//! Every balance number of the crate, in one place.
//!
//! The design docs define only the logic, never the numbers. All values here are
//! placeholder, tune in sim (`tools/sim`); later they move to registry tables.

use deadroll_types::{StateKind, Tier};

/// Balance numbers used by [`crate::resolve_hit`] and the state helpers.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Balance {
    /// Crit "more" bonus: damage x `(BP + crit_more_bp) / BP`.
    pub crit_more_bp: u32,
    /// Armor constant `K`: physical damage x `K / (armor + K)`. Armor equal to `K` halves it.
    pub armor_k: u32,
    /// Resistance counterweight cap. 30_000 = at most a 75% cut, so no resistance is immunity.
    pub resist_cap_bp: u32,
    /// Every rolled chance is clamped into `[chance_min_pct, chance_max_pct]` (percent, 0..=100).
    pub chance_min_pct: u32,
    pub chance_max_pct: u32,
    /// Damage of a hit on a non-immune target is never below this.
    pub min_damage: u32,
    /// Tint chance by tier, percent, index `tier - 1` (tier 1 is best).
    pub tint_chance_pct: [u32; 14],
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
    /// Shock strength before the stat increase and the aspect resistance: a damage multiplier, bp.
    pub shock_vuln_bp: u32,
    /// Penetration strength before the stat increase and the aspect resistance: armor counterweight, bp.
    pub penetration_power_bp: u32,
    /// Keenness: armor penetration added when a Piercing tint's resonance triggers, by tier, bp.
    pub keenness_pen_bp: [u32; 14],
    /// Chance of a resonance's own state roll ("a chance of Stun"), percent, before the op's
    /// scale and the stat increase.
    pub extra_state_chance_pct: u32,
    /// Resonance ops (docs/DAMAGE_TYPES_AND_MIXES.md, section 6.4), by the tint's tier, index
    /// `tier - 1`. Each op scales its curve by the table value (`crate::tables`).
    /// Extra turns of the native state ("Burning lasts longer").
    pub res_turns: [u8; 14],
    /// Increase on the power of states a resonance makes or strengthens.
    pub res_power_bp: [u32; 14],
    /// Strength of a timed modifier a resonance applies (Weakening, Slow), bp.
    pub res_mod_bp: [u32; 14],
    /// Duration of that modifier in the bearer's own turns.
    pub res_mod_turns: u8,
    /// Addition from a tint trait (Fury): flat `[min, max]` of the trait's aspect.
    pub tint_added: [[u32; 2]; 14],
    /// Accuracy (the d20 bonus) of an accuracy buff (Pale Flame).
    pub res_accuracy: [u32; 14],
    /// Duration of the accuracy buff in the attacker's own turns.
    pub res_accuracy_turns: u8,
    /// Rounds a cell set on fire by a resonance keeps burning.
    pub burning_cell_rounds: u8,
    /// Patch 9b: the share of a target's resistance to an aspect that also counters the chance
    /// of that aspect's native state, bp (the state resistance counters it in full).
    pub aspect_state_resist_share_bp: u32,
    /// Exposed (a resonance's mark on the target): crit chance every attacker gets against it,
    /// percent, by the tint's tier, index `tier - 1`. Lasts `res_mod_turns`.
    pub res_exposed_pct: [u32; 14],
}

impl Balance {
    /// Placeholder, tune in sim.
    pub const DEFAULT: Balance = Balance {
        crit_more_bp: 5_000,
        armor_k: 100,
        resist_cap_bp: 30_000,
        chance_min_pct: 5,
        chance_max_pct: 95,
        min_damage: 1,
        // tier 1 .. tier 14, percent (the old bp table, divided by 100, half up)
        tint_chance_pct: [40, 37, 35, 32, 30, 27, 25, 22, 20, 17, 15, 12, 10, 7],
        resonance_more_bp: [
            3_600, 3_400, 3_200, 3_000, 2_800, 2_600, 2_400, 2_200, 2_000, 1_800, 1_600, 1_400,
            1_200, 1_000,
        ],
        pure_pair_bp: [
            5_900, 5_600, 5_300, 5_000, 4_700, 4_400, 4_100, 3_800, 3_500, 3_200, 2_900, 2_600,
            2_300, 2_000,
        ],
        // Burning, Chill, Freeze, Bleeding, Shock, Stun, Penetration
        state_turns: [3, 3, 1, 3, 2, 1, 2],
        state_turns_max: 5,
        dot_share_bp: 3_000,
        chill_buildup_bp: 5_000,
        freeze_threshold_bp: 10_000,
        chill_slow_share_bp: 10_000,
        shock_vuln_bp: 2_000,
        penetration_power_bp: 5_000,
        // Same shape as `resonance_more_bp` until the bench tunes it.
        keenness_pen_bp: [
            3_600, 3_400, 3_200, 3_000, 2_800, 2_600, 2_400, 2_200, 2_000, 1_800, 1_600, 1_400,
            1_200, 1_000,
        ],
        extra_state_chance_pct: 30,
        res_turns: [2, 2, 2, 2, 2, 1, 1, 1, 1, 1, 1, 1, 1, 1],
        res_power_bp: [
            5_900, 5_600, 5_300, 5_000, 4_700, 4_400, 4_100, 3_800, 3_500, 3_200, 2_900, 2_600,
            2_300, 2_000,
        ],
        res_mod_bp: [
            5_000, 4_750, 4_500, 4_250, 4_000, 3_750, 3_500, 3_250, 3_000, 2_750, 2_500, 2_250,
            2_000, 1_750,
        ],
        res_mod_turns: 2,
        // Same shape as the race trait's added damage until the bench tunes it.
        tint_added: [
            [4, 8],
            [4, 7],
            [4, 7],
            [3, 6],
            [3, 6],
            [3, 5],
            [3, 5],
            [2, 4],
            [2, 4],
            [2, 3],
            [2, 3],
            [1, 3],
            [1, 2],
            [1, 2],
        ],
        res_accuracy: [4, 4, 3, 3, 3, 3, 2, 2, 2, 2, 1, 1, 1, 1],
        res_accuracy_turns: 2,
        burning_cell_rounds: 2,
        aspect_state_resist_share_bp: 2_500,
        res_exposed_pct: [20, 19, 18, 17, 16, 15, 14, 13, 12, 11, 10, 9, 8, 7],
    };

    pub const fn tint_chance(&self, tier: Tier) -> u32 {
        self.tint_chance_pct[tier.get() as usize - 1]
    }

    pub const fn keenness_pen(&self, tier: Tier) -> u32 {
        self.keenness_pen_bp[tier.get() as usize - 1]
    }

    pub const fn resonance_more(&self, tier: Tier) -> u32 {
        self.resonance_more_bp[tier.get() as usize - 1]
    }

    pub const fn res_turns(&self, tier: Tier) -> u8 {
        self.res_turns[tier.get() as usize - 1]
    }

    pub const fn res_power(&self, tier: Tier) -> u32 {
        self.res_power_bp[tier.get() as usize - 1]
    }

    pub const fn res_mod(&self, tier: Tier) -> u32 {
        self.res_mod_bp[tier.get() as usize - 1]
    }

    pub const fn tint_added(&self, tier: Tier) -> [u32; 2] {
        self.tint_added[tier.get() as usize - 1]
    }

    pub const fn res_accuracy(&self, tier: Tier) -> u32 {
        self.res_accuracy[tier.get() as usize - 1]
    }

    pub const fn res_exposed(&self, tier: Tier) -> u32 {
        self.res_exposed_pct[tier.get() as usize - 1]
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

    /// Clamp a chance into `[chance_min_pct, chance_max_pct]`.
    pub const fn clamp_chance(&self, pct: u32) -> u32 {
        if pct < self.chance_min_pct {
            self.chance_min_pct
        } else if pct > self.chance_max_pct {
            self.chance_max_pct
        } else {
            pct
        }
    }
}

impl Default for Balance {
    fn default() -> Self {
        Self::DEFAULT
    }
}

const _: () = assert!(Balance::DEFAULT.chance_max_pct <= 100);
const _: () = assert!(Balance::DEFAULT.armor_k > 0);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tier_tables_improve_with_better_tier() {
        let b = Balance::DEFAULT;
        for i in 0..13 {
            assert!(b.tint_chance_pct[i] > b.tint_chance_pct[i + 1]);
            assert!(b.resonance_more_bp[i] > b.resonance_more_bp[i + 1]);
            assert!(b.pure_pair_bp[i] > b.pure_pair_bp[i + 1]);
            assert!(b.keenness_pen_bp[i] > b.keenness_pen_bp[i + 1]);
            assert!(b.res_power_bp[i] > b.res_power_bp[i + 1]);
            assert!(b.res_mod_bp[i] > b.res_mod_bp[i + 1]);
            assert!(b.res_turns[i] >= b.res_turns[i + 1]);
            assert!(b.res_accuracy[i] >= b.res_accuracy[i + 1]);
            assert!(b.res_exposed_pct[i] > b.res_exposed_pct[i + 1]);
            assert!(b.tint_added[i][1] >= b.tint_added[i + 1][1]);
        }
        assert_eq!(b.tint_chance(Tier::BEST), 40);
        assert_eq!(b.tint_chance(Tier::WORST), 7);
        assert_eq!(b.turns(StateKind::Penetration), 2);
    }

    #[test]
    fn clamp_chance_bounds() {
        let b = Balance::DEFAULT;
        assert_eq!(b.clamp_chance(0), 5);
        assert_eq!(b.clamp_chance(50), 50);
        assert_eq!(b.clamp_chance(100), 95);
    }

    #[test]
    fn turns_capped() {
        let mut b = Balance::DEFAULT;
        b.state_turns[StateKind::Burning as usize] = 200;
        assert_eq!(b.turns(StateKind::Burning), b.state_turns_max);
        assert_eq!(b.turns(StateKind::Freeze), 1);
    }
}
