//! The map checks the website builder runs through mapcheck.wasm (`app/src/chain/mapcheckWasm.test.ts`
//! replays every case on the committed module and compares the verdict, its encoder compares the
//! request bytes of the demo preset).
//!
//! UPDATE_TS_FIXTURES=1 cargo test -p deadroll-mapcheck-wasm --test ts_fixtures, then rebuild
//! mapcheck.wasm (bash crates/mapcheck/wasm/build.sh).
use std::path::PathBuf;

use deadroll_mapcheck::secrets::{Board, Leaves};
use deadroll_mapcheck::MapCheckError;
use deadroll_mapcheck_wasm::{
    check, check_bytes, check_design, check_design_bytes, check_steps, check_steps_bytes,
    DesignRequest, MapCheckRequest, Tables, ABI_VERSION,
};
use deadroll_rules::registry::Registry;
use deadroll_types::{
    cell_byte, Cell, Facing, MapPoint, PackLayout, PointKind, Spawn, TileKind, ARENA_CELLS,
    ARENA_SIDE_MAX, PACKS_CAP,
};
use deadroll_types::{CellSecret, FoundSecret};
use serde_json::{json, Value};

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../..")
}

fn tables() -> Tables {
    let r = Registry::shipped().unwrap();
    Tables {
        bestiary: r.bestiary,
        monster_skills: r.monster_skills,
        rewards: r.rewards,
        tileset: r.tileset,
    }
}

