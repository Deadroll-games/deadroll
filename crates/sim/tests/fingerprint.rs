//! New counters must never change a fight: the same seeds give the same outcomes. The literal
//! below was recorded before the history counters were added; it covers every shipped encounter
//! and a few parties, and hashes every number a fight already reported.
use std::path::PathBuf;

use deadroll_rules::BalanceConfig;
use deadroll_sim::{
    fight, presets::Kind, resolve_fight, resolve_party, BattleStats, Outcome, Presets, Settings,
};

fn shipped() -> Presets {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tools/balance/presets");
    let (p, errors) = Presets::load(&root);
    assert!(errors.is_empty(), "{errors:?}");
    p
}

struct Fnv(u64);

impl Fnv {
    fn put(&mut self, v: u64) {
        for byte in v.to_le_bytes() {
            self.0 ^= byte as u64;
            self.0 = self.0.wrapping_mul(0x0000_0100_0000_01B3);
        }
    }
}

fn feed(h: &mut Fnv, s: &BattleStats) {
    h.put(match s.outcome {
        Outcome::Victory => 1,
        Outcome::Defeat => 2,
        Outcome::Draw => 3,
    });
    h.put(s.rounds as u64);
    for x in &s.slots {
        for v in [
            x.damage,
            x.crit_extra,
            x.dot_damage,
            x.healed,
            x.turns as u64,
            x.attacks as u64,
            x.hits as u64,
            x.crits as u64,
            x.effects_landed as u64,
            x.died as u64,
        ] {
            h.put(v);
        }
    }
    for k in &s.kills {
        h.put(*k as u64);
    }
    for v in [
        s.monster_turns,
        s.monster_turns_skipped,
        s.resonances,
        s.triples,
        s.jumps,
        s.knockbacks,
        s.cells_ignited,
        s.burned_by_cells,
        s.reactions,
        s.line_extra_hits,
        s.aura_reach,
        s.monster_skills,
        s.monster_reactions,
        s.packs,
        s.packs_won,
        s.xp,
    ] {
        h.put(v as u64);
    }
    for v in s.states_landed.iter().chain(s.ops_acted.iter()) {
        h.put(*v as u64);
    }
    h.put(s.monster_healed);
    h.put(s.hp_left);
    h.put(s.hp_max);
}

#[test]
fn fights_of_fixed_seeds_stay_the_same() {
    let p = shipped();
    let cfg = BalanceConfig::DEFAULT;
    let parties: [&[&str]; 3] = [
        &["human_warrior", "elf_mage"],
        &["human_warrior"],
        &["elf_mage", "human_warrior", "human_warrior"],
    ];
    let mut h = Fnv(0xcbf2_9ce4_8422_2325);
    let mut fights = 0;
    for ids in parties {
        let ids: Vec<String> = ids.iter().map(|s| s.to_string()).collect();
        let (team, _) = resolve_party(&p, &ids, &cfg);
        for enc in p.ids(Kind::Encounters) {
            let (f, _) = resolve_fight(&p, &enc, &cfg);
            let Some(f) = f else { continue };
            for seed in 0..40 {
                feed(&mut h, &fight(&cfg, &team, &f, Settings::default(), seed));
                fights += 1;
            }
        }
    }
    println!("fights {fights} fingerprint {:#018x}", h.0);
    assert_eq!((fights, h.0), (EXPECTED_FIGHTS, EXPECTED));
}

const EXPECTED_FIGHTS: u32 = 720;
const EXPECTED: u64 = 0x94ab_851c_c5d9_55e1;
