//! What the bench remembers between Runs, in the `private/` folder (never committed):
//! `balance-log-last.json` is the snapshot of the last Run; `balance-log-state.json` holds the last
//! run id and, per party / encounter / fights / seed, the numbers of the last Run with that key.
use std::{
    collections::BTreeMap,
    fs, io,
    path::{Path, PathBuf},
};

use deadroll_sim::metrics::Metric;
use serde::{Deserialize, Serialize};
use serde_json::Value;

/// The numbers the Runs sheet compares from one Run to the next.
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Summary {
    pub win_rate: f64,
    pub rounds_p50: f64,
    pub damage_per_turn: f64,
    pub deaths_per_fight: f64,
}

/// The last Run with one key.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Previous {
    pub summary: Summary,
    /// By [`metric_key`].
    pub metrics: BTreeMap<String, f64>,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct State {
    pub last_run_id: u64,
    pub previous: BTreeMap<String, Previous>,
}

/// Runs compare only with Runs of the same party, encounter, number of fights and seed.
pub fn run_key(party: &[String], encounter: &str, fights: u32, seed: u64) -> String {
    format!("{}|{encounter}|{fights}|{seed}", party.join("+"))
}

const SEP: char = '\u{1f}';

pub fn metric_key(group: &str, who: &str, metric: &str) -> String {
    format!("{group}{SEP}{who}{SEP}{metric}")
}

/// `(group, who, metric)` of a [`metric_key`].
pub fn split_key(key: &str) -> Option<(&str, &str, &str)> {
    let mut parts = key.splitn(3, SEP);
    Some((parts.next()?, parts.next()?, parts.next()?))
}

pub struct Store {
    dir: PathBuf,
}

impl Store {
    pub fn new(dir: impl Into<PathBuf>) -> Self {
        Store { dir: dir.into() }
    }

    fn path(&self, name: &str) -> PathBuf {
        self.dir.join(name)
    }

    /// A missing or unreadable file is an empty state: the log starts over rather than failing.
    pub fn state(&self) -> State {
        fs::read_to_string(self.path("balance-log-state.json"))
            .ok()
            .and_then(|text| serde_json::from_str(&text).ok())
            .unwrap_or_default()
    }

    pub fn save_state(&self, state: &State) -> io::Result<()> {
        let text = serde_json::to_string(state).map_err(io::Error::other)?;
        write_atomic(&self.path("balance-log-state.json"), &text)
    }

    pub fn snapshot(&self) -> Option<Value> {
        let text = fs::read_to_string(self.path("balance-log-last.json")).ok()?;
        serde_json::from_str(&text).ok()
    }

    pub fn save_snapshot(&self, snapshot: &Value) -> io::Result<()> {
        let text = serde_json::to_string(snapshot).map_err(io::Error::other)?;
        write_atomic(&self.path("balance-log-last.json"), &text)
    }
}

/// Writes `text` to a temporary file next to `path` and moves it over, so a crash leaves the old
/// file whole.
pub fn write_atomic(path: &Path, text: &str) -> io::Result<()> {
    if let Some(dir) = path.parent() {
        fs::create_dir_all(dir)?;
    }
    let tmp = path.with_extension("tmp");
    fs::write(&tmp, text)?;
    fs::rename(&tmp, path)
}

/// One number of a Run next to the same number of the previous Run.
#[derive(Clone, Debug, PartialEq)]
pub struct Row {
    pub group: String,
    pub who: String,
    pub metric: String,
    pub previous: Option<f64>,
    pub value: f64,
    pub delta: Option<f64>,
    /// Empty (`None`) when there is no previous number or it is zero.
    pub delta_pct: Option<f64>,
}

fn round6(v: f64) -> f64 {
    if v.is_finite() {
        (v * 1e6).round() / 1e6
    } else {
        0.0
    }
}

/// The rows of a Run: its numbers against `previous`, the last Run with the same key.
///
/// With a previous Run, a number it did not have counts as 0 there and a number only it had counts
/// as 0 here; a number that is zero in both is left out. Without one, zeros are left out and
/// nothing is compared.
pub fn compare(current: &[Metric], previous: Option<&Previous>) -> Vec<Row> {
    let mut rows = Vec::new();
    let mut seen = std::collections::HashSet::new();
    for m in current {
        let key = metric_key(m.group, &m.who, &m.metric);
        let before = previous.map(|p| p.metrics.get(&key).copied().unwrap_or(0.0));
        seen.insert(key);
        rows.push(make(m.group, &m.who, &m.metric, before, m.value));
    }
    if let Some(p) = previous {
        for (key, was) in &p.metrics {
            if seen.contains(key) {
                continue;
            }
            if let Some((group, who, metric)) = split_key(key) {
                rows.push(make(group, who, metric, Some(*was), 0.0));
            }
        }
    }
    rows.retain(|r| r.value != 0.0 || r.previous.is_some_and(|p| p != 0.0));
    rows
}

fn make(group: &str, who: &str, metric: &str, before: Option<f64>, value: f64) -> Row {
    let value = round6(value);
    let previous = before.map(round6);
    let delta = previous.map(|p| round6(value - p));
    Row {
        group: group.to_string(),
        who: who.to_string(),
        metric: metric.to_string(),
        previous,
        value,
        delta,
        delta_pct: match (previous, delta) {
            (Some(p), Some(d)) if p != 0.0 => Some(round6(d / p.abs() * 100.0)),
            _ => None,
        },
    }
}

