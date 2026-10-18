//! The balance report attached to a PR that changes balance (docs/ARCHITECTURE.md, section 5.1):
//! two parties against the shipped encounters, fixed seeds, Markdown on stdout.
//!
//! cargo run -p deadroll-sim --release --example report
use std::path::PathBuf;

use deadroll_rules::BalanceConfig;
use deadroll_sim::{resolve_fight, resolve_party, run_batch, Presets, Progress, Settings};
use deadroll_types::StateKind;

const FIGHTS: u32 = 4_000;
const SEED: u64 = 7;
const ENCOUNTERS: [&str; 4] = ["wolves", "goblins", "chieftain", "demo_map"];

fn main() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tools/balance/presets");
    let (presets, errors) = Presets::load(&root);
    assert!(errors.is_empty(), "{errors:?}");
    let cfg = BalanceConfig::DEFAULT;
    let parties: [&[&str]; 2] = [
        &["human_warrior", "elf_mage"],
        &["human_warrior", "elf_mage", "human_support"],
    ];
    println!("{FIGHTS} fights per row, seed {SEED}.\n");
    println!(
        "| party | encounter | win % | rounds p50 | hits to kill p50 | party dpt mean | crit share | dot share | control | states / fight | resonances / fight | reactives / fight | line extra / fight | aura reach / fight | monster skills / fight | monster heal / fight | monster reactives / fight |"
    );
    println!("|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|");
    for party in parties {
        let ids: Vec<String> = party.iter().map(|s| s.to_string()).collect();
        let (team, issues) = resolve_party(&presets, &ids, &cfg);
        assert!(issues.errors.is_empty(), "{:?}", issues.errors);
        for enc in ENCOUNTERS {
            let (fight, issues) = resolve_fight(&presets, enc, &cfg);
            assert!(issues.errors.is_empty(), "{:?}", issues.errors);
            let r = run_batch(
                &cfg,
                &team,
                &fight.expect(enc),
                Settings::default(),
                FIGHTS,
                SEED,
                &Progress::default(),
            )
            .expect("not cancelled");
            let states: Vec<String> = StateKind::ALL
                .iter()
                .zip(r.states_per_fight)
                .filter(|(_, n)| *n > 0.0)
                .map(|(k, n)| format!("{k:?} {n:.2}"))
                .collect();
            let res = format!(
                "{:.2} (triples {:.2}, jumps {:.2}, knockbacks {:.2}, cells {:.2}, burned {:.2})",
                r.resonances_per_fight,
                r.triples_per_fight,
                r.jumps_per_fight,
                r.knockbacks_per_fight,
                r.cells_per_fight,
                r.cell_burns_per_fight
            );
            println!(
                "| {} | {enc} | {:.1} | {:.0} | {:.0} | {:.2} | {:.3} | {:.3} | {:.3} | {} | {res} | {:.2} | {:.2} | {:.2} | {:.2} | {:.2} | {:.2} |",
                party.join(" + "),
                r.win_rate * 100.0,
                r.rounds_to_win.p50,
                r.hits_to_kill.p50,
                r.party_dpt.mean,
                r.crit_share,
                r.effect_share,
                r.control_uptime,
                states.join(", "),
                r.reactions_per_fight,
                r.line_extra_per_fight,
                r.aura_reach_per_fight,
                r.monster_skills_per_fight,
                r.monster_healed_per_fight,
                r.monster_reactions_per_fight,
            );
        }
    }
}
