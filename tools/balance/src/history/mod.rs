//! The run history: after every finished Run the bench sends its results to a Google Sheet.
//!
//! - [`snapshot`]: the whole state as one JSON value, and what changed between two of them;
//! - [`store`]: what is kept in `private/` between Runs and the comparison with the previous Run;
//! - [`sheet`]: the request body (the `Runs` and `Metrics` sheets);
//! - [`sender`]: the background sender with its queue file.
//!
//! The history is off without a sheet URL (`DEADROLL_SHEET_URL` or `private/balance-sheet-url.txt`):
//! the bench then works as before and nothing is written to `private/`.
pub mod sender;
pub mod sheet;
pub mod snapshot;
pub mod store;

use std::path::PathBuf;

use deadroll_sim::{metrics::metrics, Report, Settings};
use serde_json::Value;

use self::{
    sender::{Sender, Status, UrlSource},
    sheet::RunInfo,
    store::{compare, remember, run_key, Row, Store, Summary},
};

/// What a Run was, captured when it started.
pub struct RunInput<'a> {
    pub party: &'a [String],
    pub encounter: &'a str,
    pub fights: u32,
    pub seed: u64,
    pub settings: Settings,
    pub snapshot: Value,
    pub note: &'a str,
    pub report: &'a Report,
}

/// What the Results tab shows of the last logged Run.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct LastRun {
    pub run_id: u64,
    /// The ten largest changes by absolute delta %.
    pub top: Vec<Row>,
    /// Whether it had a previous Run to compare with.
    pub compared: bool,
    pub changes: String,
    /// A problem saving the local files (the request is sent anyway).
    pub warning: Option<String>,
}

pub struct History {
    store: Store,
    source: UrlSource,
    sender: Option<Sender>,
    /// The one-line note sent with the next Runs, kept until changed.
    pub note: String,
    pub last: Option<LastRun>,
    off_for_good: bool,
}

/// `private/` of the repository this bench was built in.
fn private_dir() -> PathBuf {
    std::env::var_os("DEADROLL_PRIVATE")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../private"))
}

impl History {
    /// The bench's history: `private/` of the repository, the URL from the environment or file.
    pub fn new(repaint: impl Fn() + Send + 'static) -> History {
        History::with(private_dir(), UrlSource::Default(private_dir()), repaint)
    }

    /// A history that never logs or sends (headless tests of the window).
    #[cfg(test)]
    pub fn off() -> History {
        History {
            store: Store::new(private_dir()),
            source: UrlSource::Default(private_dir()),
            sender: None,
            note: String::new(),
            last: None,
            off_for_good: true,
        }
    }

    pub(crate) fn with(
        dir: PathBuf,
        source: UrlSource,
        repaint: impl Fn() + Send + 'static,
    ) -> History {
        let sender = source
            .available()
            .then(|| Sender::start(dir.clone(), source.clone(), repaint));
        // Requests an earlier session could not send go out first.
        if let Some(s) = sender.as_ref().filter(|s| s.status().queued > 0) {
            s.flush();
        }
        History {
            store: Store::new(dir),
            source,
            sender,
            note: String::new(),
            last: None,
            off_for_good: false,
        }
    }

    /// Whether Runs are logged: there is a sheet URL.
    pub fn enabled(&self) -> bool {
        !self.off_for_good && self.source.available()
    }

    pub fn status(&self) -> Option<Status> {
        self.sender.as_ref().map(Sender::status)
    }

    /// Logs a finished Run: compares it with the last one, keeps what the next Run compares with,
    /// and hands the request to the sender. Returns `None` when the history is off.
    pub fn record(&mut self, input: RunInput<'_>) -> Option<&LastRun> {
        if !self.enabled() {
            return None;
        }
        let state_before = self.store.state();
        let old = self.store.snapshot();
        let changes = old
            .as_ref()
            .map(|old| snapshot::changes(old, &input.snapshot))
            .unwrap_or_default();
        let current = metrics(input.report);
        let summary = summary_of(input.report);
        let key = run_key(input.party, input.encounter, input.fights, input.seed);
        let previous = state_before.previous.get(&key);
        let rows = compare(&current, previous);
        let run_id = state_before.last_run_id + 1;
        let info = RunInfo {
            run_id,
            when: sheet::now_utc(),
            party: input.party.to_vec(),
            encounter: input.encounter.to_string(),
            fights: input.fights,
            seed: input.seed,
            max_rounds: input.settings.max_rounds,
            heal_below_pct: input.settings.heal_below_pct,
            changes: changes.clone(),
            note: input.note.to_string(),
        };
        let body = sheet::request(&info, &summary, previous.map(|p| &p.summary), &rows);
        // Keep what the next Run compares with before sending: a failed send must not reuse the id.
        let mut state = state_before.clone();
        state.last_run_id = run_id;
        state.previous.insert(key, remember(summary, &current));
        let saved = self
            .store
            .save_state(&state)
            .and(self.store.save_snapshot(&input.snapshot));
        if let Some(sender) = &self.sender {
            sender.submit(body.to_string());
        }
        let mut top: Vec<Row> = rows.into_iter().filter(|r| r.delta_pct.is_some()).collect();
        top.sort_by(|a, b| {
            let (x, y) = (
                a.delta_pct.unwrap_or(0.0).abs(),
                b.delta_pct.unwrap_or(0.0).abs(),
            );
            y.total_cmp(&x)
        });
        top.truncate(10);
        self.last = Some(LastRun {
            run_id,
            top,
            compared: previous.is_some(),
            changes,
            warning: saved
                .err()
                .map(|_| "could not save the local history files in private/".to_string()),
        });
        self.last.as_ref()
    }
}

