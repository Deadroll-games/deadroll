//! Map presets in `assets/registry/maps/*.json` must pass every builder rule with the shipped
//! registry (roles, minimum tiers and skill pools of `assets/registry/bestiary.json`).
#![cfg(feature = "serde")]
//!
//! Preset format:
//! ```json
//! { "name": "demo_16x16", "width": 16, "height": 16, "tier": 1,
//!   "rows": ["################", "#E.............#", ...],
//!   "spawns": [{ "bestiary_id": 0, "x": 5, "y": 2, "level": 1, "skills": 0,
//!                "flags": 0, "aspects": 0, "pack": 1 }] }
//! ```
//! Row characters: `#` Wall, `.` Floor, `~` Water, `D` Door, `E` Entrance, `X` Exit, `T` Trap.
//! The entrance and exit cells come from the `E` and `X` characters.

use std::path::PathBuf;

use deadroll_mapcheck::{check_map, tiles::cell_index};
use deadroll_rules::{registry::Registry, BalanceConfig};
use deadroll_types::{Cell, MapLayoutData, Pubkey, Spawn, Tile, MAP_CELLS, SPAWNS_MAX};
use serde_json::Value;

fn tile(ch: char) -> Tile {
    match ch {
        '#' => Tile::Wall,
        '.' => Tile::Floor,
        '~' => Tile::Water,
        'D' => Tile::Door,
        'E' => Tile::Entrance,
        'X' => Tile::Exit,
        'T' => Tile::Trap,
        other => panic!("unknown tile character {other:?}"),
    }
}

fn u8_field(v: &Value, key: &str) -> u8 {
    v[key]
        .as_u64()
        .unwrap_or_else(|| panic!("missing {key}"))
        .try_into()
        .unwrap()
}

/// Builds the layout described by a preset, as the seed script writes it on-chain.
fn load(path: &PathBuf) -> (u8, u8, u8, MapLayoutData) {
    let json: Value = serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap();
    let (width, height, tier) = (
        u8_field(&json, "width"),
        u8_field(&json, "height"),
        u8_field(&json, "tier"),
    );

    let rows = json["rows"].as_array().expect("rows");
    assert_eq!(rows.len(), height as usize, "rows count != height");
    let mut tiles = [Tile::Wall as u8; MAP_CELLS];
    let (mut entrance, mut exit) = (Cell::default(), Cell::default());
    for (y, row) in rows.iter().enumerate() {
        let row = row.as_str().expect("row string");
        assert_eq!(
            row.chars().count(),
            width as usize,
            "row {y} length != width"
        );
        for (x, ch) in row.chars().enumerate() {
            let t = tile(ch);
            let cell = Cell {
                x: x as u8,
                y: y as u8,
            };
            match t {
                Tile::Entrance => entrance = cell,
                Tile::Exit => exit = cell,
                _ => {}
            }
            tiles[cell_index(cell.x, cell.y)] = t as u8;
        }
    }

    let list = json["spawns"].as_array().expect("spawns");
    assert!(list.len() <= SPAWNS_MAX);
    let mut spawns = [Spawn::default(); SPAWNS_MAX];
    for (slot, s) in spawns.iter_mut().zip(list) {
        *slot = Spawn {
            bestiary_id: u8_field(s, "bestiary_id"),
            x: u8_field(s, "x"),
            y: u8_field(s, "y"),
            level: u8_field(s, "level"),
            skills: s["skills"].as_u64().expect("skills").try_into().unwrap(),
            flags: u8_field(s, "flags"),
            aspects: u8_field(s, "aspects"),
            pack: u8_field(s, "pack"),
        };
    }

    let layout = MapLayoutData {
        version: 1,
        map: Pubkey::default(),
        tiles,
        spawns,
        spawn_count: list.len() as u8,
        entrance,
        exit,
        bump: 0,
    };
    (width, height, tier, layout)
}

#[test]
fn every_preset_passes_the_builder_rules() {
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../assets/registry/maps");
    let reg = Registry::shipped().unwrap();
    let cfg = BalanceConfig::DEFAULT;
    let mut checked = 0;
    for entry in std::fs::read_dir(&dir).expect("assets/registry/maps") {
        let path = entry.unwrap().path();
        if path.extension().is_some_and(|e| e == "json") {
            let (width, height, tier, layout) = load(&path);
            assert_eq!(
                check_map(width, height, tier, &layout, |id| reg
                    .spawn_rules(id, &cfg.monsters)),
                Ok(()),
                "{}",
                path.display()
            );
            checked += 1;
        }
    }
    assert!(checked > 0, "no presets in {}", dir.display());
}
