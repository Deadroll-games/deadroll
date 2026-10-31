//! Patch C: the secrets' Merkle tree as `sdk/src/secrets.ts` rebuilds it (`sdk/test/secrets.test.ts`
//! compares every hash, the root and the proofs, the demo preset's split and threats).
//!
//! UPDATE_TS_FIXTURES=1 cargo test -p deadroll-mapcheck-wasm --test secrets_fixture
use std::path::PathBuf;

use deadroll_mapcheck::secrets::{build, cell_leaf, depth, node, pack_leaf, Leaves};
use deadroll_types::{CellSecret, Facing, PackLayout, PackRoster, Spawn, PACKS_MAX, SPAWN_ELITE};
use serde_json::{json, Value};

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

fn spawn_json(s: &Spawn) -> Value {
    json!({
        "bestiaryId": s.bestiary_id, "x": s.x, "y": s.y, "level": s.level, "skills": s.skills,
        "flags": s.flags, "aspects": s.aspects, "pack": s.pack,
    })
}

#[test]
fn secrets_fixture_is_fresh() {
    let map: [u8; 32] = core::array::from_fn(|i| i as u8 * 7 + 1);
    let salt =
        |i: u16| -> [u8; 16] { core::array::from_fn(|j| (i as u8).wrapping_mul(31) ^ j as u8) };

    // A 10x9 map with packs 1 (6x7 arena) and 4 (8x6, an ambush).
    let mut packs = [PackLayout::EMPTY; PACKS_MAX];
    packs[1] = PackLayout {
        visible: 0,
        x: 3,
        y: 2,
        facing: Facing::S,
        width: 6,
        height: 7,
    };
    packs[4] = PackLayout {
        visible: 0xFF,
        x: 0,
        y: 0,
        facing: Facing::W,
        width: 8,
        height: 6,
    };
    let leaves = Leaves::new(10, 9, &packs);

    let mut roster = PackRoster {
        count: 2,
        ..PackRoster::default()
    };
    roster.spawns[0] = Spawn {
        bestiary_id: 1,
        x: 2,
        y: 3,
        level: 2,
        skills: 0x0105,
        flags: SPAWN_ELITE,
        aspects: 4,
        pack: 1,
    };
    roster.spawns[1] = Spawn {
        bestiary_id: 1,
        x: 4,
        y: 3,
        level: 1,
        pack: 1,
        ..Spawn::default()
    };
    let secrets = [
        (leaves.count - 1, CellSecret::trap(7, 12)),
        (8 + 2 * 10 + 5, CellSecret::ambush(4)),
        (8 + 8 * 10 + 9, CellSecret::stash(1)),
        (8 + 10, CellSecret::door()),
    ];
    let mut hashes = vec![[0u8; 32]; 1 << depth(leaves.count)];
    for leaf in 0..leaves.count {
        hashes[leaf as usize] = if leaf == 1 {
            pack_leaf(&map, leaf, &roster, &salt(leaf))
        } else if (leaf as usize) < PACKS_MAX {
            pack_leaf(&map, leaf, &PackRoster::default(), &salt(leaf))
        } else {
            let s = secrets
                .iter()
                .find(|(l, _)| *l == leaf)
                .map_or(CellSecret::NONE, |(_, s)| *s);
            cell_leaf(&map, leaf, &s, &salt(leaf))
        };
    }
    let leaf_hashes: Vec<String> = hashes[..leaves.count as usize]
        .iter()
        .map(|h| hex(h))
        .collect();
    let mut proofs = Vec::new();
    let mut root = [0u8; 32];
    for index in [0usize, 1, 37, leaves.count as usize - 1] {
        let mut level = hashes.clone();
        let mut proof = [[0u8; 32]; 12];
        root = build(&mut level, leaves.count as usize, index, &mut proof);
        let d = depth(leaves.count);
        proofs.push(json!({ "index": index, "proof": proof[..d].iter().map(|h| hex(h)).collect::<Vec<_>>() }));
    }

    let fixture = json!({
        "map": hex(&map),
        "salt": (0..leaves.count).map(|i| hex(&salt(i))).collect::<Vec<_>>(),
        "width": 10, "height": 9,
        "packs": packs.iter().map(|p| if p.is_used() { json!({"width": p.width, "height": p.height}) } else { Value::Null }).collect::<Vec<_>>(),
        "leafCount": leaves.count,
        "roster": { "count": roster.count, "spawns": roster.spawns.iter().map(spawn_json).collect::<Vec<_>>() },
        "secrets": secrets.iter().map(|(l, s)| json!({"leaf": l, "kind": s.kind as u8, "a": s.a, "b": s.b, "c": s.c})).collect::<Vec<_>>(),
        "leaves": leaf_hashes,
        "node": hex(&node(&hashes[0], &hashes[1])),
        "root": hex(&root),
        "proofs": proofs,
    });
    let text = serde_json::to_string_pretty(&fixture).unwrap() + "\n";
    let path =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../sdk/test/fixtures/secrets.json");
    if std::env::var_os("UPDATE_TS_FIXTURES").is_some() {
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, &text).unwrap();
        return;
    }
    let committed = std::fs::read_to_string(&path)
        .unwrap_or_default()
        .replace("\r\n", "\n");
    assert!(
        committed == text,
        "{} is stale or missing: run UPDATE_TS_FIXTURES=1 cargo test -p deadroll-mapcheck-wasm --test secrets_fixture",
        path.display()
    );
}
