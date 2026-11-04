//! The history counters add up: they only read what a fight did.
use std::path::PathBuf;

use deadroll_rules::BalanceConfig;
use deadroll_sim::{
    fight, presets::Kind, resolve_fight, resolve_party, run_batch, Presets, Progress, Settings,
};
use deadroll_types::Aspect;

fn shipped() -> Presets {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tools/balance/presets");
    let (p, errors) = Presets::load(&root);
    assert!(errors.is_empty(), "{errors:?}");
    p
}

fn parties() -> Vec<Vec<String>> {
    [
        &["human_warrior", "elf_mage"][..],
        &["human_warrior"],
        &["elf_mage", "human_warrior", "human_warrior"],
    ]
    .iter()
    .map(|ids| ids.iter().map(|s| s.to_string()).collect())
    .collect()
}

#[test]
fn aspect_shares_skills_and_monsters_add_up() {
    let p = shipped();
    let cfg = BalanceConfig::DEFAULT;
    let mut checked = 0;
    for ids in parties() {
        let (team, _) = resolve_party(&p, &ids, &cfg);
        for enc in p.ids(Kind::Encounters) {
            let (f, _) = resolve_fight(&p, &enc, &cfg);
            let Some(f) = f else { continue };
            for seed in 0..25 {
                let s = fight(&cfg, &team, &f, Settings::default(), seed);
                for (i, slot) in s.slots.iter().enumerate() {
                    let ctx = format!("{ids:?} {enc} seed {seed} slot {i}");
                    assert_eq!(slot.by_aspect.iter().sum::<u64>(), slot.damage, "{ctx}");
                    assert_eq!(slot.attacks, slot.hits + slot.misses, "{ctx}");
                    let tallies = &slot.skills;
                    assert_eq!(
                        slot.hits,
                        tallies.iter().map(|t| t.hits).sum::<u32>(),
                        "{ctx}"
                    );
                    assert_eq!(
                        slot.misses,
                        tallies.iter().map(|t| t.misses).sum::<u32>(),
                        "{ctx}"
                    );
                    assert_eq!(
                        slot.crits,
                        tallies.iter().map(|t| t.crits).sum::<u32>(),
                        "{ctx}"
                    );
                    assert!(slot.largest_hit as u64 <= slot.hit_damage, "{ctx}");
                    assert!(slot.hit_damage <= slot.damage, "{ctx}");
                }
                let dealt: u64 = s.monsters.values().map(|m| m.damage_dealt).sum();
                let taken: u64 = s.slots.iter().map(|x| x.damage_taken).sum();
                assert!(dealt <= taken, "{ids:?} {enc} seed {seed}");
                let hurt: u64 = s.monsters.values().map(|m| m.damage_taken).sum();
                let done: u64 = s.slots.iter().map(|x| x.damage).sum();
                assert!(hurt <= done, "{ids:?} {enc} seed {seed}");
                let died: u32 = s.monsters.values().map(|m| m.died).sum();
                assert_eq!(died as usize, s.kills.len());
                let spawned: u32 = s.monsters.values().map(|m| m.spawned).sum();
                assert!(spawned >= died);
                checked += 1;
            }
        }
    }
    assert!(checked > 100);
}

#[test]
fn a_batch_report_carries_the_breakdowns() {
    let p = shipped();
    let cfg = BalanceConfig::DEFAULT;
    let ids = vec!["human_warrior".to_string(), "elf_mage".to_string()];
    let (team, _) = resolve_party(&p, &ids, &cfg);
    let (f, _) = resolve_fight(&p, "goblins", &cfg);
    let r = run_batch(
        &cfg,
        &team,
        &f.unwrap(),
        Settings::default(),
        200,
        5,
        &Progress::default(),
    )
    .unwrap();
    for slot in &r.slots {
        assert_eq!(slot.by_aspect.iter().sum::<u64>(), slot.damage);
        assert!(slot.skills.iter().any(|s| s.uses > 0), "{}", slot.name);
    }
    assert!(!r.monsters.is_empty());
    assert!(r.monsters.iter().all(|m| !m.name.is_empty()));
    assert_eq!(r.slots[0].by_aspect.len(), Aspect::COUNT);
}
