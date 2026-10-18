//! Writes the program defaults as JSON, for files that must match them:
//!
//! cargo run -p deadroll-sim --example export_balance -- config > app/src/chain/balance.json
//! cargo run -p deadroll-sim --example export_balance -- items > tools/balance/presets/items/default.json
//! cargo run -p deadroll-sim --example export_balance -- skills tools/balance/presets
//!
//! `skills` writes every class starter skill (`deadroll_skillcheck::presets`) as
//! `<root>/skills/<id>.json`, in the format the bench saves. `crates/sim/tests/sim.rs` fails when
//! a committed copy is stale.
use std::path::PathBuf;

use deadroll_rules::BalanceConfig;
use deadroll_sim::{
    config_io,
    presets::{slug, SkillPreset},
};
use deadroll_skillcheck::presets;
use deadroll_types::Class;

fn main() {
    let mut args = std::env::args().skip(1);
    match args.next().as_deref() {
        Some("config") => print!("{}", config_io::to_json(&BalanceConfig::DEFAULT)),
        Some("items") => print!(
            "{}",
            config_io::items_to_json(&BalanceConfig::DEFAULT.items)
        ),
        Some("skills") => {
            let root = PathBuf::from(args.next().unwrap_or_else(|| usage()));
            let dir = root.join("skills");
            std::fs::create_dir_all(&dir).expect("skills dir");
            for &class in Class::ALL {
                for p in presets::for_class(class) {
                    let mut s = SkillPreset::from_blocks(p.name, &p.blocks, &p.ext);
                    s.description = p.description.to_string();
                    let text = serde_json::to_string_pretty(&s).expect("json") + "\n";
                    std::fs::write(dir.join(format!("{}.json", slug(p.name))), text)
                        .expect("write");
                }
            }
        }
        _ => usage(),
    }
}

fn usage() -> ! {
    eprintln!("usage: export_balance config | items | skills <presets root>");
    std::process::exit(2);
}
