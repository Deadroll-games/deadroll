//! Golden fixture for the TypeScript mirror of the bag layout check (`app/src/chain/bagLayout.ts`).
//! Test-only: no crate logic lives here.
//!
//! A seeded set of `(bag before, cache, bag after)` cases, mostly made by legal moves from the
//! before state and then sometimes broken (another kind, one more unit, a stack too big, a wrong
//! section, a dirty empty slot), with the verdict of `deadroll_rules::loot::check_bag_layout`.
//! When a rule or a number changes, regenerate it and update the TS mirror:
//!
//! ```text
//! UPDATE_TS_FIXTURES=1 cargo test -p deadroll-rules --features serde --test bag_ts_fixtures
//! ```
#![cfg(feature = "serde")]
use std::{fs, path::PathBuf};

use deadroll_rules::{loot::check_bag_layout, BalanceConfig};
use deadroll_types::{BagLayout, BagSlot, CacheSlot, RewardKind, CACHE_SLOTS};
use serde_json::{json, Value};

const CASES: usize = 500;
/// Fixed seed; changing it rewrites the fixture.
const SEED: u64 = 0xD3AD_0011_BA65_0001;

fn fixture_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../app/src/chain/__fixtures__/bag-layout.json")
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

    fn below(&mut self, n: usize) -> usize {
        (self.next() % n as u64) as usize
    }
}

fn kind(rng: &mut Rng) -> RewardKind {
    RewardKind::ALL[rng.below(RewardKind::COUNT)]
}

/// A legal bag: a few stacks in the right sections, at most a stack each.
fn random_bag(rng: &mut Rng, t: &deadroll_rules::LootTuning) -> BagLayout {
    let mut bag = BagLayout::EMPTY;
    for _ in 0..rng.below(6) {
        let k = kind(rng);
        let id = rng.below(4) as u16;
        if k == RewardKind::Consumable {
            let i = rng.below(bag.consumables.len());
            bag.consumables[i] = BagSlot {
                kind: k,
                id,
                qty: 1 + rng.below(t.consumable_stack_max as usize) as u8,
            };
        } else {
            let i = rng.below(bag.resources.len());
            bag.resources[i] = BagSlot {
                kind: k,
                id,
                qty: 1 + rng.below(t.resource_stack_max as usize) as u8,
            };
        }
    }
    bag
}

fn random_cache(rng: &mut Rng) -> [CacheSlot; CACHE_SLOTS] {
    let mut cache = [CacheSlot::EMPTY; CACHE_SLOTS];
    for c in cache.iter_mut().take(rng.below(5)) {
        *c = CacheSlot {
            kind: kind(rng),
            id: rng.below(4) as u16,
            qty: 1 + rng.below(14) as u16,
        };
    }
    cache
}

/// The layout after: the before bag with some cache rewards taken in (legal or not) and a few
/// slots moved, emptied or broken.
fn after_of(
    rng: &mut Rng,
    before: &BagLayout,
    cache: &[CacheSlot],
    t: &deadroll_rules::LootTuning,
) -> BagLayout {
    let mut after = *before;
    for c in cache.iter().filter(|c| !c.is_empty()) {
        if rng.below(3) == 0 {
            continue;
        }
        let consumable = c.kind == RewardKind::Consumable;
        let stack = if consumable {
            t.consumable_stack_max
        } else {
            t.resource_stack_max
        };
        let qty = (1 + rng.below(c.qty as usize)).min(stack as usize) as u8;
        let section = if consumable {
            &mut after.consumables[..]
        } else {
            &mut after.resources[..]
        };
        if let Some(s) = section.iter_mut().find(|s| s.is_empty()) {
            *s = BagSlot {
                kind: c.kind,
                id: c.id,
                qty,
            };
        }
    }
    // Throw something away now and then.
    if rng.below(4) == 0 {
        let i = rng.below(after.resources.len());
        after.resources[i] = BagSlot::EMPTY;
    }
    // Then break it in one way, half of the time.
    let i = rng.below(after.resources.len());
    match rng.below(12) {
        0 => {
            after.resources[i] = BagSlot {
                kind: RewardKind::Resource,
                id: 9,
                qty: 1,
            }
        }
        1 => {
            if let Some(s) = after.resources.iter_mut().find(|s| !s.is_empty()) {
                s.qty = s.qty.saturating_add(1);
            }
        }
        2 => {
            after.resources[i] = BagSlot {
                kind: RewardKind::Resource,
                id: 0,
                qty: t.resource_stack_max + 1,
            }
        }
        3 => {
            after.resources[i] = BagSlot {
                kind: RewardKind::Consumable,
                id: 0,
                qty: 1,
            }
        }
        4 => {
            after.consumables[0] = BagSlot {
                kind: RewardKind::Resource,
                id: 0,
                qty: 1,
            }
        }
        5 => {
            after.resources[i] = BagSlot {
                kind: RewardKind::Recipe,
                id: 2,
                qty: 0,
            }
        }
        _ => {}
    }
    after
}

fn slots(s: &[BagSlot]) -> Value {
    Value::Array(
        s.iter()
            .map(|x| json!([x.kind as u8, x.id, x.qty]))
            .collect(),
    )
}

fn bag_json(b: &BagLayout) -> Value {
    json!({ "resources": slots(&b.resources), "consumables": slots(&b.consumables) })
}

#[test]
fn bag_check_matches_the_ts_fixture() {
    let t = BalanceConfig::DEFAULT.loot;
    let mut rng = Rng(SEED);
    let cases: Vec<Value> = (0..CASES)
        .map(|_| {
            let before = random_bag(&mut rng, &t);
            let cache = random_cache(&mut rng);
            let after = after_of(&mut rng, &before, &cache, &t);
            let verdict = match check_bag_layout(&before, &cache, &after, &t) {
                Ok(()) => Value::Null,
                Err(e) => json!(format!("{e:?}")),
            };
            let cache: Vec<Value> = cache.iter().map(|c| json!([c.kind as u8, c.id, c.qty])).collect();
            json!({ "before": bag_json(&before), "cache": cache, "after": bag_json(&after), "err": verdict })
        })
        .collect();
    let fixture = json!({
        "note": "generated by crates/rules/tests/bag_ts_fixtures.rs",
        "resourceStackMax": t.resource_stack_max,
        "consumableStackMax": t.consumable_stack_max,
        "cases": cases,
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
        "stale fixture: run UPDATE_TS_FIXTURES=1 cargo test -p deadroll-rules --features serde --test bag_ts_fixtures"
    );
}
