//! Map presets in `assets/registry/maps/*.json` must pass every builder rule with the shipped
//! registry (roles, minimum tiers and skill pools of `assets/registry/bestiary.json`).
#![cfg(feature = "serde")]
//!
//! Preset format:
//! ```json
//! { "name": "demo_16x16", "width": 16, "height": 16, "tier": 1,
//!   "min_players": 1, "boss_table": 1, "chests": [{ "x": 9, "y": 10, "table": 0 }],
//!   "rows": ["################", "#E.............#", ...],
//!   "spawns": [{ "bestiary_id": 0, "x": 5, "y": 2, "level": 1, "skills": 0,
//!                "flags": 0, "aspects": 0, "pack": 1 }] }
//! ```
//! Row characters (patch 13, `TileKind`, variant 0): space Void, `.` StoneFloor, `=` WoodenFloor,
//! `,` Dirt, `#` Wall, `D` Door, `~` ShallowWater, `W` DeepWater, `>` Stairs, `%` Rubble,
//! `_` Chasm. `E` and `X` are a stone floor with the entrance or the exit point. Patch 11:
//! `min_players`, `boss_table` (`reward_tables.json` id) and the visible `chests`. The layout's
//! points are the entrance, the exit, then the chests in order.

use std::path::PathBuf;

use deadroll_mapcheck::{check_map, loot::MapLoot, tiles::cell_index};
use deadroll_rules::{loot::map_rules, registry::Registry, tiles::tile_rules, BalanceConfig};
use deadroll_types::{
    cell_byte, Cell, MapLayoutDataV2, MapPoint, PointKind, Pubkey, Spawn, TileKind, CHESTS_MAX,
    MAP_CELLS, POINTS_MAX, SPAWNS_MAX,
};
use serde_json::Value;

fn tile(ch: char) -> TileKind {
    use TileKind as T;
    match ch {
        ' ' => T::Void,
        '.' | 'E' | 'X' => T::StoneFloor,
        '=' => T::WoodenFloor,
        ',' => T::Dirt,
        '#' => T::Wall,
        'D' => T::Door,
        '~' => T::ShallowWater,
        'W' => T::DeepWater,
        '>' => T::Stairs,
        '%' => T::Rubble,
        '_' => T::Chasm,
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
fn load(path: &PathBuf) -> (u8, u8, u8, u8, MapLayoutDataV2) {
    let json: Value = serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap();
    let (width, height, tier) = (
        u8_field(&json, "width"),
        u8_field(&json, "height"),
        u8_field(&json, "tier"),
    );

    let rows = json["rows"].as_array().expect("rows");
    assert_eq!(rows.len(), height as usize, "rows count != height");
    let mut cells = [cell_byte(TileKind::Wall as u8, 0); MAP_CELLS];
    let (mut entrance, mut exit) = (None, None);
    for (y, row) in rows.iter().enumerate() {
        let row = row.as_str().expect("row string");
        assert_eq!(
            row.chars().count(),
            width as usize,
            "row {y} length != width"
        );
        for (x, ch) in row.chars().enumerate() {
            let cell = Cell {
                x: x as u8,
                y: y as u8,
            };
            match ch {
                'E' => assert!(entrance.replace(cell).is_none(), "two entrances"),
                'X' => assert!(exit.replace(cell).is_none(), "two exits"),
                _ => {}
            }
            cells[cell_index(cell.x, cell.y)] = cell_byte(tile(ch) as u8, 0);
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

    let chests = json["chests"].as_array().expect("chests");
    assert!(chests.len() <= CHESTS_MAX);
    let end = |kind, cell: Option<Cell>| {
        let cell = cell.expect("an E and an X");
        MapPoint {
            kind,
            x: cell.x,
            y: cell.y,
            data: 0,
        }
    };
    let mut points = [MapPoint::default(); POINTS_MAX];
    points[0] = end(PointKind::Entrance, entrance);
    points[1] = end(PointKind::Exit, exit);
    for (slot, c) in points[2..].iter_mut().zip(chests) {
        *slot = MapPoint {
            kind: PointKind::Chest,
            x: u8_field(c, "x"),
            y: u8_field(c, "y"),
            data: u8_field(c, "table"),
        };
    }

    let layout = MapLayoutDataV2 {
        version: 1,
        map: Pubkey::default(),
        cells,
        spawns,
        spawn_count: list.len() as u8,
        points,
        point_count: 2 + chests.len() as u8,
        boss_table: u8_field(&json, "boss_table"),
        bump: 0,
    };
    (width, height, tier, u8_field(&json, "min_players"), layout)
}

#[test]
fn every_preset_passes_the_builder_rules() {
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../assets/registry/maps");
    let reg = Registry::shipped().unwrap();
    let cfg = BalanceConfig::DEFAULT;
    let tiles = tile_rules(&reg.tileset);
    let mut checked = 0;
    for entry in std::fs::read_dir(&dir).expect("assets/registry/maps") {
        let path = entry.unwrap().path();
        if path.extension().is_some_and(|e| e == "json") {
            let (width, height, tier, min_players, layout) = load(&path);
            let loot = MapLoot {
                min_players,
                rules: map_rules(&cfg.loot, tier),
                table_of: |id: u8| reg.reward_table(id).map(|t| t.rules()),
            };
            assert_eq!(
                check_map(
                    width,
                    height,
                    tier,
                    &layout,
                    &tiles,
                    |id| reg.spawn_rules(id, &cfg.monsters),
                    &loot
                ),
                Ok(()),
                "{}",
                path.display()
            );
            checked += 1;
        }
    }
    assert!(checked > 0, "no presets in {}", dir.display());
}