fn kind_of(ch: char) -> TileKind {
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

fn u8_of(v: &Value, key: &str) -> u8 {
    v[key].as_u64().unwrap_or(0) as u8
}

/// The packs and arenas of a preset (arena update, `presetToArenas` on the website): the 8 pack
/// numbers of a Small map.
fn arenas(json: &Value) -> (Vec<PackLayout>, Vec<u8>) {
    let mut packs = vec![PackLayout::EMPTY; 8];
    let mut cells = vec![0u8; 8 * ARENA_CELLS];
    for p in json["packs"].as_array().unwrap() {
        let n = u8_of(p, "pack") as usize;
        let rows = p["rows"].as_array().unwrap();
        let facing = p["facing"].as_str().unwrap();
        packs[n] = PackLayout {
            visible: u8_of(p, "visible"),
            x: u8_of(p, "x"),
            y: u8_of(p, "y"),
            facing: *Facing::ALL
                .iter()
                .find(|f| format!("{f:?}") == facing)
                .unwrap(),
            width: rows[0].as_str().unwrap().chars().count() as u8,
            height: rows.len() as u8,
        };
        for (y, row) in rows.iter().enumerate() {
            for (x, ch) in row.as_str().unwrap().chars().enumerate() {
                cells[n * ARENA_CELLS + y * ARENA_SIDE_MAX + x] = cell_byte(kind_of(ch) as u8, 0);
            }
        }
    }
    (packs, cells)
}

/// The demo preset as the website builder sends it (`presetToLayout`, `mapPoints`).
fn demo() -> MapCheckRequest {
    let path = root().join("assets/registry/maps/demo_16x16.json");
    let json: Value = serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap();
    let width = u8_of(&json, "width") as usize;
    let mut cells =
        vec![cell_byte(TileKind::Wall as u8, 0); width * u8_of(&json, "height") as usize];
    let (mut entrance, mut exit) = (Cell::default(), Cell::default());
    for (y, row) in json["rows"].as_array().unwrap().iter().enumerate() {
        for (x, ch) in row.as_str().unwrap().chars().enumerate() {
            let cell = Cell {
                x: x as u8,
                y: y as u8,
            };
            if ch == 'E' {
                entrance = cell;
            }
            if ch == 'X' {
                exit = cell;
            }
            cells[y * width + x] = cell_byte(kind_of(ch) as u8, 0);
        }
    }
    let list = json["spawns"].as_array().unwrap();
    let spawns: Vec<Spawn> = list
        .iter()
        .map(|s| Spawn {
            bestiary_id: u8_of(s, "bestiary_id"),
            x: u8_of(s, "x"),
            y: u8_of(s, "y"),
            level: u8_of(s, "level"),
            skills: s["skills"].as_u64().unwrap() as u16,
            flags: u8_of(s, "flags"),
            aspects: u8_of(s, "aspects"),
            pack: u8_of(s, "pack"),
        })
        .collect();
    let point = |kind, c: Cell, data| MapPoint {
        kind,
        x: c.x,
        y: c.y,
        data,
    };
    let mut points = vec![
        point(PointKind::Entrance, entrance, 0),
        point(PointKind::Exit, exit, 0),
    ];
    for c in json["chests"].as_array().unwrap() {
        let at = Cell {
            x: u8_of(c, "x"),
            y: u8_of(c, "y"),
        };
        points.push(point(PointKind::Chest, at, u8_of(c, "table")));
    }
    let (packs, arenas) = arenas(&json);
    MapCheckRequest {
        width: u8_of(&json, "width"),
        height: u8_of(&json, "height"),
        tier: u8_of(&json, "tier"),
        min_players: u8_of(&json, "min_players"),
        cells,
        spawns,
        points,
        boss_table: u8_of(&json, "boss_table"),
        packs,
        arenas,
    }
}

fn arena_at(r: &mut MapCheckRequest, pack: usize, x: u8, y: u8, kind: TileKind) {
    r.arenas[pack * ARENA_CELLS + y as usize * ARENA_SIDE_MAX + x as usize] =
        cell_byte(kind as u8, 0);
}

fn at(r: &mut MapCheckRequest, x: u8, y: u8, kind: TileKind, variant: u8) {
    let w = r.width as usize;
    r.cells[y as usize * w + x as usize] = cell_byte(kind as u8, variant);
}

/// Patch D: the demo in the top-left corner of a bigger open map (a floor hall in a wall ring),
/// with the class's pack numbers.
fn grown(width: u8, height: u8) -> MapCheckRequest {
    let small = demo();
    let (w, h) = (width as usize, height as usize);
    let floor = cell_byte(TileKind::StoneFloor as u8, 0);
    let wall = cell_byte(TileKind::Wall as u8, 0);
    let mut cells = vec![floor; w * h];
    for y in 0..h {
        for x in 0..w {
            if x == 0 || y == 0 || x == w - 1 || y == h - 1 {
                cells[y * w + x] = wall;
            }
        }
    }
    let sw = small.width as usize;
    for y in 0..small.height as usize {
        for x in 0..sw {
            cells[y * w + x] = small.cells[y * sw + x];
        }
    }
    // The demo's exit wall opens to the hall.
    let class = deadroll_rules::BalanceConfig::DEFAULT
        .map_size
        .class_of(width, height)
        .unwrap_or(3) as usize;
    let packs_n = deadroll_types::CLASS_PACKS[class];
    let mut packs = small.packs.clone();
    packs.resize(packs_n, PackLayout::EMPTY);
    let mut arenas = small.arenas.clone();
    arenas.resize(packs_n * ARENA_CELLS, 0);
    let mut r = MapCheckRequest {
        width,
        height,
        cells,
        packs,
        arenas,
        ..small
    };
    // Exit far in the hall.
    let exit = r
        .points
        .iter_mut()
        .find(|p| p.kind == PointKind::Exit)
        .unwrap();
    exit.x = width - 2;
    exit.y = height - 2;
    for y in 0..small.height {
        at(&mut r, small.width - 1, y, TileKind::StoneFloor, 0);
    }
    r
}

/// Patch D: `extra` more wolf packs in the hall of a grown map (a wolf each, its own open arena).
fn more_packs(mut r: MapCheckRequest, extra: usize) -> MapCheckRequest {
    let first = (4..r.packs.len()).find(|&n| !r.packs[n].is_used()).unwrap();
    let floor = cell_byte(TileKind::StoneFloor as u8, 0);
    for i in 0..extra {
        let n = first + i;
        let wolf = r.spawns[0];
        r.packs[n] = PackLayout {
            visible: r.spawns.len() as u8,
            x: 20 + 3 * i as u8,
            y: 20,
            facing: Facing::S,
            width: 8,
            height: 8,
        };
        r.spawns.push(Spawn {
            pack: n as u8,
            x: 4,
            y: 4,
            ..wolf
        });
        for c in &mut r.arenas[n * ARENA_CELLS..(n + 1) * ARENA_CELLS] {
            *c = floor;
        }
    }
    r
}

/// Named drafts and the verdict each one must get.
fn cases() -> Vec<(&'static str, MapCheckRequest, Result<(), MapCheckError>)> {
    use MapCheckError as E;
    let base = demo();
    let exit = base
        .points
        .iter()
        .find(|p| p.kind == PointKind::Exit)
        .unwrap()
        .cell();
    let edit = |f: &dyn Fn(&mut MapCheckRequest)| {
        let mut r = demo();
        f(&mut r);
        r
    };
    vec![
        ("demo", demo(), Ok(())),
        (
            "variants",
            edit(&|r| {
                for x in 1..15 {
                    at(r, x, 14, TileKind::StoneFloor, x % 8);
                }
            }),
            Ok(()),
        ),
        (
            "no entrance",
            edit(&|r| {
                r.points.remove(0);
            }),
            Err(E::EntranceCount),
        ),
        (
            "no exit",
            edit(&|r| {
                r.points.remove(1);
            }),
            Err(E::ExitCount),
        ),
        (
            "exit on a chasm",
            edit(&|r| at(r, exit.x, exit.y, TileKind::Chasm, 0)),
            Err(E::PointNotPassable),
        ),
        (
            "entrance on the exit",
            edit(&|r| {
                r.points[0].x = exit.x;
                r.points[0].y = exit.y;
            }),
            Err(E::PointOverlap),
        ),
        (
            "a wall under a spawn in its arena",
            edit(&|r| {
                let s = r.spawns[0];
                arena_at(r, s.pack as usize, s.x, s.y, TileKind::Wall);
            }),
            Err(E::SpawnNotPassable),
        ),
        (
            "reserved tile type",
            edit(&|r| r.cells[3 * 16 + 2] = cell_byte(TileKind::COUNT as u8, 0)),
            Err(E::InvalidTile),
        ),
        (
            "two wolves on one arena cell",
            edit(&|r| {
                r.spawns[1].x = r.spawns[0].x;
                r.spawns[1].y = r.spawns[0].y;
            }),
            Err(E::SpawnOverlap),
        ),
        (
            "pack on the exit",
            edit(&|r| {
                r.packs[1].x = exit.x;
                r.packs[1].y = exit.y;
            }),
            Err(E::PackOverlap),
        ),
        (
            "pack next to the entrance",
            edit(&|r| {
                r.packs[1].x = 2;
                r.packs[1].y = 1;
            }),
            Err(E::PackAtEntrance),
        ),
        (
            "visible monster of another pack",
            edit(&|r| r.packs[1].visible = 2),
            Err(E::VisibleNotInPack),
        ),
        (
            "spawn without an arena",
            edit(&|r| r.packs[2] = PackLayout::EMPTY),
            Err(E::PackWithoutArena),
        ),
        (
            "arena height 5",
            edit(&|r| r.packs[1].height = 5),
            Err(E::ArenaSizeOutOfRange),
        ),
        (
            "arena side 12",
            edit(&|r| {
                r.packs[1].width = 12;
                for x in 8..12 {
                    for y in 0..8 {
                        arena_at(r, 1, x, y, TileKind::StoneFloor);
                    }
                }
            }),
            Ok(()),
        ),
        (
            "wolves walled in",
            edit(&|r| {
                for y in 0..8 {
                    arena_at(r, 1, 3, y, TileKind::Wall);
                    arena_at(r, 1, 6, y, TileKind::Wall);
                }
                for x in 3..7 {
                    arena_at(r, 1, x, 3, TileKind::Wall);
                    arena_at(r, 1, x, 6, TileKind::Wall);
                }
            }),
            Err(E::ArenaUnreachable),
        ),
        (
            "no boss",
            edit(&|r| r.spawns.truncate(2)),
            Err(E::BossCount),
        ),
        // The corridor at (11, 7) is the only way down to the exit.
        (
            "corridor in deep water",
            edit(&|r| at(r, 11, 7, TileKind::DeepWater, 0)),
            Err(E::ExitUnreachable),
        ),
        (
            "corridor in rubble",
            edit(&|r| at(r, 11, 7, TileKind::Rubble, 5)),
            Ok(()),
        ),
        // Patch 13: the corridor cell walled, the cell beside it open. (11, 6) -> (12, 7) ->
        // (11, 8) would cut the corners of the walls (12, 6), (11, 7) and (12, 8).
        (
            "a diagonal past wall corners",
            edit(&|r| {
                at(r, 11, 7, TileKind::Wall, 0);
                at(r, 12, 7, TileKind::StoneFloor, 0);
            }),
            Err(E::ExitUnreachable),
        ),
        (
            "two chests on tier 1",
            edit(&|r| {
                let mut chest = r.points[2];
                chest.x += 1;
                r.points.push(chest);
            }),
            Err(E::TooManyChests),
        ),
        (
            "no boss table",
            edit(&|r| r.boss_table = deadroll_types::NO_TABLE),
            Err(E::BossTableMissing),
        ),
        (
            "minimum players 0",
            edit(&|r| r.min_players = 0),
            Err(E::MinPlayersOutOfRange),
        ),
        (
            "tier 2 levels",
            edit(&|r| r.tier = 2),
            Err(E::LevelOutOfTier),
        ),
        ("side 7", edit(&|r| r.width = 7), Err(E::SizeOutOfRange)),
        // Patch D: map sizes (docs/MAP_DESCRIPTION.md 17.6).
        ("medium 72x72", grown(72, 72), Ok(())),
        ("dungeon 143x143", grown(143, 143), Ok(())),
        ("side 144", grown(144, 20), Err(E::SizeOutOfRange)),
        // Tier 1 allows 3 regular packs on a Small map, 9 on a Medium one.
        (
            "medium: 6 regular packs",
            more_packs(grown(72, 72), 4),
            Ok(()),
        ),
        (
            "big: 20 regular packs",
            more_packs(grown(101, 101), 15),
            Err(E::TooManyPacks),
        ),
        (
            "medium: a pack number past the class",
            edit_grown(&|r| {
                let p = r.packs[1];
                r.packs.push(PackLayout { x: 30, y: 30, ..p });
                let a = r.arenas[ARENA_CELLS..2 * ARENA_CELLS].to_vec();
                r.arenas.extend_from_slice(&a);
            }),
            Err(E::PackOutOfRange),
        ),
        (
            "dungeon: the exit walled off",
            edit_dungeon(&|r| {
                let (w, h) = (r.width, r.height);
                for x in 1..w - 1 {
                    at(r, x, h - 5, TileKind::Wall, 0);
                }
            }),
            Err(E::ExitUnreachable),
        ),
    ]
}

fn edit_grown(f: &dyn Fn(&mut MapCheckRequest)) -> MapCheckRequest {
    let mut r = grown(72, 72);
    f(&mut r);
    r
}

fn edit_dungeon(f: &dyn Fn(&mut MapCheckRequest)) -> MapCheckRequest {
    let mut r = grown(143, 143);
    f(&mut r);
    r
}

/// Patch C: the demo preset's design (its trap and secret door) and a few broken ones.
fn design_cases() -> Vec<(&'static str, DesignRequest, Result<(), MapCheckError>)> {
    let map = demo();
    let mut packs = [PackLayout::EMPTY; PACKS_CAP];
    packs[..map.packs.len()].copy_from_slice(&map.packs);
    let leaves = Leaves::new(map.width, map.height, 0, &packs);
    let at = |x, y| leaves.of(Board::Map(Cell { x, y })).unwrap();
    let trap = FoundSecret {
        leaf: at(9, 9),
        secret: CellSecret::trap(4, 10),
    };
    let door = FoundSecret {
        leaf: at(4, 11),
        secret: CellSecret::door(),
    };
    let req = |declared: [u8; 4], secrets: Vec<FoundSecret>| DesignRequest {
        map: map.clone(),
        declared,
        secrets,
    };
    use MapCheckError as E;
    vec![
        (
            "design: the demo's trap and door",
            req([1, 0, 0, 1], vec![trap, door]),
            Ok(()),
        ),
        (
            "design: one secret more than declared",
            req([1, 0, 0, 0], vec![trap, door]),
            Err(E::TooManySecrets),
        ),
        (
            "design: no trap on tier 1",
            req([0, 0, 0, 0], vec![]),
            Err(E::TooFewTraps),
        ),
        (
            "design: a door on the floor",
            req(
                [1, 0, 0, 1],
                vec![
                    trap,
                    FoundSecret {
                        leaf: at(9, 10),
                        ..door
                    },
                ],
            ),
            Err(E::DoorNotWall),
        ),
    ]
}

fn build() -> Value {
    let t = tables();
    let design: Vec<Value> = design_cases()
        .into_iter()
        .map(|(name, request, expected)| {
            let got = check_design(&request, &t);
            assert_eq!(got, expected, "case {name}");
            let input = borsh::to_vec(&request).unwrap();
            let verdict = check_design_bytes(&input, &t);
            assert_eq!(verdict, got.map_or_else(|e| 1 + e.code() as i32, |_| 0));
            json!({ "name": name, "input": hex(&input), "verdict": verdict })
        })
        .collect();
    let cases: Vec<Value> = cases()
        .into_iter()
        .map(|(name, request, expected)| {
            let got = check(&request, &t);
            // Pins the meaning of each case; the fixture keeps only the verdict.
            assert_eq!(got, expected, "case {name}");
            let input = borsh::to_vec(&request).unwrap();
            let verdict = check_bytes(&input, &t);
            assert_eq!(verdict, got.map_or_else(|e| 1 + e.code() as i32, |_| 0));
            // Patch D: the `check_step` calls the map needs (negative: the check stops).
            let steps = check_steps_bytes(&input, &t);
            if let Ok(n) = check_steps(&request, &t) {
                assert_eq!(steps, n as i32);
            }
            json!({ "name": name, "input": hex(&input), "verdict": verdict, "steps": steps })
        })
        .collect();
    json!({
        "abiVersion": ABI_VERSION,
        "tables": {
            "bestiary": hex(&t.bestiary),
            "monster_skills": hex(&t.monster_skills),
            "rewards": hex(&t.rewards),
            "tileset": hex(&t.tileset),
        },
        "cases": cases,
        "designCases": design,
    })
}

#[test]
fn bad_input_is_refused() {
    let t = tables();
    let input = borsh::to_vec(&demo()).unwrap();
    assert_eq!(check_bytes(&input[..input.len() - 1], &t), -2);
    assert_eq!(check_bytes(&[input.clone(), vec![0]].concat(), &t), -2);
    assert_eq!(check_bytes(&input, &t), 0);
}

#[test]
fn ts_fixtures_are_current() {
    let path = root().join("app/src/chain/__fixtures__/mapcheck-wasm.json");
    let text = serde_json::to_string_pretty(&build()).unwrap() + "\n";
    if std::env::var_os("UPDATE_TS_FIXTURES").is_some() {
        std::fs::write(&path, &text).unwrap();
        return;
    }
    let committed = std::fs::read_to_string(&path)
        .unwrap_or_default()
        .replace("\r\n", "\n");
    assert!(
        committed == text,
        "{} is stale or missing: run UPDATE_TS_FIXTURES=1 cargo test -p deadroll-mapcheck-wasm \
         --test ts_fixtures, then rebuild mapcheck.wasm (bash crates/mapcheck/wasm/build.sh)",
        path.display()
    );
}
