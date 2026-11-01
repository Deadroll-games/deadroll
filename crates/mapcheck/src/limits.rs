//! Builder limits (docs/MAP_DESCRIPTION.md, sections 2.3 and 10.2). Starter values, tuned at
//! balancing.

use deadroll_types::{CLASS_PACKS, MAP_CLASSES, MAP_MAX, MAP_SIDE_CAP, MONSTERS_CAP};

/// Smallest map side.
pub const SIDE_MIN: u8 = 8;
/// Largest side of a Small map (patch D: the base class).
pub const SIDE_MAX: u8 = MAP_MAX as u8;
/// Patch D: largest map side of any class (docs/MAP_DESCRIPTION.md 17.6).
pub const SIDE_CAP: u8 = MAP_SIDE_CAP as u8;
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

/// Patch D: a map's size class and what it multiplies the Small limits by
/// (`deadroll_rules::BalanceConfig.map_size`, docs/MAP_DESCRIPTION.md 17.6).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SizeScale {
    /// `0` Small, `1` Medium, `2` Big, `3` Dungeon.
    pub class: u8,
    /// Most cells of this class.
    pub max_cells: u16,
    pub monsters: u8,
    pub packs: u8,
    pub elites: u8,
}

impl SizeScale {
    /// The Small class of the starter numbers: the limits as they are.
    pub const SMALL: SizeScale = SizeScale {
        class: 0,
        max_cells: (SIDE_MAX as u16) * (SIDE_MAX as u16),
        monsters: 1,
        packs: 1,
        elites: 1,
    };

    /// The map's sides are within the limits and its area within the class.
    pub const fn fits(&self, width: u8, height: u8) -> bool {
        width >= SIDE_MIN
            && height >= SIDE_MIN
            && width <= SIDE_CAP
            && height <= SIDE_CAP
            && (self.class as usize) < MAP_CLASSES
            && (width as u32) * (height as u32) <= self.max_cells as u32
    }
}

const fn times(base: u8, by: u8, cap: usize) -> u8 {
    let v = base as usize * by as usize;
    let v = if v > cap { cap } else { v };
    if v > u8::MAX as usize {
        u8::MAX
    } else {
        v as u8
    }
}

impl TierLimits {
    /// The limits of a map of size `scale`, within what its class stores: the boss pack and its
    /// helpers come on top of the regular packs and monsters.
    pub const fn scaled(self, scale: SizeScale) -> TierLimits {
        let class = if (scale.class as usize) < MAP_CLASSES {
            scale.class as usize
        } else {
            MAP_CLASSES - 1
        };
        TierLimits {
            level_min: self.level_min,
            level_max: self.level_max,
            monsters: times(
                self.monsters,
                scale.monsters,
                MONSTERS_CAP - 1 - BOSS_HELPERS as usize,
            ),
            packs: times(
                self.packs,
                scale.packs,
                CLASS_PACKS[class] - BOSSES as usize,
            ),
            elites: times(self.elites, scale.elites, MONSTERS_CAP),
        }
    }
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

    #[test]
    fn scaled_limits_stay_within_the_class() {
        let base = tier_limits(2).unwrap();
        assert_eq!(base.scaled(SizeScale::SMALL), base);
        let dungeon = SizeScale {
            class: 3,
            max_cells: 20449,
            monsters: 8,
            packs: 8,
            elites: 8,
        };
        let big = base.scaled(dungeon);
        assert_eq!((big.monsters, big.packs, big.elites), (64, 32, 16));
        let huge = base.scaled(SizeScale {
            packs: 50,
            monsters: 50,
            ..dungeon
        });
        assert_eq!(huge.packs as usize, CLASS_PACKS[3] - 1);
        assert_eq!(huge.monsters as usize, MONSTERS_CAP - 3);
        assert!(dungeon.fits(143, 143));
        assert!(!dungeon.fits(144, 100));
        assert!(!dungeon.fits(7, 100));
        assert!(SizeScale::SMALL.fits(32, 32));
        assert!(!SizeScale::SMALL.fits(33, 32));
        assert!(SizeScale::SMALL.fits(128, 8));
    }
}
