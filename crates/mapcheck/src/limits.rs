//! Builder limits (docs/MAP_DESCRIPTION.md, sections 2.3 and 10.2). Starter values, tuned at
//! balancing.

use deadroll_types::MAP_MAX;

/// Smallest map side.
pub const SIDE_MIN: u8 = 8;
/// Largest map side.
pub const SIDE_MAX: u8 = MAP_MAX as u8;
/// Map tiers with limits defined.
pub const TIER_MIN: u8 = 1;
pub const TIER_MAX: u8 = 2;
/// Bosses per map.
pub const BOSSES: u8 = 1;
/// Helpers the boss starts combat with (docs/BESTIARY.md, section 6).
pub const BOSS_HELPERS: u8 = 2;
/// Unused since the arena update: a pack stands in its own arena, anywhere in it.
pub const PACK_RADIUS: u8 = 4;

/// Limits for one map tier.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TierLimits {
    /// Monster levels allowed on this tier, inclusive.
    pub level_min: u8,
    pub level_max: u8,
    /// Monsters outside the boss pack.
    pub monsters: u8,
    /// Packs outside the boss pack.
    pub packs: u8,
    /// Elite spawns (docs/BESTIARY.md 5: the limit per map is set by the map tier).
    pub elites: u8,
}

pub const fn tier_limits(tier: u8) -> Option<TierLimits> {
    match tier {
        1 => Some(TierLimits {
            level_min: 1,
            level_max: 2,
            monsters: 4,
            packs: 3,
            elites: 1,
        }),
        2 => Some(TierLimits {
            level_min: 3,
            level_max: 4,
            monsters: 8,
            packs: 4,
            elites: 2,
        }),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_tier_in_range_has_limits() {
        for tier in TIER_MIN..=TIER_MAX {
            let limits = tier_limits(tier).unwrap();
            assert!(limits.level_min <= limits.level_max);
            assert!(limits.packs <= limits.monsters);
        }
        assert_eq!(tier_limits(0), None);
        assert_eq!(tier_limits(TIER_MAX + 1), None);
    }
}