/// The numbers the Runs sheet compares from Run to Run.
pub fn summary_of(r: &Report) -> Summary {
    let damage: u64 = r.slots.iter().map(|s| s.damage).sum();
    let turns: u64 = r.slots.iter().map(|s| s.turns).sum();
    Summary {
        win_rate: r.win_rate * 100.0,
        rounds_p50: r.rounds_to_win.p50,
        damage_per_turn: if turns == 0 {
            0.0
        } else {
            damage as f64 / turns as f64
        },
        deaths_per_fight: r.deaths_per_fight,
    }
}

#[cfg(test)]
mod tests {
    use std::{fs, sync::Mutex};

    use deadroll_sim::SlotReport;
    use serde_json::json;

    use super::*;

    /// One test at a time may touch the environment variable.
    static ENV: Mutex<()> = Mutex::new(());

    fn report(win_rate: f64, damage: u64) -> Report {
        Report {
            fights: 100,
            win_rate,
            slots: vec![SlotReport {
                name: "mage".into(),
                damage,
                turns: 100,
                hits: 50,
                ..Default::default()
            }],
            ..Default::default()
        }
    }

    fn run<'a>(report: &'a Report, snapshot: Value, party: &'a [String]) -> RunInput<'a> {
        RunInput {
            party,
            encounter: "wolves",
            fights: 100,
            seed: 1,
            settings: Settings::default(),
            snapshot,
            note: "",
            report,
        }
    }

    #[test]
    fn without_a_url_nothing_is_logged_or_written() {
        let _guard = ENV.lock().unwrap();
        if std::env::var_os("DEADROLL_SHEET_URL").is_some() {
            return;
        }
        let dir = std::env::temp_dir().join(format!("deadroll-hist-off-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        let mut h = History::with(dir.clone(), UrlSource::Default(dir.clone()), || {});
        assert!(!h.enabled());
        let party = vec!["mage".to_string()];
        let r = report(0.5, 500);
        assert!(h.record(run(&r, json!({}), &party)).is_none());
        assert!(h.status().is_none());
        assert!(!dir.exists());
    }

    #[test]
    fn runs_are_numbered_compared_and_the_changes_named() {
        let _guard = ENV.lock().unwrap();
        let dir = std::env::temp_dir().join(format!("deadroll-hist-on-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        // A dead address: the requests only queue, which is all this test needs.
        let dead = {
            let l = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
            format!("http://{}/exec", l.local_addr().unwrap())
        };
        let mut h = History::with(dir.clone(), UrlSource::Fixed(dead), || {});
        let party = vec!["mage".to_string()];
        let first = report(0.5, 500);
        let a = h
            .record(run(&first, json!({"config": {"crit": 1}}), &party))
            .unwrap()
            .clone();
        assert_eq!((a.run_id, a.compared, a.changes.as_str()), (1, false, ""));
        let second = report(0.6, 600);
        let b = h
            .record(run(&second, json!({"config": {"crit": 2}}), &party))
            .unwrap()
            .clone();
        assert_eq!((b.run_id, b.compared), (2, true));
        assert_eq!(b.changes, "config.crit: 1 -> 2");
        assert!(!b.top.is_empty() && b.top.len() <= 10);
        let biggest = b.top[0].delta_pct.unwrap().abs();
        assert!(b.top.iter().all(|r| r.delta_pct.unwrap().abs() <= biggest));
        // A different seed is a different comparison; the run id keeps counting.
        let mut other = run(&second, json!({"config": {"crit": 2}}), &party);
        other.seed = 2;
        let c = h.record(other).unwrap().clone();
        assert_eq!((c.run_id, c.compared, c.changes.as_str()), (3, false, ""));
        // A new session carries on from the files.
        let again = Store::new(&dir);
        assert_eq!(again.state().last_run_id, 3);
        assert_eq!(again.snapshot(), Some(json!({"config": {"crit": 2}})));
        drop(h);
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn the_summary_is_damage_over_turns_and_percent() {
        let r = report(0.25, 300);
        let s = summary_of(&r);
        assert_eq!((s.win_rate, s.damage_per_turn), (25.0, 3.0));
    }
}
