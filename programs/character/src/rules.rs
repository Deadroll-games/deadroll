//! Creation and growth rules (docs/CHARACTER.md, sections 2, 4, 5, 6.3, 7).
//! Creation and HP numbers come from `deadroll_rules::PlayerTuning::DEFAULT` (shared with the
//! session program and `tools/balance`); the rest marked BALANCE are placeholders.

use deadroll_rules::{player, BalanceConfig, PlayerTuning};
use deadroll_types::{Aspect, Class, Race, STATS_COUNT};

/// The balance table the program plays with (item numbers and starter kits: `BALANCE.items`).
pub const BALANCE: &BalanceConfig = &BalanceConfig::DEFAULT;
/// Creation and HP numbers.
const TUNING: &PlayerTuning = &BALANCE.players;

/// Account layout version written by this program.
pub const ACCOUNT_VERSION: u8 = 1;
/// `Character` layout version: 2 has `gear` after `nft_frozen`.
pub const CHARACTER_VERSION: u8 = 2;

/// Every stat starts here before the player spends points.
pub const STAT_BASE: u16 = TUNING.stat_base;
/// Points the player distributes at creation.
pub const STAT_POINTS_START: u16 = TUNING.stat_points_start;
/// Highest value of one stat at creation.
pub const STAT_MAX_START: u16 = TUNING.stat_max_start;
/// Stat points per level gained. BALANCE.
pub const STAT_POINTS_PER_LEVEL: u8 = 2;

/// HP: class base + Endurance x per point + per level, indexed by `Class`.
pub const CLASS_HP_BASE: [u32; Class::COUNT] = TUNING.hp_base;
pub const HP_PER_ENDURANCE: u32 = TUNING.hp_per_endurance;
pub const CLASS_HP_PER_LEVEL: [u32; Class::COUNT] = TUNING.hp_per_level;

/// Total XP needed for each level, levels 1..=4 in the first version (section 5). BALANCE.
pub const LEVEL_XP: [u32; 4] = [0, 100, 300, 600];
pub const LEVEL_MAX: u16 = LEVEL_XP.len() as u16;

/// Race and class trait tiers start low and only grow deterministically (section 6.3).
pub const START_TIER: u8 = TUNING.start_tier;

/// Section 7.1: humans pick a physical aspect, forest elves an elemental one.
pub const fn predisposition_allowed(race: Race, aspect: Aspect) -> bool {
    player::predisposition_allowed(race, aspect)
}

/// Each stat in `STAT_BASE..=STAT_MAX_START` and exactly `STAT_POINTS_START` points spent.
pub fn stats_valid(stats: &[u16; STATS_COUNT]) -> bool {
    player::stats_valid(stats, TUNING)
}

pub fn hp_max(class: Class, stats: &[u16; STATS_COUNT], level: u16) -> u32 {
    player::hp_max(class, stats, level, TUNING)
}

/// The level reached with `xp` total experience.
pub fn level_for(xp: u32) -> u16 {
    LEVEL_XP.iter().take_while(|need| xp >= **need).count() as u16
}

/// Name rules: printable ASCII, not empty, zero-padded after the text (one canonical encoding).
pub fn name_valid(name: &[u8]) -> bool {
    let len = name.iter().position(|b| *b == 0).unwrap_or(name.len());
    len > 0
        && name[..len].iter().all(|b| (0x20..=0x7e).contains(b))
        && name[0] != b' '
        && name[len..].iter().all(|b| *b == 0)
}

/// The text part of a zero-padded name.
pub fn name_str(name: &[u8]) -> &str {
    let len = name.iter().position(|b| *b == 0).unwrap_or(name.len());
    core::str::from_utf8(&name[..len]).unwrap_or("")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn levels() {
        assert_eq!(level_for(0), 1);
        assert_eq!(level_for(99), 1);
        assert_eq!(level_for(100), 2);
        assert_eq!(level_for(600), 4);
        assert_eq!(level_for(u32::MAX), LEVEL_MAX);
    }

    #[test]
    fn stats() {
        assert!(stats_valid(&[10, 5, 5, 10]));
        assert!(stats_valid(&[15, 5, 5, 5]));
        assert!(!stats_valid(&[16, 5, 4, 5]));
        assert!(!stats_valid(&[10, 5, 5, 9]));
        assert!(!stats_valid(&[10, 5, 5, 11]));
    }

    #[test]
    fn names() {
        let mut n = [0u8; 8];
        n[..4].copy_from_slice(b"Bran");
        assert!(name_valid(&n));
        assert_eq!(name_str(&n), "Bran");
        assert!(name_valid(b"Full8chr"));
        assert!(!name_valid(&[0u8; 8]));
        assert!(!name_valid(b" Leading"));
        n[6] = b'x';
        assert!(!name_valid(&n));
        assert!(!name_valid(&[b'a', 0xc3, 0xa9, 0, 0]));
    }

    #[test]
    fn predispositions() {
        assert!(predisposition_allowed(Race::Human, Aspect::Piercing));
        assert!(!predisposition_allowed(Race::Human, Aspect::Cold));
        assert!(predisposition_allowed(Race::ForestElf, Aspect::Cold));
        assert!(!predisposition_allowed(Race::ForestElf, Aspect::Blood));
    }
}