/// The numbers to keep for the next Run with the same key.
pub fn remember(summary: Summary, current: &[Metric]) -> Previous {
    Previous {
        summary,
        metrics: current
            .iter()
            .map(|m| (metric_key(m.group, &m.who, &m.metric), m.value))
            .collect(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn m(who: &str, metric: &str, value: f64) -> Metric {
        Metric {
            group: "hero",
            who: who.into(),
            metric: metric.into(),
            value,
        }
    }

    fn prev(items: &[Metric]) -> Previous {
        remember(Summary::default(), items)
    }

    #[test]
    fn the_key_is_party_encounter_fights_and_seed() {
        let party = vec!["a".to_string(), "b".to_string()];
        assert_eq!(run_key(&party, "wolves", 1000, 7), "a+b|wolves|1000|7");
        assert_ne!(
            run_key(&party, "wolves", 1000, 7),
            run_key(&party, "wolves", 1000, 8)
        );
        assert_ne!(
            run_key(&party, "wolves", 1000, 7),
            run_key(&party, "wolves", 2000, 7)
        );
    }

    #[test]
    fn previous_delta_and_percent() {
        let before = prev(&[m("x", "damage", 10.0)]);
        let rows = compare(&[m("x", "damage", 15.0)], Some(&before));
        assert_eq!(rows.len(), 1);
        assert_eq!(
            (rows[0].previous, rows[0].delta, rows[0].delta_pct),
            (Some(10.0), Some(5.0), Some(50.0))
        );
        let rows = compare(&[m("x", "damage", 5.0)], Some(&before));
        assert_eq!(rows[0].delta_pct, Some(-50.0));
    }

    #[test]
    fn a_number_that_is_new_counts_as_previous_zero_without_a_percent() {
        let before = prev(&[m("x", "damage", 10.0)]);
        let rows = compare(
            &[m("x", "damage", 10.0), m("x", "heal", 4.0)],
            Some(&before),
        );
        let heal = rows.iter().find(|r| r.metric == "heal").unwrap();
        assert_eq!(
            (heal.previous, heal.delta, heal.delta_pct),
            (Some(0.0), Some(4.0), None)
        );
    }

    #[test]
    fn a_number_that_vanished_shows_as_zero_and_zero_in_both_is_left_out() {
        let before = prev(&[
            m("x", "damage", 10.0),
            m("x", "old", 3.0),
            m("x", "zero", 0.0),
        ]);
        let rows = compare(
            &[
                m("x", "damage", 10.0),
                m("x", "zero", 0.0),
                m("x", "idle", 0.0),
            ],
            Some(&before),
        );
        let old = rows.iter().find(|r| r.metric == "old").unwrap();
        assert_eq!(
            (old.value, old.previous, old.delta, old.delta_pct),
            (0.0, Some(3.0), Some(-3.0), Some(-100.0))
        );
        assert!(rows
            .iter()
            .all(|r| r.metric != "zero" && r.metric != "idle"));
    }

    #[test]
    fn without_a_previous_run_nothing_is_compared_and_zeros_are_left_out() {
        let rows = compare(&[m("x", "damage", 10.0), m("x", "zero", 0.0)], None);
        assert_eq!(rows.len(), 1);
        assert_eq!(
            (rows[0].previous, rows[0].delta, rows[0].delta_pct),
            (None, None, None)
        );
    }

    #[test]
    fn the_right_run_is_picked_by_its_key() {
        let mut state = State::default();
        let a = run_key(&["a".into()], "e", 10, 1);
        let b = run_key(&["a".into()], "e", 10, 2);
        state
            .previous
            .insert(a.clone(), prev(&[m("x", "damage", 10.0)]));
        state
            .previous
            .insert(b.clone(), prev(&[m("x", "damage", 99.0)]));
        let rows = compare(&[m("x", "damage", 100.0)], state.previous.get(&b));
        assert_eq!(rows[0].previous, Some(99.0));
        let other = run_key(&["a".into()], "e", 10, 3);
        assert!(!state.previous.contains_key(&other));
    }

    #[test]
    fn keys_split_back() {
        let key = metric_key("skill", "mage / Bolt", "damage");
        assert_eq!(split_key(&key), Some(("skill", "mage / Bolt", "damage")));
    }

    #[test]
    fn state_and_snapshot_round_trip_on_disk() {
        let dir = std::env::temp_dir().join(format!("deadroll-store-{}", std::process::id()));
        let store = Store::new(&dir);
        assert_eq!(store.state(), State::default());
        assert!(store.snapshot().is_none());
        let mut state = State {
            last_run_id: 4,
            ..Default::default()
        };
        state
            .previous
            .insert("k".into(), prev(&[m("x", "damage", 1.5)]));
        store.save_state(&state).unwrap();
        store.save_snapshot(&serde_json::json!({"a": 1})).unwrap();
        assert_eq!(store.state(), state);
        assert_eq!(store.snapshot(), Some(serde_json::json!({"a": 1})));
        fs::remove_dir_all(&dir).unwrap();
    }
}
