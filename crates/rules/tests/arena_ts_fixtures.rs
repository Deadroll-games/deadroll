//! Golden fixture for the TypeScript mirror of patch B (`app/src/chain/timeline.ts`,
//! `app/src/chain/footprint.ts`): the arena numbers, the delay under a Slow, the timeline
//! forecast of the turn bar, and the distances and line ends between one-cell and 2x2 units.
//! Test-only: no crate logic lives here. When a rule or a number changes, regenerate it and
//! update the TS mirror:
//!
//! ```text
//! UPDATE_TS_FIXTURES=1 cargo test -p deadroll-rules --features serde --test arena_ts_fixtures
//! ```
#![cfg(feature = "serde")]
use std::{fs, path::PathBuf};

use deadroll_combat::{support::ModKind, TimedMods};
use deadroll_rules::{
    arena::{effective_initiative, next_unit, turn_delay, upcoming, TimelineUnit},
    line::line_ends,
    BalanceConfig, Footprint,
};
use deadroll_types::Cell;
use serde_json::{json, Value};

const CASES: usize = 300;
/// Fixed seed; changing it rewrites the fixture.
const SEED: u64 = 0xD3AD_00B0_0551_0002;

fn fixture_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../app/src/chain/__fixtures__/arena.json")
}

/// Small deterministic generator (SplitMix64), so the fixture is stable across runs.
struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }

    fn below(&mut self, n: u64) -> u64 {
        self.next() % n
    }
}

fn slowed(power_bp: u32) -> TimedMods {
    let mut m = TimedMods::default();
    if power_bp > 0 {
        m[0].kind = ModKind::Slow;
        m[0].turns = 1;
        m[0].power_bp = power_bp;
    }
    m
}

fn fp(f: Footprint) -> Value {
    json!({ "x": f.at.x, "y": f.at.y, "size": f.size })
}

fn cell(c: Cell) -> Value {
    json!({ "x": c.x, "y": c.y })
}

#[test]
fn arena_rules_match_the_ts_fixture() {
    let t = BalanceConfig::DEFAULT.arena;
    let mut rng = Rng(SEED);

    let delays: Vec<Value> = (0..CASES)
        .map(|_| {
            let initiative = rng.below(40) as u16;
            let slow = [0, 2_500, 5_000, 10_000, 30_000][rng.below(5) as usize];
            let mods = slowed(slow);
            json!({
                "initiative": initiative,
                "slowBp": slow,
                "effective": effective_initiative(initiative, &mods, &t),
                "delay": turn_delay(initiative, &mods, &t),
            })
        })
        .collect();

    let timelines: Vec<Value> = (0..CASES / 3)
        .map(|_| {
            let n = 2 + rng.below(9) as usize;
            let units: Vec<TimelineUnit> = (0..n)
                .map(|i| TimelineUnit {
                    // Players 0..4 by slot, then monsters.
                    id: i as u8,
                    time: rng.below(300) as u32,
                    tie: rng.below(6) as u16,
                    alive: rng.below(6) != 0,
                })
                .collect();
            let steps: Vec<u32> = (0..n).map(|_| 30 + rng.below(170) as u32).collect();
            let mut out = [0u8; 8];
            let len = upcoming(&units, &steps, &mut out);
            json!({
                "units": units
                    .iter()
                    .map(|u| json!({ "id": u.id, "time": u.time, "tie": u.tie, "alive": u.alive }))
                    .collect::<Vec<_>>(),
                "delays": steps,
                "next": next_unit(&units),
                "upcoming": out[..len].to_vec(),
            })
        })
        .collect();

    let squares: Vec<Value> = (0..CASES)
        .map(|_| {
            let mut one = || {
                Footprint::new(
                    Cell {
                        x: rng.below(12) as u8,
                        y: rng.below(12) as u8,
                    },
                    1 + rng.below(2) as u8,
                )
            };
            let (a, b) = (one(), one());
            let probe = Cell {
                x: rng.below(13) as u8,
                y: rng.below(13) as u8,
            };
            let (from, through) = line_ends(a, b);
            json!({
                "a": fp(a),
                "b": fp(b),
                "probe": cell(probe),
                "distance": a.distance(&b),
                "covers": a.covers(probe),
                "nearest": cell(a.nearest(probe)),
                "lineFrom": cell(from),
                "lineThrough": cell(through),
            })
        })
        .collect();

    let fixture = json!({
        "arena": {
            "timeScale": t.time_scale,
            "initiativeBase": t.initiative_base,
            "roundTime": t.round_time,
            "slowInitiativeBp": t.slow_initiative_bp,
        },
        "delays": delays,
        "timelines": timelines,
        "squares": squares,
    });
    let text = serde_json::to_string(&fixture).unwrap() + "\n";
    let path = fixture_path();
    if std::env::var_os("UPDATE_TS_FIXTURES").is_some() {
        fs::write(&path, &text).unwrap();
    }
    let committed = fs::read_to_string(&path)
        .unwrap_or_default()
        .replace("\r\n", "\n");
    assert!(
        committed == text,
        "stale fixture: run UPDATE_TS_FIXTURES=1 cargo test -p deadroll-rules --features serde --test arena_ts_fixtures"
    );
}
