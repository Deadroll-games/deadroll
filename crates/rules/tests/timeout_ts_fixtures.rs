//! Golden fixture for the TypeScript mirror of the stuck-party rules
//! (`app/src/chain/timeouts.ts`): where a party stands with its DM, and when a reveal, a turn or
//! a VRF request is overdue. Test-only: no crate logic lives here. When a rule or a number
//! changes, regenerate it and update the TS mirror:
//!
//! ```text
//! UPDATE_TS_FIXTURES=1 cargo test -p deadroll-rules --features serde --test timeout_ts_fixtures
//! ```
#![cfg(feature = "serde")]
use std::{fs, path::PathBuf};

use deadroll_rules::{
    timeout::{reveal_overdue, turn_overdue, vrf_retry_due, DmPhase, DmWatch},
    BalanceConfig, TimeoutTuning,
};
use serde_json::{json, Value};

const CASES: usize = 300;
/// Fixed seed; changing it rewrites the fixture.
const SEED: u64 = 0xD3AD_5717_C4ED_0001;

fn fixture_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../app/src/chain/__fixtures__/timeouts.json")
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

fn phase(p: DmPhase) -> Value {
    match p {
        DmPhase::Present => json!({ "kind": "present" }),
        DmPhase::Missing { until } => json!({ "kind": "missing", "until": until }),
        DmPhase::Gone { ended_at } => json!({ "kind": "gone", "endedAt": ended_at }),
        DmPhase::Back { until } => json!({ "kind": "back", "until": until }),
        DmPhase::Unconfirmed { ended_at } => {
            json!({ "kind": "unconfirmed", "endedAt": ended_at })
        }
    }
}

#[test]
fn timeout_fixture_matches() {
    let t = BalanceConfig::DEFAULT.timeouts;
    let mut rng = Rng(SEED);
    let base: i64 = 1_800_000_000;
    let span = (t.dm_return_confirm_secs as u64) * 2;
    let cases: Vec<Value> = (0..CASES)
        .map(|_| {
            let settle = 600 + rng.below(36_000) as u32;
            let missing_at = if rng.below(5) == 0 {
                0
            } else {
                base + rng.below(1_000) as i64
            };
            let back_at = if missing_at == 0 || rng.below(2) == 0 {
                0
            } else {
                missing_at + rng.below(settle as u64 + 1) as i64
            };
            let watch = DmWatch {
                missing_at,
                back_at,
                confirmed: rng.below(16) as u8,
            };
            let now = base + rng.below(span) as i64;
            let clock = base + rng.below(span) as i64;
            let slot = rng.below(1_000);
            let sent = rng.below(1_000);
            json!({
                "missingAt": watch.missing_at,
                "backAt": watch.back_at,
                "settle": settle,
                "now": now,
                "phase": phase(watch.phase(now, settle, &t)),
                "clock": clock,
                "turnOverdue": turn_overdue(clock, now, &t),
                "revealOverdue": reveal_overdue(clock, now, &t),
                "sent": sent,
                "slot": slot,
                "vrfRetryDue": vrf_retry_due(sent, slot, &t),
            })
        })
        .collect();

    let fixture = json!({
        "timeouts": {
            "revealTimeoutSecs": t.reveal_timeout_secs,
            "dmReturnConfirmSecs": t.dm_return_confirm_secs,
            "turnTimeoutSecs": t.turn_timeout_secs,
            "vrfRetrySlots": t.vrf_retry_slots,
        },
        "localnet": {
            "revealTimeoutSecs": TimeoutTuning::LOCALNET.reveal_timeout_secs,
            "dmReturnConfirmSecs": TimeoutTuning::LOCALNET.dm_return_confirm_secs,
            "turnTimeoutSecs": TimeoutTuning::LOCALNET.turn_timeout_secs,
            "vrfRetrySlots": TimeoutTuning::LOCALNET.vrf_retry_slots,
        },
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
        "stale fixture: run UPDATE_TS_FIXTURES=1 cargo test -p deadroll-rules --features serde --test timeout_ts_fixtures"
    );
}
