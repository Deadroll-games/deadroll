//! Patch D: map size classes and the multiplied limits (docs/MAP_DESCRIPTION.md 17.6).

use deadroll_mapcheck::limits::tier_limits;
use deadroll_rules::{loot::map_rules, BalanceConfig, MapSizeTuning};
use deadroll_types::{CLASS_CHESTS, CLASS_PACKS, CLASS_SECRETS};

const T: MapSizeTuning = MapSizeTuning::DEFAULT;

#[test]
fn the_class_follows_the_area() {
    assert_eq!(T.class_of(8, 8), Some(0));
    assert_eq!(T.class_of(32, 32), Some(0));
    assert_eq!(T.class_of(128, 8), Some(0), "the area, not the side");
    assert_eq!(T.class_of(33, 32), Some(1));
    assert_eq!(T.class_of(72, 72), Some(1));
    assert_eq!(T.class_of(73, 72), Some(2));
    assert_eq!(T.class_of(101, 101), Some(2));
    assert_eq!(T.class_of(102, 101), Some(3));
    assert_eq!(T.class_of(143, 143), Some(3));
    assert_eq!(T.class_of(144, 8), None);
    assert_eq!(T.class_of(7, 100), None);
    for c in 0..4u8 {
        assert!(T.scale(c).fits(8, 8));
    }
    assert!(!T.scale(0).fits(33, 32));
}

#[test]
fn every_limit_is_multiplied_within_the_class() {
    let base = tier_limits(2).unwrap();
    let dungeon = base.scaled(T.scale(3));
    assert_eq!(dungeon.packs, base.packs * 8);
    assert_eq!(dungeon.monsters, base.monsters * 8);
    assert_eq!(dungeon.elites, base.elites * 8);
    assert!((dungeon.packs as usize) < CLASS_PACKS[3]);
    let medium = base.scaled(T.scale(1));
    assert_eq!((medium.packs, medium.monsters), (12, 24));

    let cfg = BalanceConfig::DEFAULT;
    for tier in 1..=2u8 {
        let loot = map_rules(&cfg.loot, tier);
        for c in 0..4u8 {
            let scaled = T.loot(loot, c);
            let by = [1, 3, 5, 8][c as usize];
            assert_eq!(scaled.budget, loot.budget.map(|b| b * by));
            assert!(scaled.chests as usize <= CLASS_CHESTS[c as usize]);
            let secrets = T.secrets(cfg.secrets.limits(tier).unwrap(), c);
            assert_eq!(secrets.traps_min, cfg.secrets.traps_min[tier as usize - 1]);
            let total: usize = secrets.declared_max.iter().map(|d| *d as usize).sum();
            assert!(total <= CLASS_SECRETS[c as usize], "tier {tier} class {c}");
        }
        assert_eq!(T.loot(loot, 3).chests, loot.chests * 8);
    }
}

#[test]
fn all_ones_switch_a_multiplier_off() {
    let off = MapSizeTuning {
        packs: [1; 4],
        stash: [1; 4],
        budget: [1; 4],
        ..T
    };
    let base = tier_limits(1).unwrap();
    assert_eq!(base.scaled(off.scale(3)).packs, base.packs);
    assert_eq!(base.scaled(off.scale(3)).monsters, base.monsters * 8);
    let cfg = BalanceConfig::DEFAULT;
    let limits = cfg.secrets.limits(1).unwrap();
    assert_eq!(
        off.secrets(limits, 3).declared_max[2],
        limits.declared_max[2]
    );
    let loot = map_rules(&cfg.loot, 1);
    assert_eq!(off.loot(loot, 3).budget, loot.budget);
}
