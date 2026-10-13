//! Writes the program defaults as JSON, for files that must match them:
//!
//! cargo run -p deadroll-sim --example export_balance -- config > app/src/chain/balance.json
//! cargo run -p deadroll-sim --example export_balance -- items > tools/balance/presets/items/default.json
//!
//! `crates/sim/tests/sim.rs` fails when a committed copy is stale.
use deadroll_rules::BalanceConfig;
use deadroll_sim::config_io;

fn main() {
    match std::env::args().nth(1).as_deref() {
        Some("config") => print!("{}", config_io::to_json(&BalanceConfig::DEFAULT)),
        Some("items") => print!(
            "{}",
            config_io::items_to_json(&BalanceConfig::DEFAULT.items)
        ),
        _ => {
            eprintln!("usage: export_balance config|items");
            std::process::exit(2);
        }
    }
}
