//! The registry bytes the simulator reads are the bytes the seed script uploads: the TS encoder
//! (tools/scripts/lib/registry-records.ts) checks itself against this fixture.
//!
//! UPDATE_TS_FIXTURES=1 cargo test -p deadroll-rules --features serde --test registry
#![cfg(feature = "serde")]
use std::{fs, path::PathBuf};

use deadroll_rules::registry::Registry;

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

#[test]
fn registry_bytes_match_the_seed_fixture() {
    let r = Registry::shipped().unwrap();
    let json = format!(
        "{{\n  \"note\": \"assets/registry encoded by deadroll-rules (crates/rules/tests/registry.rs)\",\n  \"aspects\": \"{}\",\n  \"traits\": \"{}\",\n  \"resonances\": \"{}\",\n  \"triples\": \"{}\",\n  \"bestiary\": \"{}\",\n  \"monster_skills\": \"{}\",\n  \"rewards\": \"{}\",\n  \"tileset\": \"{}\"\n}}\n",
        hex(&r.aspects),
        hex(&r.traits),
        hex(&r.resonances),
        hex(&r.triples),
        hex(&r.bestiary),
        hex(&r.monster_skills),
        hex(&r.rewards),
        hex(&r.tileset)
    );
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../tools/scripts/lib/__fixtures__/registry-tables.json");
    if std::env::var_os("UPDATE_TS_FIXTURES").is_some() {
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(&path, &json).unwrap();
    }
    let committed = fs::read_to_string(&path)
        .unwrap_or_default()
        .replace("\r\n", "\n");
    assert_eq!(
        committed, json,
        "stale fixture: run UPDATE_TS_FIXTURES=1 cargo test -p deadroll-rules --features serde --test registry"
    );
}
